//! Places each photo's crop into the clip frame and streams the frames, in order, to an encoder.

use crate::pixels::{saturate_u8, Image};
use crate::track::Rect;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Condvar, Mutex};

/// Runs `work(i)` for i in 0..n on `threads` threads and hands each result to `take` in order. At most
/// `ahead` results are in flight or waiting, so memory stays bounded whatever n is.
pub fn ordered<T: Send>(
	n: usize,
	threads: usize,
	ahead: usize,
	work: impl Fn(usize) -> T + Sync,
	mut take: impl FnMut(usize, T),
) {
	assert!(threads >= 1 && ahead >= threads);
	struct State<T> {
		next_task: usize,
		next_take: usize,
		done: BTreeMap<usize, T>,
	}
	let state = Mutex::new(State { next_task: 0, next_take: 0, done: BTreeMap::new() });
	let (ready, room) = (Condvar::new(), Condvar::new());
	std::thread::scope(|scope| {
		for _ in 0..threads {
			scope.spawn(|| loop {
				let i = {
					let mut s = state.lock().unwrap();
					while s.next_task < n && s.next_task >= s.next_take + ahead {
						s = room.wait(s).unwrap();
					}
					if s.next_task >= n {
						return;
					}
					s.next_task += 1;
					s.next_task - 1
				};
				let result = work(i);
				state.lock().unwrap().done.insert(i, result);
				ready.notify_all();
			});
		}
		for i in 0..n {
			let result = {
				let mut s = state.lock().unwrap();
				loop {
					if let Some(result) = s.done.remove(&i) {
						s.next_take = i + 1;
						break result;
					}
					s = ready.wait(s).unwrap();
				}
			};
			room.notify_all();
			take(i, result);
		}
	});
}

/// Decoded photos held from the first pass for render, within a byte budget. A photo that does not fit
/// is decoded again at render, so memory stays bounded for any photo count.
pub struct Keep {
	budget: Mutex<(usize, HashMap<usize, Image>)>,
}

impl Keep {
	pub fn new(budget: usize) -> Keep {
		Keep { budget: Mutex::new((budget, HashMap::new())) }
	}

	pub fn offer(&self, i: usize, image: Image) {
		let mut kept = self.budget.lock().unwrap();
		if image.pixels.len() <= kept.0 {
			kept.0 -= image.pixels.len();
			kept.1.insert(i, image);
		}
	}

	pub fn take(&self, i: usize) -> Option<Image> {
		self.budget.lock().unwrap().1.remove(&i)
	}
}

/// Source taps for one output position: the in-image pixels with their weights, and the weight that
/// falls outside the image (padding).
struct Taps {
	index: Vec<usize>,
	weight: Vec<f32>,
	start: Vec<usize>,
	outside: Vec<f32>,
}

/// OpenCV's bicubic weights (a = -0.75) for an offset t in [0, 1).
fn cubic(t: f32) -> [f32; 4] {
	const A: f32 = -0.75;
	let w0 = ((A * (t + 1.0) - 5.0 * A) * (t + 1.0) + 8.0 * A) * (t + 1.0) - 4.0 * A;
	let w1 = ((A + 2.0) * t - (A + 3.0)) * t * t + 1.0;
	let w2 = ((A + 2.0) * (1.0 - t) - (A + 3.0)) * (1.0 - t) * (1.0 - t) + 1.0;
	[w0, w1, w2, 1.0 - w0 - w1 - w2]
}

/// Taps along one axis: output pixel j samples the photo at `origin + j / f` (pixel centres). Enlarging
/// uses bicubic weights; shrinking averages the 1/f-wide span the output pixel covers.
fn taps(origin: f64, f: f64, out: usize, size: usize, replicate: bool) -> Taps {
	let mut t = Taps { index: vec![], weight: vec![], start: vec![], outside: vec![] };
	let add = |t: &mut Taps, i: i64, w: f32, outside: &mut f32| {
		if (0..size as i64).contains(&i) {
			t.index.push(i as usize);
			t.weight.push(w);
		} else if replicate {
			t.index.push(i.clamp(0, size as i64 - 1) as usize);
			t.weight.push(w);
		} else {
			*outside += w;
		}
	};
	for j in 0..out {
		t.start.push(t.index.len());
		let centre = origin + j as f64 / f;
		let mut outside = 0f32;
		if f >= 1.0 {
			let base = centre.floor();
			for (k, w) in cubic((centre - base) as f32).into_iter().enumerate() {
				add(&mut t, base as i64 - 1 + k as i64, w, &mut outside);
			}
		} else {
			// ponytail: one area pass, a little sharper than anchor.py's INTER_AREA resize then bilinear
			// warp (shrunk Girona frames differ by 32-39 dB, the rest by 59). Compose both filters' taps
			// here if frames must match anchor.py's more closely.
			let (lo, hi) = (centre + 0.5 - 0.5 / f, centre + 0.5 + 0.5 / f);
			for i in lo.floor() as i64..hi.ceil() as i64 {
				let overlap = (hi.min(i as f64 + 1.0) - lo.max(i as f64)).max(0.0);
				add(&mut t, i, (overlap * f) as f32, &mut outside);
			}
		}
		t.outside.push(outside);
	}
	t.start.push(t.index.len());
	t
}

