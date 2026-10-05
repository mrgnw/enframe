//! SIFT as OpenCV 4/5 computes it (`cv::SIFT::create()` defaults, `detectAndCompute` without a mask).
//!
//! The arithmetic follows OpenCV's float operation order, including where clang fuses a multiply and an
//! add, so keypoints and descriptors come out the same. The structure differs: each octave is built,
//! searched and described, then dropped, so only one octave's pyramid is in memory at a time.

use crate::pixels::Image;

const LAYERS: usize = 3;
const SIGMA: f32 = 1.6;
const CONTRAST: f32 = 0.04;
const EDGE: f32 = 10.0;
const BORDER: usize = 5;
const MAX_INTERP_STEPS: usize = 5;
const ORI_BINS: usize = 36;
const ORI_SIG_FCTR: f32 = 1.5;
const ORI_RADIUS: f32 = 3.0 * ORI_SIG_FCTR;
const ORI_PEAK_RATIO: f32 = 0.8;
const DESCR_WIDTH: usize = 4;
const DESCR_BINS: usize = 8;
const DESCR_SCL_FCTR: f32 = 3.0;
const DESCR_MAG_THR: f32 = 0.2;
const INT_DESCR_FCTR: f32 = 512.0;
pub const DESCRIPTOR_LEN: usize = DESCR_WIDTH * DESCR_WIDTH * DESCR_BINS;

#[derive(Clone, Copy, Debug)]
pub struct KeyPoint {
	pub x: f32,
	pub y: f32,
	pub size: f32,
	pub angle: f32,
	pub response: f32,
	pub octave: i32,
}

/// Keypoints sorted as OpenCV returns them, with one 128-byte descriptor each. OpenCV's float
/// descriptors are whole numbers from 0 to 255, so bytes hold them exactly.
#[derive(Default)]
pub struct Features {
	pub keypoints: Vec<KeyPoint>,
	pub descriptors: Vec<[u8; DESCRIPTOR_LEN]>,
}

struct Plane {
	width: usize,
	height: usize,
	data: Vec<f32>,
}

impl Plane {
	#[inline(always)]
	fn at(&self, r: usize, c: usize) -> f32 {
		self.data[r * self.width + c]
	}

	fn row(&self, r: usize) -> &[f32] {
		&self.data[r * self.width..(r + 1) * self.width]
	}
}

pub fn detect_and_compute(gray: &Image) -> Features {
	assert_eq!(gray.channels, 1);
	let sig_diff = (SIGMA * SIGMA - 0.5 * 0.5 * 4.0).max(0.01).sqrt();
	let mut base = blur(&upscale2(gray), sig_diff as f64);
	// One more octave than the doubled image's size gives, because it starts at the doubled scale.
	let octaves =
		((base.width.min(base.height) as f64).ln() / 2f64.ln() - 2.0).round_ties_even() as usize + 1;
	let sigmas = layer_sigmas();
	let mut found: Vec<(KeyPoint, [u8; DESCRIPTOR_LEN])> = Vec::new();
	let mut samples = Samples::default();
	for o in 0..octaves {
		let mut gauss = Vec::with_capacity(LAYERS + 3);
		gauss.push(base);
		for i in 1..LAYERS + 3 {
			gauss.push(blur(&gauss[i - 1], sigmas[i]));
		}
		let dog: Vec<Plane> = gauss.windows(2).map(|w| subtract(&w[1], &w[0])).collect();
		find_extrema(o, &gauss, &dog, &mut found, &mut samples);
		base = if o + 1 < octaves {
			halve_nearest(&gauss[LAYERS])
		} else {
			Plane { width: 0, height: 0, data: vec![] }
		};
	}
	// KeyPointsFilter::removeDuplicatedSorted, then back from the doubled image to the input's scale.
	found.sort_by(|(a, _), (b, _)| keypoint_order(a, b));
	found.dedup_by(|(b, _), (a, _)| a.x == b.x && a.y == b.y && a.size == b.size && a.angle == b.angle);
	let mut features = Features::default();
	for (mut kp, descriptor) in found {
		kp.x *= 0.5;
		kp.y *= 0.5;
		kp.size *= 0.5;
		kp.octave = (kp.octave & !255) | ((kp.octave - 1) & 255);
		features.keypoints.push(kp);
		features.descriptors.push(descriptor);
	}
	features
}

fn keypoint_order(a: &KeyPoint, b: &KeyPoint) -> std::cmp::Ordering {
	let by = |x: f32, y: f32| x.partial_cmp(&y).unwrap();
	by(a.x, b.x)
		.then(by(a.y, b.y))
		.then(by(b.size, a.size))
		.then(by(a.angle, b.angle))
		.then(by(b.response, a.response))
		.then(b.octave.cmp(&a.octave))
}

