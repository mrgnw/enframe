//! Comparisons with dumps from `anchor.py`'s own functions (`uv run analysis/dump_sift.py NAME`).
//! Run with `cargo test --release -- --ignored --nocapture`.

use crate::{avif, pixels};
use std::path::Path;

const SAMPLES: &str = "../samples/girona clouds";
const DUMPS: &str = "../out";

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