/// Scales the source rect (photo pixels, may overhang the photo) to `size`, in one separable pass per
/// axis straight from the photo. Overhang shows `fill`; without fill, edge pixels repeat.
pub fn place(photo: &Image, source: Rect, size: (u32, u32), fill: Option<&[u8]>) -> Image {
	let (x, y, w, _) = source;
	let (ow, oh) = (size.0 as usize, size.1 as usize);
	let f = ow as f64 / w;
	let cn = fill.map_or(3, |c| c.len());
	let fill_value = |c: usize| fill.map_or(0.0, |v| v[c] as f32);
	let replicate = fill.is_none();
	let xt = taps(x, f, ow, photo.width, replicate);
	let yt = taps(y, f, oh, photo.height, replicate);
	// Horizontal pass of one photo row into `out`, with padding mixed in where the taps overhang.
	let horizontal = |r: usize, out: &mut [f32]| {
		let src = photo.row(r);
		for j in 0..ow {
			let mut acc = [0f32; 4];
			for k in xt.start[j]..xt.start[j + 1] {
				let (p, wt) = (&src[xt.index[k] * 3..xt.index[k] * 3 + 3], xt.weight[k]);
				for c in 0..3 {
					acc[c] = (p[c] as f32).mul_add(wt, acc[c]);
				}
				acc[3] += 255.0 * wt;
			}
			for c in 0..cn {
				out[j * cn + c] = xt.outside[j].mul_add(fill_value(c), acc[c]);
			}
		}
	};
	// Vertical pass. The rows it reads only move down, so a few horizontally filtered rows are kept at a
	// time, not the whole crop.
	let mut window: VecDeque<(usize, Vec<f32>)> = VecDeque::new();
	let mut spare: Vec<Vec<f32>> = Vec::new();
	let mut pixels = vec![0u8; ow * oh * cn];
	let mut acc = vec![0f32; ow * cn];
	for i in 0..oh {
		let rows = &yt.index[yt.start[i]..yt.start[i + 1]];
		while window.front().is_some_and(|(r, _)| rows.first().is_some_and(|lowest| r < lowest)) {
			spare.push(window.pop_front().unwrap().1);
		}
		for &r in rows {
			if window.iter().all(|(q, _)| *q != r) {
				let mut buf = spare.pop().unwrap_or_else(|| vec![0f32; ow * cn]);
				horizontal(r, &mut buf);
				window.push_back((r, buf));
			}
		}
		let outside = yt.outside[i];
		for (j, a) in acc.iter_mut().enumerate() {
			*a = outside * fill_value(j % cn);
		}
		for (&r, &wt) in rows.iter().zip(&yt.weight[yt.start[i]..yt.start[i + 1]]) {
			let row = &window.iter().find(|(q, _)| *q == r).unwrap().1;
			for (a, v) in acc.iter_mut().zip(row) {
				*a = v.mul_add(wt, *a);
			}
		}
		for (p, a) in pixels[i * ow * cn..(i + 1) * ow * cn].iter_mut().zip(&acc) {
			*p = saturate_u8(*a);
		}
	}
	Image { width: ow, height: oh, channels: cn, pixels }
}

#[cfg(test)]
mod tests {
	use super::*;

	fn gradient(w: usize, h: usize) -> Image {
		let pixels = (0..w * h).flat_map(|i| [(i % w) as u8, (i / w) as u8, 100]).collect();
		Image { width: w, height: h, channels: 3, pixels }
	}

	#[test]
	fn same_size_crop_is_a_copy() {
		let photo = gradient(40, 30);
		let frame = place(&photo, (5.0, 4.0, 20.0, 10.0), (20, 10), None);
		assert_eq!(frame.row(0)[..6], [5, 4, 100, 6, 4, 100]);
		assert_eq!(frame.row(9)[57..60], [24, 13, 100]);
	}

	#[test]
	fn overhang_is_transparent() {
		let photo = gradient(40, 30);
		let frame = place(&photo, (-10.0, 0.0, 20.0, 10.0), (10, 5), Some(&[0, 0, 0, 0]));
		assert_eq!(frame.row(2)[3], 0, "left of the photo is transparent");
		assert_eq!(frame.row(2)[9 * 4 + 3], 255, "inside the photo is opaque");
	}

	#[test]
	fn ordered_keeps_order() {
		let mut seen = vec![];
		ordered(50, 4, 8, |i| i * 2, |i, v| seen.push((i, v)));
		assert_eq!(seen, (0..50).map(|i| (i, i * 2)).collect::<Vec<_>>());
	}
}