/// Blur per layer. OpenCV keeps sigma as the double 1.6 here, not the float it uses elsewhere.
fn layer_sigmas() -> [f64; LAYERS + 3] {
	const SIGMA: f64 = 1.6;
	let mut sig = [SIGMA; LAYERS + 3];
	let k = 2f64.powf(1.0 / LAYERS as f64);
	for (i, s) in sig.iter_mut().enumerate().skip(1) {
		let prev = k.powf((i - 1) as f64) * SIGMA;
		let total = prev * k;
		*s = (total * total - prev * prev).sqrt();
	}
	sig
}

/// `resize(gray, 2x, INTER_LINEAR)` on floats: weights of 1/4 and 3/4, exact on 8-bit input.
fn upscale2(gray: &Image) -> Plane {
	let (w, h) = (gray.width, gray.height);
	let taps = |d: usize, n: usize| -> (usize, usize, f32) {
		let s = d as f32 * 0.5 - 0.25;
		if s <= 0.0 {
			(0, 0, 0.0)
		} else if s >= (n - 1) as f32 {
			(n - 1, n - 1, 0.0)
		} else {
			let s0 = s.floor();
			(s0 as usize, s0 as usize + 1, s - s0)
		}
	};
	let mut rows = vec![0f32; 2 * w * h];
	for y in 0..h {
		let src = gray.row(y);
		for dx in 0..2 * w {
			let (a, b, f) = taps(dx, w);
			rows[y * 2 * w + dx] = src[a] as f32 * (1.0 - f) + src[b] as f32 * f;
		}
	}
	let mut data = vec![0f32; 4 * w * h];
	for dy in 0..2 * h {
		let (a, b, f) = taps(dy, h);
		for dx in 0..2 * w {
			data[dy * 2 * w + dx] = rows[a * 2 * w + dx] * (1.0 - f) + rows[b * 2 * w + dx] * f;
		}
	}
	Plane { width: 2 * w, height: 2 * h, data }
}

/// `resize(src, src.size()/2, INTER_NEAREST)`.
fn halve_nearest(src: &Plane) -> Plane {
	let (w, h) = (src.width / 2, src.height / 2);
	let (fx, fy) = (1.0 / (w as f64 / src.width as f64), 1.0 / (h as f64 / src.height as f64));
	let mut data = Vec::with_capacity(w * h);
	for y in 0..h {
		let sy = ((y as f64 * fy).floor() as usize).min(src.height - 1);
		for x in 0..w {
			data.push(src.at(sy, ((x as f64 * fx).floor() as usize).min(src.width - 1)));
		}
	}
	Plane { width: w, height: h, data }
}

fn subtract(a: &Plane, b: &Plane) -> Plane {
	let data = a.data.iter().zip(&b.data).map(|(x, y)| x - y).collect();
	Plane { width: a.width, height: a.height, data }
}

/// `getGaussianKernel(n, sigma, CV_32F)`, with n = round(8 sigma + 1) | 1 as `GaussianBlur` picks it.
fn gaussian_kernel(sigma: f64) -> Vec<f32> {
	let n = ((sigma * 8.0 + 1.0).round_ties_even() as usize) | 1;
	let half = (n - 1) / 2;
	let scale2x = -0.125 / (sigma * sigma);
	let values: Vec<f64> = (0..half)
		.map(|i| {
			let x = 1 - n as i64 + 2 * i as i64;
			((x * x) as f64 * scale2x).exp()
		})
		.collect();
	let sum = values.iter().sum::<f64>() * 2.0 + 1.0;
	let mul = 1.0 / sum;
	let mut kernel = vec![0f32; n];
	for (i, v) in values.iter().enumerate() {
		kernel[i] = (v * mul) as f32;
		kernel[n - 1 - i] = kernel[i];
	}
	kernel[half] = mul as f32;
	kernel
}

/// `borderInterpolate(i, n, BORDER_REFLECT_101)`, reflecting again while a wide kernel overhangs a small octave.
fn reflect(mut i: isize, n: usize) -> usize {
	let n = n as isize;
	if n == 1 {
		return 0;
	}
	while i < 0 || i >= n {
		i = if i < 0 { -i } else { 2 * n - 2 - i };
	}
	i as usize
}

