//! Photo input decoding. Dispatches by file signatures so renamed photos still work.

use crate::pixels::{orient, Image};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Format {
	Avif,
	Heic,
	Jpeg,
	Jxl,
	Png,
}

/// Conservative decode working-set costs measured by `analysis/rust_task_memory.py`.
pub fn memory_cost(path: &Path) -> Result<(usize, usize), String> {
	Ok(match format(path)? {
		Format::Png => (8, 13),
		// ImageIO retains a substantial decoded surface while the tracker and renderer work.
		// `rust_task_memory.py` measured both 8- and 10-bit HEIC at 8/11 B/px and JXL
		// (using the higher lossless/lossy result) at 10/14 B/px; always round up.
		Format::Heic => (8, 11),
		Format::Jxl => (10, 14),
		Format::Avif | Format::Jpeg => (6, 11),
	})
}

fn format_from_magic(bytes: &[u8]) -> Option<Format> {
	if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
		let mut brands = bytes[8..].chunks_exact(4);
		if brands.clone().any(|brand| matches!(brand, b"avif" | b"avis")) {
			Some(Format::Avif)
		} else if brands
			.any(|brand| matches!(brand, b"heic" | b"heix" | b"hevc" | b"heim" | b"heis" | b"mif1" | b"msf1"))
		{
			Some(Format::Heic)
		} else {
			None
		}
	} else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
		Some(Format::Jpeg)
	} else if bytes.starts_with(&[0xff, 0x0a]) || bytes.starts_with(b"\0\0\0\x0cJXL \r\n\x87\n") {
		Some(Format::Jxl)
	} else if bytes.starts_with(&PNG_SIGNATURE) {
		Some(Format::Png)
	} else {
		None
	}
}

fn format(path: &Path) -> Result<Format, String> {
	let mut bytes = [0; 32];
	let count = File::open(path).map_err(|e| e.to_string())?.read(&mut bytes).map_err(|e| e.to_string())?;
	format_from_magic(&bytes[..count])
		.ok_or_else(|| "unsupported format (supported: AVIF, HEIC, JPEG XL, JPEG, PNG)".into())
}

/// The first image as 8-bit RGB, oriented for display.
pub fn decode(path: &Path) -> Result<Image, String> {
	match format(path)? {
		Format::Avif => crate::avif::decode(path, 1),
		Format::Heic | Format::Jxl => crate::imageio::decode(path),
		Format::Jpeg => decode_jpeg(path),
		Format::Png => decode_png(path),
	}
}

/// Width and height after orientation, without decoding pixels.
pub fn oriented_size(path: &Path) -> Result<(u32, u32), String> {
	match format(path)? {
		Format::Avif => crate::avif::oriented_size(path),
		Format::Heic | Format::Jxl => crate::imageio::oriented_size(path),
		Format::Jpeg => jpeg_size(path),
		Format::Png => png_size(path),
	}
}

fn jpeg_orientation(path: &Path) -> u8 {
	let Ok(file) = File::open(path) else { return 1 };
	let mut reader = BufReader::new(file);
	exif::Reader::new()
		.read_from_container(&mut reader)
		.ok()
		.and_then(|exif| {
			exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
				.and_then(|field| field.value.get_uint(0))
		})
		.and_then(|value| u8::try_from(value).ok())
		.filter(|&value| (1..=8).contains(&value))
		.unwrap_or(1)
}

fn jpeg_size(path: &Path) -> Result<(u32, u32), String> {
	let file = File::open(path).map_err(|e| e.to_string())?;
	let mut decoder = zune_jpeg::JpegDecoder::new(BufReader::new(file));
	decoder.decode_headers().map_err(|e| format!("read header: {e}"))?;
	let info = decoder.info().ok_or("read header: JPEG has no image information")?;
	let (width, height) = (u32::from(info.width), u32::from(info.height));
	Ok(if matches!(jpeg_orientation(path), 5..=8) { (height, width) } else { (width, height) })
}

fn decode_jpeg(path: &Path) -> Result<Image, String> {
	let file = File::open(path).map_err(|e| e.to_string())?;
	let mut decoder = zune_jpeg::JpegDecoder::new(BufReader::new(file));
	let pixels = decoder.decode().map_err(|e| format!("decode JPEG: {e}"))?;
	let info = decoder.info().ok_or("decode JPEG: no image information")?;
	let (width, height) = (usize::from(info.width), usize::from(info.height));
	if pixels.len() != width * height * 3 {
		return Err(format!("decode JPEG: expected RGB pixels, got {} bytes", pixels.len()));
	}
	Ok(orient(Image { width, height, channels: 3, pixels }, jpeg_orientation(path)))
}

