//! HEIC and JPEG XL through macOS ImageIO, preserving the decoder's native pixels.

use crate::pixels::{Image, orient};
use std::ffi::c_void;
use std::fs;
use std::path::Path;
use std::ptr;

type CFTypeRef = *const c_void;
type CFDataRef = *const c_void;
type CGImageSourceRef = *const c_void;
type CGImageRef = *const c_void;

const K_CF_NUMBER_SINT32_TYPE: i32 = 3;
const BYTE_ORDER_16_LITTLE: u32 = 0x1000;
const BYTE_ORDER_32_LITTLE: u32 = 0x2000;
const BYTE_ORDER_MASK: u32 = 0x7000;
const ALPHA_INFO_MASK: u32 = 0x1f;
const ALPHA_NONE: u32 = 0;
const ALPHA_PREMULTIPLIED_LAST: u32 = 1;
const ALPHA_PREMULTIPLIED_FIRST: u32 = 2;
const ALPHA_LAST: u32 = 3;
const ALPHA_FIRST: u32 = 4;
const ALPHA_NONE_SKIP_LAST: u32 = 5;
const ALPHA_NONE_SKIP_FIRST: u32 = 6;

#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
	static kCGImagePropertyOrientation: CFTypeRef;
	fn CGImageSourceCreateWithData(data: CFDataRef, options: CFTypeRef) -> CGImageSourceRef;
	fn CGImageSourceCreateImageAtIndex(
		source: CGImageSourceRef,
		index: usize,
		options: CFTypeRef,
	) -> CGImageRef;
	fn CGImageSourceCopyPropertiesAtIndex(
		source: CGImageSourceRef,
		index: usize,
		options: CFTypeRef,
	) -> CFTypeRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
	fn CGImageGetWidth(image: CGImageRef) -> usize;
	fn CGImageGetHeight(image: CGImageRef) -> usize;
	fn CGImageGetBitsPerComponent(image: CGImageRef) -> usize;
	fn CGImageGetBitsPerPixel(image: CGImageRef) -> usize;
	fn CGImageGetBytesPerRow(image: CGImageRef) -> usize;
	fn CGImageGetBitmapInfo(image: CGImageRef) -> u32;
	fn CGImageGetDataProvider(image: CGImageRef) -> CFTypeRef;
	fn CGDataProviderCopyData(provider: CFTypeRef) -> CFDataRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
	static kCFAllocatorDefault: CFTypeRef;
	fn CFDataCreate(allocator: CFTypeRef, bytes: *const u8, length: isize) -> CFDataRef;
	fn CFDataGetBytePtr(data: CFDataRef) -> *const u8;
	fn CFDataGetLength(data: CFDataRef) -> isize;
	fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
	fn CFNumberGetValue(number: CFTypeRef, number_type: i32, value: *mut c_void) -> bool;
	fn CFRelease(value: CFTypeRef);
}

struct Release(CFTypeRef);

impl Drop for Release {
	fn drop(&mut self) {
		if !self.0.is_null() {
			unsafe { CFRelease(self.0) };
		}
	}
}

struct Source {
	source: Release,
	// CGImageSourceCreateWithData is documented to retain the data, but retaining it ourselves
	// also makes that lifetime explicit at this FFI boundary.
	_data: Release,
}

fn source(path: &Path) -> Result<Source, String> {
	let bytes = fs::read(path).map_err(|e| e.to_string())?;
	let length = isize::try_from(bytes.len()).map_err(|_| "ImageIO input is too large")?;
	let data = unsafe { CFDataCreate(kCFAllocatorDefault, bytes.as_ptr(), length) };
	let data = Release(data);
	if data.0.is_null() {
		return Err("ImageIO could not allocate input data".into());
	}
	let source = Release(unsafe { CGImageSourceCreateWithData(data.0, ptr::null()) });
	if source.0.is_null() {
		return Err("ImageIO could not read image".into());
	}
	Ok(Source { source, _data: data })
}

fn orientation(source: CGImageSourceRef) -> u8 {
	let properties = Release(unsafe { CGImageSourceCopyPropertiesAtIndex(source, 0, ptr::null()) });
	if properties.0.is_null() {
		return 1;
	}
	let value = unsafe { CFDictionaryGetValue(properties.0, kCGImagePropertyOrientation) };
	let mut orientation = 1i32;
	if value.is_null()
		|| !unsafe { CFNumberGetValue(value, K_CF_NUMBER_SINT32_TYPE, (&raw mut orientation).cast()) }
	{
		return 1;
	}
	u8::try_from(orientation).ok().filter(|v| (1..=8).contains(v)).unwrap_or(1)
}

