import argparse
import json
import math
import subprocess
import sys
from pathlib import Path
from typing import NamedTuple

import cv2
import numpy as np
from PIL import Image, ImageOps

MIN_INLIERS = 8
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


class Hit(NamedTuple):
	x: float
	y: float
	scale: float
	found: bool
	note: str


def track(frames: list[np.ndarray], boxes: dict[int, tuple[int, int, int, int]]) -> list[Hit]:
	"""Per frame: anchor position, its scale relative to the first box, and how it was found.

	boxes maps a frame index to an x,y,w,h the user drew; frame 0 needs one, later ones fix lost frames.
	"""
	assert 0 in boxes, 'the first photo needs a box'
	# One factor for every frame, so differing resolutions show up as scale, like a lens change.
	k = min(1.0, TRACK_EDGE / max(max(f.shape[:2]) for f in frames))
	small = [resize(gray(f), k) for f in frames]
	sift = cv2.SIFT_create()
	first_area = boxes[0][2] * boxes[0][3]
	w, h = boxes[0][2] * k, boxes[0][3] * k

	path: list[Hit] = []
	cx = cy = 0.0
	scale = 1.0
	kp, des = None, None
	for i, g in enumerate(small):
		if i in boxes:
			bx, by, bw, bh = boxes[i]
			fh, fw = frames[i].shape[:2]
			assert bw > 0 and bh > 0 and bx >= 0 and by >= 0, f'empty box on photo {i}'
			assert bx + bw <= fw and by + bh <= fh, f'box outside photo {i}'
			cx, cy = (bx + bw / 2) * k, (by + bh / 2) * k
			# A drawn box says how big the anchor is, so a fix works across a lens switch too.
			scale = float(np.sqrt(bw * bh / first_area))
			region = (bw * k, bh * k)
			note = 'your box'
		else:
			kp2, des2 = sift.detectAndCompute(g, None)
			M, agreed = similarity(kp, des, kp2, des2, RANSAC_TOLERANCE * w * scale)
			if M is None or agreed < MIN_INLIERS:
				path.append(Hit(cx / k, cy / k, scale, False, f'LOST ({agreed} features), estimated'))
				continue
			scale *= float(np.hypot(M[0, 0], M[1, 0]))
			cx, cy = M @ np.array([cx, cy, 1.0])
			region = (w * scale, h * scale)
			note = f'{agreed} features'
		path.append(Hit(cx / k, cy / k, scale, True, note))
		# Clouds deform, so features come from the latest find, never from a lost frame.
		fresh = features_in(sift, g, cx, cy, *region)
		if i in boxes:
			assert len(fresh[0]) >= MIN_INLIERS, f'only {len(fresh[0])} features in the box on photo {i}'
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


def crop_rects(sizes, anchors, pad: bool) -> list[tuple[float, float, int, int]]:
	"""Same-size crop holding every anchor at one spot; all values in first-frame scale.

	Unpadded, it is the largest crop every photo fills. Padded, each side takes the median margin, so
	half the photos fill each edge and the rest show padding.
	"""
	# ponytail: median margins when padding; a percentile knob if half-filled edges look wrong
	pick = np.median if pad else min
	left = float(pick([x for x, _ in anchors]))
	top = float(pick([y for _, y in anchors]))
	right = float(pick([fw - x for (fw, _), (x, _) in zip(sizes, anchors)]))
	bottom = float(pick([fh - y for (_, fh), (_, y) in zip(sizes, anchors)]))
	# Even sizes keep yuv420p happy; flooring only shrinks the crop, so bounds still hold.
	cw = int(left + right) // 2 * 2
	ch = int(top + bottom) // 2 * 2
	assert cw >= MIN_CROP and ch >= MIN_CROP, (
		f'anchor wanders too far: only a {cw}x{ch} crop fits every frame'
	)

	rects = [(x - left, y - top, cw, ch) for x, y in anchors]
	if not pad:
		for (fw, fh), (rx, ry, rw, rh) in zip(sizes, rects):
			assert -1e-6 <= rx and -1e-6 <= ry and rx + rw <= fw + 1e-6 and ry + rh <= fh + 1e-6
	return rects


