//! Photo input decoding. Dispatches by file signatures so renamed photos still work.

use crate::colour::{self, Profile};
use crate::pixels::{Image, orient};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::OnceLock;

// AVIF decodes through ImageIO on macOS, so the macOS build has no C; elsewhere through libavif.
#[cfg(not(target_os = "macos"))]
use crate::avif;
#[cfg(target_os = "macos")]
use crate::imageio as avif;

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
		// AVIF through ImageIO measured 12/16 B/px at 12 MP (over the planner's fixed costs), more than its
		// 8/10 at 48 MP; libavif, off macOS, fits within JPEG's 6/11.
		Format::Avif if cfg!(target_os = "macos") => (12, 16),
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

/// The photo's format from its first bytes. Off macOS, HEIC and JPEG XL are refused here, before any
/// decoding, since they decode through ImageIO (`imageio.rs`).
fn format(path: &Path) -> Result<Format, String> {
	let mut bytes = [0; 32];
	let count = File::open(path).map_err(|e| e.to_string())?.read(&mut bytes).map_err(|e| e.to_string())?;
	let format = format_from_magic(&bytes[..count])
		.ok_or_else(|| "unsupported format (supported: AVIF, HEIC, JPEG XL, JPEG, PNG)".to_string())?;
	if !cfg!(target_os = "macos") && matches!(format, Format::Heic | Format::Jxl) {
		let name = if format == Format::Heic { "HEIC" } else { "JPEG XL" };
		return Err(format!("{name} input needs macOS (ImageIO); not supported on this platform yet"));
	}
	Ok(format)
}

/// The decoder and version that produce this photo's pixels, for the feature cache key. Read once per run.
pub fn decoder_id(path: &Path) -> Result<&'static str, String> {
	static AVIF: OnceLock<String> = OnceLock::new();
	static IMAGEIO: OnceLock<String> = OnceLock::new();
	Ok(match format(path)? {
		Format::Avif => AVIF.get_or_init(avif::decoder_version),
		Format::Heic | Format::Jxl => IMAGEIO.get_or_init(crate::imageio::decoder_version),
		Format::Jpeg => concat!("zune-jpeg ", env!("PINFRAME_VERSION_ZUNE_JPEG")),
		Format::Png => concat!("png ", env!("PINFRAME_VERSION_PNG")),
	})
}

/// The first image as 8-bit RGB, oriented for display.
pub fn decode(path: &Path) -> Result<Image, String> {
	match format(path)? {
		Format::Avif => avif::decode(path),
		Format::Heic | Format::Jxl => crate::imageio::decode(path),
		Format::Jpeg => decode_jpeg(path),
		Format::Png => decode_png(path),
	}
}

/// The photo's colour profile from its header. A photo with no label is sRGB, as browsers show it.
pub fn profile(path: &Path) -> Result<Profile, String> {
	match format(path)? {
		Format::Avif => avif::profile(path),
		Format::Heic | Format::Jxl => crate::imageio::profile(path),
		Format::Jpeg => jpeg_profile(path),
		Format::Png => png_profile(path),
	}
}

/// Width and height after orientation, without decoding pixels.
pub fn oriented_size(path: &Path) -> Result<(u32, u32), String> {
	match format(path)? {
		Format::Avif => avif::oriented_size(path),
		Format::Heic | Format::Jxl => crate::imageio::oriented_size(path),
		Format::Jpeg => jpeg_size(path),
		Format::Png => png_size(path),
	}
}

/// A sort key with the exact ordering of EXIF `DateTimeOriginal`/`SubSecTimeOriginal`: the date string's
/// fixed-width fields already sort chronologically, and the subsecond digits are padded to a common
/// width on the right so a 3-digit and a 6-digit reading of the same instant still compare equal-ish
/// (finer only breaks genuine ties).
fn capture_time_key(date: &str, subsec: &str) -> Option<String> {
	if date.len() != 19 || !date.is_ascii() {
		return None;
	}
	let subsec: String = subsec.chars().filter(char::is_ascii_digit).collect();
	Some(format!("{date}.{subsec:0<9}"))
}

/// The ASCII value of an EXIF field, trimmed of the trailing NUL the TIFF spec pads strings with.
fn ascii_field(exif: &exif::Exif, tag: exif::Tag) -> Option<String> {
	match &exif.get_field(tag, exif::In::PRIMARY)?.value {
		exif::Value::Ascii(values) => {
			let text = String::from_utf8_lossy(values.first()?).trim_end_matches('\0').trim().to_string();
			(!text.is_empty()).then_some(text)
		}
		_ => None,
	}
}