/// The macOS build (`kern.osversion`, e.g. 25A354): ImageIO's output can change with an OS update.
pub fn decoder_version() -> String {
	let mut build = [0u8; 64];
	let mut len = build.len();
	let status = unsafe {
		libc::sysctlbyname(
			c"kern.osversion".as_ptr(),
			build.as_mut_ptr().cast(),
			&raw mut len,
			ptr::null_mut(),
			0,
		)
	};
	assert!(status == 0 && len > 0 && len <= build.len(), "sysctl kern.osversion failed");
	let build = std::ffi::CStr::from_bytes_until_nul(&build[..len]).expect("kern.osversion is a C string");
	format!("ImageIO macOS {}", build.to_string_lossy())
}

pub fn oriented_size(path: &Path) -> Result<(u32, u32), String> {
	let source = source(path)?;
	let image = Release(unsafe { CGImageSourceCreateImageAtIndex(source.source.0, 0, ptr::null()) });
	if image.0.is_null() {
		return Err("ImageIO could not decode image header".into());
	}
	let width =
		u32::try_from(unsafe { CGImageGetWidth(image.0) }).map_err(|_| "ImageIO image is too wide")?;
	let height =
		u32::try_from(unsafe { CGImageGetHeight(image.0) }).map_err(|_| "ImageIO image is too tall")?;
	Ok(if matches!(orientation(source.source.0), 5..=8) { (height, width) } else { (width, height) })
}

/// Decodes the first image. 10- and 16-bit samples are rounded to the nearest 8-bit value;
/// premultiplied alpha is undone before alpha is discarded. Colour profiles are deliberately ignored.
pub fn decode(path: &Path) -> Result<Image, String> {
	let source = source(path)?;
	let image = Release(unsafe { CGImageSourceCreateImageAtIndex(source.source.0, 0, ptr::null()) });
	if image.0.is_null() {
		return Err("ImageIO could not decode image".into());
	}
	let width = unsafe { CGImageGetWidth(image.0) };
	let height = unsafe { CGImageGetHeight(image.0) };
	let bits = unsafe { CGImageGetBitsPerComponent(image.0) };
	let pixel_bits = unsafe { CGImageGetBitsPerPixel(image.0) };
	let row = unsafe { CGImageGetBytesPerRow(image.0) };
	let info = unsafe { CGImageGetBitmapInfo(image.0) };
	let provider = unsafe { CGImageGetDataProvider(image.0) };
	let data = Release(unsafe { CGDataProviderCopyData(provider) });
	if data.0.is_null() {
		return Err("ImageIO could not read decoded pixels".into());
	}
	let length =
		usize::try_from(unsafe { CFDataGetLength(data.0) }).map_err(|_| "ImageIO pixel data is too large")?;
	let raw = unsafe { std::slice::from_raw_parts(CFDataGetBytePtr(data.0), length) };
	let pixels = unpack(raw, width, height, bits, pixel_bits, row, info)?;
	Ok(orient(Image { width, height, channels: 3, pixels }, orientation(source.source.0)))
}