/// `GaussianBlur` on floats: a row pass that accumulates taps left to right with fused multiply-adds,
/// then a symmetric column pass that adds mirrored rows before multiplying, as OpenCV's filters do.
/// The compiler vectorizes both inner loops across the row.
fn blur(src: &Plane, sigma: f64) -> Plane {
	let kernel = gaussian_kernel(sigma);
	let r = kernel.len() / 2;
	let (w, h) = (src.width, src.height);
	let mut rows = vec![0f32; w * h];
	let mut ext = vec![0f32; w + 2 * r];
	for (y, out) in rows.chunks_exact_mut(w).enumerate() {
		let s = src.row(y);
		ext[r..r + w].copy_from_slice(s);
		for i in (0..r).chain(r + w..w + 2 * r) {
			ext[i] = s[reflect(i as isize - r as isize, w)];
		}
		for (o, e) in out.iter_mut().zip(&ext) {
			*o = e * kernel[0];
		}
		for (k, &kk) in kernel.iter().enumerate().skip(1) {
			for (o, e) in out.iter_mut().zip(&ext[k..]) {
				*o = e.mul_add(kk, *o);
			}
		}
	}
	let mut data = vec![0f32; w * h];
	for (y, out) in data.chunks_exact_mut(w).enumerate() {
		for (o, c) in out.iter_mut().zip(&rows[y * w..(y + 1) * w]) {
			*o = c * kernel[r];
		}
		for k in 1..=r {
			let (below, above) = (reflect(y as isize + k as isize, h), reflect(y as isize - k as isize, h));
			let (a, b) = (&rows[below * w..(below + 1) * w], &rows[above * w..(above + 1) * w]);
			let kk = kernel[r + k];
			for ((o, a), b) in out.iter_mut().zip(a).zip(b) {
				*o = (a + b).mul_add(kk, *o);
			}
		}
	}
	Plane { width: w, height: h, data }
}
fn find_extrema(
	o: usize,
	gauss: &[Plane],
	dog: &[Plane],
	found: &mut Vec<(KeyPoint, [u8; DESCRIPTOR_LEN])>,
	samples: &mut Samples,
) {
	let threshold = (0.5 * CONTRAST as f64 / LAYERS as f64 * 255.0).floor() as f32;
	let (w, h) = (dog[0].width, dog[0].height);
	let mut hist = [0f32; ORI_BINS];
	for layer in 1..=LAYERS {
		let (prev, cur, next) = (&dog[layer - 1].data, &dog[layer].data, &dog[layer + 1].data);
		for r in BORDER..h.saturating_sub(BORDER) {
			for c in BORDER..w.saturating_sub(BORDER) {
				let i = r * w + c;
				let val = cur[i];
				if val.abs() <= threshold || !is_extremum(val, i, w, prev, cur, next) {
					continue;
				}
				let Some((kp, layer, r1, c1)) = adjust_local_extrema(dog, o, layer, r, c) else { continue };
				let scl_octv = kp.size * 0.5 / (1 << o) as f32;
				let image = &gauss[layer];
				let radius = (ORI_RADIUS * scl_octv).round_ties_even() as i32;
				let omax =
					orientation_hist(image, r1, c1, radius, ORI_SIG_FCTR * scl_octv, &mut hist, samples);
				let mag_thr = omax * ORI_PEAK_RATIO;
				for j in 0..ORI_BINS {
					let l = if j > 0 { j - 1 } else { ORI_BINS - 1 };
					let r2 = if j < ORI_BINS - 1 { j + 1 } else { 0 };
					if !(hist[j] > hist[l] && hist[j] > hist[r2] && hist[j] >= mag_thr) {
						continue;
					}
					let mut bin = j as f32
						+ 0.5 * (hist[l] - hist[r2]) / ((-2.0f32).mul_add(hist[j], hist[l]) + hist[r2]);
					let n = ORI_BINS as f32;
					bin = if bin < 0.0 {
						n + bin
					} else if bin >= n {
						bin - n
					} else {
						bin
					};
					let mut kp = kp;
					kp.angle = (-(360.0 / n)).mul_add(bin, 360.0);
					if (kp.angle - 360.0).abs() < f32::EPSILON {
						kp.angle = 0.0;
					}
					let mut angle = 360.0 - kp.angle;
					if (angle - 360.0).abs() < f32::EPSILON {
						angle = 0.0;
					}
					let scale = 1.0 / (1 << o) as f32;
					let (px, py) = (kp.x * scale, kp.y * scale);
					found.push((kp, descriptor(image, px, py, angle, kp.size * scale * 0.5, samples)));
				}
			}
		}
	}
}

#[inline(always)]
fn is_extremum(val: f32, i: usize, w: usize, prev: &[f32], cur: &[f32], next: &[f32]) -> bool {
	if val > 0.0 {
		beats(|v| val >= v, i, w, prev, cur, next)
	} else {
		beats(|v| val <= v, i, w, prev, cur, next)
	}
}

