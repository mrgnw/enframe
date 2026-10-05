//! Comparisons with dumps from `anchor.py`'s own functions (`uv run analysis/dump_sift.py NAME`).
//! Run with `cargo test --release -- --ignored --nocapture`.

use crate::{avif, pixels};
use std::path::Path;

const SAMPLES: &str = "../samples/girona clouds";
const DUMPS: &str = "../out";

/// float32 values of a .npy file written by numpy.save.
fn read_npy_f32(path: &str) -> Vec<f32> {
	let bytes = std::fs::read(path).unwrap();
	let header = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
	bytes[10 + header..].chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect()
}

#[test]
#[ignore]
fn sift_matches_opencv() {
	for name in ["IMG_4667", "IMG_4791"] {
		let rgb = avif::decode(&Path::new(SAMPLES).join(format!("{name}.avif")), 1).unwrap();
		let small = pixels::shrink_area(&pixels::gray(&rgb), 0.5);
		let started = std::time::Instant::now();
		let features = crate::sift::detect_and_compute(&small);
		let took = started.elapsed();
		let text = std::fs::read_to_string(format!("{DUMPS}/{name}.kp.json")).unwrap();
		let opencv: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
		let des = read_npy_f32(&format!("{DUMPS}/{name}.des.npy"));
		println!("{name}: {} keypoints in {took:?}, OpenCV {}", features.keypoints.len(), opencv.len());
		let mut worst = [0f64; 5];
		let mut same_descriptors = 0;
		for (i, (kp, cv)) in features.keypoints.iter().zip(&opencv).enumerate() {
			let ours = [kp.x, kp.y, kp.size, kp.angle, kp.response];
			for (w, (v, key)) in
				worst.iter_mut().zip(ours.iter().zip(["x", "y", "size", "angle", "response"]))
			{
				*w = w.max((*v as f64 - cv[key].as_f64().unwrap()).abs());
			}
			let theirs = &des[i * 128..(i + 1) * 128];
			same_descriptors +=
				features.descriptors[i].iter().zip(theirs).all(|(a, b)| *a as f32 == *b) as usize;
		}
		println!(
			"{name}: worst diff in x y size angle response {worst:?}; {same_descriptors} identical descriptors"
		);
	}
}

#[test]
#[ignore]
fn decode_and_gray_match_python() {
	for name in ["IMG_4667", "IMG_4791"] {
		let rgb = avif::decode(&Path::new(SAMPLES).join(format!("{name}.avif")), 1).unwrap();
		let python = std::fs::read(format!("{DUMPS}/{name}.rgb.raw")).unwrap();
		assert_eq!(rgb.pixels.len(), python.len());
		let off = rgb.pixels.iter().zip(&python).filter(|(a, b)| a != b).count();
		let worst = rgb.pixels.iter().zip(&python).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
		println!("{name}: {off} of {} RGB values differ, by at most {worst}", python.len());
		let small = pixels::shrink_area(&pixels::gray(&rgb), 0.5);
		let decoder = png::Decoder::new(std::io::BufReader::new(
			std::fs::File::open(format!("{DUMPS}/{name}.gray.png")).unwrap(),
		));
		let mut reader = decoder.read_info().unwrap();
		let mut gray = vec![0; reader.output_buffer_size().unwrap()];
		reader.next_frame(&mut gray).unwrap();
		let off = small.pixels.iter().zip(&gray).filter(|(a, b)| a != b).count();
		println!("{name}: {off} of {} gray values differ", gray.len());
	}
}