fn unpack(
	raw: &[u8],
	width: usize,
	height: usize,
	bits: usize,
	pixel_bits: usize,
	row: usize,
	info: u32,
) -> Result<Vec<u8>, String> {
	if !matches!(bits, 8 | 10 | 16)
		|| !matches!(pixel_bits, 24 | 32 | 48 | 64)
		|| row < width * (pixel_bits / 8)
		|| raw.len() < row * height
	{
		return Err(format!(
			"ImageIO unsupported pixel layout: {bits}-bit components, {pixel_bits}-bit pixels"
		));
	}
	// ImageIO decodes HEVC Main 10 as `l10r`: little-endian 0b00_RRRRRRRRRR_GGGGGGGGGG_BBBBBBBBBB.
	// It has two unused high bits, not an alpha channel.
	if bits == 10 && pixel_bits == 32 && info & BYTE_ORDER_MASK == BYTE_ORDER_32_LITTLE {
		let mut out = Vec::with_capacity(width * height * 3);
		for y in 0..height {
			for x in 0..width {
				let offset = y * row + x * 4;
				let value = u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap());
				out.extend([reduce_10(value >> 20), reduce_10(value >> 10), reduce_10(value)]);
			}
		}
		return Ok(out);
	}
	let alpha = info & ALPHA_INFO_MASK;
	let has_alpha =
		matches!(alpha, ALPHA_PREMULTIPLIED_LAST | ALPHA_PREMULTIPLIED_FIRST | ALPHA_LAST | ALPHA_FIRST);
	let premultiplied = matches!(alpha, ALPHA_PREMULTIPLIED_LAST | ALPHA_PREMULTIPLIED_FIRST);
	let first = matches!(alpha, ALPHA_PREMULTIPLIED_FIRST | ALPHA_FIRST | ALPHA_NONE_SKIP_FIRST);
	if !matches!(
		alpha,
		ALPHA_NONE
			| ALPHA_PREMULTIPLIED_LAST
			| ALPHA_PREMULTIPLIED_FIRST
			| ALPHA_LAST
			| ALPHA_FIRST
			| ALPHA_NONE_SKIP_LAST
			| ALPHA_NONE_SKIP_FIRST
	) {
		return Err(format!("ImageIO unsupported alpha layout {alpha}"));
	}
	let channels = pixel_bits / bits;
	let order = info & BYTE_ORDER_MASK;
	let mut out = Vec::with_capacity(width * height * 3);
	for y in 0..height {
		for x in 0..width {
			let p = &raw[y * row + x * (pixel_bits / 8)..];
			let sample = |channel: usize| sample(p, channel, bits, order);
			let (mut r, mut g, mut b, a) = if channels == 3 {
				(sample(0), sample(1), sample(2), u16::MAX)
			} else if bits == 8 && order == BYTE_ORDER_32_LITTLE {
				(sample(2), sample(1), sample(0), sample(3))
			} else if first {
				(sample(1), sample(2), sample(3), sample(0))
			} else {
				(sample(0), sample(1), sample(2), sample(3))
			};
			if !has_alpha || matches!(alpha, ALPHA_NONE_SKIP_LAST | ALPHA_NONE_SKIP_FIRST) {
				// Skip-alpha layouts have an unused fourth sample.
			} else if premultiplied && a != 0 {
				r = ((u32::from(r) * u32::from(u16::MAX) + u32::from(a) / 2) / u32::from(a))
					.min(u32::from(u16::MAX)) as u16;
				g = ((u32::from(g) * u32::from(u16::MAX) + u32::from(a) / 2) / u32::from(a))
					.min(u32::from(u16::MAX)) as u16;
				b = ((u32::from(b) * u32::from(u16::MAX) + u32::from(a) / 2) / u32::from(a))
					.min(u32::from(u16::MAX)) as u16;
			} else if premultiplied {
				r = 0;
				g = 0;
				b = 0;
			}
			out.extend([reduce(r, bits), reduce(g, bits), reduce(b, bits)]);
		}
	}
	Ok(out)
}

fn sample(bytes: &[u8], channel: usize, bits: usize, order: u32) -> u16 {
	if bits == 8 {
		u16::from(bytes[channel]) * 257
	} else {
		let bytes = [bytes[channel * 2], bytes[channel * 2 + 1]];
		if order == BYTE_ORDER_16_LITTLE { u16::from_le_bytes(bytes) } else { u16::from_be_bytes(bytes) }
	}
}

fn reduce(sample: u16, bits: usize) -> u8 {
	if bits == 8 { (sample / 257) as u8 } else { ((u32::from(sample) + 128) / 257) as u8 }
}

fn reduce_10(sample: u32) -> u8 {
	(((sample & 1023) * 255 + 511) / 1023) as u8
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn reduces_16_bit_samples_with_rounding() {
		assert_eq!(reduce(0, 16), 0);
		assert_eq!(reduce(128, 16), 0);
		assert_eq!(reduce(129, 16), 1);
		assert_eq!(reduce(u16::MAX, 16), 255);
	}

	#[test]
	fn reduces_10_bit_samples_with_rounding() {
		assert_eq!(reduce_10(0), 0);
		assert_eq!(reduce_10(2), 0);
		assert_eq!(reduce_10(3), 1);
		assert_eq!(reduce_10(1023), 255);
	}
}