/// Whether `wins` holds against all 26 neighbours in scale space. Generic, so each comparison inlines.
#[inline(always)]
fn beats(wins: impl Fn(f32) -> bool, i: usize, w: usize, prev: &[f32], cur: &[f32], next: &[f32]) -> bool {
	let around = |p: &[f32]| {
		wins(p[i - w - 1])
			&& wins(p[i - w])
			&& wins(p[i - w + 1])
			&& wins(p[i - 1])
			&& wins(p[i + 1])
			&& wins(p[i + w - 1])
			&& wins(p[i + w])
			&& wins(p[i + w + 1])
	};
	around(cur) && wins(next[i]) && around(next) && wins(prev[i]) && around(prev)
}

/// Interpolates an extremum to sub-pixel and sub-scale position and rejects weak or edge-like ones.
/// Returns the keypoint in doubled-image coordinates, and the layer, row and column it settled on.
fn adjust_local_extrema(
	dog: &[Plane],
	o: usize,
	mut layer: usize,
	mut r: usize,
	mut c: usize,
) -> Option<(KeyPoint, usize, usize, usize)> {
	let img_scale = 1.0f32 / 255.0;
	let deriv_scale = img_scale * 0.5;
	let second_deriv_scale = img_scale;
	let cross_deriv_scale = img_scale * 0.25;
	let (mut xi, mut xr, mut xc) = (0f32, 0f32, 0f32);
	let mut step = 0;
	while step < MAX_INTERP_STEPS {
		let (img, prev, next) = (&dog[layer], &dog[layer - 1], &dog[layer + 1]);
		let d = [
			(img.at(r, c + 1) - img.at(r, c - 1)) * deriv_scale,
			(img.at(r + 1, c) - img.at(r - 1, c)) * deriv_scale,
			(next.at(r, c) - prev.at(r, c)) * deriv_scale,
		];
		let v2 = img.at(r, c) * 2.0;
		let dxx = (img.at(r, c + 1) + img.at(r, c - 1) - v2) * second_deriv_scale;
		let dyy = (img.at(r + 1, c) + img.at(r - 1, c) - v2) * second_deriv_scale;
		let dss = (next.at(r, c) + prev.at(r, c) - v2) * second_deriv_scale;
		let dxy = (img.at(r + 1, c + 1) - img.at(r + 1, c - 1) - img.at(r - 1, c + 1) + img.at(r - 1, c - 1))
			* cross_deriv_scale;
		let dxs = (next.at(r, c + 1) - next.at(r, c - 1) - prev.at(r, c + 1) + prev.at(r, c - 1))
			* cross_deriv_scale;
		let dys = (next.at(r + 1, c) - next.at(r - 1, c) - prev.at(r + 1, c) + prev.at(r - 1, c))
			* cross_deriv_scale;
		let x = solve3([[dxx, dxy, dxs], [dxy, dyy, dys], [dxs, dys, dss]], d);
		(xi, xr, xc) = (-x[2], -x[1], -x[0]);
		if xi.abs() < 0.5 && xr.abs() < 0.5 && xc.abs() < 0.5 {
			break;
		}
		let limit = (i32::MAX / 3) as f32;
		if xi.abs() > limit || xr.abs() > limit || xc.abs() > limit {
			return None;
		}
		let nc = c as i64 + xc.round_ties_even() as i64;
		let nr = r as i64 + xr.round_ties_even() as i64;
		let nl = layer as i64 + xi.round_ties_even() as i64;
		let (w, h) = (img.width as i64, img.height as i64);
		if nl < 1
			|| nl > LAYERS as i64
			|| nc < BORDER as i64
			|| nc >= w - BORDER as i64
			|| nr < BORDER as i64
			|| nr >= h - BORDER as i64
		{
			return None;
		}
		(c, r, layer) = (nc as usize, nr as usize, nl as usize);
		step += 1;
	}
	if step >= MAX_INTERP_STEPS {
		return None;
	}
	let (img, prev, next) = (&dog[layer], &dog[layer - 1], &dog[layer + 1]);
	let d = [
		(img.at(r, c + 1) - img.at(r, c - 1)) * deriv_scale,
		(img.at(r + 1, c) - img.at(r - 1, c)) * deriv_scale,
		(next.at(r, c) - prev.at(r, c)) * deriv_scale,
	];
	let t = d[2].mul_add(xi, d[1].mul_add(xr, d[0] * xc));
	let contr = img.at(r, c).mul_add(img_scale, t * 0.5);
	if contr.abs() * (LAYERS as f32) < CONTRAST {
		return None;
	}
	let v2 = img.at(r, c) * 2.0;
	let dxx = (img.at(r, c + 1) + img.at(r, c - 1) - v2) * second_deriv_scale;
	let dyy = (img.at(r + 1, c) + img.at(r - 1, c) - v2) * second_deriv_scale;
	let dxy = (img.at(r + 1, c + 1) - img.at(r + 1, c - 1) - img.at(r - 1, c + 1) + img.at(r - 1, c - 1))
		* cross_deriv_scale;
	let tr = dxx + dyy;
	let det = dxx.mul_add(dyy, -(dxy * dxy));
	if det <= 0.0 || tr * tr * EDGE >= (EDGE + 1.0) * (EDGE + 1.0) * det {
		return None;
	}
	let octave_scale = (1 << o) as f32;
	let kp = KeyPoint {
		x: (c as f32 + xc) * octave_scale,
		y: (r as f32 + xr) * octave_scale,
		size: SIGMA * 2f32.powf((layer as f32 + xi) / LAYERS as f32) * octave_scale * 2.0,
		angle: 0.0,
		response: contr.abs(),
		octave: o as i32
			+ ((layer as i32) << 8)
			+ ((((xi as f64 + 0.5) * 255.0).round_ties_even() as i32) << 16),
	};
	Some((kp, layer, r, c))
}