def place(rgb: np.ndarray, source, size: tuple[int, int], fill: tuple[int, ...] | None) -> np.ndarray:
	"""Scale the source rect (photo pixels, may overhang the photo) to size; overhang shows fill."""
	x, y, w, h = source
	out_w, out_h = size
	fh, fw = rgb.shape[:2]
	f = out_w / w
	# Cubic resampling reads 2 px past each edge; cut that much extra photo so only real overhang is padded.
	a0, b0 = max(0, math.floor(x) - 2), max(0, math.floor(y) - 2)
	a1, b1 = min(fw, math.ceil(x + w) + 2), min(fh, math.ceil(y + h) + 2)
	assert a1 > a0 and b1 > b0, 'crop misses the photo entirely'

	piece = rgb[b0:b1, a0:a1]
	if fill is not None and len(fill) == 4:
		piece = np.dstack([piece, np.full(piece.shape[:2], 255, np.uint8)])
	if f < 1:
		piece = cv2.resize(piece, None, fx=f, fy=f, interpolation=cv2.INTER_AREA)
	px, py = piece.shape[1] / (a1 - a0), piece.shape[0] / (b1 - b0)
	M = np.float64([[f / px, 0, (a0 - x) * f], [0, f / py, (b0 - y) * f]])
	# Unpadded crops sit inside the photo, so replicating only ever fills a rounding seam.
	border = cv2.BORDER_REPLICATE if fill is None else cv2.BORDER_CONSTANT
	return cv2.warpAffine(
		piece,
		M,
		(out_w, out_h),
		flags=cv2.INTER_CUBIC if f >= 1 else cv2.INTER_LINEAR,
		borderMode=border,
		borderValue=fill or 0,
	)


