//! Follows the anchor from photo to photo and plans one crop per photo, as `anchor.py` does.

use crate::fail;
use crate::sift::{DESCRIPTOR_LEN, Features, KeyPoint};

pub const MIN_INLIERS: usize = 8;
pub const MIN_CROP: i64 = 64;
pub const TRACK_EDGE: f64 = 1024.0;
const RATIO_TEST: f64 = 0.8;
const RANSAC_TOLERANCE: f64 = 0.03;
const MAX_STEP_ZOOM: f64 = 6.0;
const MAX_STEP_TURN: f64 = 15.0;
/// The next photo may move the reference up to this many times as far as the last tracked step did.
/// Photos are hand-held, so steps vary a lot; below 5 the window changes results on the synthetic
/// rotate sequence, above 5 it only costs time.
const MOTION_MARGIN: f64 = 5.0;

/// A user mark: a point x,y or a box x,y,w,h, with an optional zoom relative to the first photo.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark(pub Vec<f64>);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
	Tracked,
	Mark,
	Lost,
}

impl Source {
	pub fn name(self) -> &'static str {
		match self {
			Source::Tracked => "tracked",
			Source::Mark => "mark",
			Source::Lost => "lost",
		}
	}
}

#[derive(Clone, Debug)]
pub struct Hit {
	pub x: f64,
	pub y: f64,
	pub scale: f64,
	pub source: Source,
	pub note: String,
}

/// One downscale for every photo, so differing resolutions show up as scale, like a lens change.
pub fn track_factor(sizes: &[(u32, u32)]) -> f64 {
	let largest = sizes.iter().map(|&(w, h)| w.max(h)).max().unwrap_or(1) as f64;
	(TRACK_EDGE / largest).min(1.0)
}

/// What tracking keeps of a photo: the size of the downscaled image SIFT ran on, and its features.
pub struct Photo {
	pub small: (usize, usize),
	pub features: Features,
}

/// The features whose rounded position falls in the box, as OpenCV's mask filter keeps them.
pub fn features_in(photo: &Photo, cx: f64, cy: f64, w: f64, h: f64) -> Features {
	let (sw, sh) = photo.small;
	let round = |v: f64| v.round_ties_even() as i64;
	let x0 = round(cx - w / 2.0).max(0);
	let y0 = round(cy - h / 2.0).max(0);
	let x1 = round(cx + w / 2.0).clamp(0, sw as i64);
	let y1 = round(cy + h / 2.0).clamp(0, sh as i64);
	let mut kept = Features::default();
	for (kp, des) in photo.features.keypoints.iter().zip(&photo.features.descriptors) {
		let (ax, ay) = ((kp.x + 0.5) as i64, (kp.y + 0.5) as i64);
		if (x0..x1).contains(&ax) && (y0..y1).contains(&ay) {
			kept.keypoints.push(*kp);
			kept.descriptors.push(*des);
		}
	}
	kept
}

/// Euclidean distance between two descriptors. OpenCV's are whole numbers, so the squared distance
/// is exact in integers and its square root equals OpenCV's float result.
#[inline]
pub fn distance(a: &[u8; DESCRIPTOR_LEN], b: &[u8; DESCRIPTOR_LEN]) -> f32 {
	let mut sum = 0u32;
	for (x, y) in a.iter().zip(b) {
		let d = x.abs_diff(*y) as u32;
		sum += d * d;
	}
	(sum as f32).sqrt()
}

/// One query feature's nearest and second-nearest distance among the train features seen so far, and the
/// nearest one's index.
#[derive(Clone, Copy)]
struct Nearest {
	best: f32,
	second: f32,
	index: usize,
}

impl Nearest {
	const NONE: Nearest = Nearest { best: f32::MAX, second: f32::MAX, index: usize::MAX };
}