/// `Matx33f::solve(b, DECOMP_LU)`: Cramer's rule, as OpenCV's 3x3 fast path writes it.
fn solve3(a: [[f32; 3]; 3], b: [f32; 3]) -> [f32; 3] {
	let det = a[0][2].mul_add(
		a[1][0].mul_add(a[2][1], -(a[2][0] * a[1][1])),
		a[0][0].mul_add(
			a[1][1].mul_add(a[2][2], -(a[2][1] * a[1][2])),
			-(a[0][1] * a[1][0].mul_add(a[2][2], -(a[2][0] * a[1][2]))),
		),
	);
	if det == 0.0 {
		return [0.0; 3];
	}
	let d = 1.0 / det;
	let x0 = a[0][2].mul_add(
		b[1].mul_add(a[2][1], -(a[1][1] * b[2])),
		b[0].mul_add(
			a[1][1].mul_add(a[2][2], -(a[1][2] * a[2][1])),
			-(a[0][1] * b[1].mul_add(a[2][2], -(a[1][2] * b[2]))),
		),
	);
	let x1 = a[0][2].mul_add(
		a[1][0].mul_add(b[2], -(b[1] * a[2][0])),
		a[0][0].mul_add(
			b[1].mul_add(a[2][2], -(a[1][2] * b[2])),
			-(b[0] * a[1][0].mul_add(a[2][2], -(a[1][2] * a[2][0]))),
		),
	);
	let x2 = b[0].mul_add(
		a[1][0].mul_add(a[2][1], -(a[1][1] * a[2][0])),
		a[0][0].mul_add(
			a[1][1].mul_add(b[2], -(b[1] * a[2][1])),
			-(a[0][1] * a[1][0].mul_add(b[2], -(b[1] * a[2][0]))),
		),
	);
	[d * x0, d * x1, d * x2]
}

/// OpenCV's `hal::exp32f`: a table of 2^(j/64) and a degree-4 polynomial, not libm's exp. The table
/// lookup runs in its own loop so the arithmetic loop vectorizes; multiplying by the power of two first
/// is exact, so the product is the same as OpenCV's for any weight above 1e-37.
fn exp32f(values: &mut [f32], index: &mut Vec<u8>) {
	const A0: f64 = 0.9670371139572337719125840413672004409288e-2;
	const A1: f32 = (0.5550339366753125211915322047004666939128e-1 / A0) as f32;
	const A2: f32 = (0.2402265109513301490103372422686535526573 / A0) as f32;
	const A3: f32 = (0.6931471805521448196800669615864773144641 / A0) as f32;
	const A4: f32 = (1.000000000000002438532970795181890933776 / A0) as f32;
	const PRESCALE: f64 = 1.4426950408889634073599246810019 * 64.0;
	const MAX_VAL: f64 = 3000.0 * 64.0;
	static TABLE: std::sync::LazyLock<[f32; 64]> =
		std::sync::LazyLock::new(|| std::array::from_fn(|j| (2f64.powf(j as f64 / 64.0) * A0) as f32));
	let table = &*TABLE;
	let (minval, maxval) = ((-MAX_VAL / PRESCALE) as f32, (MAX_VAL / PRESCALE) as f32);
	index.clear();
	index.resize(values.len(), 0);
	for (v, ix) in values.iter_mut().zip(index.iter_mut()) {
		let x = v.max(minval).min(maxval) * PRESCALE as f32;
		let xi = x.round_ties_even() as i32;
		let x = (x - xi as f32) * (1.0 / 64.0);
		let power = f32::from_bits((((xi >> 6) + 127).clamp(0, 255) as u32) << 23);
		*v = (x + A1).mul_add(x, A2).mul_add(x, A3).mul_add(x, A4) * power;
		*ix = (xi & 63) as u8;
	}
	for (v, &ix) in values.iter_mut().zip(index.iter()) {
		*v *= table[ix as usize];
	}
}

