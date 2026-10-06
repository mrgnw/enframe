//! pinframe in Rust: pin one feature in place across a photo sequence. Same flags and outputs as
//! `anchor.py`, for AVIF photos.

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

use colour::Profile;
use profile::{Work, step, work};
use render::{Keep, ordered, place};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::sync_channel;
use track::{Hit, Mark, Photo, Rect};

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

const USAGE: &str = "usage: pinframe PHOTO... --points x,y[/x,y]... [--at PHOTO=x,y[/x,y]...[@zoom]]...
				[--skip PHOTO]... [--pad COLOUR] [--fps N] [--out CLIP-or-JSON] [--frames DIR]
				[--vf FILTERS] [--cache DIR|--no-cache] [--order time|name]";

/// The message given when `--box` or an old `x,y,w,h` mark is used: the conversion to point form.
const BOX_REMOVED: &str = "--box is removed; use --points, and turn a box x,y,w,h into its four quarter \
	centres: x+w/4,y+h/4/x+3w/4,y+h/4/x+w/4,y+3h/4/x+3w/4,y+3h/4";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Order {
	/// EXIF capture time, falling back to filename order if any photo lacks one.
	Time,
	Name,
}

/// Stops on input or photos pinframe cannot use: a message and exit code 1. Asserts stay for invariants.
pub fn fail(message: &str) -> ! {
	for partial in PARTIALS.lock().map(|p| p.clone()).unwrap_or_default() {
		let _ = std::fs::remove_file(partial);
	}
	eprintln!("pinframe: {message}");
	std::process::exit(1);
}

/// This run's partial outputs, removed by `fail` so a failed run leaves nothing behind.
static PARTIALS: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

struct Args {
	photos: Vec<PathBuf>,
	points: Option<String>,
	at: Vec<String>,
	skip: Vec<String>,
	pad: Option<String>,
	fps: f64,
	out: PathBuf,
	frames: Option<PathBuf>,
	vf: Option<String>,
	cache: Option<PathBuf>,
	no_cache: bool,
	order: Order,
}

