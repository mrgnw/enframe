//! pinframe as a library: pin one feature in place across a photo sequence. [`track`] checks a [`Job`],
//! follows the marked feature from photo to photo and plans one crop per photo; [`Tracked::sidecar`] gives
//! the result as the `.json` sidecar holds it, and [`Tracked::write`] renders the clip and writes the
//! sidecar. The `pinframe` binary is a thin CLI over these, so a server can run the same pipeline
//! in-process.
//!
//! A [`Job`] holds the CLI's options, typed, and is checked in the CLI's order with its messages: an error
//! is the message the CLI prints after `pinframe: `, naming its flags (`--at`, `--pad`). Progress lines,
//! the ones the CLI prints, go to a `log` callback.

mod avif;
mod avif_sys;
mod cache;
#[cfg(test)]
mod checks;
mod colour;
mod decode;
#[cfg(target_os = "macos")]
mod imageio;
#[cfg(not(target_os = "macos"))]
#[path = "imageio_unavailable.rs"]
mod imageio;
mod pixels;
mod profile;
mod render;
mod sift;
mod track;

pub use track::Mark;

use colour::Profile;
use profile::{Work, step, work};
use render::{Keep, ordered, place};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::mpsc::sync_channel;
use track::{Hit, Photo, Rect};

/// Threads and memory are planned at start from the cores, the free memory and these measured costs
/// (`analysis/rust_task_memory.py`, M4 Pro, 2026-10-05), rounded up. Re-measure them after changing
/// libraries or TRACK_EDGE. PNG's measured working set is larger than AVIF and JPEG's.
const PREPARE_FIXED_BYTES: usize = 130 << 20; // SIFT's pyramid at TRACK_EDGE, whatever the photo size
const FEATURE_BYTES: usize = 1 << 20; // one photo's keypoints and byte descriptors; train clouds's are under 1 MB
const CACHE_HASH_BYTES: usize = 1 << 20; // one streaming BLAKE3 input buffer per prepare worker
/// Results queued per thread; they come back in order, so a slow photo must not idle the others.
const QUEUE_DEPTH: usize = 3;
/// Policy: plan within this share of the memory free at start, leaving the rest to other apps.
const MEMORY_SHARE: f64 = 0.75;
const AVIF_SPEED: i32 = 10;
const AVIF_QUALITY: i32 = 75;
const CLIP_SUFFIXES: [&str; 4] = ["avif", "mp4", "mov", "webm"];

/// The message given when `--box` or an old `x,y,w,h` mark is used: the conversion to point form.
pub const BOX_REMOVED: &str = "--box is removed; use --points, and turn a box x,y,w,h into its four quarter \
	centres: x+w/4,y+h/4/x+3w/4,y+h/4/x+w/4,y+3h/4/x+3w/4,y+3h/4";

/// How [`track`] orders the photos (`--order`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Order {
	/// EXIF capture time, falling back to filename order if any photo lacks one.
	#[default]
	Time,
	/// Filename order.
	Name,
}

/// The user's marks in the CLI's text form, so they are checked with the same messages wherever they come
/// from. [`Marks::new`] and its `with_` methods build them from points.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Marks {
	/// The first photo's points, `x,y[/x,y]...` in photo pixels (`--points`). Required.
	pub points: Option<String>,
	/// `PHOTO=x,y[/x,y]...[@zoom]` per marked photo, PHOTO its file name (`--at`).
	pub at: Vec<String>,
	/// File names of photos whose feature is hidden (`--skip`).
	pub skip: Vec<String>,
	/// `PHOTO=x,y[/x,y]...[@zoom]` per soft-marked photo: placed there, but not tracked from (`--near`).
	pub near: Vec<String>,
}

impl Marks {
	/// The first photo's points, in photo pixels, with no other photo marked or skipped.
	pub fn new(points: &[(f64, f64)]) -> Marks {
		Marks { points: Some(points_text(points)), ..Marks::default() }
	}

	/// Adds a mark on the photo with this file name.
	pub fn with_at(mut self, photo: &str, mark: &Mark) -> Marks {
		self.at.push(mark_text(photo, mark));
		self
	}

	/// Adds a soft mark on the photo with this file name.
	pub fn with_near(mut self, photo: &str, mark: &Mark) -> Marks {
		self.near.push(mark_text(photo, mark));
		self
	}

	/// Skips the photo with this file name.
	pub fn with_skip(mut self, photo: &str) -> Marks {
		self.skip.push(photo.to_string());
		self
	}
}

/// `PHOTO=x,y/x,y...[@zoom]`.
fn mark_text(photo: &str, mark: &Mark) -> String {
	let zoom = mark.zoom.map(|zoom| format!("@{zoom}")).unwrap_or_default();
	format!("{photo}={}{zoom}", points_text(&mark.points))
}

/// `x,y/x,y...`. An `f64` prints as the shortest text that parses back to it, so nothing is lost.
fn points_text(points: &[(f64, f64)]) -> String {
	points.iter().map(|(x, y)| format!("{x},{y}")).collect::<Vec<_>>().join("/")
}

/// What [`Tracked::write`] writes. The default is the CLI's: `out.avif` at 4 frames per second.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
	/// A clip (`.avif`, `.mp4`, `.mov`, `.webm`), written with its `.json` sidecar beside it, or a `.json`
	/// alone (`--out`).
	pub out: PathBuf,
	/// Frames per second (`--fps`).
	pub fps: f64,
	/// A folder to also save each frame to, as `0001.png` on (`--frames`).
	pub frames: Option<PathBuf>,
	/// ffmpeg filters run on a video clip before it is encoded (`--vf`).
	pub vf: Option<String>,
}

impl Default for Output {
	fn default() -> Output {
		Output { out: PathBuf::from("out.avif"), fps: 4.0, frames: None, vf: None }
	}
}

/// One run: the photos, the marks and the options, as the CLI's flags give them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Job {
	/// The photos, in any order; [`Job::order`] sorts them.
	pub photos: Vec<PathBuf>,
	/// How the photos are ordered (`--order`).
	pub order: Order,
	/// Where the feature is in which photos.
	pub marks: Marks,
	/// Padding where a crop leaves its photo: `transparent`, `black`, `white` or `#rrggbb` (`--pad`).
	/// Without it, every crop stays inside its photo.
	pub pad: Option<String>,
	/// The feature cache's folder (`--cache`); the platform's cache folder when not given.
	pub cache: Option<PathBuf>,
	/// Neither read nor write the feature cache (`--no-cache`).
	pub no_cache: bool,
	/// What [`Tracked::write`] will write, checked before anything is decoded. `None` only tracks, like a
	/// `.json` output that is never written.
	pub output: Option<Output>,
}

/// 1 to 32 points per mark (A5b's design): enough for any hand-marked feature, bounded so a typo (a
/// pasted list, a stray `/`) fails fast instead of handing the tracker an unbounded point set.
const MAX_POINTS: usize = 32;

/// One point `x,y` within a `/`-joined list. A 4-number segment is the old box form; `parse_points`
/// reports that case separately, with the quarter-centres conversion, before reaching this generic check.
fn parse_point(flag: &str, text: &str) -> Result<(f64, f64), String> {
	let parts: Vec<&str> = text.split(',').collect();
	if parts.len() != 2 {
		return Err(format!("{flag}: a point is x,y, not {} numbers ({text})", parts.len()));
	}
	let x: f64 = parts[0].trim().parse().map_err(|_| format!("{flag}: point {text} is not x,y"))?;
	let y: f64 = parts[1].trim().parse().map_err(|_| format!("{flag}: point {text} is not x,y"))?;
	Ok((x, y))
}

