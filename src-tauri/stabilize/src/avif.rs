//! AVIF through libavif: photo headers and decode (dav1d), and the animated clip encode (libaom).

use crate::avif_sys::*;
use crate::colour::{self, Profile};
use crate::pixels::{Image, orient};
use std::ffi::{CStr, CString};
use std::path::Path;

fn check(result: avifResult, what: &str) -> Result<(), String> {
	if result == AVIF_RESULT_OK {
		return Ok(());
	}
	let reason = unsafe { CStr::from_ptr(avifResultToString(result)) };
	Err(format!("{what}: {}", reason.to_string_lossy()))
}

struct Decoder(*mut avifDecoder);

impl Drop for Decoder {
	fn drop(&mut self) {
		unsafe { avifDecoderDestroy(self.0) };
	}
}

impl Decoder {
	/// Parses the file's header only; pixels are read by `decode`.
	fn open(path: &Path, threads: i32) -> Result<Decoder, String> {
		let name = CString::new(path.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
		let decoder = Decoder(unsafe { avifDecoderCreate() });
		assert!(!decoder.0.is_null(), "libavif could not allocate a decoder");
		unsafe {
			(*decoder.0).maxThreads = threads;
			(*decoder.0).ignoreExif = AVIF_TRUE as avifBool;
			(*decoder.0).ignoreXMP = AVIF_TRUE as avifBool;
			check(avifDecoderSetIOFile(decoder.0, name.as_ptr()), "open")?;
			check(avifDecoderParse(decoder.0), "read header")?;
		}
		Ok(decoder)
	}

	fn image(&self) -> &avifImage {
		unsafe { &*(*self.0).image }
	}

	/// EXIF-style orientation (1-8) from the AVIF irot/imir boxes, as Pillow reports it.
	fn orientation(&self) -> u8 {
		let image = self.image();
		let rotated = image.transformFlags & AVIF_TRANSFORM_IROT != 0;
		let mirrored = image.transformFlags & AVIF_TRANSFORM_IMIR != 0;
		let angle = if rotated { image.irot.angle } else { 0 };
		let axis = image.imir.axis;
		// HEIF rotates anticlockwise first, then mirrors; axis 0 mirrors left-right.
		match (angle, mirrored, axis) {
			(0, false, _) => 1,
			(0, true, 0) => 2,
			(0, true, _) => 4,
			(1, false, _) => 8,
			(1, true, 0) => 7,
			(1, true, _) => 5,
			(2, false, _) => 3,
			(2, true, 0) => 4,
			(2, true, _) => 2,
			(3, false, _) => 6,
			(3, true, 0) => 5,
			(3, true, _) => 7,
			_ => 1,
		}
	}
}

/// libavif's version and its codecs' (dav1d, aom), for the feature cache key.
pub fn decoder_version() -> String {
	let mut codecs = [0 as std::ffi::c_char; 256];
	unsafe { avifCodecVersions(codecs.as_mut_ptr()) };
	let libavif = unsafe { CStr::from_ptr(avifVersion()) };
	let codecs = unsafe { CStr::from_ptr(codecs.as_ptr()) };
	format!("libavif {}; {}", libavif.to_string_lossy(), codecs.to_string_lossy())
}

/// Width and height after orientation, from the header alone.
pub fn oriented_size(path: &Path) -> Result<(u32, u32), String> {
	let decoder = Decoder::open(path, 1)?;
	let image = decoder.image();
	let turned = matches!(decoder.orientation(), 5..=8);
	Ok(if turned { (image.height, image.width) } else { (image.width, image.height) })
}

/// The photo's colour profile: its ICC profile if it has one, else its nclx code points.
pub fn profile(path: &Path) -> Result<Profile, String> {
	let decoder = Decoder::open(path, 1)?;
	let image = decoder.image();
	if image.icc.size > 0 {
		let icc = unsafe { std::slice::from_raw_parts(image.icc.data, image.icc.size) };
		return colour::from_icc(icc);
	}
	colour::from_cicp(image.colorPrimaries, image.transferCharacteristics)
}

/// The first frame as 8-bit RGB, oriented for display.
pub fn decode(path: &Path, threads: i32) -> Result<Image, String> {
	let decoder = Decoder::open(path, threads)?;
	unsafe { check(avifDecoderNextImage(decoder.0), "decode")? };
	let image = decoder.image();
	let (w, h) = (image.width as usize, image.height as usize);
	let mut pixels = vec![0u8; w * h * 3];
	if libyuv_jpeg_444(image) {
		yuv444_to_rgb(image, &mut pixels);
	} else {
		// ponytail: other layouts go through libavif's own conversion, within a level or two of Pillow's
		unsafe {
			let mut rgb = avifRGBImage::default();
			avifRGBImageSetDefaults(&mut rgb, image);
			rgb.format = AVIF_RGB_FORMAT_RGB;
			rgb.depth = 8;
			rgb.pixels = pixels.as_mut_ptr();
			rgb.rowBytes = (w * 3) as u32;
			check(avifImageYUVToRGB(image, &mut rgb), "convert to RGB")?;
		}
	}
	Ok(orient(Image { width: w, height: h, channels: 3, pixels }, decoder.orientation()))
}

/// Pillow's libavif converts with libyuv; this crate's Homebrew libavif has no libyuv. For full-range
/// BT.601 4:4:4, the layout of the train clouds photos, convert here exactly as libyuv does.
fn libyuv_jpeg_444(image: &avifImage) -> bool {
	let matrix = image.matrixCoefficients as u32;
	image.yuvFormat == AVIF_PIXEL_FORMAT_YUV444
		&& image.yuvRange == AVIF_RANGE_FULL
		&& matches!(image.depth, 8 | 10 | 12)
		&& [
			AVIF_MATRIX_COEFFICIENTS_BT601,
			AVIF_MATRIX_COEFFICIENTS_BT470BG,
			AVIF_MATRIX_COEFFICIENTS_UNSPECIFIED,
		]
		.contains(&matrix)
}

/// libavif first truncates deeper samples to 8 bits (`Convert16To8Plane`), then libyuv's `YuvPixel` with
/// its JPEG constants: 6-bit fixed-point weights.
fn yuv444_to_rgb(image: &avifImage, rgb: &mut [u8]) {
	const YG: u32 = 16320;
	const YB: i32 = 32;
	const UB: i32 = 113;
	const UG: i32 = 22;
	const VG: i32 = 46;
	const VR: i32 = 90;
	let (bb, bg, br) = (UB * 128 - YB, UG * 128 + VG * 128 + YB, VR * 128 - YB);
	let w = image.width as usize;
	let shift = image.depth - 8;
	let sample = |plane: usize, x: usize, y: usize| -> i32 {
		let row = unsafe { image.yuvPlanes[plane].add(y * image.yuvRowBytes[plane] as usize) };
		if shift == 0 {
			unsafe { *row.add(x) as i32 }
		} else {
			let v = unsafe { *(row as *const u16).add(x) } as i32;
			(v >> shift).min(255)
		}
	};
	for (y, out) in rgb.chunks_exact_mut(w * 3).enumerate() {
		for (x, px) in out.chunks_exact_mut(3).enumerate() {
			let (luma, u, v) = (sample(0, x, y) as u32, sample(1, x, y), sample(2, x, y));
			let y1 = ((luma * 0x0101 * YG) >> 16) as i32;
			let clamp = |v: i32| (v >> 6).clamp(0, 255) as u8;
			px[0] = clamp(y1 + v * VR - br);
			px[1] = clamp(y1 + bg - (u * UG + v * VG));
			px[2] = clamp(y1 + u * UB - bb);
		}
	}
}

/// An animated AVIF written frame by frame, with the settings `anchor.py` passes to Pillow.
pub struct Encoder {
	encoder: *mut avifEncoder,
	frame_ms: u64,
	xmp: Vec<u8>,
	profile: Profile,
	frames: usize,
}

impl Drop for Encoder {
	fn drop(&mut self) {
		unsafe { avifEncoderDestroy(self.encoder) };
	}
}

impl Encoder {
	pub fn new(
		speed: i32,
		quality: i32,
		frame_ms: u64,
		threads: i32,
		xmp: Vec<u8>,
		profile: Profile,
	) -> Encoder {
		let encoder = unsafe { avifEncoderCreate() };
		assert!(!encoder.is_null(), "libavif could not allocate an encoder");
		unsafe {
			(*encoder).maxThreads = threads;
			(*encoder).speed = speed;
			(*encoder).quality = quality;
			(*encoder).timescale = 1000;
			(*encoder).autoTiling = AVIF_TRUE as avifBool;
		}
		Encoder { encoder, frame_ms, xmp, profile, frames: 0 }
	}

