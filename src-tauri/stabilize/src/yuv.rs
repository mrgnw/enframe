//! RGB(A) clip frames to 8-bit YUV 4:2:0 for the MP4 encoder: BT.709 matrix, limited range.

use crate::pixels::Image;

/// A clip frame as YUV 4:2:0.
pub struct Yuv {
	pub width: usize,
	pub planes: [Vec<u8>; 3],
}

/// The luma weights of red and blue (H.273 matrix 1, KR and KB).
const KR: f32 = 0.2126;
const KB: f32 = 0.0722;
/// Limited ("tv") range, H.273 equations 30 to 32: Y 16 to 235, Cb and Cr 16 to 240.
const OFFSET: f32 = 16.0;
const Y_SCALE: f32 = 219.0;
const C_SCALE: f32 = 224.0;

/// RGB(A) to YUV 4:2:0, chroma averaged over each 2x2 block; alpha is dropped.
/// Done here on the render threads, not on the encoding thread.
pub fn to_yuv420(frame: &Image) -> Yuv {
	let kg = 1.0 - KR - KB;
	let (w, h, cn) = (frame.width, frame.height, frame.channels);
	assert!(w % 2 == 0 && h % 2 == 0 && (cn == 3 || cn == 4), "clip frames are even-sized RGB(A)");
	let (cw, ch) = (w / 2, h / 2);
	let mut y_plane = vec![0u8; w * h];
	let (mut u_plane, mut v_plane) = (vec![0u8; cw * ch], vec![0u8; cw * ch]);
	let code = |scale: f32, v: f32| (OFFSET + scale * v).round().clamp(0.0, 255.0) as u8;
	for cy in 0..ch {
		let mut u_sum = vec![0f32; cw];
		let mut v_sum = vec![0f32; cw];
		for y in [2 * cy, 2 * cy + 1] {
			let row = frame.row(y);
			for x in 0..w {
				let p = &row[x * cn..x * cn + 3];
				let (r, g, b) = (p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0);
				let luma = KR * r + kg * g + KB * b;
				y_plane[y * w + x] = code(Y_SCALE, luma);
				u_sum[x / 2] += (b - luma) / (2.0 * (1.0 - KB));
				v_sum[x / 2] += (r - luma) / (2.0 * (1.0 - KR));
			}
		}
		for x in 0..cw {
			u_plane[cy * cw + x] = code(C_SCALE, u_sum[x] / 4.0 + 0.5);
			v_plane[cy * cw + x] = code(C_SCALE, v_sum[x] / 4.0 + 0.5);
		}
	}
	Yuv { width: w, planes: [y_plane, u_plane, v_plane] }
}

#[cfg(test)]
mod tests {
	use super::*;

	fn solid(rgb: [u8; 3]) -> Image {
		Image { width: 2, height: 2, channels: 3, pixels: rgb.repeat(4) }
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
			let yuv = to_yuv420(&solid(rgb));
			assert_eq!([yuv.planes[0][0], yuv.planes[1][0], yuv.planes[2][0]], expected, "{rgb:?}");
		}
	}
}