/// A mark's points and its optional zoom.
type Points = (Vec<(f64, f64)>, Option<f64>);

/// A mark's points and optional zoom: `x,y[/x,y]...[@zoom]`. The old box form `x,y,w,h` is refused here
/// (a 4-number point) with the conversion to quarter centres, before `parse_point`'s generic count error.
fn parse_points(flag: &str, text: &str) -> Result<Points, String> {
	let (points_text, zoom) = match text.split_once('@') {
		Some((p, z)) => {
			let zoom: f64 = z.trim().parse().map_err(|_| format!("{flag}: @{z} is not a number"))?;
			(p, Some(zoom))
		}
		None => (text, None),
	};
	if points_text.is_empty() {
		return Err(format!("{flag}: needs at least one point"));
	}
	// A single x,y,w,h segment (no /) is the old box form, not a 4-number point.
	if !points_text.contains('/') && points_text.split(',').count() == 4 {
		return Err(format!("{flag}: {points_text} is a box x,y,w,h. {BOX_REMOVED}"));
	}
	let points: Vec<(f64, f64)> =
		points_text.split('/').map(|p| parse_point(flag, p)).collect::<Result<_, _>>()?;
	if points.len() > MAX_POINTS {
		return Err(format!("{flag}: {} points, at most {MAX_POINTS}", points.len()));
	}
	if let Some(zoom) = zoom
		&& zoom.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
	{
		return Err(format!("{flag}: zoom must be positive"));
	}
	Ok((points, zoom))
}

/// Every point of a mark must land inside its photo.
fn check_points(name: &str, points: &[(f64, f64)], (fw, fh): (u32, u32)) -> Result<(), String> {
	let (fw, fh) = (fw as f64, fh as f64);
	for &(px, py) in points {
		if !(0.0 <= px && px < fw && 0.0 <= py && py < fh) {
			return Err(format!("{name}: point {px},{py} is outside the {fw}x{fh} photo"));
		}
	}
	Ok(())
}

/// The photos in order, each one's 35 mm focal length, and why that order.
type Ordered = (Vec<PathBuf>, Vec<Option<f64>>, String);

/// Orders photos by EXIF capture time, falling back to filename order when `--order name` was given or
/// any photo lacks a capture time (a mix of cameras, or an export that stripped EXIF). Marks are matched
/// to photos by file name (`read_marks`), so reordering here never loses track of which mark goes where.
/// Each photo's 35 mm focal length comes back in the same order.
fn order_photos(paths: Vec<PathBuf>, order: Order) -> Result<Ordered, String> {
	let exifs: Vec<decode::Exif> = paths
		.iter()
		.map(|p| decode::exif(p).map_err(|e| format!("{}: {e}", p.display())))
		.collect::<Result<_, String>>()?;
	let missing = exifs.iter().filter(|e| e.capture_time.is_none()).count();
	let mut sorted: Vec<usize> = (0..paths.len()).collect();
	let reason = if order == Order::Name {
		sorted.sort_by(|&a, &b| paths[a].cmp(&paths[b]));
		"name: --order name".to_string()
	} else if missing > 0 {
		sorted.sort_by(|&a, &b| paths[a].cmp(&paths[b]));
		let s = if missing == 1 { "" } else { "s" };
		let has = if missing == 1 { "has" } else { "have" };
		format!("name: {missing} photo{s} {has} no capture time")
	} else {
		sorted.sort_by(|&a, &b| {
			exifs[a].capture_time.cmp(&exifs[b].capture_time).then_with(|| paths[a].cmp(&paths[b]))
		});
		"time: every photo has a capture time".to_string()
	};
	let focals = sorted.iter().map(|&i| exifs[i].focal_35mm).collect();
	Ok((sorted.into_iter().map(|i| paths[i].clone()).collect(), focals, reason))
}

