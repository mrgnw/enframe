//! RGB(A) clip frames to 8-bit YUV 4:2:0, for the AVIF and MP4 encoders.

use crate::pixels::Image;

/// A clip frame as YUV 4:2:0, with alpha when the padding is transparent.
pub struct Yuv {
	pub width: usize,
	pub height: usize,
	pub planes: [Vec<u8>; 3],
	pub alpha: Option<Vec<u8>>,
}

/// The luma weights of red and blue (H.273 KR and KB).
#[derive(Clone, Copy)]
pub struct Matrix {
	kr: f32,
	kb: f32,
}

/// H.273 matrix 5/6: what AVIF clips use.
pub const BT601: Matrix = Matrix { kr: 0.299, kb: 0.114 };
/// H.273 matrix 1: what MP4 clips use.
pub const BT709: Matrix = Matrix { kr: 0.2126, kb: 0.0722 };

/// Code values are `offset + scale * v`, for luma and for chroma shifted to 0 to 1.
#[derive(Clone, Copy)]
pub struct Range {
	offset: f32,
	y_scale: f32,
	c_scale: f32,
}

/// Full range, as libavif computes it.
pub const FULL: Range = Range { offset: 0.0, y_scale: 255.0, c_scale: 255.0 };
/// Limited ("tv") range, H.273 equations 30 to 32: Y 16 to 235, Cb and Cr 16 to 240.
pub const LIMITED: Range = Range { offset: 16.0, y_scale: 219.0, c_scale: 224.0 };

/// RGB(A) to YUV 4:2:0, chroma averaged over each 2x2 block.
/// Done here on the render threads; libavif without libyuv takes 8 ms a frame on the encoding thread.
pub fn to_yuv420(frame: &Image, matrix: Matrix, range: Range) -> Yuv {
	let Matrix { kr, kb } = matrix;
	let kg = 1.0 - kr - kb;
	let (w, h, cn) = (frame.width, frame.height, frame.channels);
	assert!(w % 2 == 0 && h % 2 == 0 && (cn == 3 || cn == 4), "clip frames are even-sized RGB(A)");
	let (cw, ch) = (w / 2, h / 2);
	let mut y_plane = vec![0u8; w * h];
	let (mut u_plane, mut v_plane) = (vec![0u8; cw * ch], vec![0u8; cw * ch]);
	let mut alpha = (cn == 4).then(|| vec![0u8; w * h]);
	let code = |scale: f32, v: f32| (range.offset + scale * v).round().clamp(0.0, 255.0) as u8;
	for cy in 0..ch {
		let mut u_sum = vec![0f32; cw];
		let mut v_sum = vec![0f32; cw];
		for y in [2 * cy, 2 * cy + 1] {
			let row = frame.row(y);
			for x in 0..w {
				let p = &row[x * cn..x * cn + 3];
				let (r, g, b) = (p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0);
				let luma = kr * r + kg * g + kb * b;
				y_plane[y * w + x] = code(range.y_scale, luma);
				u_sum[x / 2] += (b - luma) / (2.0 * (1.0 - kb));
				v_sum[x / 2] += (r - luma) / (2.0 * (1.0 - kr));
				if let Some(a) = alpha.as_mut() {
					a[y * w + x] = row[x * cn + 3];
				}
			}
		}
		for x in 0..cw {
			u_plane[cy * cw + x] = code(range.c_scale, u_sum[x] / 4.0 + 0.5);
			v_plane[cy * cw + x] = code(range.c_scale, v_sum[x] / 4.0 + 0.5);
		}
	}
	Yuv { width: w, height: h, planes: [y_plane, u_plane, v_plane], alpha }
}

#[cfg(test)]
mod tests {
	use super::*;

	fn solid(rgb: [u8; 3]) -> Image {
		Image { width: 2, height: 2, channels: 3, pixels: rgb.repeat(4) }
	}

	fn first(frame: &Image, matrix: Matrix, range: Range) -> [u8; 3] {
		let yuv = to_yuv420(frame, matrix, range);
		[yuv.planes[0][0], yuv.planes[1][0], yuv.planes[2][0]]
	}

	#[test]
	fn bt709_limited_code_values() {
		for (rgb, expected) in [
			([255, 255, 255], [235, 128, 128]),
			([0, 0, 0], [16, 128, 128]),
			([255, 0, 0], [63, 102, 240]),
			([0, 255, 0], [173, 42, 26]),
			([0, 0, 255], [32, 240, 118]),
		] {
			assert_eq!(first(&solid(rgb), BT709, LIMITED), expected, "{rgb:?}");
		}
	}

	#[test]
	fn bt601_full_code_values() {
		for (rgb, expected) in
			[([255, 255, 255], [255, 128, 128]), ([0, 0, 0], [0, 128, 128]), ([255, 0, 0], [76, 84, 255])]
		{
			assert_eq!(first(&solid(rgb), BT601, FULL), expected, "{rgb:?}");
		}
	}
}
