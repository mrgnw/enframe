//! pinframe in Rust: pin one feature in place across a photo sequence. Same flags and outputs as
//! `anchor.py`, for AVIF photos.

mod avif;
mod avif_sys;
#[cfg(test)]
mod checks;
mod pixels;
mod profile;
mod render;
mod sift;
mod track;

use profile::{Work, step, work};
use render::{Keep, ordered, place};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::sync_channel;
use track::{Hit, Mark, Photo, Rect};

/// Threads and memory are planned at start from the cores, the free memory and these measured costs
/// (Girona, M4 Pro). Re-measure them after changing libraries or TRACK_EDGE.
const PREPARE_FIXED_BYTES: usize = 150 << 20;
const PREPARE_BYTES_PER_PIXEL: usize = 18;
const RENDER_BYTES_PER_PIXEL: usize = 25;
const FEATURE_BYTES: usize = 4 << 20;
/// Results queued per thread; they come back in order, so a slow photo must not idle the others.
const QUEUE_DEPTH: usize = 3;
/// Policy: plan within this share of the memory free at start, leaving the rest to other apps.
const MEMORY_SHARE: f64 = 0.75;
const AVIF_SPEED: i32 = 10;
const AVIF_QUALITY: i32 = 75;
const CLIP_SUFFIXES: [&str; 4] = ["avif", "mp4", "mov", "webm"];

const USAGE: &str = "usage: pinframe PHOTO... --box x,y,w,h [--at PHOTO=x,y[,w,h[,zoom]]]... [--pad COLOUR]
                [--fps N] [--out CLIP] [--frames DIR] [--vf FILTERS]";

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
	box_: Option<String>,
	at: Vec<String>,
	pad: Option<String>,
	fps: f64,
	out: PathBuf,
	frames: Option<PathBuf>,
	vf: Option<String>,
}