fn file_name(path: &Path) -> String {
	path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Every photo's size from its header, so bad input stops before any decoding.
fn read_sizes(paths: &[PathBuf]) -> Result<Vec<(u32, u32)>, String> {
	if paths.len() < 2 {
		return Err("need at least two photos".into());
	}
	let mut names: Vec<String> = paths.iter().map(|p| file_name(p)).collect();
	names.sort();
	if names.windows(2).any(|w| w[0] == w[1]) {
		return Err("two photos share a file name, so --at and the .json could not tell them apart".into());
	}
	paths.iter().map(|p| decode::oriented_size(p).map_err(|e| format!("{}: {e}", p.display()))).collect()
}

/// Every photo's colour profile from its header; a profile pinframe cannot label stops the run.
fn read_profiles(paths: &[PathBuf]) -> Result<Vec<Profile>, String> {
	paths.iter().map(|p| decode::profile(p).map_err(|e| format!("{}: {e}", p.display()))).collect()
}

/// --points, every --at and every --near, by photo index, checked against the photo sizes, and which of
/// them are soft (--near).
fn read_marks(
	points: Option<&str>,
	ats: &[String],
	nears: &[String],
	paths: &[PathBuf],
	sizes: &[(u32, u32)],
) -> Result<(Vec<Option<Mark>>, Vec<bool>), String> {
	let names: Vec<String> = paths.iter().map(|p| file_name(p)).collect();
	// ponytail: no point drawing (anchor.py opens an OpenCV window); --points is required here
	let (first_points, first_zoom) =
		parse_points("--points", points.ok_or("--points x,y[/x,y]... is required")?)?;
	if first_zoom.is_some() {
		return Err("--points takes no zoom; the first photo defines scale 1".into());
	}
	let mut marks: Vec<Option<Mark>> = vec![None; paths.len()];
	marks[0] = Some(Mark { points: first_points, zoom: None });
	let mut soft = vec![false; paths.len()];
	for (flag, at) in ats.iter().map(|at| ("--at", at)).chain(nears.iter().map(|near| ("--near", near))) {
		let Some((name, value)) = at.split_once('=').filter(|(n, _)| names.iter().any(|m| m == n)) else {
			return Err(format!("{flag} {at}: expected PHOTO=x,y with PHOTO one of the photo file names"));
		};
		let i = names.iter().position(|m| m == name).unwrap();
		if marks[i].is_some() {
			let why = match (i, flag) {
				(0, "--at") => "use --points for the first photo",
				(0, _) => "the first photo cannot be soft-marked",
				_ => "marked twice",
			};
			return Err(format!("{flag} {name}: {why}"));
		}
		let (points, zoom) = parse_points(&format!("{flag} {name}"), value)?;
		marks[i] = Some(Mark { points, zoom });
		soft[i] = flag == "--near";
	}
	for (i, mark) in marks.iter().enumerate() {
		if let Some(m) = mark {
			check_points(&names[i], &m.points, sizes[i])?;
		}
	}
	Ok((marks, soft))
}

/// Every `--skip`, by photo index, matched to a photo file name like `--at`. A skipped photo may not also
/// carry a mark: the tracker has nothing to read either the feature or the user's point from.
fn read_skips(skips: &[String], paths: &[PathBuf], marks: &[Option<Mark>]) -> Result<Vec<bool>, String> {
	let names: Vec<String> = paths.iter().map(|p| file_name(p)).collect();
	let mut flags = vec![false; paths.len()];
	for name in skips {
		let Some(i) = names.iter().position(|n| n == name) else {
			return Err(format!("--skip {name}: not one of the photo file names"));
		};
		if i == 0 {
			return Err(format!("--skip {name}: the first photo cannot be skipped"));
		}
		if marks[i].is_some() {
			return Err(format!("--skip {name}: already marked with --at or --near"));
		}
		if flags[i] {
			return Err(format!("--skip {name}: skipped twice"));
		}
		flags[i] = true;
	}
	Ok(flags)
}

fn parse_fill(value: &str) -> Result<Vec<u8>, String> {
	if value == "transparent" {
		return Ok(vec![0, 0, 0, 0]);
	}
	let hex = match value {
		"black" => "000000",
		"white" => "ffffff",
		other => other.strip_prefix('#').unwrap_or(other),
	};
	let channel = |i: usize| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
	match (hex.len(), channel(0), channel(2), channel(4)) {
		(6, Some(r), Some(g), Some(b)) => Ok(vec![r, g, b]),
		_ => Err(format!("--pad {value}: use transparent, black, white or a #rrggbb colour")),
	}
}

fn suffix(path: &Path) -> String {
	path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default()
}

fn on_path(program: &str) -> bool {
	std::env::var_os("PATH")
		.is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

fn folder_of(path: &Path) -> PathBuf {
	match path.parent() {
		Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
		_ => PathBuf::from("."),
	}
}

fn check_output(out: &Path, alpha: bool, filters: Option<&str>) -> Result<(), String> {
	let ext = suffix(out);
	if ext != "json" && !CLIP_SUFFIXES.contains(&ext.as_str()) {
		return Err(format!("--out {}: use .json, .avif, .mp4, .mov or .webm", out.display()));
	}
	if !folder_of(out).is_dir() {
		return Err(format!("--out {}: folder {} does not exist", out.display(), folder_of(out).display()));
	}
	if ext == "json" {
		if filters.is_some() {
			return Err("--vf needs a video --out, not .json".into());
		}
		return Ok(());
	}
	if ext == "avif" {
		if filters.is_some() {
			return Err("--vf runs through ffmpeg: use a .mp4, .mov or .webm --out".into());
		}
		return Ok(());
	}
	if !on_path("ffmpeg") {
		return Err(format!("--out .{ext} needs ffmpeg on PATH; .avif does not"));
	}
	if alpha && ext == "mp4" {
		return Err("transparent padding needs a .avif, .mov or .webm --out".into());
	}
	Ok(())
}
struct Resources {
	prepare_threads: usize,
	render_threads: usize,
	keep_bytes: usize,
}

/// Every core. Unlike anchor.py, whose OpenCV threads ran slower once the efficiency cores joined, the
/// Rust prepare step gets faster with them (train clouds: 1.73 s on 8 threads, 1.47 s on 12).
fn cores() -> usize {
	std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// Memory free now or reclaimable without swapping: the kernel's own estimate, `MemAvailable`.
#[cfg(not(target_os = "macos"))]
fn available_bytes() -> usize {
	std::fs::read_to_string("/proc/meminfo")
		.unwrap_or_default()
		.lines()
		.find_map(|line| line.strip_prefix("MemAvailable:"))
		.and_then(|rest| rest.split_whitespace().next())
		.and_then(|kb| kb.parse::<usize>().ok())
		.map_or(0, |kb| kb * 1024)
}

/// Memory free now or reclaimable without swapping, as psutil's `available` counts it on macOS.
#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn available_bytes() -> usize {
	unsafe {
		let mut vm: libc::vm_statistics64 = std::mem::zeroed();
		let mut count = libc::HOST_VM_INFO64_COUNT;
		let host = libc::mach_host_self();
		libc::host_statistics64(
			host,
			libc::HOST_VM_INFO64,
			&mut vm as *mut _ as libc::host_info64_t,
			&mut count,
		);
		let page = libc::sysconf(libc::_SC_PAGESIZE) as usize;
		let (free, inactive) = (vm.free_count, vm.inactive_count);
		(free as usize + inactive as usize) * page
	}
}

fn plan_resources(
	largest_pixels: usize,
	prepare_decode_bytes: usize,
	render_decode_bytes: usize,
	free_bytes: usize,
	cores: usize,
) -> Result<Resources, String> {
	let budget = (free_bytes as f64 * MEMORY_SHARE) as usize;
	let prepare_cost =
		PREPARE_FIXED_BYTES + prepare_decode_bytes + QUEUE_DEPTH * FEATURE_BYTES + CACHE_HASH_BYTES;
	// A queued clip frame is RGBA, and the clip is never larger than the photos it is cut from.
	let render_cost = render_decode_bytes + QUEUE_DEPTH * 4 * largest_pixels;
	let prepare_threads = cores.min(budget / prepare_cost);
	let render_threads = cores.min(budget / render_cost);
	if prepare_threads < 1 || render_threads < 1 {
		let need = prepare_cost.max(render_cost) as f64 / MEMORY_SHARE / (1u64 << 30) as f64;
		let free = free_bytes as f64 / (1u64 << 30) as f64;
		return Err(format!(
			"a {:.0} MP photo needs about {need:.1} GB free; {free:.1} GB is",
			largest_pixels as f64 / 1e6
		));
	}
	// Kept photos last through both passes, so they get what neither pass's threads need.
	let working = (prepare_threads * prepare_cost).max(render_threads * render_cost);
	Ok(Resources { prepare_threads, render_threads, keep_bytes: budget - working })
}

fn find_features(small: &pixels::Image) -> sift::Features {
	sift::detect_and_compute(small)
}

fn prepare(
	path: &Path,
	size: (u32, u32),
	k: f64,
	keep: Option<&Keep>,
	i: usize,
	cache: Option<&cache::FeatureCache>,
) -> Result<Photo, String> {
	let small = ((size.0 as f64 * k).round() as usize, (size.1 as f64 * k).round() as usize);
	let key = cache.zip(
		decode::decoder_id(path)
			.and_then(|decoder| cache::key(path, decoder, k, small, track::TRACK_EDGE))
			.ok(),
	);
	if let Some(cached) = key.as_ref().and_then(|(cache, key)| cache.load(key)) {
		if let Some(keep) = keep {
			let rgb = work(Work::Decode, || decode::decode(path))
				.map_err(|e| format!("{}: {e}", path.display()))?;
			keep.offer(i, rgb);
		}
		return Ok(Photo { small: cached.small, features: cached.features });
	}
	let rgb = work(Work::Decode, || decode::decode(path)).map_err(|e| format!("{}: {e}", path.display()))?;
	assert_eq!(
		(rgb.width as u32, rgb.height as u32),
		size,
		"{}: decoded size differs from the header",
		path.display()
	);
	let photo = work(Work::Sift, || {
		let gray = pixels::gray(&rgb);
		let small = if k < 1.0 { pixels::shrink_area(&gray, k) } else { gray };
		Photo { small: (small.width, small.height), features: find_features(&small) }
	});
	if let Some(keep) = keep {
		keep.offer(i, rgb);
	}
	if let Some((cache, key)) = key {
		cache.store(&key, photo.small, &photo.features);
	}
	Ok(photo)
}

/// Where each frame goes: an animated AVIF through libavif, or ffmpeg for the other formats.
enum Sink {
	Avif(avif::Encoder),
	Ffmpeg(std::process::Child),
}

struct SinkOptions<'a> {
	size: (u32, u32),
	alpha: bool,
	fps: f64,
	filters: Option<&'a str>,
	run: &'a str,
	profile: Profile,
}

fn open_sink(out: &Path, o: &SinkOptions) -> Result<Sink, String> {
	if suffix(out) == "avif" {
		let threads = std::thread::available_parallelism().map_or(1, |n| n.get()) as i32;
		let frame_ms = (1000.0 / o.fps).round_ties_even() as u64;
		let xmp = run_xmp(o.run);
		return Ok(Sink::Avif(avif::Encoder::new(
			AVIF_SPEED,
			AVIF_QUALITY,
			frame_ms,
			threads,
			xmp,
			o.profile,
		)));
	}
	let (encoder, pix_fmt): (Vec<&str>, &str) = match (suffix(out).as_str(), o.alpha) {
		("webm", alpha) => (
			vec!["-c:v", "libvpx-vp9", "-crf", "30", "-b:v", "0"],
			if alpha { "yuva420p" } else { "yuv420p" },
		),
		("mov", true) => (vec!["-c:v", "prores_ks", "-profile:v", "4444"], "yuva444p10le"),
		_ => (vec!["-c:v", "libx264", "-crf", "18"], "yuv420p"),
	};
	// RGB to YUV with the matrix and range the labels name; the values stay in the photos' profile.
	// The labels go on the frames (setparams): ffmpeg 9 takes them from there, not from -color_primaries.
	let to_yuv = format!(
		"scale=out_color_matrix=bt709:out_range=tv,format={pix_fmt},\
		setparams=color_primaries={}:color_trc=iec61966-2-1:colorspace=bt709:range=tv",
		o.profile.ffmpeg_primaries()
	);
	let vf = o.filters.map_or(to_yuv.clone(), |f| format!("{f},{to_yuv}"));
	let mut cmd = std::process::Command::new("ffmpeg");
	cmd.args([
		"-y",
		"-loglevel",
		"error",
		"-f",
		"rawvideo",
		"-pix_fmt",
		if o.alpha { "rgba" } else { "rgb24" },
	]);
	cmd.args(["-s", &format!("{}x{}", o.size.0, o.size.1), "-r", &o.fps.to_string(), "-i", "-"]);
	cmd.args(["-vf", &vf]).args(&encoder).args(["-pix_fmt", pix_fmt]);
	cmd.args(["-metadata", &format!("comment=pinframe-run={}", o.run)]).arg(out);
	let child = cmd.stdin(std::process::Stdio::piped()).spawn().map_err(|e| format!("ffmpeg: {e}"))?;
	Ok(Sink::Ffmpeg(child))
}

/// A minimal XMP packet carrying the run ID, so a clip can be matched with its .json.
fn run_xmp(run: &str) -> Vec<u8> {
	format!(
		"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
		<rdf:Description xmlns:pinframe=\"https://pinframe.local/ns/1.0/\" pinframe:run=\"{run}\"/>\
		</rdf:RDF></x:xmpmeta>"
	)
	.into_bytes()
}

/// A --frames PNG, labelled with the sRGB chunk or Display P3's ICC profile.
fn save_png(path: &Path, frame: &pixels::Image, profile: Profile) -> Result<(), String> {
	static P3_ICC: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
	let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
	let mut info = png::Info::with_size(frame.width as u32, frame.height as u32);
	match profile {
		Profile::Srgb => info.srgb = Some(png::SrgbRenderingIntent::Perceptual),
		Profile::DisplayP3 => {
			info.icc_profile = Some(P3_ICC.get_or_init(colour::display_p3_icc).as_slice().into())
		}
	}
	let mut encoder =
		png::Encoder::with_info(std::io::BufWriter::new(file), info).map_err(|e| e.to_string())?;
	encoder.set_color(if frame.channels == 4 { png::ColorType::Rgba } else { png::ColorType::Rgb });
	let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
	writer.write_image_data(&frame.pixels).map_err(|e| e.to_string())
}

struct RenderJob<'a> {
	paths: &'a [PathBuf],
	sources: &'a [Rect],
	size: (u32, u32),
	fill: Option<&'a [u8]>,
	keep: Option<&'a Keep>,
	threads: usize,
	frames: Option<&'a Path>,
	avif: bool,
	profile: Profile,
	/// Photos to convert from sRGB to the clip's Display P3 before placing.
	convert: &'a [bool],
}