/// OpenCV's `fastAtan2` in degrees: a polynomial good to about 0.3 degrees. Written with selects, not
/// branches, so loops over it vectorize.
#[inline(always)]
fn fast_atan2(y: f32, x: f32) -> f32 {
	const DEG: f32 = (180.0 / std::f64::consts::PI) as f32;
	const P1: f32 = 0.9997878412794807 * DEG;
	const P3: f32 = -0.3258083974640975 * DEG;
	const P5: f32 = 0.1555786518463281 * DEG;
	const P7: f32 = -0.04432655554792128 * DEG;
	let (ax, ay) = (x.abs(), y.abs());
	let c = ax.min(ay) / (ax.max(ay) + f64::EPSILON as f32);
	let cc = c * c;
	let a = cc.mul_add(P7, P5).mul_add(cc, P3).mul_add(cc, P1) * c;
	let a = if ax >= ay { a } else { 90.0 - a };
	let a = if x < 0.0 { 180.0 - a } else { a };
	if y < 0.0 { 360.0 - a } else { a }
}

/// Gradient samples around a keypoint: x and y differences, the squared-distance weight and histogram
/// coordinates, then angles, magnitudes and weights. One per photo, reused from keypoint to keypoint.
#[derive(Default)]
struct Samples {
	dx: Vec<f32>,
	dy: Vec<f32>,
	weight: Vec<f32>,
	rbin: Vec<f32>,
	cbin: Vec<f32>,
	angle: Vec<f32>,
	magnitude: Vec<f32>,
	table_index: Vec<u8>,
}

impl Samples {
	fn clear(&mut self) {
		for v in [&mut self.dx, &mut self.dy, &mut self.weight, &mut self.rbin, &mut self.cbin] {
			v.clear();
		}
	}

	#[inline(always)]
	fn push(&mut self, img: &Plane, r: usize, c: usize, weight: f32) {
		let (above, here, below) = (img.row(r - 1), img.row(r), img.row(r + 1));
		self.dx.push(here[c + 1] - here[c - 1]);
		self.dy.push(above[c] - below[c]);
		self.weight.push(weight);
	}

	/// `exp32f` on the weights in place, then `fastAtan2` and `magnitude` of the differences.
	fn finish(&mut self) {
		exp32f(&mut self.weight, &mut self.table_index);
		self.angle.clear();
		self.angle.extend(self.dy.iter().zip(&self.dx).map(|(&y, &x)| fast_atan2(y, x)));
		self.magnitude.clear();
		self.magnitude.extend(self.dx.iter().zip(&self.dy).map(|(&x, &y)| x.mul_add(x, y * y).sqrt()));
	}
}

fn orientation_hist(
	img: &Plane,
	r: usize,
	c: usize,
	radius: i32,
	sigma: f32,
	hist: &mut [f32; ORI_BINS],
	samples: &mut Samples,
) -> f32 {
	let expf_scale = -1.0 / (2.0 * sigma * sigma);
	samples.clear();
	for i in -radius..=radius {
		let y = r as i32 + i;
		if y <= 0 || y >= img.height as i32 - 1 {
			continue;
		}
		for j in -radius..=radius {
			let x = c as i32 + j;
			if x <= 0 || x >= img.width as i32 - 1 {
				continue;
			}
			samples.push(img, y as usize, x as usize, (i * i + j * j) as f32 * expf_scale);
		}
	}
	samples.finish();
	let n = ORI_BINS as i32;
	let mut temp = [0f32; ORI_BINS + 4];
	// OpenCV's vector loop multiplies, then adds; its scalar tail (the last len % 4) fuses the two.
	let vector_len = samples.angle.len() / 4 * 4;
	let weighted = samples.angle.iter().zip(samples.weight.iter().zip(&samples.magnitude));
	for (k, (&angle, (&w, &mag))) in weighted.enumerate() {
		let mut bin = (angle * (n as f32 / 360.0)).round_ties_even() as i32;
		if bin >= n {
			bin -= n;
		}
		if bin < 0 {
			bin += n;
		}
		let h = &mut temp[bin as usize + 2];
		*h = if k < vector_len { *h + w * mag } else { w.mul_add(mag, *h) };
	}
	temp[1] = temp[ORI_BINS + 1];
	temp[0] = temp[ORI_BINS];
	temp[ORI_BINS + 2] = temp[2];
	temp[ORI_BINS + 3] = temp[3];
	let mut maxval = f32::MIN;
	for i in 0..ORI_BINS {
		let t = &temp[i..i + 5];
		hist[i] = (t[0] + t[4]).mul_add(1.0 / 16.0, (t[1] + t[3]).mul_add(4.0 / 16.0, t[2] * (6.0 / 16.0)));
		maxval = maxval.max(hist[i]);
	}
	maxval
}