	/// Encodes one frame. libavif reads the planes in place; it neither copies nor frees them.
	pub fn add(&mut self, frame: &crate::yuv::Yuv) -> Result<(), String> {
		unsafe {
			let image = avifImageCreate(frame.width as u32, frame.height as u32, 8, AVIF_PIXEL_FORMAT_YUV420);
			assert!(!image.is_null(), "libavif could not allocate a frame");
			(*image).yuvRange = AVIF_RANGE_FULL;
			(*image).colorPrimaries = self.profile.primaries();
			(*image).transferCharacteristics = colour::SRGB_TRANSFER;
			(*image).matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT601 as _;
			for (i, plane) in frame.planes.iter().enumerate() {
				(*image).yuvPlanes[i] = plane.as_ptr() as *mut u8;
				(*image).yuvRowBytes[i] = (if i == 0 { frame.width } else { frame.width / 2 }) as u32;
			}
			(*image).imageOwnsYUVPlanes = AVIF_FALSE as avifBool;
			if let Some(alpha) = &frame.alpha {
				(*image).alphaPlane = alpha.as_ptr() as *mut u8;
				(*image).alphaRowBytes = frame.width as u32;
				(*image).imageOwnsAlphaPlane = AVIF_FALSE as avifBool;
			}
			let mut result = AVIF_RESULT_OK;
			if self.frames == 0 {
				result = avifImageSetMetadataXMP(image, self.xmp.as_ptr(), self.xmp.len());
			}
			if result == AVIF_RESULT_OK {
				result = avifEncoderAddImage(self.encoder, image, self.frame_ms, AVIF_ADD_IMAGE_FLAG_NONE);
			}
			avifImageDestroy(image);
			check(result, "encode frame")?;
		}
		self.frames += 1;
		Ok(())
	}

	pub fn finish(self, out: &Path) -> Result<(), String> {
		let mut data = avifRWData::default();
		unsafe { check(avifEncoderFinish(self.encoder, &mut data), "finish clip")? };
		let bytes = unsafe { std::slice::from_raw_parts(data.data, data.size) };
		let written = std::fs::write(out, bytes).map_err(|e| format!("{}: {e}", out.display()));
		unsafe { avifRWDataFree(&mut data) };
		written
	}
}