/// A placed frame as the sink takes it: YUV for libavif, RGB(A) bytes for ffmpeg.
enum Frame {
	Yuv(avif::Yuv),
	Rgb(pixels::Image),
}

/// Places each photo, kept or decoded again, on worker threads and hands the frames to the sink in order.
/// The first error stops the workers from starting another photo.
fn render(job: &RenderJob, sink: &mut Sink) -> Result<(), String> {
	let stop = AtomicBool::new(false);
	let place_one = |i: usize| -> Result<Frame, String> {
		if stop.load(Relaxed) {
			return Err(String::new());
		}
		let mut photo = match job.keep.and_then(|keep| keep.take(i)) {
			Some(photo) => photo,
			None => {
				let path = &job.paths[i];
				work(Work::Decode, || decode::decode(path)).map_err(|e| format!("{}: {e}", path.display()))?
			}
		};
		if job.convert[i] {
			work(Work::Place, || colour::srgb_to_p3(&mut photo));
		}
		let frame = work(Work::Place, || place(&photo, job.sources[i], job.size, job.fill));
		drop(photo);
		if let Some(dir) = job.frames {
			let path = dir.join(format!("{:04}.png", i + 1));
			save_png(&path, &frame, job.profile).map_err(|e| format!("{}: {e}", path.display()))?;
		}
		Ok(if job.avif {
			Frame::Yuv(work(Work::Place, || avif::to_yuv420(&frame)))
		} else {
			Frame::Rgb(frame)
		})
	};
	let mut failed = None;
	let mut waited = std::time::Instant::now();
	ordered(job.paths.len(), job.threads, QUEUE_DEPTH * job.threads, place_one, |_, frame| {
		profile::add(Work::EncoderWait, waited.elapsed());
		if failed.is_none() {
			let added = frame.and_then(|frame| match (&mut *sink, frame) {
				(Sink::Avif(encoder), Frame::Yuv(yuv)) => work(Work::Encode, || encoder.add(&yuv)),
				(Sink::Ffmpeg(child), Frame::Rgb(rgb)) => {
					// ffmpeg prints why it quit (a bad --vf, say); its exit code is reported below.
					let _ = child.stdin.as_mut().unwrap().write_all(&rgb.pixels);
					Ok(())
				}
				_ => unreachable!("frames are made for the sink"),
			});
			if let Err(e) = added {
				failed = Some(e);
				stop.store(true, Relaxed);
			}
		}
		waited = std::time::Instant::now();
	});
	failed.map_or(Ok(()), Err)
}

fn finish_sink(sink: Sink, out: &Path) -> Result<(), String> {
	match sink {
		Sink::Avif(encoder) => work(Work::Encode, || encoder.finish(out)),
		Sink::Ffmpeg(mut child) => {
			drop(child.stdin.take());
			let status = child.wait().map_err(|e| format!("ffmpeg: {e}"))?;
			if !status.success() {
				return Err(format!("ffmpeg exited {}", status.code().unwrap_or(-1)));
			}
			Ok(())
		}
	}
}