/// What Stabilize reads from a photo's EXIF: its capture time as a sort key (`capture_time_key`), and its
/// focal length in 35 mm film terms, which names the lens and any digital crop.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Exif {
	pub capture_time: Option<String>,
	pub focal_35mm: Option<f64>,
}

/// `FocalLengthIn35mmFilm` as a positive length; 0 means unknown in the EXIF spec.
fn focal_35mm(exif: &exif::Exif) -> Option<f64> {
	let value = exif.get_field(exif::Tag::FocalLengthIn35mmFilm, exif::In::PRIMARY)?.value.get_uint(0)?;
	(value > 0).then_some(f64::from(value))
}

/// Both fields in one read via kamadak-exif, which parses AVIF, HEIC, JPEG and PNG containers itself.
fn read_exif(path: &Path) -> Result<Exif, String> {
	let file = File::open(path).map_err(|e| e.to_string())?;
	let exif = match exif::Reader::new().read_from_container(&mut BufReader::new(file)) {
		Ok(exif) => exif,
		Err(exif::Error::Io(e)) => return Err(e.to_string()),
		Err(_) => return Ok(Exif::default()),
	};
	let capture_time = ascii_field(&exif, exif::Tag::DateTimeOriginal).and_then(|date| {
		let subsec = ascii_field(&exif, exif::Tag::SubSecTimeOriginal).unwrap_or_default();
		capture_time_key(&date, &subsec)
	});
	Ok(Exif { capture_time, focal_35mm: focal_35mm(&exif) })
}