#[test]
#[ignore]
fn gray_from_python_rgb_matches() {
	let name = "IMG_4667";
	let python = std::fs::read(format!("{DUMPS}/{name}.rgb.raw")).unwrap();
	let rgb = pixels::Image { width: 2048, height: 1536, channels: 3, pixels: python };
	let small = pixels::shrink_area(&pixels::gray(&rgb), 0.5);
	let decoder = png::Decoder::new(std::io::BufReader::new(
		std::fs::File::open(format!("{DUMPS}/{name}.gray.png")).unwrap(),
	));
	let mut reader = decoder.read_info().unwrap();
	let mut gray = vec![0; reader.output_buffer_size().unwrap()];
	reader.next_frame(&mut gray).unwrap();
	let off = small.pixels.iter().zip(&gray).filter(|(a, b)| a != b).count();
	println!("from Python's RGB: {off} gray values differ; libyuv {}", unsafe {
		crate::avif_sys::avifLibYUVVersion()
	});
	assert_eq!(off, 0);
}

#[test]
#[ignore]
fn sift_exactness() {
	let name = "IMG_4667";
	let rgb = avif::decode(&Path::new(SAMPLES).join(format!("{name}.avif")), 1).unwrap();
	let features = crate::sift::detect_and_compute(&pixels::shrink_area(&pixels::gray(&rgb), 0.5));
	let text = std::fs::read_to_string(format!("{DUMPS}/{name}.kp.json")).unwrap();
	let opencv: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
	let mut exact = [0usize; 4];
	let mut octaves = std::collections::BTreeMap::new();
	for (kp, cv) in features.keypoints.iter().zip(&opencv) {
		let ours = [kp.x, kp.y, kp.size, kp.angle];
		let same: Vec<bool> = ours
			.iter()
			.zip(["x", "y", "size", "angle"])
			.map(|(v, k)| *v == cv[k].as_f64().unwrap() as f32)
			.collect();
		for (e, s) in exact.iter_mut().zip(&same) {
			*e += *s as usize;
		}
		let entry = octaves.entry(kp.octave & 255).or_insert((0, 0));
		entry.0 += 1;
		entry.1 += same.iter().all(|s| *s) as usize;
	}
	println!("exact x y size angle {exact:?} of {}; per octave (total, exact) {octaves:?}", opencv.len());
}

#[test]
#[ignore]
fn blur_matches_opencv() {
	let name = "IMG_4667";
	let rgb = avif::decode(&Path::new(SAMPLES).join(format!("{name}.avif")), 1).unwrap();
	let layers = crate::sift::first_octave(&pixels::shrink_area(&pixels::gray(&rgb), 0.5));
	for (i, ours) in layers.iter().enumerate() {
		let bytes = std::fs::read(format!("{DUMPS}/{name}.g{i}.f32")).unwrap();
		let theirs: Vec<f32> =
			bytes.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
		let off = ours.iter().zip(&theirs).filter(|(a, b)| a != b).count();
		let worst = ours.iter().zip(&theirs).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
		let first = ours.iter().zip(&theirs).position(|(a, b)| a != b);
		println!("layer {i}: {off} of {} differ, worst {worst}, first at {first:?}", theirs.len());
	}
}

/// Fastest of 7 runs of SIFT on one photo, and a checksum of its output that optimizations must keep.
#[test]
#[ignore]
fn sift_bench() {
	let rgb = avif::decode(&Path::new(SAMPLES).join("IMG_4667.avif"), 1).unwrap();
	let small = pixels::shrink_area(&pixels::gray(&rgb), 0.5);
	let mut best = std::time::Duration::MAX;
	let mut sum = 0u64;
	let runs = std::env::var("SIFT_BENCH_RUNS").map_or(7, |v| v.parse().unwrap());
	for _ in 0..runs {
		let started = std::time::Instant::now();
		let features = crate::sift::detect_and_compute(&small);
		best = best.min(started.elapsed());
		sum = features.descriptors.iter().flatten().map(|&b| b as u64).sum::<u64>()
			+ features
				.keypoints
				.iter()
				.map(|k| (k.x.to_bits() ^ k.size.to_bits() ^ k.angle.to_bits()) as u64)
				.sum::<u64>();
	}
	println!("sift_bench: fastest {best:?}, checksum {sum}");
}