/// A failed render's sink: ffmpeg is stopped rather than left to finish a clip nobody will publish.
fn abandon_sink(sink: Sink) {
	if let Sink::Ffmpeg(mut child) = sink {
		let _ = child.kill();
		let _ = child.wait();
	}
}

/// What the `.json` sidecar holds: the run, its clip, and each photo's crop and points.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Sidecar {
	/// The run ID, also in the clip's metadata, so a clip can be matched with its .json.
	pub run: String,
	/// The clip's file name; `None` when only the .json is written.
	pub clip: Option<String>,
	/// One row per photo, in clip order.
	pub photos: Vec<Row>,
}

/// One photo in the sidecar. Positions are fractions of the upright photo's width and height.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Row {
	/// The photo's file name.
	pub file: String,
	/// The crop's left edge.
	pub x: f64,
	/// The crop's top edge.
	pub y: f64,
	/// The crop's width.
	pub w: f64,
	/// The crop's height.
	pub h: f64,
	/// The feature's size relative to the first photo, to 4 decimal places.
	pub scale: f64,
	/// How the feature was found: `tracked`, `mark`, `lost` (estimated) or `skipped` (estimated).
	pub source: &'static str,
	/// Every point of the set, `[x, y]`; their mean is the anchor.
	pub points: Vec<[f64; 2]>,
}

impl Sidecar {
	/// The .json's bytes: tab-indented, with a final newline.
	pub fn to_json(&self) -> Vec<u8> {
		let mut text = Vec::new();
		let mut serializer = serde_json::Serializer::with_formatter(
			&mut text,
			serde_json::ser::PrettyFormatter::with_indent(b"\t"),
		);
		serde::Serialize::serialize(self, &mut serializer).unwrap();
		text.push(b'\n');
		text
	}
}

/// A new run ID: 32 random hex digits.
pub fn run_id() -> String {
	let mut bytes = [0u8; 16];
	cache::random_bytes(&mut bytes);
	bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// This run's partial outputs. Dropping it removes the ones still there, so a failed run leaves nothing
/// behind; after `publish` has renamed them into place, there are none.
#[derive(Default)]
struct Partials(Vec<PathBuf>);

impl Partials {
	/// A new hidden file beside `path`, unique to this run, keeping the extension for the encoder.
	fn create(&mut self, path: &Path, run: &str) -> Result<PathBuf, String> {
		let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
		let name = format!(".{stem}.{run}.partial.{}", suffix(path));
		let partial = folder_of(path).join(name);
		std::fs::OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&partial)
			.map_err(|e| format!("{}: {e}", partial.display()))?;
		self.0.push(partial.clone());
		Ok(partial)
	}
}

impl Drop for Partials {
	fn drop(&mut self) {
		for partial in &self.0 {
			let _ = std::fs::remove_file(partial);
		}
	}
}

/// Renames the clip and its .json into place together, under an exclusive lock on the output folder.
fn publish(pairs: &[(&Path, &Path)], folder: &Path) -> Result<(), String> {
	let dir = std::fs::File::open(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
	use std::os::fd::AsRawFd;
	unsafe { libc::flock(dir.as_raw_fd(), libc::LOCK_EX) };
	for &(from, to) in pairs {
		std::fs::rename(from, to).map_err(|e| format!("{}: {e}", to.display()))?;
	}
	Ok(())
}

/// `~/Library/Caches/pinframe` on macOS; `$XDG_CACHE_HOME/pinframe`, default `~/.cache/pinframe`, elsewhere.
fn default_cache_dir() -> PathBuf {
	let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
	if cfg!(target_os = "macos") {
		home.join("Library/Caches/pinframe")
	} else {
		std::env::var_os("XDG_CACHE_HOME").map_or_else(|| home.join(".cache"), PathBuf::from).join("pinframe")
	}
}

/// A tracked [`Job`]: the photos in order, where the feature is in each, and the crop planned around it.
/// When the job's output is a clip, it also holds the decoded photos that fit its memory plan, for
/// [`Tracked::write`].
pub struct Tracked {
	paths: Vec<PathBuf>,
	sizes: Vec<(u32, u32)>,
	path: Vec<Hit>,
	sources: Vec<Rect>,
	size: (u32, u32),
	fill: Option<Vec<u8>>,
	output: Option<Output>,
	keep: Option<Keep>,
	profiles: Vec<Profile>,
	render_threads: usize,
}

/// Checks the job, then decodes the photos, finds their features (or reads them from the feature cache),
/// tracks the marked feature through them and plans the crop. Nothing is written but cache entries and,
/// when the job's output asks for frames, the `--frames` folder. `log` gets the lines the CLI prints:
/// the order, the memory plan, one line per photo and the clip size.
///
/// The memory plan takes 75% of the memory free when it starts; jobs run side by side each plan their own.
pub fn track(job: &Job, log: &mut dyn FnMut(&str)) -> Result<Tracked, String> {
	let output = job.output.as_ref();
	let clip = output.is_some_and(|o| suffix(&o.out) != "json");
	let (paths, sizes, marks, skips, focals, fill, k, resources, cache, profiles) =
		step("check input and plan resources", || -> Result<_, String> {
			let (paths, focals, order) = order_photos(job.photos.clone(), job.order)?;
			log(&format!("order: {order}"));
			let fill = job.pad.as_deref().map(parse_fill).transpose()?;
			if let Some(o) = output {
				check_output(&o.out, fill.as_ref().is_some_and(|f| f.len() == 4), o.vf.as_deref())?;
			}
			if job.no_cache && job.cache.is_some() {
				return Err("use either --cache DIR or --no-cache".into());
			}
			if let Some(o) = output {
				if !clip && o.frames.is_some() {
					return Err("--frames needs a clip --out, not .json".into());
				}
				if o.fps.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
					return Err("--fps must be positive".into());
				}
			}
			let sizes = read_sizes(&paths)?;
			let (marks, soft) =
				read_marks(job.marks.points.as_deref(), &job.marks.at, &job.marks.near, &paths, &sizes)?;
			// track() places a skipped photo with a mark at it, without tracking from it: a soft mark.
			let skips = read_skips(&job.marks.skip, &paths, &marks)?;
			let skips: Vec<bool> = skips.into_iter().zip(soft).map(|(skip, soft)| skip || soft).collect();
			if let Some(dir) = output.and_then(|o| o.frames.as_ref()) {
				std::fs::create_dir_all(dir).map_err(|e| format!("--frames {}: {e}", dir.display()))?;
				let has_png = std::fs::read_dir(dir).unwrap().flatten().any(|e| suffix(&e.path()) == "png");
				if has_png {
					return Err(format!("--frames {}: already has PNGs; empty it first", dir.display()));
				}
			}
			let k = track::track_factor(&sizes);
			let free = available_bytes();
			let largest = sizes.iter().map(|&(w, h)| w as usize * h as usize).max().unwrap();
			let (prepare_decode_bytes, render_decode_bytes) = paths
				.iter()
				.zip(&sizes)
				.map(|(path, &(w, h))| {
					let pixels = w as usize * h as usize;
					decode::memory_cost(path)
						.map(|(prepare, render)| (prepare * pixels, render * pixels))
						.map_err(|e| format!("{}: {e}", path.display()))
				})
				.collect::<Result<Vec<_>, _>>()?
				.into_iter()
				.fold((0, 0), |(prepare_max, render_max), (prepare, render)| {
					(prepare_max.max(prepare), render_max.max(render))
				});
			let resources =
				plan_resources(largest, prepare_decode_bytes, render_decode_bytes, free, cores())?;
			log(&format!(
				"{:.1} GB free: {} prepare threads, {} render threads, up to {:.1} GB of kept photos",
				free as f64 / (1u64 << 30) as f64,
				resources.prepare_threads,
				resources.render_threads,
				resources.keep_bytes as f64 / (1u64 << 30) as f64
			));
			let cache = if job.no_cache {
				None
			} else {
				let root = job.cache.clone().unwrap_or_else(default_cache_dir);
				Some(cache::FeatureCache::new(root)?)
			};
			let profiles = if clip { read_profiles(&paths)? } else { vec![] };
			Ok((paths, sizes, marks, skips, focals, fill, k, resources, cache, profiles))
		})?;
	let keep = clip.then(|| Keep::new(resources.keep_bytes));
	let path = step("decode, find features, track", || {
		let threads = resources.prepare_threads;
		// Set once a photo fails or tracking stops early, so the workers start no more photos.
		let stop = AtomicBool::new(false);
		std::thread::scope(|scope| {
			let (tx, rx) = sync_channel(1);
			let (paths, sizes, keep, cache, stop) = (&paths, &sizes, keep.as_ref(), cache.as_ref(), &stop);
			scope.spawn(move || {
				let prepare_one = |i: usize| {
					if stop.load(Relaxed) {
						return Err(String::new());
					}
					prepare(&paths[i], sizes[i], k, keep, i, cache)
				};
				ordered(paths.len(), threads, QUEUE_DEPTH * threads, prepare_one, |_, photo| {
					// Tracking stops at the first error, and its receiver is gone once it stops.
					let failed = photo.is_err();
					if tx.send(photo).is_err() || failed {
						stop.store(true, Relaxed);
					}
				});
			});
			track::track(rx.into_iter(), sizes, k, &marks, &focals, &skips)
		})
	})?;
	let (path, sources, size) = step("plan the crop", || -> Result<_, String> {
		let path = track::fill_lost(path, &sizes)?;
		let (sources, size) = track::plan(&path, &sizes, fill.is_some())?;
		Ok((path, sources, size))
	})?;
	for ((p, &(fw, fh)), hit) in paths.iter().zip(&sizes).zip(&path) {
		log(&format!(
			"{}  {fw}x{fh}  anchor {:.0},{:.0}  scale {:.2}  {}",
			file_name(p),
			hit.x,
			hit.y,
			hit.scale,
			hit.note
		));
	}
	log(&format!("clip {}x{}", size.0, size.1));
	let output = job.output.clone();
	let render_threads = resources.render_threads;
	Ok(Tracked { paths, sizes, path, sources, size, fill, output, keep, profiles, render_threads })
}

