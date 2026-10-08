//! pinframe in Rust: pin one feature in place across a photo sequence. Same flags and outputs as
//! `anchor.py`, for AVIF photos. A thin CLI over the `pinframe` library: it reads the flags into a
//! [`Job`], prints the library's progress lines, and stops with the library's message on an error.

use pinframe::{BOX_REMOVED, Job, Order, Output};
use std::path::PathBuf;

const USAGE: &str = "usage: pinframe PHOTO... --points x,y[/x,y]... [--at PHOTO=x,y[/x,y]...[@zoom]]...
				[--near PHOTO=x,y[/x,y]...[@zoom]]... [--skip PHOTO]... [--pad COLOUR] [--fps N]
				[--out CLIP-or-JSON] [--frames DIR] [--vf FILTERS] [--cache DIR|--no-cache] [--order time|name]";

/// Stops on input or photos pinframe cannot use: a message and exit code 1.
fn fail(message: &str) -> ! {
	eprintln!("pinframe: {message}");
	std::process::exit(1);
}

fn parse_args() -> Job {
	let mut output = Output::default();
	let mut job = Job::default();
	let mut it = std::env::args().skip(1);
	while let Some(arg) = it.next() {
		let mut value = || it.next().unwrap_or_else(|| fail(&format!("{arg} needs a value\n{USAGE}")));
		match arg.as_str() {
			"--points" => job.marks.points = Some(value()),
			"--box" => fail(&format!("{BOX_REMOVED}\n{USAGE}")),
			"--at" => job.marks.at.push(value()),
			"--skip" => job.marks.skip.push(value()),
			"--near" => job.marks.near.push(value()),
			"--pad" => job.pad = Some(value()),
			"--fps" => output.fps = value().parse().unwrap_or_else(|_| fail("--fps must be a number")),
			"--out" => output.out = PathBuf::from(value()),
			"--frames" => output.frames = Some(PathBuf::from(value())),
			"--vf" => output.vf = Some(value()),
			"--cache" => job.cache = Some(PathBuf::from(value())),
			"--no-cache" => job.no_cache = true,
			"--order" => {
				job.order = match value().as_str() {
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
			photo => job.photos.push(PathBuf::from(photo)),
		}
	}
	job.output = Some(output);
	job
}

fn main() {
	pinframe::start_profile();
	let job = parse_args();
	let mut log = |line: &str| println!("{line}");
	let tracked = pinframe::track(&job, &mut log).unwrap_or_else(|e| fail(&e));
	tracked.write(&mut log).unwrap_or_else(|e| fail(&e));
	println!("wrote {}", job.output.as_ref().unwrap().out.display());
	pinframe::finish_profile();
}