/// A field is `None` when the photo does not carry it, so callers fall back to filename order and to
/// tracking without a lens prior. JXL is not among kamadak-exif's containers: both fields read through
/// ImageIO instead.
pub fn exif(path: &Path) -> Result<Exif, String> {
	match format(path)? {
		Format::Jxl => {
			let (date_time, focal_35mm) = crate::imageio::exif_date_time_and_focal_35mm(path)?;
			let capture_time = date_time.and_then(|dt| capture_time_key(&dt.date, &dt.subsec));
			Ok(Exif { capture_time, focal_35mm })
		}
		Format::Avif | Format::Heic | Format::Jpeg | Format::Png => read_exif(path),
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

fn jpeg_profile(path: &Path) -> Result<Profile, String> {
	let file = File::open(path).map_err(|e| e.to_string())?;
	let mut decoder = zune_jpeg::JpegDecoder::new(BufReader::new(file));
	decoder.decode_headers().map_err(|e| format!("read header: {e}"))?;
	decoder.icc_profile().map_or(Ok(Profile::Srgb), |icc| colour::from_icc(&icc))
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

/// cICP outranks iCCP, which outranks sRGB (PNG third edition); gAMA and cHRM alone are ignored.
fn png_profile(path: &Path) -> Result<Profile, String> {
	let reader = png_reader(path)?;
	let info = reader.info();
	if let Some(cicp) = info.coding_independent_code_points {
		return colour::from_cicp(cicp.color_primaries.into(), cicp.transfer_function.into());
	}
	info.icc_profile.as_deref().map_or(Ok(Profile::Srgb), colour::from_icc)
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
	fn capture_time_key_sorts_chronologically_and_rejects_malformed_dates() {
		let a = capture_time_key("2024:01:01 00:00:01", "010").unwrap();
		let b = capture_time_key("2024:01:01 00:00:01", "5").unwrap();
		let c = capture_time_key("2024:01:01 00:00:02", "").unwrap();
		assert!(a < b, "010 then 5 (0.010s before 0.5s), despite '5' < '010' byte-wise");
		assert!(b < c);
		assert_eq!(capture_time_key("", ""), None, "an empty date is not 19 characters");
		assert_eq!(capture_time_key("not a valid 19-char date!!", ""), None, "still not 19 characters");
	}

	/// A 1x1 PNG whose `eXIf` chunk holds one field, `FocalLengthIn35mmFilm` = `focal`.
	fn png_with_focal(path: &Path, focal: u16) {
		let mut tiff = Vec::new();
		tiff.extend(b"II");
		tiff.extend(42u16.to_le_bytes());
		tiff.extend(8u32.to_le_bytes()); // IFD0 at offset 8
		tiff.extend(1u16.to_le_bytes()); // IFD0: one entry
		tiff.extend(0x8769u16.to_le_bytes()); // ExifIFDPointer
		tiff.extend(4u16.to_le_bytes()); // LONG
		tiff.extend(1u32.to_le_bytes());
		tiff.extend(26u32.to_le_bytes()); // Exif IFD at offset 26
		tiff.extend(0u32.to_le_bytes()); // no next IFD
		tiff.extend(1u16.to_le_bytes()); // Exif IFD: one entry
		tiff.extend(0xa405u16.to_le_bytes()); // FocalLengthIn35mmFilm
		tiff.extend(3u16.to_le_bytes()); // SHORT
		tiff.extend(1u32.to_le_bytes());
		tiff.extend(focal.to_le_bytes());
		tiff.extend([0, 0]); // the value field's padding
		tiff.extend(0u32.to_le_bytes()); // no next IFD
		let mut info = png::Info::with_size(1, 1);
		info.color_type = png::ColorType::Rgb;
		let file = std::io::BufWriter::new(File::create(path).unwrap());
		let mut writer = png::Encoder::with_info(file, info).unwrap().write_header().unwrap();
		writer.write_chunk(png::chunk::ChunkType(*b"eXIf"), &tiff).unwrap();
		writer.write_image_data(&[0, 0, 0]).unwrap();
	}

	#[test]
	fn focal_length_reads_from_exif_and_zero_means_unknown() {
		let path = std::env::temp_dir().join(format!("stabilize-focal-{}.png", std::process::id()));
		png_with_focal(&path, 120);
		assert_eq!(exif(&path), Ok(Exif { capture_time: None, focal_35mm: Some(120.0) }));
		png_with_focal(&path, 0);
		assert_eq!(exif(&path), Ok(Exif::default()));
		std::fs::remove_file(path).unwrap();
		assert_eq!(exif(Path::new("testdata/orientation-6.png")).map(|e| e.focal_35mm), Ok(None));
	}

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
		let path = std::env::temp_dir().join(format!("stabilize-unsupported-{}.bin", std::process::id()));
		std::fs::write(&path, b"not a photo").unwrap();
		assert_eq!(
			oriented_size(&path),
			Err("unsupported format (supported: AVIF, HEIC, JPEG XL, JPEG, PNG)".into())
		);
		std::fs::remove_file(path).unwrap();
	}

	#[cfg(target_os = "macos")]
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
			assert_eq!(
				oriented_size(&path).unwrap(),
				(expected.width as u32, expected.height as u32),
				"{name}"
			);
			let worst = image.pixels.iter().zip(expected.pixels).map(|(a, b)| a.abs_diff(b)).max().unwrap();
			assert!(worst <= 16, "{name}: reference arrangement differs by {worst}");
		}
	}

	/// Off macOS, HEIC and JPEG XL are refused by every entry point, with the same message, before decoding.
	#[cfg(not(target_os = "macos"))]
	#[test]
	fn heic_and_jxl_are_refused_off_macos() {
		for (name, expected) in [
			("orientation-6.heic", "HEIC input needs macOS (ImageIO); not supported on this platform yet"),
			("orientation-6.jxl", "JPEG XL input needs macOS (ImageIO); not supported on this platform yet"),
		] {
			let path = Path::new("testdata").join(name);
			assert_eq!(oriented_size(&path), Err(expected.into()), "{name}");
			assert_eq!(profile(&path), Err(expected.into()), "{name}");
			assert_eq!(exif(&path), Err(expected.into()), "{name}");
			assert_eq!(decoder_id(&path), Err(expected.into()), "{name}");
			assert_eq!(memory_cost(&path), Err(expected.into()), "{name}");
			assert_eq!(decode(&path).map(|i| i.width), Err(expected.into()), "{name}");
		}
	}

	#[cfg(target_os = "macos")]
	#[test]
	fn profiles_from_fixtures_and_png_labels() {
		for name in ["orientation-6.heic", "orientation-6.jxl", "orientation-6.png"] {
			assert_eq!(profile(&Path::new("testdata").join(name)), Ok(Profile::Srgb), "{name}");
		}
		let path = std::env::temp_dir().join(format!("stabilize-profile-{}.png", std::process::id()));
		let p3_icc = colour::display_p3_icc();
		let adobe_icc = std::fs::read("/System/Library/ColorSync/Profiles/AdobeRGB1998.icc").unwrap();
		let write = |icc: Option<&[u8]>, cicp: Option<[u8; 4]>| {
			let mut info = png::Info::with_size(1, 1);
			info.color_type = png::ColorType::Rgb;
			info.icc_profile = icc.map(|icc| icc.to_vec().into());
			let file = std::io::BufWriter::new(File::create(&path).unwrap());
			let mut writer = png::Encoder::with_info(file, info).unwrap().write_header().unwrap();
			if let Some(cicp) = cicp {
				writer.write_chunk(png::chunk::ChunkType(*b"cICP"), &cicp).unwrap();
			}
			writer.write_image_data(&[0, 0, 0]).unwrap();
		};
		write(None, None);
		assert_eq!(profile(&path), Ok(Profile::Srgb), "unlabelled");
		write(Some(&p3_icc), None);
		assert_eq!(profile(&path), Ok(Profile::DisplayP3), "iCCP");
		write(None, Some([12, 13, 0, 1]));
		assert_eq!(profile(&path), Ok(Profile::DisplayP3), "cICP");
		write(Some(&adobe_icc), Some([1, 13, 0, 1]));
		assert_eq!(profile(&path), Ok(Profile::Srgb), "cICP outranks iCCP");
		write(Some(&adobe_icc), None);
		assert!(profile(&path).is_err(), "Adobe RGB");
		std::fs::remove_file(path).unwrap();
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
		println!(
			"mean {:?}, max {max}, PSNR {:.2} dB",
			sum.map(|v| v as f64 / n),
			10.0 * (255.0 * 255.0 / mse).log10()
		);
	}

	#[test]
	#[ignore]
	fn decode_bench() {
		let dir = std::env::var("BENCH_DIR").unwrap();
		let mut paths: Vec<_> = std::fs::read_dir(dir)
			.unwrap()
			.flatten()
			.map(|entry| entry.path())
			.filter(|path| path.is_file() && !path.file_name().unwrap().to_string_lossy().starts_with('.'))
			.collect();
		paths.sort();
		paths.truncate(10);
		let mut wall = vec![];
		let mut cpu = vec![];
		for _ in 0..std::env::var("BENCH_ROUNDS").map_or(5, |v| v.parse().unwrap()) {
			let mut before: libc::rusage = unsafe { std::mem::zeroed() };
			unsafe { libc::getrusage(libc::RUSAGE_SELF, &raw mut before) };
			let start = std::time::Instant::now();
			for path in &paths {
				std::hint::black_box(decode(path).unwrap());
			}
			wall.push(start.elapsed().as_secs_f64() / paths.len() as f64);
			let mut after = before;
			unsafe { libc::getrusage(libc::RUSAGE_SELF, &raw mut after) };
			let seconds = |v: libc::timeval| v.tv_sec as f64 + v.tv_usec as f64 / 1e6;
			cpu.push(
				(seconds(after.ru_utime) + seconds(after.ru_stime)
					- seconds(before.ru_utime)
					- seconds(before.ru_stime))
					/ paths.len() as f64,
			);
		}
		wall.sort_by(f64::total_cmp);
		cpu.sort_by(f64::total_cmp);
		println!(
			"decode_bench: wall fastest {:.4}, median {:.4}; CPU fastest {:.4}, median {:.4} s/photo",
			wall[0],
			wall[wall.len() / 2],
			cpu[0],
			cpu[cpu.len() / 2]
		);
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
				.join(format!("stabilize-orientation-{orientation}-{}.jpg", std::process::id()));
			std::fs::write(&path, jpeg).unwrap();
			let image = decode(&path).unwrap();
			assert_eq!(image.pixels, hex(expected), "orientation {orientation}");
			std::fs::remove_file(path).unwrap();
		}
	}

	/// A JXL re-encoded from a train clouds AVIF (JXL_PATH, e.g. via `sips`/`cjxl`) reports the same
	/// `FocalLengthIn35mmFilm` as the AVIF (AVIF_PATH) it came from, read through ImageIO.
	#[test]
	#[ignore]
	fn jxl_focal_35mm_matches_avif() {
		let avif = exif(Path::new(&std::env::var("AVIF_PATH").unwrap())).unwrap();
		let jxl = exif(Path::new(&std::env::var("JXL_PATH").unwrap())).unwrap();
		assert!(avif.focal_35mm.is_some(), "AVIF fixture has no focal length");
		assert_eq!(jxl.focal_35mm, avif.focal_35mm);
	}

	#[test]
	#[ignore]
	fn zune_jpeg_difference_from_pillow() {
		for name in ["IMG_4667", "IMG_4669", "IMG_4670"] {
			let image =
				decode(Path::new("../out/train-clouds-jpeg").join(format!("{name}.jpeg")).as_path()).unwrap();
			let pillow = std::fs::read(format!("../out/jpeg-rgb/{name}.raw")).unwrap();
			let worst = image.pixels.iter().zip(pillow).map(|(a, b)| a.abs_diff(b)).max().unwrap();
			println!("{name}: zune-jpeg vs Pillow maximum RGB difference {worst}");
		}
	}

	fn hex(text: &str) -> Vec<u8> {
		(0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
	}
}