impl Tracked {
	/// The clip's width and height, in pixels.
	pub fn size(&self) -> (u32, u32) {
		self.size
	}

	/// The sidecar for this tracking, as `run` with `clip` (a file name) would write it.
	pub fn sidecar(&self, run: &str, clip: Option<&str>) -> Sidecar {
		let photos = self
			.paths
			.iter()
			.zip(&self.sizes)
			.zip(&self.sources)
			.zip(&self.path)
			.map(|(((p, &(fw, fh)), &(x, y, w, h)), hit)| {
				let points: Vec<[f64; 2]> =
					hit.points.iter().map(|&(px, py)| [px / fw as f64, py / fh as f64]).collect();
				assert!(
					points.iter().flatten().all(|v| v.is_finite()),
					"non-finite point for {}",
					file_name(p)
				);
				Row {
					file: file_name(p),
					x: x / fw as f64,
					y: y / fh as f64,
					w: w / fw as f64,
					h: h / fh as f64,
					scale: format!("{:.4}", hit.scale).parse().unwrap(),
					source: hit.source.name(),
					points,
				}
			})
			.collect();
		Sidecar { run: run.to_string(), clip: clip.map(str::to_string), photos }
	}

	/// Writes the job's output under a new run ID: the clip, if it is one, and the .json sidecar. Both are
	/// written under names unique to the run, then renamed into place together under a lock on their
	/// folder: a failed run leaves the previous pair as it was, and runs sharing an output publish whole
	/// pairs, one at a time. `log` gets the line the CLI prints when sRGB photos are converted. Returns the
	/// sidecar as written; a job without an output has nothing to write.
	pub fn write(self, log: &mut dyn FnMut(&str)) -> Result<Sidecar, String> {
		let Some(output) = &self.output else {
			return Err("nothing to write: the job has no output".into());
		};
		let clip = suffix(&output.out) != "json";
		let run = run_id();
		let rects = if clip { output.out.with_extension("json") } else { output.out.clone() };
		let mut partials = Partials::default();
		let rects_partial = partials.create(&rects, &run)?;
		let clip_partial = if clip { Some(partials.create(&output.out, &run)?) } else { None };
		let alpha = self.fill.as_ref().is_some_and(|f| f.len() == 4);
		if let Some(clip_partial) = clip_partial.as_ref() {
			let profile = colour::clip_profile(&self.profiles);
			let convert: Vec<bool> = self.profiles.iter().map(|&p| p != profile).collect();
			let converted = convert.iter().filter(|&&c| c).count();
			if converted > 0 {
				log(&format!("converting {converted} sRGB photos to Display P3, the other photos' profile"));
			}
			step("render: place and encode", || -> Result<(), String> {
				let filters = output.vf.as_deref();
				let options =
					SinkOptions { size: self.size, alpha, fps: output.fps, filters, run: &run, profile };
				let mut sink = open_sink(clip_partial, &options)?;
				let job = RenderJob {
					paths: &self.paths,
					sources: &self.sources,
					size: self.size,
					fill: self.fill.as_deref(),
					keep: self.keep.as_ref(),
					threads: self.render_threads,
					frames: output.frames.as_deref(),
					avif: suffix(&output.out) == "avif",
					profile,
					convert: &convert,
				};
				if let Err(e) = render(&job, &mut sink) {
					abandon_sink(sink);
					return Err(e);
				}
				finish_sink(sink, clip_partial)
			})?;
		}
		step("write the JSON", || {
			let sidecar = self.sidecar(&run, clip.then(|| file_name(&output.out)).as_deref());
			std::fs::write(&rects_partial, sidecar.to_json())
				.map_err(|e| format!("{}: {e}", rects_partial.display()))?;
			let pairs = if let Some(clip_partial) = clip_partial.as_ref() {
				vec![
					(clip_partial.as_path(), output.out.as_path()),
					(rects_partial.as_path(), rects.as_path()),
				]
			} else {
				vec![(rects_partial.as_path(), rects.as_path())]
			};
			publish(&pairs, &folder_of(&output.out))?;
			Ok(sidecar)
		})
	}
}

/// Starts measuring the steps and work of the runs that follow, when the environment variable
/// `PINFRAME_PROFILE` names a file to write to; otherwise nothing is measured.
pub fn start_profile() {
	profile::start();
}