fn parse_args() -> Args {
	let mut args = Args {
		photos: vec![],
		points: None,
		at: vec![],
		skip: vec![],
		pad: None,
		fps: 4.0,
		out: PathBuf::from("out.avif"),
		frames: None,
		vf: None,
		cache: None,
		no_cache: false,
		order: Order::Time,
	};
	let mut it = std::env::args().skip(1);
	while let Some(arg) = it.next() {
		let mut value = || it.next().unwrap_or_else(|| fail(&format!("{arg} needs a value\n{USAGE}")));
		match arg.as_str() {
			"--points" => args.points = Some(value()),
			"--box" => fail(&format!("{BOX_REMOVED}\n{USAGE}")),
			"--at" => args.at.push(value()),
			"--skip" => args.skip.push(value()),
			"--pad" => args.pad = Some(value()),
			"--fps" => args.fps = value().parse().unwrap_or_else(|_| fail("--fps must be a number")),
			"--out" => args.out = PathBuf::from(value()),
			"--frames" => args.frames = Some(PathBuf::from(value())),
			"--vf" => args.vf = Some(value()),
			"--cache" => args.cache = Some(PathBuf::from(value())),
			"--no-cache" => args.no_cache = true,
			"--order" => {
				args.order = match value().as_str() {
					"time" => Order::Time,
					"name" => Order::Name,
					other => fail(&format!("--order {other}: use time or name")),
				}
			}
			"-h" | "--help" => {
				println!("{USAGE}");
				std::process::exit(0);
			}
			flag if flag.starts_with("--") => fail(&format!("unknown option {flag}\n{USAGE}")),
			photo => args.photos.push(PathBuf::from(photo)),
		}
	}
	args
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

/// Orders photos by EXIF capture time, falling back to filename order when `--order name` was given or
/// any photo lacks a capture time (a mix of cameras, or an export that stripped EXIF). Marks are matched
/// to photos by file name (`read_marks`), so reordering here never loses track of which mark goes where.
/// Each photo's 35 mm focal length comes back in the same order.
fn order_photos(paths: Vec<PathBuf>, order: Order) -> (Vec<PathBuf>, Vec<Option<f64>>, String) {
	let exifs: Vec<decode::Exif> = paths
		.iter()
		.map(|p| decode::exif(p).unwrap_or_else(|e| fail(&format!("{}: {e}", p.display()))))
		.collect();
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
	(sorted.into_iter().map(|i| paths[i].clone()).collect(), focals, reason)
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

/// --points and every --at, by photo index, checked against the photo sizes.
fn read_marks(
	points: Option<&str>,
	ats: &[String],
	paths: &[PathBuf],
	sizes: &[(u32, u32)],
) -> Result<Vec<Option<Mark>>, String> {
	let names: Vec<String> = paths.iter().map(|p| file_name(p)).collect();
	// ponytail: no point drawing (anchor.py opens an OpenCV window); --points is required here
	let (first_points, first_zoom) =
		parse_points("--points", points.ok_or("--points x,y[/x,y]... is required")?)?;
	if first_zoom.is_some() {
		return Err("--points takes no zoom; the first photo defines scale 1".into());
	}
	let mut marks: Vec<Option<Mark>> = vec![None; paths.len()];
	marks[0] = Some(Mark { points: first_points, zoom: None });
	for at in ats {
		let Some((name, value)) = at.split_once('=').filter(|(n, _)| names.iter().any(|m| m == n)) else {
			return Err(format!("--at {at}: expected PHOTO=x,y with PHOTO one of the photo file names"));
		};
		let i = names.iter().position(|m| m == name).unwrap();
		if marks[i].is_some() {
			let why = if i == 0 { "use --points for the first photo" } else { "marked twice" };
			return Err(format!("--at {name}: {why}"));
		}
		let (points, zoom) = parse_points(&format!("--at {name}"), value)?;
		marks[i] = Some(Mark { points, zoom });
	}
	for (i, mark) in marks.iter().enumerate() {
		if let Some(m) = mark {
			check_points(&names[i], &m.points, sizes[i])?;
		}
	}
	Ok(marks)
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
			return Err(format!("--skip {name}: already marked with --at"));
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
) -> Photo {
	let small = ((size.0 as f64 * k).round() as usize, (size.1 as f64 * k).round() as usize);
	let key = cache.zip(
		decode::decoder_id(path)
			.and_then(|decoder| cache::key(path, decoder, k, small, track::TRACK_EDGE))
			.ok(),
	);
	if let Some(cached) = key.as_ref().and_then(|(cache, key)| cache.load(key)) {
		if let Some(keep) = keep {
			let rgb = work(Work::Decode, || decode::decode(path))
				.unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
			keep.offer(i, rgb);
		}
		return Photo { small: cached.small, features: cached.features };
	}
	let rgb = work(Work::Decode, || decode::decode(path))
		.unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
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
	photo
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

fn open_sink(out: &Path, o: &SinkOptions) -> Sink {
	if suffix(out) == "avif" {
		let threads = std::thread::available_parallelism().map_or(1, |n| n.get()) as i32;
		let frame_ms = (1000.0 / o.fps).round_ties_even() as u64;
		let xmp = run_xmp(o.run);
		return Sink::Avif(avif::Encoder::new(AVIF_SPEED, AVIF_QUALITY, frame_ms, threads, xmp, o.profile));
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
	let child =
		cmd.stdin(std::process::Stdio::piped()).spawn().unwrap_or_else(|e| fail(&format!("ffmpeg: {e}")));
	Sink::Ffmpeg(child)
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
	keep: &'a Keep,
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
fn render(job: &RenderJob, sink: &mut Sink) {
	let place_one = |i: usize| {
		let mut photo = job.keep.take(i).unwrap_or_else(|| {
			let path = &job.paths[i];
			work(Work::Decode, || decode::decode(path))
				.unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())))
		});
		if job.convert[i] {
			work(Work::Place, || colour::srgb_to_p3(&mut photo));
		}
		let frame = work(Work::Place, || place(&photo, job.sources[i], job.size, job.fill));
		drop(photo);
		if let Some(dir) = job.frames {
			let path = dir.join(format!("{:04}.png", i + 1));
			save_png(&path, &frame, job.profile)
				.unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
		}
		if job.avif { Frame::Yuv(work(Work::Place, || avif::to_yuv420(&frame))) } else { Frame::Rgb(frame) }
	};
	let mut waited = std::time::Instant::now();
	ordered(job.paths.len(), job.threads, QUEUE_DEPTH * job.threads, place_one, |_, frame| {
		profile::add(Work::EncoderWait, waited.elapsed());
		match (&mut *sink, frame) {
			(Sink::Avif(encoder), Frame::Yuv(yuv)) => {
				work(Work::Encode, || encoder.add(&yuv)).unwrap_or_else(|e| fail(&e))
			}
			(Sink::Ffmpeg(child), Frame::Rgb(rgb)) => {
				// ffmpeg prints why it quit (a bad --vf, say); its exit code is reported below.
				let _ = child.stdin.as_mut().unwrap().write_all(&rgb.pixels);
			}
			_ => unreachable!("frames are made for the sink"),
		}
		waited = std::time::Instant::now();
	});
}

fn finish_sink(sink: Sink, out: &Path) {
	match sink {
		Sink::Avif(encoder) => work(Work::Encode, || encoder.finish(out)).unwrap_or_else(|e| fail(&e)),
		Sink::Ffmpeg(mut child) => {
			drop(child.stdin.take());
			let status = child.wait().unwrap_or_else(|e| fail(&format!("ffmpeg: {e}")));
			if !status.success() {
				fail(&format!("ffmpeg exited {}", status.code().unwrap_or(-1)));
			}
		}
	}
}

#[derive(serde::Serialize)]
struct Row {
	file: String,
	x: f64,
	y: f64,
	w: f64,
	h: f64,
	scale: f64,
	source: &'static str,
	points: Vec<[f64; 2]>,
}

#[derive(serde::Serialize)]
struct Sidecar {
	run: String,
	clip: Option<String>,
	photos: Vec<Row>,
}

fn write_rects(
	paths: &[PathBuf],
	sizes: &[(u32, u32)],
	sources: &[Rect],
	path: &[Hit],
	out: &Path,
	run: &str,
	clip: Option<&str>,
) {
	let photos = paths
		.iter()
		.zip(sizes)
		.zip(sources)
		.zip(path)
		.map(|(((p, &(fw, fh)), &(x, y, w, h)), hit)| {
			let points: Vec<[f64; 2]> =
				hit.points.iter().map(|&(px, py)| [px / fw as f64, py / fh as f64]).collect();
			assert!(points.iter().flatten().all(|v| v.is_finite()), "non-finite point for {}", file_name(p));
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
	let sidecar = Sidecar { run: run.to_string(), clip: clip.map(str::to_string), photos };
	let mut text = Vec::new();
	let mut serializer = serde_json::Serializer::with_formatter(
		&mut text,
		serde_json::ser::PrettyFormatter::with_indent(b"\t"),
	);
	serde::Serialize::serialize(&sidecar, &mut serializer).unwrap();
	text.push(b'\n');
	std::fs::write(out, text).unwrap_or_else(|e| fail(&format!("{}: {e}", out.display())));
}

fn random_hex() -> String {
	let mut bytes = [0u8; 16];
	cache::random_bytes(&mut bytes);
	bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A new hidden file beside `path`, unique to this run, keeping the extension for the encoder.
fn partial(path: &Path, run: &str) -> PathBuf {
	let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
	let name = format!(".{stem}.{run}.partial.{}", suffix(path));
	let partial = folder_of(path).join(name);
	std::fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(&partial)
		.unwrap_or_else(|e| fail(&format!("{}: {e}", partial.display())));
	PARTIALS.lock().unwrap().push(partial.clone());
	partial
}

/// Renames the clip and its .json into place together, under an exclusive lock on the output folder.
fn publish(pairs: &[(&Path, &Path)], folder: &Path) {
	let dir = std::fs::File::open(folder).unwrap_or_else(|e| fail(&format!("{}: {e}", folder.display())));
	use std::os::fd::AsRawFd;
	unsafe { libc::flock(dir.as_raw_fd(), libc::LOCK_EX) };
	for &(from, to) in pairs {
		std::fs::rename(from, to).unwrap_or_else(|e| fail(&format!("{}: {e}", to.display())));
	}
}

fn main() {
	profile::start();
	let args = parse_args();
	let (paths, sizes, marks, skips, focals, fill, k, resources, cache, json_only, profiles) =
		step("check input and plan resources", || {
			let (paths, focals, order) = order_photos(args.photos.clone(), args.order);
			println!("order: {order}");
			let fill = args.pad.as_deref().map(parse_fill).transpose().unwrap_or_else(|e| fail(&e));
			check_output(&args.out, fill.as_ref().is_some_and(|f| f.len() == 4), args.vf.as_deref())
				.unwrap_or_else(|e| fail(&e));
			if args.no_cache && args.cache.is_some() {
				fail("use either --cache DIR or --no-cache")
			}
			let json_only = suffix(&args.out) == "json";
			if json_only && args.frames.is_some() {
				fail("--frames needs a clip --out, not .json")
			}
			if args.fps.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
				fail("--fps must be positive");
			}
			let sizes = read_sizes(&paths).unwrap_or_else(|e| fail(&e));
			let marks =
				read_marks(args.points.as_deref(), &args.at, &paths, &sizes).unwrap_or_else(|e| fail(&e));
			let skips = read_skips(&args.skip, &paths, &marks).unwrap_or_else(|e| fail(&e));
			if let Some(dir) = &args.frames {
				std::fs::create_dir_all(dir)
					.unwrap_or_else(|e| fail(&format!("--frames {}: {e}", dir.display())));
				let has_png = std::fs::read_dir(dir).unwrap().flatten().any(|e| suffix(&e.path()) == "png");
				if has_png {
					fail(&format!("--frames {}: already has PNGs; empty it first", dir.display()));
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
				.collect::<Result<Vec<_>, _>>()
				.unwrap_or_else(|e| fail(&e))
				.into_iter()
				.fold((0, 0), |(prepare_max, render_max), (prepare, render)| {
					(prepare_max.max(prepare), render_max.max(render))
				});
			let resources = plan_resources(largest, prepare_decode_bytes, render_decode_bytes, free, cores())
				.unwrap_or_else(|e| fail(&e));
			println!(
				"{:.1} GB free: {} prepare threads, {} render threads, up to {:.1} GB of kept photos",
				free as f64 / (1u64 << 30) as f64,
				resources.prepare_threads,
				resources.render_threads,
				resources.keep_bytes as f64 / (1u64 << 30) as f64
			);
			let cache = if args.no_cache {
				None
			} else {
				let root = args.cache.clone().unwrap_or_else(default_cache_dir);
				Some(cache::FeatureCache::new(root).unwrap_or_else(|e| fail(&e)))
			};
			let profiles =
				if json_only { vec![] } else { read_profiles(&paths).unwrap_or_else(|e| fail(&e)) };
			(paths, sizes, marks, skips, focals, fill, k, resources, cache, json_only, profiles)
		});
	let keep = (!json_only).then(|| Keep::new(resources.keep_bytes));
	let path = step("decode, find features, track", || {
		let threads = resources.prepare_threads;
		std::thread::scope(|scope| {
			let (tx, rx) = sync_channel(1);
			let (paths, sizes, keep, cache) = (&paths, &sizes, keep.as_ref(), cache.as_ref());
			scope.spawn(move || {
				let prepare_one = |i: usize| prepare(&paths[i], sizes[i], k, keep, i, cache);
				ordered(paths.len(), threads, QUEUE_DEPTH * threads, prepare_one, |_, photo| {
					tx.send(photo).unwrap()
				});
			});
			track::track(rx.into_iter(), sizes, k, &marks, &focals, &skips)
		})
	});
	let (path, sources, size) = step("plan the crop", || {
		let path = track::fill_lost(path, &sizes);
		let (sources, size) = track::plan(&path, &sizes, fill.is_some());
		(path, sources, size)
	});
	for ((p, &(fw, fh)), hit) in paths.iter().zip(&sizes).zip(&path) {
		println!(
			"{}  {fw}x{fh}  anchor {:.0},{:.0}  scale {:.2}  {}",
			file_name(p),
			hit.x,
			hit.y,
			hit.scale,
			hit.note
		);
	}
	println!("clip {}x{}", size.0, size.1);

	// Written under names unique to this run, then renamed together under the folder lock: a failed run
	// leaves the previous pair as it was, and runs sharing an output publish whole pairs, one at a time.
	let run = random_hex();
	let rects = if json_only { args.out.clone() } else { args.out.with_extension("json") };
	let rects_partial = partial(&rects, &run);
	let clip_partial = (!json_only).then(|| partial(&args.out, &run));
	let alpha = fill.as_ref().is_some_and(|f| f.len() == 4);
	if let Some(clip_partial) = clip_partial.as_ref() {
		let profile = colour::clip_profile(&profiles);
		let convert: Vec<bool> = profiles.iter().map(|&p| p != profile).collect();
		let converted = convert.iter().filter(|&&c| c).count();
		if converted > 0 {
			println!("converting {converted} sRGB photos to Display P3, the other photos' profile");
		}
		step("render: place and encode", || {
			let filters = args.vf.as_deref();
			let options = SinkOptions { size, alpha, fps: args.fps, filters, run: &run, profile };
			let mut sink = open_sink(clip_partial, &options);
			let job = RenderJob {
				paths: &paths,
				sources: &sources,
				size,
				fill: fill.as_deref(),
				keep: keep.as_ref().unwrap(),
				threads: resources.render_threads,
				frames: args.frames.as_deref(),
				avif: suffix(&args.out) == "avif",
				profile,
				convert: &convert,
			};
			render(&job, &mut sink);
			finish_sink(sink, clip_partial);
		});
	}
	step("write the JSON", || {
		write_rects(
			&paths,
			&sizes,
			&sources,
			&path,
			&rects_partial,
			&run,
			(!json_only).then(|| file_name(&args.out)).as_deref(),
		);
		let pairs = if let Some(clip_partial) = clip_partial.as_ref() {
			vec![(clip_partial.as_path(), args.out.as_path()), (rects_partial.as_path(), rects.as_path())]
		} else {
			vec![(rects_partial.as_path(), rects.as_path())]
		};
		publish(&pairs, &folder_of(&args.out));
	});
	if let Some(clip_partial) = clip_partial {
		let _ = std::fs::remove_file(clip_partial);
	}
	let _ = std::fs::remove_file(rects_partial);
	println!("wrote {}", args.out.display());
	profile::finish();
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
		let (ordered, _, reason) = order_photos(paths, Order::Time);
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
		let (ordered, _, reason) = order_photos(vec![with_time.clone(), without_time.clone()], Order::Time);
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
			assert!(read_marks(Some("1,1/9,9"), &ats, &two, &[photo, photo]).is_err(), "{ats:?}");
		}
		let marks = read_marks(Some("1,1/9,9"), &["b.avif=5,5".into()], &two, &[photo, photo]).unwrap();
		assert_eq!(marks[0], Some(Mark { points: vec![(1.0, 1.0), (9.0, 9.0)], zoom: None }));
		assert_eq!(marks[1], Some(Mark { points: vec![(5.0, 5.0)], zoom: None }));
		assert!(read_marks(None, &[], &two, &[photo, photo]).is_err(), "--points is required");
		assert!(read_marks(Some("1,1@2.0"), &[], &two, &[photo, photo]).is_err(), "--points takes no zoom");
		let box_err = read_marks(Some("0,0,10,10"), &[], &two, &[photo, photo]).unwrap_err();
		assert!(box_err.contains("quarter"), "{box_err}");
		let box_at_err =
			read_marks(Some("1,1"), &["b.avif=0,0,10,10".into()], &two, &[photo, photo]).unwrap_err();
		assert!(box_at_err.contains("quarter"), "{box_at_err}");
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