def encoder(out: Path, alpha: bool) -> list[str]:
	if out.suffix == '.webm':
		return [
			'-c:v',
			'libvpx-vp9',
			'-pix_fmt',
			'yuva420p' if alpha else 'yuv420p',
			'-crf',
			'30',
			'-b:v',
			'0',
		]
	if out.suffix == '.mov' and alpha:
		return ['-c:v', 'prores_ks', '-profile:v', '4444', '-pix_fmt', 'yuva444p10le']
	assert not alpha, 'transparent padding needs a .mov (ProRes 4444) or .webm (VP9) output'
	return ['-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '18']


def render(frames, sources, size: tuple[int, int], fill: tuple[int, ...] | None, out: Path, fps: float):
	alpha = fill is not None and len(fill) == 4
	w, h = size
	cmd = [
		'ffmpeg', '-y', '-loglevel', 'error',
		'-f', 'rawvideo', '-pix_fmt', 'rgba' if alpha else 'rgb24', '-s', f'{w}x{h}', '-r', str(fps), '-i', '-',
		*encoder(out, alpha), str(out),
	]  # fmt: skip
	with subprocess.Popen(cmd, stdin=subprocess.PIPE) as ffmpeg:
		assert ffmpeg.stdin is not None
		for rgb, source in zip(frames, sources):
			ffmpeg.stdin.write(np.ascontiguousarray(place(rgb, source, size, fill)).tobytes())
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


def plan(frames: list[np.ndarray], boxes, pad: bool = False):
	sizes = [(f.shape[1], f.shape[0]) for f in frames]
	path = fill_lost(track(frames, boxes), sizes)
	scaled_sizes = [(fw / hit.scale, fh / hit.scale) for (fw, fh), hit in zip(sizes, path)]
	scaled_anchors = [(hit.x / hit.scale, hit.y / hit.scale) for hit in path]
	rects = crop_rects(scaled_sizes, scaled_anchors, pad)
	sources = [
		(x * hit.scale, y * hit.scale, w * hit.scale, h * hit.scale) for (x, y, w, h), hit in zip(rects, path)
	]
	# Size the clip at the typical photo's scale, so one zoomed-out shot doesn't shrink the rest.
	typical = float(np.median([hit.scale for hit in path]))
	_, _, cw, ch = rects[0]
	return path, sizes, sources, (round(cw * typical / 2) * 2, round(ch * typical / 2) * 2)


def parse_box(value: str) -> tuple[int, int, int, int]:
	box = tuple(int(v) for v in value.split(','))
	assert len(box) == 4, f'{value}: a box is x,y,w,h'
	return box


def parse_fill(value: str) -> tuple[int, ...]:
	if value == 'transparent':
		return (0, 0, 0, 0)
	hex_value = {'black': '000000', 'white': 'ffffff'}.get(value, value.removeprefix('#'))
	assert len(hex_value) == 6, f'--pad {value}: use transparent, black, white or a #rrggbb colour'
	return tuple(int(hex_value[i : i + 2], 16) for i in (0, 2, 4))


def main():
	parser = argparse.ArgumentParser(description='Pin one feature in place across a photo sequence.')
	parser.add_argument('photos', nargs='+', type=Path)
	parser.add_argument('--box', help='x,y,w,h of the anchor in the first photo; omit to draw it')
	parser.add_argument(
		'--at',
		action='append',
		default=[],
		metavar='PHOTO=x,y,w,h',
		help='box the anchor in a photo the tracker lost; repeatable',
	)
	parser.add_argument(
		'--pad',
		metavar='COLOUR',
		help='let the crop overhang photos, filled with transparent, black, white or #rrggbb; '
		'omit to shrink the crop until every photo fills it',
	)
	parser.add_argument('--fps', type=float, default=4)
	parser.add_argument('--out', type=Path, default=Path('out.mp4'))
	args = parser.parse_args()

	# ponytail: filename order; the exported AVIFs carry no capture time to sort by
	paths = sorted(args.photos)
	fill = parse_fill(args.pad) if args.pad else None
	encoder(args.out, fill is not None and len(fill) == 4)
	frames = [load(p) for p in paths]
	boxes = {0: parse_box(args.box) if args.box else pick_box(frames[0])}
	names = [p.name for p in paths]
	for fix in args.at:
		name, _, value = fix.partition('=')
		assert name in names, f'--at {name}: not one of the photos'
		boxes[names.index(name)] = parse_box(value)

	path, sizes, sources, size = plan(frames, boxes, pad=fill is not None)
	for p, (fw, fh), hit in zip(paths, sizes, path):
		print(f'{p.name}  {fw}x{fh}  anchor {hit.x:.0f},{hit.y:.0f}  scale {hit.scale:.2f}  {hit.note}')
	print(f'clip {size[0]}x{size[1]}')

	render(frames, sources, size, fill, args.out, args.fps)
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
	box = (round(tx) - 80, round(ty) - 80, 160, 160)
	ux, uy, _ = truth[1]
	# Tracked alone, then with the user boxing frame 1: its box size must carry the 0.5 scale onward.
	for boxes in ({0: box}, {0: box, 1: (round(ux) - 40, round(uy) - 40, 80, 80)}):
		path = track(frames, boxes)
		for hit, (ex, ey, es) in zip(path, truth):
			assert hit.found and abs(hit.x - ex) <= 2 and abs(hit.y - ey) <= 2, (path, truth)
			assert abs(hit.scale / es - 1) < 0.03, (path, truth)
	assert path[1].note == 'your box'

	_, _, sources, size = plan(frames, {0: box})
	crops = [place(f, src, size, None).astype(int) for f, src in zip(frames, sources)]
	for c in crops[1:]:
		assert np.abs(c - crops[0]).mean() < 8, 'locked crops of a static world must match'

	# Padded: a bigger crop that overhangs some photos; where every photo shows, they must still agree.
	_, _, sources, padded = plan(frames, {0: box}, pad=True)
	assert padded[0] * padded[1] > size[0] * size[1]
	crops = [place(f, src, padded, (0, 0, 0, 0)).astype(int) for f, src in zip(frames, sources)]
	alpha = np.stack([c[..., 3] for c in crops])
	assert (alpha == 0).any(), 'some photo should fall short of the padded crop'
	solid = (alpha == 255).all(axis=0)
	assert solid.any()
	for c in crops[1:]:
		assert np.abs(c[solid] - crops[0][solid]).mean() < 8, 'padding must not move the anchor'

	assert parse_fill('transparent') == (0, 0, 0, 0) and parse_fill('#ff8000') == (255, 128, 0)

	hidden = [Hit(0, 0, 1, True, ''), Hit(9, 9, 9, False, ''), Hit(20, 40, 2, True, '')]
	assert fill_lost(hidden, [(100, 100)] * 3)[1][:3] == (10, 20, 1.5)
	print('self-check passed')


if __name__ == '__main__':
	if len(sys.argv) == 1:
		demo()
	else:
		main()
