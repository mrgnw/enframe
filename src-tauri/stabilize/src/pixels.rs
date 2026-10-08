//! 8-bit images and the conversions tracking needs, bit-exact with the OpenCV calls in `anchor.py`.

pub struct Image {
	pub width: usize,
	pub height: usize,
	pub channels: usize,
	pub pixels: Vec<u8>,
}

impl Image {
	pub fn row(&self, y: usize) -> &[u8] {
		let stride = self.width * self.channels;
		&self.pixels[y * stride..(y + 1) * stride]
	}
}

/// Turns a stored image upright for an EXIF-style orientation (1-8), as `ImageOps.exif_transpose` does.
pub fn orient(image: Image, orientation: u8) -> Image {
	if orientation <= 1 || orientation > 8 {
		return image;
	}
	let (w, h, cn) = (image.width, image.height, image.channels);
	let turned = orientation >= 5;
	let (ow, oh) = if turned { (h, w) } else { (w, h) };
	let mut out = vec![0u8; image.pixels.len()];
	for y in 0..oh {
		for x in 0..ow {
			let (sx, sy) = match orientation {
				2 => (w - 1 - x, y),
				3 => (w - 1 - x, h - 1 - y),
				4 => (x, h - 1 - y),
				5 => (y, x),
				6 => (y, h - 1 - x),
				7 => (w - 1 - y, h - 1 - x),
				_ => (w - 1 - y, x),
			};
			let (s, d) = ((sy * w + sx) * cn, (y * ow + x) * cn);
			out[d..d + cn].copy_from_slice(&image.pixels[s..s + cn]);
		}
	}
	Image { width: ow, height: oh, channels: cn, pixels: out }
}

/// `cv2.cvtColor(rgb, COLOR_RGB2GRAY)`: 15-bit fixed-point weights, rounded.
pub fn gray(rgb: &Image) -> Image {
	assert_eq!(rgb.channels, 3);
	let pixels = rgb
		.pixels
		.chunks_exact(3)
		.map(|p| ((p[0] as u32 * 9798 + p[1] as u32 * 19235 + p[2] as u32 * 3735 + (1 << 14)) >> 15) as u8)
		.collect();
	Image { width: rgb.width, height: rgb.height, channels: 1, pixels }
}

/// `cv2.resize(gray, None, fx=factor, fy=factor, interpolation=INTER_AREA)` for factor <= 1.
pub fn shrink_area(src: &Image, factor: f64) -> Image {
	assert!(src.channels == 1 && factor > 0.0 && factor <= 1.0);
	let dw = (src.width as f64 * factor).round() as usize;
	let dh = (src.height as f64 * factor).round() as usize;
	let scale = 1.0 / factor;
	if scale == 2.0 && dw * 2 == src.width && dh * 2 == src.height {
		return halve(src);
	}
	// ponytail: OpenCV's general INTER_AREA (float weights), untested against OpenCV on non-2x sizes
	let xtab = area_tab(src.width, dw, scale);
	let ytab = area_tab(src.height, dh, scale);
	let mut out = vec![0u8; dw * dh];
	let mut buf = vec![0f32; dw];
	let mut sum = vec![0f32; dw];
	let mut row_of_sum = ytab[0].0;
	for &(dy, sy, beta) in &ytab {
		buf.fill(0.0);
		let s = src.row(sy);
		for &(dx, sx, alpha) in &xtab {
			buf[dx] = (s[sx] as f32).mul_add(alpha, buf[dx]);
		}
		if dy != row_of_sum {
			for dx in 0..dw {
				out[row_of_sum * dw + dx] = saturate_u8(sum[dx]);
				sum[dx] = beta * buf[dx];
			}
			row_of_sum = dy;
		} else {
			for dx in 0..dw {
				sum[dx] = beta.mul_add(buf[dx], sum[dx]);
			}
		}
	}
	for dx in 0..dw {
		out[row_of_sum * dw + dx] = saturate_u8(sum[dx]);
	}
	Image { width: dw, height: dh, channels: 1, pixels: out }
}

/// INTER_AREA at exactly half size: the rounded mean of each 2x2 block.
fn halve(src: &Image) -> Image {
	let (dw, dh) = (src.width / 2, src.height / 2);
	let mut out = vec![0u8; dw * dh];
	for (y, row) in out.chunks_exact_mut(dw).enumerate() {
		let (a, b) = (src.row(2 * y), src.row(2 * y + 1));
		for (x, d) in row.iter_mut().enumerate() {
			let s = a[2 * x] as u16 + a[2 * x + 1] as u16 + b[2 * x] as u16 + b[2 * x + 1] as u16;
			*d = ((s + 2) >> 2) as u8;
		}
	}
	Image { width: dw, height: dh, channels: 1, pixels: out }
}

/// OpenCV's `computeResizeAreaTab`: (destination, source, weight) for each overlap.
fn area_tab(ssize: usize, dsize: usize, scale: f64) -> Vec<(usize, usize, f32)> {
	let mut tab = Vec::with_capacity(ssize * 2);
	for dx in 0..dsize {
		let fsx1 = dx as f64 * scale;
		let fsx2 = fsx1 + scale;
		let cell = scale.min(ssize as f64 - fsx1);
		let sx2 = (fsx2.floor() as usize).min(ssize - 1);
		let sx1 = (fsx1.ceil() as usize).min(sx2);
		if sx1 as f64 - fsx1 > 1e-3 {
			tab.push((dx, sx1 - 1, ((sx1 as f64 - fsx1) / cell) as f32));
		}
		for sx in sx1..sx2 {
			tab.push((dx, sx, (1.0 / cell) as f32));
		}
		if fsx2 - sx2 as f64 > 1e-3 {
			tab.push((dx, sx2, ((fsx2 - sx2 as f64).min(1.0).min(cell) / cell) as f32));
		}
	}
	tab
}

pub fn saturate_u8(v: f32) -> u8 {
	v.round_ties_even().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn orientations_match_exif_meaning() {
		// 2x1 image: left pixel 1, right pixel 2.
		let image = || Image { width: 2, height: 1, channels: 1, pixels: vec![1, 2] };
		assert_eq!(orient(image(), 2).pixels, [2, 1]);
		assert_eq!(orient(image(), 3).pixels, [2, 1]);
		assert_eq!(orient(image(), 4).pixels, [1, 2]);
		// Rotated 90 clockwise (6), the left pixel ends on top; anticlockwise (8), at the bottom.
		let cw = orient(image(), 6);
		assert_eq!((cw.width, cw.height, cw.pixels), (1, 2, vec![1, 2]));
		assert_eq!(orient(image(), 8).pixels, [2, 1]);
		assert_eq!(orient(image(), 5).pixels, [1, 2]);
		assert_eq!(orient(image(), 7).pixels, [2, 1]);
	}

	#[test]
	fn area_tab_at_half_is_pairs() {
		let tab = area_tab(4, 2, 2.0);
		assert_eq!(tab, vec![(0, 0, 0.5), (0, 1, 0.5), (1, 2, 0.5), (1, 3, 0.5)]);
	}
}