fn parse_args() -> Args {
	let mut args = Args {
		photos: vec![],
		box_: None,
		at: vec![],
		pad: None,
		fps: 4.0,
		out: PathBuf::from("out.avif"),
		frames: None,
		vf: None,
	};
	let mut it = std::env::args().skip(1);
	while let Some(arg) = it.next() {
		let mut value = || it.next().unwrap_or_else(|| fail(&format!("{arg} needs a value\n{USAGE}")));
		match arg.as_str() {
			"--box" => args.box_ = Some(value()),
			"--at" => args.at.push(value()),
			"--pad" => args.pad = Some(value()),
			"--fps" => args.fps = value().parse().unwrap_or_else(|_| fail("--fps must be a number")),
			"--out" => args.out = PathBuf::from(value()),
			"--frames" => args.frames = Some(PathBuf::from(value())),
			"--vf" => args.vf = Some(value()),
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

fn parse_numbers(flag: &str, text: &str) -> Vec<f64> {
	text.split(',')
		.map(|v| {
			v.trim()
				.parse()
				.unwrap_or_else(|_| fail(&format!("{flag} {text}: expected comma-separated numbers")))
		})
		.collect()
}

/// A mark is a point x,y or a box x,y,w,h with an optional zoom, inside its photo.
fn check_mark(name: &str, mark: &[f64], (fw, fh): (u32, u32)) {
	let (fw, fh) = (fw as f64, fh as f64);
	if ![2, 4, 5].contains(&mark.len()) {
		fail(&format!("{name}: a mark is x,y or x,y,w,h with an optional zoom, not {} numbers", mark.len()));
	}
	let (px, py) = if mark.len() == 2 {
		(mark[0], mark[1])
	} else {
		let (bx, by, bw, bh) = (mark[0], mark[1], mark[2], mark[3]);
		if !(bw > 0.0 && bh > 0.0 && bx >= 0.0 && by >= 0.0 && bx + bw <= fw && by + bh <= fh) {
			fail(&format!("{name}: box {bx},{by},{bw},{bh} is empty or outside the {fw}x{fh} photo"));
		}
		(bx + bw / 2.0, by + bh / 2.0)
	};
	if !(0.0 <= px && px < fw && 0.0 <= py && py < fh) {
		fail(&format!("{name}: point {px},{py} is outside the {fw}x{fh} photo"));
	}
	if mark.len() == 5 && !(mark[4] > 0.0) {
		fail(&format!("{name}: zoom must be positive"));
	}
}

fn file_name(path: &Path) -> String {
	path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Every photo's size from its header, so bad input stops before any decoding.
fn read_sizes(paths: &[PathBuf]) -> Vec<(u32, u32)> {
	if paths.len() < 2 {
		fail("need at least two photos");
	}
	let mut names: Vec<String> = paths.iter().map(|p| file_name(p)).collect();
	names.sort();
	if names.windows(2).any(|w| w[0] == w[1]) {
		fail("two photos share a file name, so --at and the .json could not tell them apart");
	}
	paths
		.iter()
		.map(|p| avif::oriented_size(p).unwrap_or_else(|e| fail(&format!("{}: {e}", p.display()))))
		.collect()
}

/// --box and every --at, by photo index, checked against the photo sizes.
fn read_marks(
	box_: Option<&str>,
	ats: &[String],
	paths: &[PathBuf],
	sizes: &[(u32, u32)],
) -> Vec<Option<Mark>> {
	let names: Vec<String> = paths.iter().map(|p| file_name(p)).collect();
	// ponytail: no box drawing (anchor.py opens an OpenCV window); --box is required here
	let first = parse_numbers("--box", box_.unwrap_or_else(|| fail("--box x,y,w,h is required")));
	if first.len() != 4 {
		fail("--box is x,y,w,h");
	}
	let mut marks: Vec<Option<Mark>> = vec![None; paths.len()];
	marks[0] = Some(Mark(first));
	for at in ats {
		let Some((name, value)) = at.split_once('=').filter(|(n, _)| names.iter().any(|m| m == n)) else {
			fail(&format!("--at {at}: expected PHOTO=x,y with PHOTO one of the photo file names"));
		};
		let i = names.iter().position(|m| m == name).unwrap();
		if marks[i].is_some() {
			fail(&format!(
				"--at {name}: {}",
				if i == 0 { "use --box for the first photo" } else { "marked twice" }
			));
		}
		marks[i] = Some(Mark(parse_numbers(&format!("--at {name}"), value)));
	}
	for (i, mark) in marks.iter().enumerate() {
		if let Some(Mark(m)) = mark {
			check_mark(&names[i], m, sizes[i]);
		}
	}
	marks
}

fn parse_fill(value: &str) -> Vec<u8> {
	if value == "transparent" {
		return vec![0, 0, 0, 0];
	}
	let hex = match value {
		"black" => "000000",
		"white" => "ffffff",
		other => other.strip_prefix('#').unwrap_or(other),
	};
	let channel = |i: usize| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
	match (hex.len(), channel(0), channel(2), channel(4)) {
		(6, Some(r), Some(g), Some(b)) => vec![r, g, b],
		_ => fail(&format!("--pad {value}: use transparent, black, white or a #rrggbb colour")),
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

fn check_output(out: &Path, alpha: bool, filters: Option<&str>) {
	let ext = suffix(out);
	if !CLIP_SUFFIXES.contains(&ext.as_str()) {
		fail(&format!("--out {}: use one of .avif, .mp4, .mov, .webm", out.display()));
	}
	if !folder_of(out).is_dir() {
		fail(&format!("--out {}: folder {} does not exist", out.display(), folder_of(out).display()));
	}
	if ext == "avif" {
		if filters.is_some() {
			fail("--vf runs through ffmpeg: use a .mp4, .mov or .webm --out");
		}
		return;
	}
	if !on_path("ffmpeg") {
		fail(&format!("--out .{ext} needs ffmpeg on PATH; .avif does not"));
	}
	if alpha && ext == "mp4" {
		fail("transparent padding needs a .avif, .mov or .webm --out");
	}
}

struct Resources {
	prepare_threads: usize,
	render_threads: usize,
	keep_bytes: usize,
}

/// Every core. Unlike anchor.py, whose OpenCV threads ran slower once the efficiency cores joined, the
/// Rust prepare step gets faster with them (Girona: 1.73 s on 8 threads, 1.47 s on 12).
fn cores() -> usize {
	std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// Memory free now or reclaimable without swapping, as psutil's `available` counts it on macOS.
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

fn plan_resources(largest_pixels: usize, free_bytes: usize, cores: usize) -> Resources {
	let budget = (free_bytes as f64 * MEMORY_SHARE) as usize;
	let prepare_cost =
		PREPARE_FIXED_BYTES + PREPARE_BYTES_PER_PIXEL * largest_pixels + QUEUE_DEPTH * FEATURE_BYTES;
	// A queued clip frame is RGBA, and the clip is never larger than the photos it is cut from.
	let render_cost = (RENDER_BYTES_PER_PIXEL + QUEUE_DEPTH * 4) * largest_pixels;
	let prepare_threads = cores.min(budget / prepare_cost);
	let render_threads = cores.min(budget / render_cost);
	if prepare_threads < 1 || render_threads < 1 {
		let need = prepare_cost.max(render_cost) as f64 / MEMORY_SHARE / (1u64 << 30) as f64;
		let free = free_bytes as f64 / (1u64 << 30) as f64;
		fail(&format!(
			"a {:.0} MP photo needs about {need:.1} GB free; {free:.1} GB is",
			largest_pixels as f64 / 1e6
		));
	}
	// Kept photos last through both passes, so they get what neither pass's threads need.
	let working = (prepare_threads * prepare_cost).max(render_threads * render_cost);
	Resources { prepare_threads, render_threads, keep_bytes: budget - working }
}

#[cfg(not(feature = "kornia"))]
fn find_features(small: &pixels::Image) -> sift::Features {
	sift::detect_and_compute(small)
}

/// kornia-imgproc's SIFT, with its float descriptors (whole numbers, as OpenCV's) stored as bytes.
#[cfg(feature = "kornia")]
fn find_features(small: &pixels::Image) -> sift::Features {
	use kornia_imgproc::features::{FirstOctave, SiftConfig, SiftWorkspace, sift_detect_and_compute};
	thread_local! {
		static WORKSPACE: std::cell::RefCell<SiftWorkspace> = std::cell::RefCell::new(SiftWorkspace::new());
	}
	let src: Vec<f32> = small.pixels.iter().map(|&v| v as f32).collect();
	let found = WORKSPACE.with_borrow_mut(|ws| {
		let cfg = SiftConfig::default();
		sift_detect_and_compute(
			ws,
			&src,
			small.width,
			small.height,
			&cfg,
			FirstOctave::Double,
			usize::MAX,
			false,
		)
	});
	let found = found.unwrap_or_else(|e| fail(&format!("SIFT: {e:?}")));
	let keypoints = found
		.keypoints
		.iter()
		.map(|k| sift::KeyPoint {
			x: k.x,
			y: k.y,
			size: k.size,
			angle: k.angle,
			response: k.response,
			octave: k.octave,
		})
		.collect();
	let descriptors = found
		.descriptors
		.chunks_exact(sift::DESCRIPTOR_LEN)
		.map(|d| std::array::from_fn(|i| d[i] as u8))
		.collect();
	sift::Features { keypoints, descriptors }
}

fn prepare(path: &Path, size: (u32, u32), k: f64, keep: &Keep, i: usize) -> Photo {
	let rgb = work(Work::Decode, || avif::decode(path, 1))
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
	keep.offer(i, rgb);
	photo
}

/// Where each frame goes: an animated AVIF through libavif, or ffmpeg for the other formats.
enum Sink {
	Avif(avif::Encoder),
	Ffmpeg(std::process::Child),
}

fn open_sink(out: &Path, size: (u32, u32), alpha: bool, fps: f64, filters: Option<&str>, run: &str) -> Sink {
	if suffix(out) == "avif" {
		let threads = std::thread::available_parallelism().map_or(1, |n| n.get()) as i32;
		let frame_ms = (1000.0 / fps).round_ties_even() as u64;
		return Sink::Avif(avif::Encoder::new(AVIF_SPEED, AVIF_QUALITY, frame_ms, threads, run_xmp(run)));
	}
	let encoder: Vec<&str> = match (suffix(out).as_str(), alpha) {
		("webm", _) => {
			vec![
				"-c:v",
				"libvpx-vp9",
				"-pix_fmt",
				if alpha { "yuva420p" } else { "yuv420p" },
				"-crf",
				"30",
				"-b:v",
				"0",
			]
		}
		("mov", true) => vec!["-c:v", "prores_ks", "-profile:v", "4444", "-pix_fmt", "yuva444p10le"],
		_ => vec!["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18"],
	};
	let mut cmd = std::process::Command::new("ffmpeg");
	cmd.args([
		"-y",
		"-loglevel",
		"error",
		"-f",
		"rawvideo",
		"-pix_fmt",
		if alpha { "rgba" } else { "rgb24" },
	]);
	cmd.args(["-s", &format!("{}x{}", size.0, size.1), "-r", &fps.to_string(), "-i", "-"]);
	if let Some(f) = filters {
		cmd.args(["-vf", f]);
	}
	cmd.args(&encoder).args(["-metadata", &format!("comment=pinframe-run={run}")]).arg(out);
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

fn save_png(path: &Path, frame: &pixels::Image) -> Result<(), String> {
	let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
	let mut encoder =
		png::Encoder::new(std::io::BufWriter::new(file), frame.width as u32, frame.height as u32);
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
}

/// A placed frame as the sink takes it: YUV for libavif, RGB(A) bytes for ffmpeg.
enum Frame {
	Yuv(avif::Yuv),
	Rgb(pixels::Image),
}

/// Places each photo, kept or decoded again, on worker threads and hands the frames to the sink in order.
fn render(job: &RenderJob, sink: &mut Sink) {
	let place_one = |i: usize| {
		let photo = job.keep.take(i).unwrap_or_else(|| {
			let path = &job.paths[i];
			work(Work::Decode, || avif::decode(path, 1))
				.unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())))
		});
		let frame = work(Work::Place, || place(&photo, job.sources[i], job.size, job.fill));
		drop(photo);
		if let Some(dir) = job.frames {
			let path = dir.join(format!("{:04}.png", i + 1));
			save_png(&path, &frame).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
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
}

#[derive(serde::Serialize)]
struct Sidecar {
	run: String,
	clip: String,
	photos: Vec<Row>,
}

fn write_rects(
	paths: &[PathBuf],
	sizes: &[(u32, u32)],
	sources: &[Rect],
	path: &[Hit],
	out: &Path,
	run: &str,
	clip: &str,
) {
	let photos = paths
		.iter()
		.zip(sizes)
		.zip(sources)
		.zip(path)
		.map(|(((p, &(fw, fh)), &(x, y, w, h)), hit)| Row {
			file: file_name(p),
			x: x / fw as f64,
			y: y / fh as f64,
			w: w / fw as f64,
			h: h / fh as f64,
			scale: format!("{:.4}", hit.scale).parse().unwrap(),
			source: hit.source.name(),
		})
		.collect();
	let sidecar = Sidecar { run: run.to_string(), clip: clip.to_string(), photos };
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
	unsafe { libc::arc4random_buf(bytes.as_mut_ptr() as *mut _, bytes.len()) };
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
fn publish(pairs: [(&Path, &Path); 2], folder: &Path) {
	let dir = std::fs::File::open(folder).unwrap_or_else(|e| fail(&format!("{}: {e}", folder.display())));
	use std::os::fd::AsRawFd;
	unsafe { libc::flock(dir.as_raw_fd(), libc::LOCK_EX) };
	for (from, to) in pairs {
		std::fs::rename(from, to).unwrap_or_else(|e| fail(&format!("{}: {e}", to.display())));
	}
}

fn main() {
	profile::start();
	let args = parse_args();
	let (paths, sizes, marks, fill, k, resources) = step("check input and plan resources", || {
		let mut paths = args.photos.clone();
		// ponytail: filename order; the exported AVIFs carry no capture time to sort by
		paths.sort();
		let fill = args.pad.as_deref().map(parse_fill);
		check_output(&args.out, fill.as_ref().is_some_and(|f| f.len() == 4), args.vf.as_deref());
		if !(args.fps > 0.0) {
			fail("--fps must be positive");
		}
		let sizes = read_sizes(&paths);
		let marks = read_marks(args.box_.as_deref(), &args.at, &paths, &sizes);
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
		let resources = plan_resources(largest, free, cores());
		println!(
			"{:.1} GB free: {} prepare threads, {} render threads, up to {:.1} GB of kept photos",
			free as f64 / (1u64 << 30) as f64,
			resources.prepare_threads,
			resources.render_threads,
			resources.keep_bytes as f64 / (1u64 << 30) as f64
		);
		(paths, sizes, marks, fill, k, resources)
	});
	let keep = Keep::new(resources.keep_bytes);
	let path = step("decode, find features, track", || {
		let threads = resources.prepare_threads;
		std::thread::scope(|scope| {
			let (tx, rx) = sync_channel(1);
			let (paths, sizes, keep) = (&paths, &sizes, &keep);
			scope.spawn(move || {
				let prepare_one = |i: usize| prepare(&paths[i], sizes[i], k, keep, i);
				ordered(paths.len(), threads, QUEUE_DEPTH * threads, prepare_one, |_, photo| {
					tx.send(photo).unwrap()
				});
			});
			track::track(rx.into_iter(), &sizes, k, &marks)
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
	let rects = args.out.with_extension("json");
	let (clip_partial, rects_partial) = (partial(&args.out, &run), partial(&rects, &run));
	let cleanup = || {
		let _ = std::fs::remove_file(&clip_partial);
		let _ = std::fs::remove_file(&rects_partial);
	};
	let alpha = fill.as_ref().is_some_and(|f| f.len() == 4);
	step("render: place and encode", || {
		let mut sink = open_sink(&clip_partial, size, alpha, args.fps, args.vf.as_deref(), &run);
		let job = RenderJob {
			paths: &paths,
			sources: &sources,
			size,
			fill: fill.as_deref(),
			keep: &keep,
			threads: resources.render_threads,
			frames: args.frames.as_deref(),
			avif: suffix(&args.out) == "avif",
		};
		render(&job, &mut sink);
		finish_sink(sink, &clip_partial);
	});
	step("write the JSON", || {
		write_rects(&paths, &sizes, &sources, &path, &rects_partial, &run, &file_name(&args.out));
		publish([(&clip_partial, &args.out), (&rects_partial, &rects)], &folder_of(&args.out));
	});
	cleanup();
	println!("wrote {}", args.out.display());
	profile::finish();
}
