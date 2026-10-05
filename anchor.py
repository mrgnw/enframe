import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import NamedTuple

import cv2
import numpy as np
from PIL import Image, ImageOps

MIN_INLIERS = 8
MIN_SHAPE = 0.4
MIN_CROP = 64
TRACK_EDGE = 1024
RATIO_TEST = 0.8
RANSAC_TOLERANCE = 0.03


def load(path: Path) -> np.ndarray:
	with Image.open(path) as im:
		return np.asarray(ImageOps.exif_transpose(im).convert('RGB'))


def gray(rgb: np.ndarray) -> np.ndarray:
	return cv2.cvtColor(rgb, cv2.COLOR_RGB2GRAY)


def resize(img: np.ndarray, factor: float) -> np.ndarray:
	interpolation = cv2.INTER_AREA if factor < 1 else cv2.INTER_LINEAR
	return cv2.resize(img, None, fx=factor, fy=factor, interpolation=interpolation)


def features_in(sift, g: np.ndarray, cx: float, cy: float, w: float, h: float):
	mask = np.zeros_like(g)
	x0, y0 = max(0, round(cx - w / 2)), max(0, round(cy - h / 2))
	mask[y0 : round(cy + h / 2), x0 : round(cx + w / 2)] = 255
	return sift.detectAndCompute(g, mask)


def similarity(kp, des, kp2, des2, tolerance: float):
	"""Scale + rotation + shift taking the box's features onto the next frame, and how many agree."""
	if des is None or des2 is None or len(kp2) < 2:
		return None, 0
	matcher = cv2.BFMatcher(cv2.NORM_L2)
	pairs = [m for m, n in matcher.knnMatch(des, des2, k=2) if m.distance < RATIO_TEST * n.distance]
	if len(pairs) < MIN_INLIERS:
		return None, len(pairs)
	src = np.float32([kp[m.queryIdx].pt for m in pairs])
	dst = np.float32([kp2[m.trainIdx].pt for m in pairs])
	M, inliers = cv2.estimateAffinePartial2D(src, dst, method=cv2.RANSAC, ransacReprojThreshold=tolerance)
	if M is None:
		return None, 0
	return M, int(inliers.sum())


def shape_match(prev: np.ndarray, g: np.ndarray, cx: float, cy: float, w: float, h: float):
	"""Find the anchor by overall shape at the current scale; catches a cloud whose detail has changed."""
	t = cv2.getRectSubPix(prev, (max(1, round(w)), max(1, round(h))), (cx, cy))
	th, tw = t.shape
	if th > g.shape[0] or tw > g.shape[1]:
		return None
	_, score, _, (x, y) = cv2.minMaxLoc(cv2.matchTemplate(g, t, cv2.TM_CCOEFF_NORMED))
	if score < MIN_SHAPE:
		return None
	return x + tw / 2, y + th / 2, score


class Hit(NamedTuple):
	x: float
	y: float
	scale: float
	found: bool
	note: str


def track(frames: list[np.ndarray], box: tuple[int, int, int, int]) -> list[Hit]:
	"""Per frame: anchor position, its scale relative to the first frame, and how it was found."""
	bx, by, bw, bh = box
	fh, fw = frames[0].shape[:2]
	assert bw > 0 and bh > 0, 'empty box'
	assert bx >= 0 and by >= 0 and bx + bw <= fw and by + bh <= fh, 'box outside first frame'

	# One factor for every frame, so differing resolutions show up as scale, like a lens change.
	k = min(1.0, TRACK_EDGE / max(max(f.shape[:2]) for f in frames))
	small = [resize(gray(f), k) for f in frames]
	sift = cv2.SIFT_create()
	cx, cy, w, h = (bx + bw / 2) * k, (by + bh / 2) * k, bw * k, bh * k
	kp, des = features_in(sift, small[0], cx, cy, w, h)
	assert len(kp) >= MIN_INLIERS, f'only {len(kp)} features in the box; draw a bigger one'

	path = [Hit(cx / k, cy / k, 1.0, True, 'your box')]
	scale = 1.0
	last = small[0]
	for g in small[1:]:
		kp2, des2 = sift.detectAndCompute(g, None)
		M, agreed = similarity(kp, des, kp2, des2, RANSAC_TOLERANCE * w * scale)
		if M is not None and agreed >= MIN_INLIERS:
			scale *= float(np.hypot(M[0, 0], M[1, 0]))
			cx, cy = M @ np.array([cx, cy, 1.0])
			note = f'{agreed} features'
		else:
			shape = shape_match(last, g, cx, cy, w * scale, h * scale)
			if shape is None:
				path.append(Hit(cx / k, cy / k, scale, False, f'LOST ({agreed} features), estimated'))
				continue
			cx, cy, score = shape
			note = f'shape {score:.2f}'
		path.append(Hit(cx / k, cy / k, scale, True, note))
		# Clouds deform, so match each frame against the last good one, never a lost one.
		last = g
		fresh = features_in(sift, g, cx, cy, w * scale, h * scale)
		if len(fresh[0]) >= MIN_INLIERS:
			kp, des = fresh
	return path