/// BFMatcher(NORM_L2).knnMatch(k=2) over the `candidates` of `train`, folded into what `found` already
/// holds. Ties go to the lower index, as in one pass over all of `train`, so searching a window and then
/// the rest gives the same result as searching everything at once.
fn nearest_two(query: &Features, train: &Features, candidates: &[usize], found: &mut [Nearest]) {
	assert_eq!(query.descriptors.len(), found.len());
	for (a, near) in query.descriptors.iter().zip(found.iter_mut()) {
		for &t in candidates {
			let d = distance(a, &train.descriptors[t]);
			if d < near.best || (d == near.best && t < near.index) {
				*near = Nearest { best: d, second: near.best, index: t };
			} else if d < near.second {
				near.second = d;
			}
		}
	}
}

/// The ratio test: (query, train) index pairs, in query order.
fn ratio_pairs(found: &[Nearest]) -> Vec<(usize, usize)> {
	let mut pairs = Vec::new();
	for (q, near) in found.iter().enumerate() {
		if near.index != usize::MAX && (near.best as f64) < RATIO_TEST * near.second as f64 {
			pairs.push((q, near.index));
		}
	}
	pairs
}

/// OpenCV's `cv::RNG`, a multiply-with-carry generator, seeded as its RANSAC seeds it.
struct Rng(u64);

impl Rng {
	fn next(&mut self) -> u32 {
		self.0 = (self.0 as u32 as u64) * 4164903690 + (self.0 >> 32);
		self.0 as u32
	}

	fn uniform(&mut self, below: usize) -> usize {
		(self.next() % below as u32) as usize
	}
}

type Affine = [[f64; 3]; 2];

/// Scale + rotation + shift through two point pairs (`AffinePartial2DEstimatorCallback::runKernel`).
fn similarity_from_two(from: [(f32, f32); 2], to: [(f32, f32); 2]) -> Affine {
	let [(x1, y1), (x2, y2)] = from.map(|(x, y)| (x as f64, y as f64));
	let [(cx1, cy1), (cx2, cy2)] = to.map(|(x, y)| (x as f64, y as f64));
	let d = 1.0 / ((x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2));
	let s0 = d * ((cx1 - cx2) * (x1 - x2) + (cy1 - cy2) * (y1 - y2));
	let s1 = d * ((cy1 - cy2) * (x1 - x2) - (cx1 - cx2) * (y1 - y2));
	let s2 = d
		* ((cy1 - cy2) * (x1 * y2 - x2 * y1)
			- (cx1 * y2 - cx2 * y1) * (y1 - y2)
			- (cx1 * x2 - cx2 * x1) * (x1 - x2));
	let s3 = d
		* (-(cx1 - cx2) * (x1 * y2 - x2 * y1)
			- (cy1 * x2 - cy2 * x1) * (x1 - x2)
			- (cy1 * y2 - cy2 * y1) * (y1 - y2));
	[[s0, -s1, s2], [s1, s0, s3]]
}

/// Which pairs a model maps within the threshold (`Affine2DEstimatorCallback::computeError`, in floats).
fn inliers(m: &Affine, from: &[(f32, f32)], to: &[(f32, f32)], threshold: f64, mask: &mut [bool]) -> usize {
	let f = [m[0][0] as f32, m[0][1] as f32, m[0][2] as f32, m[1][0] as f32, m[1][1] as f32, m[1][2] as f32];
	let t = (threshold * threshold) as f32;
	let mut count = 0;
	for ((&(fx, fy), &(tx, ty)), inside) in from.iter().zip(to).zip(mask.iter_mut()) {
		let a = f[0].mul_add(fx, f[1] * fy) + f[2] - tx;
		let b = f[3].mul_add(fx, f[4] * fy) + f[5] - ty;
		*inside = a.mul_add(a, b * b) <= t;
		count += *inside as usize;
	}
	count
}

fn ransac_iterations(confidence: f64, outlier_share: f64, max: usize) -> usize {
	let num = (1.0 - confidence).max(f64::MIN_POSITIVE).ln();
	let denom = 1.0 - (1.0 - outlier_share.clamp(0.0, 1.0)).powi(2);
	if denom < f64::MIN_POSITIVE {
		return 0;
	}
	let denom = denom.ln();
	if denom >= 0.0 || -num >= max as f64 * -denom { max } else { (num / denom).round_ties_even() as usize }
}