/// The columns of row i (relative to the keypoint) that can land in the rotated 4x4 grid, where both
/// rotated coordinates lie within 2.5 histogram widths of the centre. OpenCV tests every pixel of the
/// circle; testing only this span, widened by a pixel each side, keeps exactly the same samples.
fn grid_columns(i: i32, cos_t: f32, sin_t: f32, radius: i32) -> (i32, i32) {
	let half = (DESCR_WIDTH as f64 + 1.0) * 0.5;
	let (mut lo, mut hi) = (-radius as f64, radius as f64);
	let i = i as f64;
	for (slope, offset) in [(cos_t as f64, -i * sin_t as f64), (sin_t as f64, i * cos_t as f64)] {
		if slope.abs() < 1e-3 {
			if offset.abs() > half + 0.01 {
				return (1, 0);
			}
			continue;
		}
		let (a, b) = ((-half - offset) / slope, (half - offset) / slope);
		(lo, hi) = (lo.max(a.min(b)), hi.min(a.max(b)));
	}
	if lo > hi {
		return (1, 0);
	}
	((lo.floor() as i32 - 1).max(-radius), (hi.ceil() as i32 + 1).min(radius))
}

fn descriptor(
	img: &Plane,
	x: f32,
	y: f32,
	ori: f32,
	scl: f32,
	samples: &mut Samples,
) -> [u8; DESCRIPTOR_LEN] {
	const D: usize = DESCR_WIDTH;
	const N: usize = DESCR_BINS;
	let (pr, pc) = (y.round_ties_even() as i32, x.round_ties_even() as i32);
	let rad = ori * (std::f64::consts::PI / 180.0) as f32;
	let bins_per_rad = N as f32 / 360.0;
	let exp_scale = -1.0 / (D as f32 * D as f32 * 0.5);
	let hist_width = DESCR_SCL_FCTR * scl;
	let diagonal = ((img.width as f64).powi(2) + (img.height as f64).powi(2)).sqrt() as i32;
	let radius = ((hist_width * std::f32::consts::SQRT_2 * (D + 1) as f32 * 0.5).round_ties_even() as i32)
		.min(diagonal);
	let cos_t = rad.cos() / hist_width;
	let sin_t = rad.sin() / hist_width;
	samples.clear();
	for i in -radius..=radius {
		let (lo, hi) = grid_columns(i, cos_t, sin_t, radius);
		for j in lo..=hi {
			let (jf, i_f) = (j as f32, i as f32);
			let c_rot = jf.mul_add(cos_t, -(i_f * sin_t));
			let r_rot = jf.mul_add(sin_t, i_f * cos_t);
			let rbin = r_rot + (D / 2) as f32 - 0.5;
			let cbin = c_rot + (D / 2) as f32 - 0.5;
			let (r, c) = (pr + i, pc + j);
			if rbin > -1.0
				&& rbin < D as f32
				&& cbin > -1.0
				&& cbin < D as f32
				&& r > 0 && r < img.height as i32 - 1
				&& c > 0 && c < img.width as i32 - 1
			{
				samples.push(img, r as usize, c as usize, c_rot.mul_add(c_rot, r_rot * r_rot) * exp_scale);
				samples.rbin.push(rbin);
				samples.cbin.push(cbin);
			}
		}
	}
	samples.finish();
	let mut hist = [0f32; (D + 2) * (D + 2) * (N + 2)];
	let s = &*samples;
	for k in 0..s.angle.len() {
		let (rbin, cbin) = (s.rbin[k], s.cbin[k]);
		let obin = (s.angle[k] - ori) * bins_per_rad;
		let mag = s.magnitude[k] * s.weight[k];
		let (r0, c0, o0) = (rbin.floor(), cbin.floor(), obin.floor());
		let (rbin, cbin, obin) = (rbin - r0, cbin - c0, obin - o0);
		let mut o0 = o0 as i32;
		if o0 < 0 {
			o0 += N as i32;
		}
		if o0 >= N as i32 {
			o0 -= N as i32;
		}
		let v_r1 = mag * rbin;
		let v_r0 = mag - v_r1;
		let v_rc11 = v_r1 * cbin;
		let v_rc10 = v_r1 - v_rc11;
		let v_rc01 = v_r0 * cbin;
		let v_rc00 = v_r0 - v_rc01;
		let v_rco111 = v_rc11 * obin;
		let v_rco110 = v_rc11 - v_rco111;
		let v_rco101 = v_rc10 * obin;
		let v_rco100 = v_rc10 - v_rco101;
		let v_rco011 = v_rc01 * obin;
		let v_rco010 = v_rc01 - v_rco011;
		let v_rco001 = v_rc00 * obin;
		let v_rco000 = v_rc00 - v_rco001;
		let idx = (((r0 as i32 + 1) * (D as i32 + 2) + c0 as i32 + 1) * (N as i32 + 2) + o0) as usize;
		hist[idx] += v_rco000;
		hist[idx + 1] += v_rco001;
		hist[idx + (N + 2)] += v_rco010;
		hist[idx + (N + 3)] += v_rco011;
		hist[idx + (D + 2) * (N + 2)] += v_rco100;
		hist[idx + (D + 2) * (N + 2) + 1] += v_rco101;
		hist[idx + (D + 3) * (N + 2)] += v_rco110;
		hist[idx + (D + 3) * (N + 2) + 1] += v_rco111;
	}
	let mut raw = [0f32; DESCRIPTOR_LEN];
	for i in 0..D {
		for j in 0..D {
			let idx = ((i + 1) * (D + 2) + (j + 1)) * (N + 2);
			hist[idx] += hist[idx + N];
			hist[idx + 1] += hist[idx + N + 1];
			raw[(i * D + j) * N..(i * D + j + 1) * N].copy_from_slice(&hist[idx..idx + N]);
		}
	}
	// Four-lane sum of squares reduced pairwise, as OpenCV's NEON path does.
	let mut lanes = [0f32; 4];
	for chunk in raw.chunks_exact(4) {
		for (l, v) in lanes.iter_mut().zip(chunk) {
			*l = v.mul_add(*v, *l);
		}
	}
	let nrm2 = (lanes[0] + lanes[1]) + (lanes[2] + lanes[3]);
	let thr = nrm2.sqrt() * DESCR_MAG_THR;
	let mut nrm2 = 0f32;
	for v in raw.iter_mut() {
		*v = v.min(thr);
		nrm2 = v.mul_add(*v, nrm2);
	}
	let nrm2 = INT_DESCR_FCTR / nrm2.sqrt().max(f32::EPSILON);
	raw.map(|v| crate::pixels::saturate_u8(v * nrm2))
}
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn kernels_match_opencv() {
		let Ok(text) = std::fs::read_to_string("../out/kernels.jsonl") else {
			eprintln!("skipped: run uv run analysis/dump_kernels.py > out/kernels.jsonl");
			return;
		};
		for line in text.lines() {
			let row: serde_json::Value = serde_json::from_str(line).unwrap();
			let kernel = gaussian_kernel(row[0].as_f64().unwrap());
			let opencv: Vec<f32> =
				row[2].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
			assert_eq!(kernel, opencv, "sigma {}", row[0]);
		}
	}

	#[test]
	fn fast_atan2_quadrants() {
		for (y, x, want) in
			[(0.0, 1.0, 0.0), (1.0, 0.0, 90.0), (0.0, -1.0, 180.0), (-1.0, 0.0, 270.0), (1.0, 1.0, 45.0)]
		{
			assert!((fast_atan2(y, x) - want).abs() < 0.01, "{y} {x}");
		}
	}

	#[test]
	fn exp32f_is_close_to_exp() {
		let mut v: Vec<f32> = (0..100).map(|i| -i as f32 * 0.1).collect();
		let want: Vec<f32> = v.iter().map(|x| x.exp()).collect();
		exp32f(&mut v, &mut Vec::new());
		for (a, b) in v.iter().zip(&want) {
			assert!((a - b).abs() <= 1e-6 * b.max(1e-30) + 1e-12, "{a} {b}");
		}
	}
}