def fill_lost(path: list[Hit], sizes) -> list[Hit]:
	"""Estimate hidden anchors from the nearest frames where it was found."""
	good = [i for i, hit in enumerate(path) if hit.found]
	assert len(good) >= 2, 'anchor found in fewer than two frames'
	every = np.arange(len(path))
	x, y, s = (np.interp(every, good, [path[i][c] for i in good]) for c in range(3))
	return [
		path[i]._replace(x=float(np.clip(x[i], 0, fw)), y=float(np.clip(y[i], 0, fh)), scale=float(s[i]))
		for i, (fw, fh) in zip(every, sizes)
	]


def crop_rects(sizes, anchors) -> list[tuple[float, float, int, int]]:
	"""Largest same-size crop holding every anchor at one spot; all values in first-frame scale."""
	left = min(x for x, _ in anchors)
	top = min(y for _, y in anchors)
	right = min(fw - x for (fw, _), (x, _) in zip(sizes, anchors))
	bottom = min(fh - y for (_, fh), (_, y) in zip(sizes, anchors))
	# Even sizes keep yuv420p happy; flooring only shrinks the crop, so bounds still hold.
	cw = int(left + right) // 2 * 2
	ch = int(top + bottom) // 2 * 2
	assert cw >= MIN_CROP and ch >= MIN_CROP, (
		f'anchor wanders too far: only a {cw}x{ch} crop fits every frame'
	)

	rects = [(x - left, y - top, cw, ch) for x, y in anchors]
	for (fw, fh), (rx, ry, rw, rh) in zip(sizes, rects):
		assert -1e-6 <= rx and -1e-6 <= ry and rx + rw <= fw + 1e-6 and ry + rh <= fh + 1e-6
	return rects


def to_source(rect, s: float, size: tuple[int, int]) -> tuple[int, int, int, int]:
	x, y, w, h = rect
	fw, fh = size
	sw, sh = min(round(w * s), fw), min(round(h * s), fh)
	# Rounding can overshoot by a pixel; nudge back inside rather than pad.
	sx = min(max(round(x * s), 0), fw - sw)
	sy = min(max(round(y * s), 0), fh - sh)
	return sx, sy, sw, sh


def cut(rgb: np.ndarray, source: tuple[int, int, int, int], size: tuple[int, int]) -> np.ndarray:
	x, y, w, h = source
	interpolation = cv2.INTER_AREA if w > size[0] else cv2.INTER_CUBIC
	return cv2.resize(rgb[y : y + h, x : x + w], size, interpolation=interpolation)


def render(frames: list[np.ndarray], sources, size: tuple[int, int], out: Path, fps: float):
	w, h = size
	cmd = [
		'ffmpeg', '-y', '-loglevel', 'error',
		'-f', 'rawvideo', '-pix_fmt', 'rgb24', '-s', f'{w}x{h}', '-r', str(fps), '-i', '-',
		'-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '18', str(out),
	]  # fmt: skip
	with subprocess.Popen(cmd, stdin=subprocess.PIPE) as ffmpeg:
		assert ffmpeg.stdin is not None
		for rgb, source in zip(frames, sources):
			ffmpeg.stdin.write(np.ascontiguousarray(cut(rgb, source, size)).tobytes())
		ffmpeg.stdin.close()
	assert ffmpeg.returncode == 0, f'ffmpeg exited {ffmpeg.returncode}'


def write_rects(paths: list[Path], sizes, sources, out: Path):
	rows = [
		{'file': p.name, 'x': x / fw, 'y': y / fh, 'w': w / fw, 'h': h / fh}
		for p, (fw, fh), (x, y, w, h) in zip(paths, sizes, sources)
	]
	out.write_text(json.dumps(rows, indent='\t') + '\n')


