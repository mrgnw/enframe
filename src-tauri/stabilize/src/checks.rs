//! Comparisons with dumps from `anchor.py`'s own functions (`uv run analysis/dump_sift.py NAME`).
//! Run with `cargo test --release -- --ignored --nocapture`.

use crate::decode::{decode, oriented_size};
use crate::pixels;
use std::path::Path;

const SAMPLES: &str = "../../samples/train clouds";
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
		let rgb = decode(&Path::new(SAMPLES).join(format!("{name}.avif"))).unwrap();
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
		let rgb = decode(&Path::new(SAMPLES).join(format!("{name}.avif"))).unwrap();
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
	println!("from Python's RGB: {off} gray values differ");
	assert_eq!(off, 0);
}

#[test]
#[ignore]
fn sift_exactness() {
	let name = "IMG_4667";
	let rgb = decode(&Path::new(SAMPLES).join(format!("{name}.avif"))).unwrap();
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
	let rgb = decode(&Path::new(SAMPLES).join(format!("{name}.avif"))).unwrap();
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
	let rgb = decode(&Path::new(SAMPLES).join("IMG_4667.avif")).unwrap();
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
/// Decode time against the gray conversion and half-size shrink that follow it.
#[test]
#[ignore]
fn decode_bench() {
	let path = Path::new(SAMPLES).join("IMG_4667.avif");
	let mut best = std::time::Duration::MAX;
	for _ in 0..7 {
		let started = std::time::Instant::now();
		std::hint::black_box(decode(&path).unwrap());
		best = best.min(started.elapsed());
	}
	println!("decode_bench: decode to RGB {best:?}");
	let rgb = decode(&path).unwrap();
	let mut best = std::time::Duration::MAX;
	for _ in 0..7 {
		let started = std::time::Instant::now();
		std::hint::black_box(pixels::shrink_area(&pixels::gray(&rgb), 0.5));
		best = best.min(started.elapsed());
	}
	println!("decode_bench: gray + half {best:?}");
}
/// The IMG_4698 to IMG_4699 match with OpenCV's own descriptors (`uv run analysis/dump_ransac.py`):
/// nearest neighbours, distances and the ratio test against BFMatcher's.
#[test]
#[ignore]
fn matching_matches_opencv() {
	let dump: serde_json::Value =
		serde_json::from_str(&std::fs::read_to_string(format!("{DUMPS}/ransac_4699.json")).unwrap()).unwrap();
	let descriptors = |key: &str| -> Vec<[u8; 128]> {
		dump[key]
			.as_array()
			.unwrap()
			.iter()
			.map(|d| std::array::from_fn(|i| d[i].as_f64().unwrap() as u8))
			.collect()
	};
	let (query, train) = (descriptors("box_descriptors"), descriptors("descriptors"));
	for (row, knn) in dump["knn"].as_array().unwrap().iter().enumerate() {
		let q = &query[row];
		let mut d: Vec<(f32, usize)> =
			train.iter().enumerate().map(|(t, b)| (crate::track::distance(q, b), t)).collect();
		d.sort_by(|a, b| a.partial_cmp(b).unwrap());
		let (m, n) = (knn[2].as_f64().unwrap() as f32, knn[4].as_f64().unwrap() as f32);
		let ours_pass = (d[0].0 as f64) < 0.8 * d[1].0 as f64;
		let theirs_pass = (m as f64) < 0.8 * n as f64;
		if d[0].0 != m || d[1].0 != n || ours_pass != theirs_pass {
			println!(
				"query {row}: ours {:?} {:?} theirs ({m}, {}) ({n}, {}); pass ours {ours_pass} theirs {theirs_pass}",
				d[0], d[1], knn[1], knn[3]
			);
		}
	}
}
/// The IMG_4698 to IMG_4699 step with this crate's SIFT: box features, pairs and inliers, to set beside
/// `analysis/dump_ransac.py`'s numbers from OpenCV.
#[test]
#[ignore]
fn box_step_4699() {
	let features = |n: u32| {
		let rgb = decode(&Path::new(SAMPLES).join(format!("IMG_{n}.avif"))).unwrap();
		crate::sift::detect_and_compute(&pixels::shrink_area(&pixels::gray(&rgb), 0.5))
	};
	let (a, b) = (features(4698), features(4699));
	let photo = crate::track::Photo { small: (1024, 768), features: a };
	let boxed = crate::track::features_in(&photo, 520.0, 362.5, 200.0, 175.0);
	let dump: serde_json::Value =
		serde_json::from_str(&std::fs::read_to_string(format!("{DUMPS}/ransac_4699.json")).unwrap()).unwrap();
	let theirs = dump["box_descriptors"].as_array().unwrap();
	println!("box_step_4699: {} box features (OpenCV {})", boxed.keypoints.len(), theirs.len());
	for (k, d) in boxed.keypoints.iter().zip(&boxed.descriptors) {
		let found = theirs
			.iter()
			.any(|t| t.as_array().unwrap().iter().zip(d).all(|(x, y)| x.as_f64().unwrap() as u8 == *y));
		if !found {
			println!("box_step_4699: ours only {k:?}");
		}
	}
	let (m, agreed) = crate::track::similarity(&boxed, &b, 6.0, None);
	println!("box_step_4699: {agreed} inliers, M {m:?}");
}
/// For each box keypoint whose descriptor differs from OpenCV's: how its fields and descriptor differ.
#[test]
#[ignore]
fn box_descriptor_diffs() {
	let rgb = decode(&Path::new(SAMPLES).join("IMG_4698.avif")).unwrap();
	let a = crate::sift::detect_and_compute(&pixels::shrink_area(&pixels::gray(&rgb), 0.5));
	let photo = crate::track::Photo { small: (1024, 768), features: a };
	let boxed = crate::track::features_in(&photo, 520.0, 362.5, 200.0, 175.0);
	let dump: serde_json::Value =
		serde_json::from_str(&std::fs::read_to_string(format!("{DUMPS}/ransac_4699.json")).unwrap()).unwrap();
	let kps = dump["box_keypoints"].as_array().unwrap();
	let des = dump["box_descriptors"].as_array().unwrap();
	for (i, (k, d)) in boxed.keypoints.iter().zip(&boxed.descriptors).enumerate() {
		let cv = &kps[i];
		let theirs: Vec<u8> = des[i].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as u8).collect();
		let off: Vec<(usize, u8, u8)> = d
			.iter()
			.zip(&theirs)
			.enumerate()
			.filter(|(_, (a, b))| a != b)
			.map(|(j, (a, b))| (j, *a, *b))
			.collect();
		if !off.is_empty() {
			let f = |j: usize| cv[j].as_f64().unwrap() as f32;
			println!(
				"box_descriptor_diffs: kp {i} ours x {} y {} size {} angle {} | cv x {} y {} size {} angle {} | {} values differ, e.g. {:?}",
				k.x,
				k.y,
				k.size,
				k.angle,
				f(0),
				f(1),
				f(2),
				f(3),
				off.len(),
				&off[..off.len().min(4)]
			);
		}
	}
}
/// Peak memory of one prepare or render task on the photo in TASK_PHOTO, run alone in this process
/// (`analysis/rust_task_memory.py` runs one per process and fits the planner's costs).
#[test]
#[ignore]
fn task_memory() {
	let Ok(photo) = std::env::var("TASK_PHOTO") else { return };
	let before = crate::profile::peak_rss();
	let path = Path::new(&photo);
	let rgb = decode(path).unwrap();
	let (w, h) = (rgb.width, rgb.height);
	if std::env::var("TASK").as_deref() == Ok("render") {
		let frame = crate::render::place(
			&rgb,
			(0.0, 0.0, w as f64, h as f64),
			(w as u32, h as u32),
			Some(&[0, 0, 0, 0]),
		);
		drop(rgb);
		std::hint::black_box(crate::yuv::to_yuv420(&frame));
	} else {
		let k = (1024.0 / w.max(h) as f64).min(1.0);
		let gray = pixels::gray(&rgb);
		let small = if k < 1.0 { pixels::shrink_area(&gray, k) } else { gray };
		std::hint::black_box(crate::sift::detect_and_compute(&small));
	}
	println!("task_memory: {} px, peak {} bytes, before {before}", w * h, crate::profile::peak_rss());
}
/// What a new mark costs when every photo's features are already in memory, as they could be in a
/// long-running app: tracking, filling lost photos and planning the crop for all 57 train clouds photos.
#[test]
#[ignore]
fn retrack_bench() {
	let mut paths: Vec<_> = std::fs::read_dir(SAMPLES)
		.unwrap()
		.flatten()
		.map(|e| e.path())
		.filter(|p| p.extension().is_some_and(|x| x == "avif"))
		.collect();
	paths.sort();
	let sizes: Vec<(u32, u32)> = paths.iter().map(|p| oriented_size(p).unwrap()).collect();
	let k = crate::track::track_factor(&sizes);
	let features: Vec<crate::sift::Features> = std::thread::scope(|scope| {
		let handles: Vec<_> = paths
			.chunks(paths.len().div_ceil(12))
			.map(|chunk| {
				scope.spawn(move || {
					chunk
						.iter()
						.map(|p| {
							let gray = pixels::gray(&decode(p).unwrap());
							crate::sift::detect_and_compute(&pixels::shrink_area(&gray, k))
						})
						.collect::<Vec<_>>()
				})
			})
			.collect();
		handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
	});
	let mut marks = vec![None; paths.len()];
	// Box 1010,410,1030,400's four quarter centres (train_clouds_all.sh's --points, A5b).
	marks[0] = Some(crate::track::Mark {
		points: vec![(1267.5, 510.0), (1782.5, 510.0), (1267.5, 710.0), (1782.5, 710.0)],
		zoom: None,
	});
	let mut best = std::time::Duration::MAX;
	for _ in 0..5 {
		let started = std::time::Instant::now();
		let photos = features.iter().zip(&sizes).map(|(f, &(w, h))| {
			Ok(crate::track::Photo {
				small: ((w as f64 * k).round() as usize, (h as f64 * k).round() as usize),
				features: crate::sift::Features {
					keypoints: f.keypoints.clone(),
					descriptors: f.descriptors.clone(),
				},
			})
		});
		let path = crate::track::fill_lost(
			crate::track::track(
				photos,
				&sizes,
				k,
				&marks,
				&vec![None; sizes.len()],
				&vec![false; sizes.len()],
			)
			.unwrap(),
			&sizes,
		)
		.unwrap();
		std::hint::black_box(crate::track::plan(&path, &sizes, true, None).unwrap());
		best = best.min(started.elapsed());
	}
	println!("retrack_bench: track + plan of {} photos from features in memory: {best:?}", paths.len());
}