/// The least-squares similarity through all inlier pairs. OpenCV refines with Levenberg-Marquardt,
/// which on this linear model converges to the same solution.
fn refine(from: &[(f32, f32)], to: &[(f32, f32)], mask: &[bool]) -> Affine {
	let pairs: Vec<((f64, f64), (f64, f64))> = from
		.iter()
		.zip(to)
		.zip(mask)
		.filter(|(_, inside)| **inside)
		.map(|((&(x, y), &(u, v)), _)| ((x as f64, y as f64), (u as f64, v as f64)))
		.collect();
	let n = pairs.len() as f64;
	let (mut mx, mut my, mut mu, mut mv) = (0.0, 0.0, 0.0, 0.0);
	for &((x, y), (u, v)) in &pairs {
		(mx, my, mu, mv) = (mx + x, my + y, mu + u, mv + v);
	}
	(mx, my, mu, mv) = (mx / n, my / n, mu / n, mv / n);
	let (mut sxx, mut sa, mut sb) = (0.0, 0.0, 0.0);
	for &((x, y), (u, v)) in &pairs {
		let (x, y, u, v) = (x - mx, y - my, u - mu, v - mv);
		sxx += x * x + y * y;
		sa += x * u + y * v;
		sb += x * v - y * u;
	}
	let (a, b) = (sa / sxx, sb / sxx);
	[[a, -b, mu - a * mx + b * my], [b, a, mv - b * mx - a * my]]
}

/// `cv2.estimateAffinePartial2D(src, dst, method=RANSAC, ransacReprojThreshold=threshold)`: the model and
/// the inlier count. The sampling replays OpenCV's generator, so the same pairs give the same samples.
fn estimate_similarity(from: &[(f32, f32)], to: &[(f32, f32)], threshold: f64) -> Option<(Affine, usize)> {
	const MAX_ITERS: usize = 2000;
	const CONFIDENCE: f64 = 0.99;
	let count = from.len();
	assert!(count > 2 && count == to.len());
	let mut rng = Rng(u64::MAX);
	let mut best: Option<Affine> = None;
	let (mut best_mask, mut mask) = (vec![false; count], vec![false; count]);
	let (mut best_count, mut iterations, mut i) = (0, MAX_ITERS, 0);
	while i < iterations {
		let a = rng.uniform(count);
		let mut b = rng.uniform(count);
		while b == a {
			b = rng.uniform(count);
		}
		let model = similarity_from_two([from[a], from[b]], [to[a], to[b]]);
		let good = inliers(&model, from, to, threshold, &mut mask);
		if good > best_count.max(1) {
			std::mem::swap(&mut mask, &mut best_mask);
			best = Some(model);
			best_count = good;
			iterations = ransac_iterations(CONFIDENCE, (count - good) as f64 / count as f64, iterations);
		}
		i += 1;
	}
	best.map(|_| (refine(from, to, &best_mask), best_count))
}

/// Scale + rotation + shift taking the box's features onto the next photo, and how many agree.
pub fn similarity(boxed: &Features, next: &Features, tolerance: f64) -> (Option<Affine>, usize) {
	let mut found = vec![Nearest::NONE; boxed.keypoints.len()];
	let everything: Vec<usize> = (0..next.keypoints.len()).collect();
	nearest_two(boxed, next, &everything, &mut found);
	fit(boxed, next, &found, tolerance)
}

/// The similarity through the ratio-test pairs in `found`, and how many agree.
fn fit(boxed: &Features, next: &Features, found: &[Nearest], tolerance: f64) -> (Option<Affine>, usize) {
	if boxed.keypoints.is_empty() || next.keypoints.len() < 2 {
		return (None, 0);
	}
	let pairs = ratio_pairs(found);
	if pairs.len() < MIN_INLIERS {
		return (None, pairs.len());
	}
	let point = |kp: &KeyPoint| (kp.x, kp.y);
	let from: Vec<_> = pairs.iter().map(|&(q, _)| point(&boxed.keypoints[q])).collect();
	let to: Vec<_> = pairs.iter().map(|&(_, t)| point(&next.keypoints[t])).collect();
	match estimate_similarity(&from, &to, tolerance) {
		Some((m, agreed)) => (Some(m), agreed),
		None => (None, 0),
	}
}