def pick_box(rgb: np.ndarray) -> tuple[int, int, int, int]:
	box = cv2.selectROI(
		'Drag a box around the anchor, then press Enter', cv2.cvtColor(rgb, cv2.COLOR_RGB2BGR)
	)
	cv2.destroyAllWindows()
	assert box[2] > 0 and box[3] > 0, 'no box drawn'
	return tuple(int(v) for v in box)


def plan(frames: list[np.ndarray], box):
	sizes = [(f.shape[1], f.shape[0]) for f in frames]
	path = fill_lost(track(frames, box), sizes)
	scaled_sizes = [(fw / hit.scale, fh / hit.scale) for (fw, fh), hit in zip(sizes, path)]
	scaled_anchors = [(hit.x / hit.scale, hit.y / hit.scale) for hit in path]
	rects = crop_rects(scaled_sizes, scaled_anchors)
	sources = [to_source(r, hit.scale, size) for r, hit, size in zip(rects, path, sizes)]
	# Size the clip at the typical photo's scale, so one zoomed-out shot doesn't shrink the rest.
	typical = float(np.median([hit.scale for hit in path]))
	_, _, cw, ch = rects[0]
	return path, sizes, sources, (round(cw * typical / 2) * 2, round(ch * typical / 2) * 2)


def main():
	parser = argparse.ArgumentParser(description='Pin one feature in place across a photo sequence.')
	parser.add_argument('photos', nargs='+', type=Path)
	parser.add_argument('--box', help='x,y,w,h of the anchor in the first photo; omit to draw it')
	parser.add_argument('--fps', type=float, default=4)
	parser.add_argument('--out', type=Path, default=Path('out.mp4'))
	args = parser.parse_args()

	# ponytail: filename order; the exported AVIFs carry no capture time to sort by
	paths = sorted(args.photos)
	frames = [load(p) for p in paths]
	box = tuple(int(v) for v in args.box.split(',')) if args.box else pick_box(frames[0])
	assert len(box) == 4, '--box needs x,y,w,h'

	path, sizes, sources, size = plan(frames, box)
	for p, (fw, fh), hit in zip(paths, sizes, path):
		print(f'{p.name}  {fw}x{fh}  anchor {hit.x:.0f},{hit.y:.0f}  scale {hit.scale:.2f}  {hit.note}')
	print(f'clip {size[0]}x{size[1]}')

	render(frames, sources, size, args.out, args.fps)
	write_rects(paths, sizes, sources, args.out.with_suffix('.json'))
	print(f'wrote {args.out}')


def demo():
	rng = np.random.default_rng(0)
	# Blurred noise: textured enough to match uniquely, smooth enough to survive rescaling, like a cloud.
	noise = cv2.GaussianBlur(rng.random((1200, 1200)).astype(np.float32), (0, 0), 4)
	world = cv2.normalize(noise, None, 0, 255, cv2.NORM_MINMAX).astype(np.uint8)
	anchor = (600, 600)
	# (x, y, w, h, scale): a landscape 1x shot, a zoomed-out portrait, then a 4x jump in.
	shots = [(400, 450, 400, 300, 1.0), (450, 300, 600, 800, 0.5), (520, 540, 200, 150, 2.0)]
	frames, truth = [], []
	for x, y, w, h, s in shots:
		frames.append(np.dstack([resize(world[y : y + h, x : x + w], s)] * 3))
		truth.append(((anchor[0] - x) * s, (anchor[1] - y) * s, s))

	tx, ty, _ = truth[0]
	box = (round(tx) - 60, round(ty) - 60, 120, 120)
	path = track(frames, box)
	for hit, (ex, ey, es) in zip(path, truth):
		assert hit.found and abs(hit.x - ex) <= 2 and abs(hit.y - ey) <= 2, (path, truth)
		assert abs(hit.scale / es - 1) < 0.03, (path, truth)

	_, _, sources, size = plan(frames, box)
	crops = [cut(f, src, size).astype(int) for f, src in zip(frames, sources)]
	for c in crops[1:]:
		assert np.abs(c - crops[0]).mean() < 8, 'locked crops of a static world must match'

	hidden = [Hit(0, 0, 1, True, ''), Hit(9, 9, 9, False, ''), Hit(20, 40, 2, True, '')]
	assert fill_lost(hidden, [(100, 100)] * 3)[1][:3] == (10, 20, 1.5)
	print('self-check passed')


if __name__ == '__main__':
	if len(sys.argv) == 1:
		demo()
	else:
		main()