/// Appends what was measured since [`start_profile`] to `PINFRAME_PROFILE`'s file, as one JSON line.
pub fn finish_profile() {
	profile::finish();
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A minimal Exif/TIFF blob (CIPA DC-008 "eXIf" chunk payload) with one field: `DateTimeOriginal`.
	fn exif_date_time_original(date: &str) -> Vec<u8> {
		assert_eq!(date.len(), 19, "EXIF dates are YYYY:MM:DD HH:MM:SS");
		let mut buf = Vec::new();
		buf.extend(b"II");
		buf.extend(42u16.to_le_bytes());
		buf.extend(8u32.to_le_bytes()); // IFD0 at offset 8
		buf.extend(1u16.to_le_bytes()); // IFD0: one entry
		buf.extend(0x8769u16.to_le_bytes()); // ExifIFDPointer
		buf.extend(4u16.to_le_bytes()); // LONG
		buf.extend(1u32.to_le_bytes());
		buf.extend(26u32.to_le_bytes()); // Exif IFD at offset 26
		buf.extend(0u32.to_le_bytes()); // no next IFD
		assert_eq!(buf.len(), 26);
		buf.extend(1u16.to_le_bytes()); // Exif IFD: one entry
		buf.extend(0x9003u16.to_le_bytes()); // DateTimeOriginal
		buf.extend(2u16.to_le_bytes()); // ASCII
		buf.extend(20u32.to_le_bytes()); // 19 chars + NUL
		buf.extend(44u32.to_le_bytes()); // string at offset 44
		buf.extend(0u32.to_le_bytes()); // no next IFD
		assert_eq!(buf.len(), 44);
		buf.extend(date.as_bytes());
		buf.push(0);
		buf
	}

	/// A 1x1 PNG, with an `eXIf` chunk carrying `DateTimeOriginal` when `date` is given.
	fn write_test_png(path: &Path, date: Option<&str>) {
		let file = std::fs::File::create(path).unwrap();
		let mut info = png::Info::with_size(1, 1);
		info.color_type = png::ColorType::Rgb;
		let mut writer =
			png::Encoder::with_info(std::io::BufWriter::new(file), info).unwrap().write_header().unwrap();
		if let Some(date) = date {
			writer.write_chunk(png::chunk::ChunkType(*b"eXIf"), &exif_date_time_original(date)).unwrap();
		}
		writer.write_image_data(&[0, 0, 0]).unwrap();
	}

	#[test]
	fn capture_time_orders_photos_whose_names_disagree_with_their_times() {
		let dir = std::env::temp_dir().join(format!("pinframe-order-{}", std::process::id()));
		std::fs::create_dir_all(&dir).unwrap();
		let files = [
			("c.png", "2024:01:01 00:00:03"),
			("a.png", "2024:01:01 00:00:01"),
			("b.png", "2024:01:01 00:00:02"),
		];
		let paths: Vec<PathBuf> = files
			.iter()
			.map(|(name, date)| {
				let path = dir.join(name);
				write_test_png(&path, Some(date));
				path
			})
			.collect();
		let (ordered, _, reason) = order_photos(paths, Order::Time).unwrap();
		let names: Vec<String> = ordered.iter().map(|p| file_name(p)).collect();
		assert_eq!(names, ["a.png", "b.png", "c.png"], "capture time, not the names, decides order");
		assert!(reason.starts_with("time:"), "{reason}");
		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[test]
	fn a_photo_with_no_capture_time_falls_back_to_name_order() {
		let dir = std::env::temp_dir().join(format!("pinframe-order-missing-{}", std::process::id()));
		std::fs::create_dir_all(&dir).unwrap();
		let with_time = dir.join("b.png");
		write_test_png(&with_time, Some("2024:01:01 00:00:01"));
		let without_time = dir.join("a.png");
		write_test_png(&without_time, None);
		let (ordered, _, reason) =
			order_photos(vec![with_time.clone(), without_time.clone()], Order::Time).unwrap();
		assert_eq!(ordered, vec![without_time, with_time], "falls back to filename order");
		assert_eq!(reason, "name: 1 photo has no capture time");
		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[test]
	fn bad_input_is_refused_and_good_input_passes() {
		for colour in ["purple", "#12345", "#gggggg", "#1234567"] {
			assert!(parse_fill(colour).is_err(), "{colour}");
		}
		assert_eq!(parse_fill("transparent"), Ok(vec![0, 0, 0, 0]));
		assert_eq!(parse_fill("#ff8000"), Ok(vec![255, 128, 0]));
		assert!(check_output(Path::new("clip.gif"), false, None).is_err());
		assert!(check_output(Path::new("clip.avif"), false, Some("deflicker")).is_err());
		assert!(check_output(Path::new("no/such/folder/clip.avif"), false, None).is_err());
		assert!(read_sizes(&[PathBuf::from("only-one.avif")]).is_err());
		assert!(read_sizes(&[PathBuf::from("a/x.avif"), PathBuf::from("b/x.avif")]).is_err());
		assert!(read_sizes(&[PathBuf::from("missing-1.avif"), PathBuf::from("missing-2.avif")]).is_err());
	}

	#[test]
	fn points_parse_one_several_and_zoom() {
		assert_eq!(parse_points("--points", "5,5").unwrap(), (vec![(5.0, 5.0)], None));
		assert_eq!(
			parse_points("--points", "1,2/3,4/5,6").unwrap(),
			(vec![(1.0, 2.0), (3.0, 4.0), (5.0, 6.0)], None)
		);
		assert_eq!(parse_points("--at a.avif", "1,2@2.5").unwrap(), (vec![(1.0, 2.0)], Some(2.5)));
		assert_eq!(
			parse_points("--at a.avif", "1,2/3,4@0.5").unwrap(),
			(vec![(1.0, 2.0), (3.0, 4.0)], Some(0.5))
		);
	}

	#[test]
	fn points_reject_bad_input() {
		assert!(parse_points("--points", "").is_err(), "empty");
		assert!(parse_points("--points", "1,2,3").is_err(), "3 numbers is not a point");
		let box_err = parse_points("--points", "1,2,3,4").unwrap_err();
		assert!(box_err.contains("quarter"), "{box_err}");
		let many = (0..=MAX_POINTS).map(|i| format!("{i},{i}")).collect::<Vec<_>>().join("/");
		assert!(parse_points("--points", &many).is_err(), "more than {MAX_POINTS} points");
		assert!(parse_points("--points", "1,2@0").is_err(), "zoom must be positive");
		assert!(parse_points("--points", "1,2@-1").is_err(), "zoom must be positive");
		assert!(parse_points("--points", "1,2@x").is_err(), "@ without a number");
	}

	#[test]
	fn points_must_land_inside_the_photo() {
		let photo = (100, 80);
		assert!(check_points("p.avif", &[(99.0, 79.0)], photo).is_ok());
		assert!(check_points("p.avif", &[(0.0, 0.0), (99.0, 79.0)], photo).is_ok());
		assert!(check_points("p.avif", &[(100.0, 40.0)], photo).is_err());
		assert!(check_points("p.avif", &[(-1.0, 40.0)], photo).is_err());
		assert!(check_points("p.avif", &[(f64::NAN, 40.0)], photo).is_err());
	}

	#[test]
	fn marks_are_read_by_photo_and_box_forms_are_refused() {
		let photo = (100, 80);
		let two = [PathBuf::from("a.avif"), PathBuf::from("b.avif")];
		let bad_ats: [&[&str]; 5] =
			[&["b.avif"], &["c.avif=1,1"], &["a.avif=1,1"], &["b.avif=1,1", "b.avif=2,2"], &["b.avif=500,1"]];
		for ats in bad_ats {
			let ats: Vec<String> = ats.iter().map(|a| a.to_string()).collect();
			assert!(read_marks(Some("1,1/9,9"), &ats, &[], &two, &[photo, photo]).is_err(), "{ats:?}");
		}
		let (marks, _) =
			read_marks(Some("1,1/9,9"), &["b.avif=5,5".into()], &[], &two, &[photo, photo]).unwrap();
		assert_eq!(marks[0], Some(Mark { points: vec![(1.0, 1.0), (9.0, 9.0)], zoom: None }));
		assert_eq!(marks[1], Some(Mark { points: vec![(5.0, 5.0)], zoom: None }));
		assert!(read_marks(None, &[], &[], &two, &[photo, photo]).is_err(), "--points is required");
		assert!(
			read_marks(Some("1,1@2.0"), &[], &[], &two, &[photo, photo]).is_err(),
			"--points takes no zoom"
		);
		let box_err = read_marks(Some("0,0,10,10"), &[], &[], &two, &[photo, photo]).unwrap_err();
		assert!(box_err.contains("quarter"), "{box_err}");
		let box_at_err =
			read_marks(Some("1,1"), &["b.avif=0,0,10,10".into()], &[], &two, &[photo, photo]).unwrap_err();
		assert!(box_at_err.contains("quarter"), "{box_at_err}");
	}

	#[test]
	fn marks_built_from_points_read_back_exactly() {
		let first = [(940.0, 637.5), (1140.1, 0.1 + 0.2)];
		let at = Mark { points: vec![(1.0 / 3.0, 2.0f64.sqrt()), (99.75, 1e-7)], zoom: Some(1.0 / 7.0) };
		let near = Mark { points: vec![(5.5, 6.25)], zoom: None };
		let marks = Marks::new(&first).with_at("b.avif", &at).with_skip("c.avif").with_near("d.avif", &near);
		let paths = ["a.avif", "b.avif", "c.avif", "d.avif"].map(PathBuf::from);
		let (read, soft) =
			read_marks(marks.points.as_deref(), &marks.at, &marks.near, &paths, &[(2000, 2000); 4]).unwrap();
		assert_eq!(read, [Some(Mark { points: first.to_vec(), zoom: None }), Some(at), None, Some(near)]);
		assert_eq!(soft, [false, false, false, true]);
		assert_eq!(read_skips(&marks.skip, &paths, &read).unwrap(), [false, false, true, false]);
		for nears in [["a.avif=1,1"], ["b.avif=1,1"], ["d.avif=3000,1"]] {
			let nears = nears.map(String::from);
			assert!(
				read_marks(marks.points.as_deref(), &marks.at, &nears, &paths, &[(2000, 2000); 4]).is_err()
			);
		}
		assert!(read_skips(&["d.avif".into()], &paths, &read).is_err(), "a soft-marked photo is not skipped");
	}

	/// A `w`x`h` RGB PNG cut at `(x, y)` from a fixed field of blurred noise, so SIFT finds features and
	/// photos cut at nearby offsets overlap.
	fn write_noise_png(path: &Path, (x, y): (usize, usize), (w, h): (usize, usize)) {
		let (fw, fh) = (320, 240);
		assert!(x + w + 4 <= fw && y + h + 4 <= fh);
		let mut seed = 0x2545_f491_4f6c_dd1du64;
		let noise: Vec<u32> = (0..fw * fh)
			.map(|_| {
				seed ^= seed << 13;
				seed ^= seed >> 7;
				seed ^= seed << 17;
				(seed >> 56) as u32
			})
			.collect();
		let mut rgb = Vec::with_capacity(w * h * 3);
		for row in y..y + h {
			for col in x..x + w {
				let blurred: u32 = (0..4)
					.flat_map(|dy| (0..4).map(move |dx| (dx, dy)))
					.map(|(dx, dy)| noise[(row + dy) * fw + col + dx])
					.sum::<u32>() / 16;
				rgb.extend([blurred as u8; 3]);
			}
		}
		let mut info = png::Info::with_size(w as u32, h as u32);
		info.color_type = png::ColorType::Rgb;
		let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
		png::Encoder::with_info(file, info).unwrap().write_header().unwrap().write_image_data(&rgb).unwrap();
	}

	#[test]
	fn a_job_without_output_tracks_in_process_and_writes_nothing() {
		let dir = std::env::temp_dir().join(format!("pinframe-job-{}", std::process::id()));
		std::fs::create_dir_all(&dir).unwrap();
		let photos: Vec<PathBuf> = [(0, 0), (6, 3), (12, 6), (18, 9)]
			.iter()
			.zip(["a.png", "b.png", "c.png", "d.png"])
			.map(|(&offset, name)| {
				write_noise_png(&dir.join(name), offset, (240, 180));
				dir.join(name)
			})
			.collect();
		let first = [(110.0, 80.0), (130.0, 80.0), (110.0, 100.0), (130.0, 100.0)];
		let at = Mark { points: first.iter().map(|&(x, y)| (x - 6.0, y - 3.0)).collect(), zoom: None };
		let marks = Marks::new(&first).with_at("b.png", &at).with_skip("c.png");
		let job = Job { photos: photos.into_iter().rev().collect(), marks, no_cache: true, ..Job::default() };
		let mut lines = Vec::new();
		let tracked = track(&job, &mut |line| lines.push(line.to_string())).unwrap();
		assert_eq!(lines[0], "order: name: 4 photos have no capture time");
		assert_eq!(lines.last().unwrap(), &format!("clip {}x{}", tracked.size().0, tracked.size().1));
		let sidecar = tracked.sidecar("run", None);
		let rows: Vec<(&str, &str)> =
			sidecar.photos.iter().map(|row| (row.file.as_str(), row.source)).collect();
		assert_eq!(rows, [("a.png", "mark"), ("b.png", "mark"), ("c.png", "skipped"), ("d.png", "tracked")]);
		let d = &sidecar.photos[3].points[0];
		assert!((d[0] * 240.0 - 92.0).abs() < 0.5 && (d[1] * 180.0 - 71.0).abs() < 0.5, "{d:?}");
		assert!(sidecar.to_json().starts_with(b"{\n\t\"run\": \"run\",\n\t\"clip\": null,"));
		assert_eq!(tracked.write(&mut |_| {}).unwrap_err(), "nothing to write: the job has no output");
		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[test]
	fn resources_stop_at_cores_then_memory() {
		let gb = 1usize << 30;
		let train_clouds =
			plan_resources(2048 * 1536, 6 * 2048 * 1536, 11 * 2048 * 1536, 4 * gb, 12).unwrap();
		assert!(
			train_clouds.prepare_threads == 12
				&& train_clouds.render_threads == 12
				&& train_clouds.keep_bytes > gb / 2
		);
		let large = plan_resources(8064 * 6048, 6 * 8064 * 6048, 11 * 8064 * 6048, 4 * gb, 12).unwrap();
		assert!((1..12).contains(&large.prepare_threads) && large.render_threads >= 1);
		assert!(
			plan_resources(8064 * 6048, 6 * 8064 * 6048, 11 * 8064 * 6048, gb / 2, 12).is_err(),
			"a 48 MP photo cannot fit in 0.5 GB"
		);
	}
}