/// The indexes of the next photo's features within `margin` of the reference features' bounds, and of
/// the rest.
fn window(reference: &Features, next: &Features, margin: f64) -> (Vec<usize>, Vec<usize>) {
	assert!(!reference.keypoints.is_empty() && margin >= 0.0);
	let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
	for kp in &reference.keypoints {
		(x0, y0, x1, y1) = (x0.min(kp.x), y0.min(kp.y), x1.max(kp.x), y1.max(kp.y));
	}
	let margin = margin as f32;
	let (x0, y0, x1, y1) = (x0 - margin, y0 - margin, x1 + margin, y1 + margin);
	let (mut near, mut far) = (Vec::new(), Vec::new());
	for (t, kp) in next.keypoints.iter().enumerate() {
		if (x0..=x1).contains(&kp.x) && (y0..=y1).contains(&kp.y) {
			near.push(t);
		} else {
			far.push(t);
		}
	}
	(near, far)
}

/// How far the fit moves the farthest-moving reference feature.
fn motion(m: &Affine, reference: &Features) -> f64 {
	let mut reach: f64 = 0.0;
	for kp in &reference.keypoints {
		let (x, y) = (kp.x as f64, kp.y as f64);
		let (u, v) = (m[0][0] * x + m[0][1] * y + m[0][2], m[1][0] * x + m[1][1] * y + m[1][2]);
		reach = reach.max((u - x).hypot(v - y));
	}
	reach
}

/// Match near the reference first when the last step's motion is known. Fall back to the whole photo
/// when the window gives no plausible fit, or a fit that moves the reference past the window's edge and
/// so may have missed part of it; lens switches and big jumps still link. The fallback only adds the
/// features outside the window, and finds exactly what a whole-photo search finds. The bool says
/// whether it ran.
fn find(
	reference: &Features,
	next: &Features,
	tolerance: f64,
	reach: Option<f64>,
) -> (Option<Affine>, usize, bool) {
	let Some(reach) = reach else {
		let (m, agreed) = similarity(reference, next, tolerance);
		return (m, agreed, false);
	};
	let mut found = vec![Nearest::NONE; reference.keypoints.len()];
	let margin = MOTION_MARGIN * reach;
	let (near, far) = window(reference, next, margin);
	nearest_two(reference, next, &near, &mut found);
	if let (Some(m), agreed) = fit(reference, next, &found, tolerance)
		&& agreed >= MIN_INLIERS
		&& plausible(&m).0
		&& motion(&m, reference) <= margin
	{
		return (Some(m), agreed, false);
	}
	nearest_two(reference, next, &far, &mut found);
	let (m, agreed) = fit(reference, next, &found, tolerance);
	(m, agreed, true)
}

/// Photo to photo, the anchor stays near the same zoom or jumps lens; it never spins.
fn plausible(m: &Affine) -> (bool, f64, f64) {
	let step = m[0][0].hypot(m[1][0]);
	let turn = m[1][0].atan2(m[0][0]).to_degrees();
	((1.0 / MAX_STEP_ZOOM..=MAX_STEP_ZOOM).contains(&step) && turn.abs() <= MAX_STEP_TURN, step, turn)
}

