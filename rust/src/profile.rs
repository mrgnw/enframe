//! Wall, CPU and peak memory per step, and thread-seconds per kind of work, written as one JSON line to
//! the file named by PINFRAME_PROFILE. Without it, nothing is measured.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub enum Work {
	Decode,
	Sift,
	Match,
	Place,
	Encode,
	EncoderWait,
}

const WORK_NAMES: [&str; 6] = ["decode", "sift", "match", "place", "encode", "encoder wait"];
static WORK: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];
static ON: AtomicBool = AtomicBool::new(false);
static STEPS: Mutex<Vec<(&str, Instant, Instant, f64)>> = Mutex::new(Vec::new());
static SAMPLES: Mutex<Vec<(Instant, u64)>> = Mutex::new(Vec::new());

#[allow(deprecated)]
pub fn resident_bytes() -> u64 {
	unsafe {
		let mut info: libc::mach_task_basic_info = std::mem::zeroed();
		let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
		let task = libc::mach_task_self();
		libc::task_info(
			task,
			libc::MACH_TASK_BASIC_INFO,
			&mut info as *mut _ as libc::task_info_t,
			&mut count,
		);
		info.resident_size
	}
}

fn cpu_seconds() -> f64 {
	let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
	unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
	let secs = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 * 1e-6;
	secs(usage.ru_utime) + secs(usage.ru_stime)
}

pub fn start() {
	if std::env::var_os("PINFRAME_PROFILE").is_none() {
		return;
	}
	ON.store(true, Ordering::Relaxed);
	std::thread::spawn(|| {
		loop {
			SAMPLES.lock().unwrap().push((Instant::now(), resident_bytes()));
			std::thread::sleep(Duration::from_millis(5));
		}
	});
}

pub fn step<T>(name: &'static str, f: impl FnOnce() -> T) -> T {
	if !ON.load(Ordering::Relaxed) {
		return f();
	}
	let (start, cpu) = (Instant::now(), cpu_seconds());
	let result = f();
	STEPS.lock().unwrap().push((name, start, Instant::now(), cpu_seconds() - cpu));
	result
}

pub fn work<T>(kind: Work, f: impl FnOnce() -> T) -> T {
	if !ON.load(Ordering::Relaxed) {
		return f();
	}
	let start = Instant::now();
	let result = f();
	WORK[kind as usize].fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
	result
}

pub fn add(kind: Work, took: Duration) {
	WORK[kind as usize].fetch_add(took.as_nanos() as u64, Ordering::Relaxed);
}

pub fn finish() {
	let Some(path) = std::env::var_os("PINFRAME_PROFILE") else { return };
	let samples = SAMPLES.lock().unwrap().clone();
	let peak = |from: Instant, to: Instant| {
		samples.iter().filter(|(t, _)| (from..=to).contains(t)).map(|(_, m)| *m).max().unwrap_or(0)
	};
	let steps: Vec<serde_json::Value> = STEPS
		.lock()
		.unwrap()
		.iter()
		.map(|&(name, from, to, cpu)| {
			let wall = (to - from).as_secs_f64();
			serde_json::json!({"step": name, "wall": wall, "cpu": cpu, "peak": peak(from, to)})
		})
		.collect();
	let work: serde_json::Map<String, serde_json::Value> = WORK_NAMES
		.iter()
		.zip(&WORK)
		.map(|(name, ns)| (name.to_string(), (ns.load(Ordering::Relaxed) as f64 * 1e-9).into()))
		.collect();
	let line = serde_json::json!({"steps": steps, "work": work, "cpu": cpu_seconds(), "peak": peak_rss()});
	use std::io::Write;
	let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
	writeln!(file, "{line}").unwrap();
}

/// The process's peak resident memory so far, in bytes.
pub fn peak_rss() -> u64 {
	let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
	unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
	usage.ru_maxrss as u64
}