/// The first octave's Gaussian layers, for comparison with OpenCV's.
#[cfg(test)]
pub fn first_octave(gray: &Image) -> Vec<Vec<f32>> {
	let sig_diff = (SIGMA * SIGMA - 0.5 * 0.5 * 4.0).max(0.01).sqrt();
	let mut layers = vec![blur(&upscale2(gray), sig_diff as f64)];
	for s in &layer_sigmas()[1..] {
		let next = blur(layers.last().unwrap(), *s);
		layers.push(next);
	}
	layers.into_iter().map(|p| p.data).collect()
}
#[cfg(test)]
#[test]
#[ignore]
fn blur_bench() {
	let data: Vec<f32> = (0..2048 * 1536).map(|i| (i % 251) as f32).collect();
	let plane = Plane { width: 2048, height: 1536, data };
	for sigma in [1.2262735, 1.9465878, 3.0900156] {
		let mut best = std::time::Duration::MAX;
		for _ in 0..10 {
			let t = std::time::Instant::now();
			std::hint::black_box(blur(&plane, sigma));
			best = best.min(t.elapsed());
		}
		let taps = gaussian_kernel(sigma).len();
		let fmas = 2048.0 * 1536.0 * (taps + taps / 2 + 1) as f64;
		println!("blur_bench: {taps} taps {best:?}, {:.1} GFMA/s", fmas / best.as_secs_f64() / 1e9);
	}
}