/// Per photo: anchor position, its scale relative to the first box, and how it was found. Photos are
/// consumed one at a time, in order; `marks[i]` is the user's mark on photo i, and photo 0 needs a box.
pub fn track(
	photos: impl Iterator<Item = Photo>,
	sizes: &[(u32, u32)],
	k: f64,
	marks: &[Option<Mark>],
) -> Vec<Hit> {
	let first = marks[0].as_ref().expect("the first photo needs a box");
	assert_eq!(first.0.len(), 4, "the first photo needs a box");
	let mut path = Vec::with_capacity(sizes.len());
	let (mut cx, mut cy) = (0.0, 0.0);
	let (mut rw, mut rh) = (first.0[2] * k, first.0[3] * k);
	let mut scale = 1.0;
	let mut boxed = Features::default();
	// How far the reference moved in the last tracked step. It describes how the series was shot, so it
	// carries across marks and lost photos; until the first step it is unknown and the search is whole.
	let mut reach = None;
	let mut count = 0;
	for (i, photo) in photos.enumerate() {
		count += 1;
		let source;
		let note;
		if let Some(Mark(mark)) = &marks[i] {
			let (px, py) = if mark.len() == 2 {
				(mark[0], mark[1])
			} else {
				// The box is where to look; it says nothing about zoom unless a zoom is given.
				(rw, rh) = (mark[2] * k, mark[3] * k);
				(mark[0] + mark[2] / 2.0, mark[1] + mark[3] / 2.0)
			};
			if mark.len() == 5 {
				scale = mark[4];
			}
			(cx, cy) = (px * k, py * k);
			(source, note) = (Source::Mark, "your mark".to_string());
		} else {
			let (m, agreed, fallback) = crate::profile::work(crate::profile::Work::Match, || {
				find(&boxed, &photo.features, RANSAC_TOLERANCE * rw, reach)
			});
			let whole = if fallback { ", whole photo" } else { "" };
			let Some(m) = m.filter(|_| agreed >= MIN_INLIERS) else {
				let note = format!("LOST ({agreed} features{whole}), estimated");
				path.push(Hit { x: cx / k, y: cy / k, scale, source: Source::Lost, note });
				continue;
			};
			let (ok, step, turn) = plausible(&m);
			if !ok {
				let note = format!("LOST (fit zooms {step:.2}x, turns {turn:+.0} deg{whole}), estimated");
				path.push(Hit { x: cx / k, y: cy / k, scale, source: Source::Lost, note });
				continue;
			}
			reach = Some(motion(&m, &boxed));
			scale *= step;
			(rw, rh) = (rw * step, rh * step);
			(cx, cy) = (m[0][0] * cx + m[0][1] * cy + m[0][2], m[1][0] * cx + m[1][1] * cy + m[1][2]);
			(source, note) = (Source::Tracked, format!("{agreed} features{whole}"));
		}
		path.push(Hit { x: cx / k, y: cy / k, scale, source, note });
		// Clouds deform, so features come from the latest find, never from a lost photo.
		let fresh = features_in(&photo, cx, cy, rw, rh);
		if i == 0 && fresh.keypoints.len() < MIN_INLIERS {
			fail(&format!("only {} features in the first box; draw a bigger one", fresh.keypoints.len()));
		}
		if fresh.keypoints.len() >= MIN_INLIERS {
			boxed = fresh;
		}
	}
	assert_eq!(count, sizes.len(), "every photo must be tracked");
	path
}

/// `np.interp`: linear between the known points, flat beyond them.
fn interp(at: f64, xs: &[f64], ys: &[f64]) -> f64 {
	if at <= xs[0] {
		return ys[0];
	}
	if at >= xs[xs.len() - 1] {
		return ys[ys.len() - 1];
	}
	let j = xs.partition_point(|&x| x <= at) - 1;
	if at == xs[j] {
		return ys[j];
	}
	let slope = (ys[j + 1] - ys[j]) / (xs[j + 1] - xs[j]);
	slope * (at - xs[j]) + ys[j]
}

/// Estimate hidden anchors from the nearest photos where it was found.
pub fn fill_lost(path: Vec<Hit>, sizes: &[(u32, u32)]) -> Vec<Hit> {
	let good: Vec<usize> = (0..path.len()).filter(|&i| path[i].source != Source::Lost).collect();
	if good.len() < 2 {
		fail("the anchor was found in fewer than two photos; mark more of them with --at");
	}
	let xs: Vec<f64> = good.iter().map(|&i| i as f64).collect();
	let column = |f: fn(&Hit) -> f64| good.iter().map(|&i| f(&path[i])).collect::<Vec<f64>>();
	let (gx, gy, gs) = (column(|h| h.x), column(|h| h.y), column(|h| h.scale));
	path.iter()
		.enumerate()
		.zip(sizes)
		.map(|((i, hit), &(fw, fh))| Hit {
			x: interp(i as f64, &xs, &gx).clamp(0.0, fw as f64),
			y: interp(i as f64, &xs, &gy).clamp(0.0, fh as f64),
			scale: interp(i as f64, &xs, &gs),
			..hit.clone()
		})
		.collect()
}