fn png_reader(path: &Path) -> Result<png::Reader<BufReader<File>>, String> {
	let file = File::open(path).map_err(|e| e.to_string())?;
	let mut decoder = png::Decoder::new(BufReader::new(file));
	decoder.set_transformations(png::Transformations::normalize_to_color8());
	decoder.read_info().map_err(|e| format!("read PNG header: {e}"))
}

fn png_size(path: &Path) -> Result<(u32, u32), String> {
	let reader = png_reader(path)?;
	Ok(reader.info().size())
}

fn decode_png(path: &Path) -> Result<Image, String> {
	let mut reader = png_reader(path)?;
	let mut raw = vec![0; reader.output_buffer_size().ok_or("decode PNG: image is too large")?];
	let info = reader.next_frame(&mut raw).map_err(|e| format!("decode PNG: {e}"))?;
	let pixels = match info.color_type {
		png::ColorType::Grayscale => raw[..info.buffer_size()].iter().flat_map(|&v| [v, v, v]).collect(),
		png::ColorType::GrayscaleAlpha => {
			raw[..info.buffer_size()].chunks_exact(2).flat_map(|p| [p[0], p[0], p[0]]).collect()
		}
		png::ColorType::Rgb => raw[..info.buffer_size()].to_vec(),
		png::ColorType::Rgba => {
			raw[..info.buffer_size()].chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
		}
		png::ColorType::Indexed => return Err("decode PNG: palette did not expand to RGB".into()),
	};
	Ok(Image { width: info.width as usize, height: info.height as usize, channels: 3, pixels })
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn detects_supported_signatures() {
		assert_eq!(format_from_magic(b"\0\0\0\x18ftypavif"), Some(Format::Avif));
		assert_eq!(format_from_magic(b"\0\0\0\x18ftypmif1\0\0\0\0avif"), Some(Format::Avif));
		assert_eq!(format_from_magic(b"\0\0\0\x18ftypheic"), Some(Format::Heic));
		assert_eq!(format_from_magic(b"\0\0\0\x18ftypmif1"), Some(Format::Heic));
		assert_eq!(format_from_magic(&[0xff, 0x0a]), Some(Format::Jxl));
		assert_eq!(format_from_magic(b"\0\0\0\x0cJXL \r\n\x87\n"), Some(Format::Jxl));
		assert_eq!(format_from_magic(&[0xff, 0xd8, 0xff, 0xe0]), Some(Format::Jpeg));
		assert_eq!(format_from_magic(&PNG_SIGNATURE), Some(Format::Png));
		assert_eq!(format_from_magic(b"RIFF....WEBP"), None);
	}

	#[test]
	fn unsupported_format_error() {
		let path = std::env::temp_dir().join(format!("pinframe-unsupported-{}.bin", std::process::id()));
		std::fs::write(&path, b"not a photo").unwrap();
		assert_eq!(
			oriented_size(&path),
			Err("unsupported format (supported: AVIF, HEIC, JPEG XL, JPEG, PNG)".into())
		);
		std::fs::remove_file(path).unwrap();
	}

	#[test]
	fn imageio_orientation_fixtures_are_applied() {
		for (name, reference) in [
			("orientation-6.heic", "orientation-6-heic.png"),
			("orientation-8.heic", "orientation-8-heic.png"),
			("orientation-6.jxl", "orientation-6-jxl.png"),
			("orientation-mirror.heic", "orientation-mirror-heic.png"),
		] {
			let path = Path::new("testdata").join(name);
			let image = decode(&path).unwrap();
			let expected = decode(Path::new("testdata").join(reference).as_path()).unwrap();
			assert_eq!((image.width, image.height), (expected.width, expected.height), "{name}");
			assert_eq!(oriented_size(&path).unwrap(), (expected.width as u32, expected.height as u32), "{name}");
			let worst = image.pixels.iter().zip(expected.pixels).map(|(a, b)| a.abs_diff(b)).max().unwrap();
			assert!(worst <= 16, "{name}: reference arrangement differs by {worst}");
		}
	}

	#[test]
	#[ignore]
	fn compare_decodes() {
		let a = decode(Path::new(&std::env::var("COMPARE_A").unwrap())).unwrap();
		let b = decode(Path::new(&std::env::var("COMPARE_B").unwrap())).unwrap();
		assert_eq!((a.width, a.height), (b.width, b.height));
		let mut sum = [0u64; 3];
		let mut max = 0u8;
		let mut square = 0u64;
		for (i, (left, right)) in a.pixels.iter().zip(&b.pixels).enumerate() {
			let d = left.abs_diff(*right);
			sum[i % 3] += u64::from(d);
			max = max.max(d);
			square += u64::from(d) * u64::from(d);
		}
		let n = (a.pixels.len() / 3) as f64;
		let mse = square as f64 / a.pixels.len() as f64;
		println!("mean {:?}, max {max}, PSNR {:.2} dB", sum.map(|v| v as f64 / n), 10.0 * (255.0 * 255.0 / mse).log10());
	}

	#[test]
	fn jpeg_orientations_match_pillow_exif_transpose() {
		// A 3x2, quality-100 JPEG and Pillow's `exif_transpose` RGB output, both dumped by
		// `uv run out/make_image_inputs.py`. Keeping the fixture here makes `cargo test` standalone.
		const JPEG: &str = concat!(
			"ffd8ffe000104a46494600010100000100010000ffe100224578696600004d4d002a0000000800010112000300000001",
			"0001000000000000ffdb0043000101010101010101010101010101010101010101010101010101010101010101010101",
			"0101010101010101010101010101010101010101010101010101010101ffdb0043010101010101010101010101010101",
			"010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101",
			"0101ffc00011080002000303011100021101031101ffc4001f0000010501010101010100000000000000000102030405",
			"060708090a0bffc400b5100002010303020403050504040000017d010203000411051221314106135161072271143281",
			"91a1082342b1c11552d1f02433627282090a161718191a25262728292a3435363738393a434445464748494a53545556",
			"5758595a636465666768696a737475767778797a838485868788898a92939495969798999aa2a3a4a5a6a7a8a9aab2b3",
			"b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7d8d9dae1e2e3e4e5e6e7e8e9eaf1f2f3f4f5f6f7f8f9faffc400",
			"1f0100030101010101010101010000000000000102030405060708090a0bffc400b51100020102040403040705040400",
			"010277000102031104052131061241510761711322328108144291a1b1c109233352f0156272d10a162434e125f11718",
			"191a262728292a35363738393a434445464748494a535455565758595a636465666768696a737475767778797a828384",
			"85868788898a92939495969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7",
			"d8d9dae2e3e4e5e6e7e8e9eaf2f3f4f5f6f7f8f9faffda000c03010002110311003f00f3bfdbe7e08fc17b7fdaf7e375",
			"9dbfc22f861059e9de20d234ad3ed21f00f8522b6b0d2f4af09f87f4dd2f4db28134958ad6c34dd3ad6dac2c2ce048ed",
			"ececade0b5b78e38218e35ff006fbf64bf13711e4dfb33fe83b4728e20cef2aa398fd1bbc33e25cc28e5b9ae3f034b1d",
			"c47c5f9061f8af8b33fc653c2e22943159df147146739c71271166b5d4f1d9d67f9ae659c6655f1398e3b1589abfee3f",
			"ecb2f043c16f15fe88143c43f14bc21f0bfc4ae3fe29fa507d3ff1fc4dc73c7fc01c29c63c61c458ea7f4fcfa4de0e9e",
			"373de26e22ca731ceb37c5c30986c3616188cc31b88ad1c361e8508cd52a54e31fffd9"
	);
		let expected = [
			"fe000001ff000000feffff0000ffffff00fe",
			"0000fe01ff00fe0000ff00fe00ffffffff00",
			"ff00fe00ffffffff000000fe01ff00fe0000",
			"ffff0000ffffff00fefe000001ff000000fe",
			"fe0000ffff0001ff0000ffff0000feff00fe",
			"ffff00fe000000ffff01ff00ff00fe0000fe",
			"ff00fe0000fe00ffff01ff00ffff00fe0000",
			"0000feff00fe01ff0000fffffe0000ffff00",
		];
		let base = hex(JPEG);
		let tag = [1, 18, 0, 3, 0, 0, 0, 1, 0, 1, 0, 0];
		let tag_offset = base.windows(tag.len()).position(|part| part == tag).unwrap();
		for (orientation, expected) in (1u8..=8).zip(expected) {
			let mut jpeg = base.clone();
			jpeg[tag_offset + 9] = orientation;
			let path = std::env::temp_dir()
				.join(format!("pinframe-orientation-{orientation}-{}.jpg", std::process::id()));
			std::fs::write(&path, jpeg).unwrap();
			let image = decode(&path).unwrap();
			assert_eq!(image.pixels, hex(expected), "orientation {orientation}");
			std::fs::remove_file(path).unwrap();
		}
	}

	#[test]
	#[ignore]
	fn zune_jpeg_difference_from_pillow() {
		for name in ["IMG_4667", "IMG_4669", "IMG_4670"] {
			let image =
				decode(Path::new("../out/girona-jpeg").join(format!("{name}.jpeg")).as_path()).unwrap();
			let pillow = std::fs::read(format!("../out/jpeg-rgb/{name}.raw")).unwrap();
			let worst = image.pixels.iter().zip(pillow).map(|(a, b)| a.abs_diff(b)).max().unwrap();
			println!("{name}: zune-jpeg vs Pillow maximum RGB difference {worst}");
		}
	}

	fn hex(text: &str) -> Vec<u8> {
		(0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
	}
}