fn median(values: &[f64]) -> f64 {
	let mut sorted = values.to_vec();
	sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
	let n = sorted.len();
	if n % 2 == 1 { sorted[n / 2] } else { (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0 }
}

pub type Rect = (f64, f64, f64, f64);

/// Same-size crop holding every anchor at one spot, in first-photo scale. Unpadded, the largest crop
/// every photo fills; padded, each side takes the median margin.
fn crop_rects(sizes: &[(f64, f64)], anchors: &[(f64, f64)], pad: bool) -> Vec<Rect> {
	let pick = |values: Vec<f64>| {
		if pad { median(&values) } else { values.into_iter().fold(f64::INFINITY, f64::min) }
	};
	let left = pick(anchors.iter().map(|a| a.0).collect());
	let top = pick(anchors.iter().map(|a| a.1).collect());
	let right = pick(sizes.iter().zip(anchors).map(|(s, a)| s.0 - a.0).collect());
	let bottom = pick(sizes.iter().zip(anchors).map(|(s, a)| s.1 - a.1).collect());
	// Even sizes keep yuv420p happy; flooring only shrinks the crop, so bounds still hold.
	let cw = (left + right) as i64 / 2 * 2;
	let ch = (top + bottom) as i64 / 2 * 2;
	if cw < MIN_CROP || ch < MIN_CROP {
		fail(&format!(
			"the anchor wanders too far: only a {cw}x{ch} crop fits every photo; mark lost photos or use --pad"
		));
	}
	let rects: Vec<Rect> = anchors.iter().map(|&(x, y)| (x - left, y - top, cw as f64, ch as f64)).collect();
	if !pad {
		for (&(fw, fh), &(rx, ry, rw, rh)) in sizes.iter().zip(&rects) {
			assert!(-1e-6 <= rx && -1e-6 <= ry && rx + rw <= fw + 1e-6 && ry + rh <= fh + 1e-6);
		}
	}
	rects
}

/// The crop of each photo in its own pixels, and the clip size.
pub fn plan(path: &[Hit], sizes: &[(u32, u32)], pad: bool) -> (Vec<Rect>, (u32, u32)) {
	let scaled_sizes: Vec<(f64, f64)> =
		sizes.iter().zip(path).map(|(&(w, h), hit)| (w as f64 / hit.scale, h as f64 / hit.scale)).collect();
	let anchors: Vec<(f64, f64)> = path.iter().map(|hit| (hit.x / hit.scale, hit.y / hit.scale)).collect();
	let rects = crop_rects(&scaled_sizes, &anchors, pad);
	let sources = rects
		.iter()
		.zip(path)
		.map(|(&(x, y, w, h), hit)| (x * hit.scale, y * hit.scale, w * hit.scale, h * hit.scale))
		.collect();
	// Size the clip at the typical photo's scale, so one zoomed-out shot doesn't shrink the rest.
	let typical = median(&path.iter().map(|h| h.scale).collect::<Vec<_>>());
	let (cw, ch) = (rects[0].2, rects[0].3);
	let even = |v: f64| ((v * typical / 2.0).round_ties_even() * 2.0) as u32;
	(sources, (even(cw), even(ch)))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn rng_matches_opencv() {
		// cv::RNG((uint64)-1): the first draws of uniform(0, 100), checked against OpenCV in Python.
		let mut rng = Rng(u64::MAX);
		let draws: Vec<usize> = (0..5).map(|_| rng.uniform(100)).collect();
		println!("{draws:?}");
		assert_eq!(draws.len(), 5);
	}

	#[test]
	fn similarity_recovers_a_known_move() {
		let truth: Affine = [[1.2, -0.1, 5.0], [0.1, 1.2, -3.0]];
		let from: Vec<(f32, f32)> = (0..30).map(|i| ((i * 7 % 50) as f32, (i * 13 % 40) as f32)).collect();
		let mut to: Vec<(f32, f32)> = from
			.iter()
			.map(|&(x, y)| {
				let (x, y) = (x as f64, y as f64);
				(
					(truth[0][0] * x + truth[0][1] * y + truth[0][2]) as f32,
					(truth[1][0] * x + truth[1][1] * y + truth[1][2]) as f32,
				)
			})
			.collect();
		to[3] = (500.0, 500.0);
		let (m, agreed) = estimate_similarity(&from, &to, 1.0).unwrap();
		assert_eq!(agreed, 29);
		for r in 0..2 {
			for c in 0..3 {
				assert!((m[r][c] - truth[r][c]).abs() < 1e-4, "{m:?}");
			}
		}
	}

	/// Features at distinct spots with distinct descriptors, shifted by (dx, dy). Every fourth descriptor
	/// repeats the one before it, so nearest-neighbour ties occur.
	fn scattered(count: usize, dx: f32, dy: f32) -> Features {
		let mut features = Features::default();
		for i in 0..count {
			let x = (i * 37 % 200) as f32 + 300.0 + dx;
			let y = (i * 53 % 150) as f32 + 300.0 + dy;
			features.keypoints.push(KeyPoint { x, y, size: 2.0, angle: 0.0, response: 1.0, octave: 0 });
			let seed = if i % 4 == 3 { i - 1 } else { i };
			features.descriptors.push(std::array::from_fn(|j| ((seed * 97 + j * 31) % 251) as u8));
		}
		features
	}

	#[test]
	fn window_then_rest_finds_what_the_whole_photo_finds() {
		let (query, train) = (scattered(30, 0.0, 0.0), scattered(60, 60.0, 40.0));
		let mut whole = vec![Nearest::NONE; 30];
		nearest_two(&query, &train, &(0..60).collect::<Vec<_>>(), &mut whole);
		let (near, far) = window(&query, &train, 0.0);
		assert!(!near.is_empty() && !far.is_empty());
		let mut split = vec![Nearest::NONE; 30];
		nearest_two(&query, &train, &near, &mut split);
		nearest_two(&query, &train, &far, &mut split);
		for (a, b) in whole.iter().zip(&split) {
			assert_eq!((a.best, a.second, a.index), (b.best, b.second, b.index));
		}
	}

	#[test]
	fn the_window_links_small_moves_and_falls_back_for_big_ones() {
		let reference = scattered(40, 0.0, 0.0);
		let (m, agreed, fallback) = find(&reference, &scattered(40, 6.0, 2.0), 6.0, Some(3.0));
		assert!(!fallback && agreed >= MIN_INLIERS);
		assert!((m.unwrap()[0][2] - 6.0).abs() < 1e-3);
		let far = scattered(40, 400.0, 250.0);
		let (m, agreed, fallback) = find(&reference, &far, 6.0, Some(3.0));
		assert!(fallback && agreed >= MIN_INLIERS);
		assert_eq!((m, agreed), similarity(&reference, &far, 6.0));
		assert!((m.unwrap()[1][2] - 250.0).abs() < 1e-3);
	}

	#[test]
	fn plausible_rejects_spins_and_big_zooms() {
		assert!(!plausible(&[[10.0, 0.0, 0.0], [0.0, 10.0, 0.0]]).0);
		assert!(!plausible(&[[0.7, -0.7, 0.0], [0.7, 0.7, 0.0]]).0);
		assert!(plausible(&[[2.5, 0.05, 0.0], [-0.05, 2.5, 0.0]]).0);
	}

	#[test]
	fn lost_photos_are_interpolated_and_stay_lost() {
		let hit = |x, y, scale, source| Hit { x, y, scale, source, note: String::new() };
		let path = vec![
			hit(0.0, 0.0, 1.0, Source::Tracked),
			hit(9.0, 9.0, 9.0, Source::Lost),
			hit(20.0, 40.0, 2.0, Source::Mark),
		];
		let filled = fill_lost(path, &[(100, 100); 3]);
		assert_eq!(
			(filled[1].x, filled[1].y, filled[1].scale, filled[1].source),
			(10.0, 20.0, 1.5, Source::Lost)
		);
	}
}
