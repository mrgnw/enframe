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
MAX_STEP_ZOOM = 6
MAX_STEP_TURN = 15


def load(path: Path) -> np.ndarray:
	with Image.open(path) as im:
		return np.asarray(ImageOps.exif_transpose(im).convert('RGB'))


def gray(rgb: np.ndarray) -> np.ndarray:
	return cv2.cvtColor(rgb, cv2.COLOR_RGB2GRAY)


def resize(img: np.ndarray, factor: float) -> np.ndarray:
	interpolation = cv2.INTER_AREA if factor < 1 else cv2.INTER_LINEAR
	return cv2.resize(img, None, fx=factor, fy=factor, interpolation=interpolation)


def box_mask(shape, cx: float, cy: float, w: float, h: float) -> np.ndarray:
	mask = np.zeros(shape, np.uint8)
	x0, y0 = max(0, round(cx - w / 2)), max(0, round(cy - h / 2))
	mask[y0 : round(cy + h / 2), x0 : round(cx + w / 2)] = 255
	return mask


def features_in(kp, des, mask: np.ndarray):
	"""The features SIFT finds with this mask: OpenCV detects on the whole photo, then drops keypoints
	whose rounded position falls outside the mask, so filtering a whole-photo run is identical."""
	if not kp:
		return (), None
	# float32 like OpenCV's (int)(pt + 0.5f), so a keypoint on a box edge lands on the same side.
	at = (np.float32([p.pt for p in kp]) + np.float32(0.5)).astype(int)
	keep = mask[at[:, 1], at[:, 0]] > 0
	return tuple(p for p, inside in zip(kp, keep) if inside), des[keep]


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
	source: str  # tracked, mark or lost
	note: str

	@property
	def found(self) -> bool:
		return self.source != 'lost'


def plausible(M: np.ndarray) -> tuple[bool, float, float]:
	"""Photo to photo, the anchor stays near the same zoom or jumps lens; it never spins."""
	step = float(np.hypot(M[0, 0], M[1, 0]))
	turn = float(np.degrees(np.arctan2(M[1, 0], M[0, 0])))
	return 1 / MAX_STEP_ZOOM <= step <= MAX_STEP_ZOOM and abs(turn) <= MAX_STEP_TURN, step, turn


def track(frames: list[np.ndarray], marks: dict[int, tuple[float, ...]]) -> list[Hit]:
	"""Per frame: anchor position, its scale relative to the first box, and how it was found.

	marks maps a frame index to where the user put the anchor: x,y or a box x,y,w,h, plus an optional
	zoom relative to the first photo. Frame 0 needs a box; later marks fix photos the tracker lost and
	keep the zoom unless they give one.
	"""
	assert 0 in marks and len(marks[0]) == 4, 'the first photo needs a box'
	# One factor for every frame, so differing resolutions show up as scale, like a lens change.
	k = min(1.0, TRACK_EDGE / max(max(f.shape[:2]) for f in frames))
	small = [resize(gray(f), k) for f in frames]
	sift = cv2.SIFT_create()

	path: list[Hit] = []
	cx = cy = 0.0
	rw, rh = marks[0][2] * k, marks[0][3] * k
	scale = 1.0
	kp, des = None, None
	for i, g in enumerate(small):
		fh, fw = frames[i].shape[:2]
		mark = marks.get(i)
		if mark is not None:
			assert len(mark) in (2, 4, 5), f'photo {i}: a mark is x,y or x,y,w,h with an optional zoom'
			if len(mark) == 2:
				px, py = mark
			else:
				bx, by, bw, bh = mark[:4]
				assert bw > 0 and bh > 0 and bx >= 0 and by >= 0, f'empty box on photo {i}'
				assert bx + bw <= fw and by + bh <= fh, f'box outside photo {i}'
				px, py = bx + bw / 2, by + bh / 2
				# The box is where to look; it says nothing about zoom unless a zoom is given.
				rw, rh = bw * k, bh * k
			assert 0 <= px < fw and 0 <= py < fh, f'mark outside photo {i}'
			if len(mark) == 5:
				assert mark[4] > 0, f'photo {i}: zoom must be positive'
				scale = mark[4]
			cx, cy = px * k, py * k
			source, note = 'mark', 'your mark'
		else:
			kp2, des2 = sift.detectAndCompute(g, None)
			M, agreed = similarity(kp, des, kp2, des2, RANSAC_TOLERANCE * rw)
			if M is None or agreed < MIN_INLIERS:
				path.append(Hit(cx / k, cy / k, scale, 'lost', f'LOST ({agreed} features), estimated'))
				continue
			ok, step, turn = plausible(M)
			if not ok:
				note = f'LOST (fit zooms {step:.2f}x, turns {turn:+.0f} deg), estimated'
				path.append(Hit(cx / k, cy / k, scale, 'lost', note))
				continue
			scale *= step
			rw, rh = rw * step, rh * step
			cx, cy = M @ np.array([cx, cy, 1.0])
			source, note = 'tracked', f'{agreed} features'
		path.append(Hit(cx / k, cy / k, scale, source, note))
		# Clouds deform, so features come from the latest find, never from a lost frame.
		mask = box_mask(g.shape, cx, cy, rw, rh)
		# A tracked photo already has its whole-photo features; a marked one skipped that run.
		fresh = features_in(kp2, des2, mask) if source == 'tracked' else sift.detectAndCompute(g, mask)
		if i == 0:
			assert len(fresh[0]) >= MIN_INLIERS, (
				f'only {len(fresh[0])} features in the box; draw a bigger one'
			)
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


def render(
	frames,
	sources,
	size: tuple[int, int],
	fill: tuple[int, ...] | None,
	out: Path,
	fps: float,
	frames_dir: Path | None = None,
	filters: str | None = None,
):
	alpha = fill is not None and len(fill) == 4
	w, h = size
	cmd = [
		'ffmpeg', '-y', '-loglevel', 'error',
		'-f', 'rawvideo', '-pix_fmt', 'rgba' if alpha else 'rgb24', '-s', f'{w}x{h}', '-r', str(fps), '-i', '-',
		*(['-vf', filters] if filters else []), *encoder(out, alpha), str(out),
	]  # fmt: skip
	with subprocess.Popen(cmd, stdin=subprocess.PIPE) as ffmpeg:
		assert ffmpeg.stdin is not None
		# ffmpeg prints why it quit (a bad --vf, say); the exit code below reports it without a traceback.
		try:
			for i, (rgb, source) in enumerate(zip(frames, sources), start=1):
				frame = np.ascontiguousarray(place(rgb, source, size, fill))
				if frames_dir is not None:
					Image.fromarray(frame).save(frames_dir / f'{i:04d}.png')
				ffmpeg.stdin.write(frame.tobytes())
			ffmpeg.stdin.close()
		except BrokenPipeError:
			pass
	assert ffmpeg.returncode == 0, f'ffmpeg exited {ffmpeg.returncode}'


def write_rects(paths: list[Path], sizes, sources, path: list[Hit], out: Path):
	rows = [
		{
			'file': p.name,
			'x': x / fw,
			'y': y / fh,
			'w': w / fw,
			'h': h / fh,
			'scale': round(hit.scale, 4),
			'source': hit.source,
		}
		for p, (fw, fh), (x, y, w, h), hit in zip(paths, sizes, sources, path)
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


def parse_box(value: str) -> tuple[float, ...]:
	return tuple(float(v) for v in value.split(','))


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
		metavar='PHOTO=x,y[,w,h[,zoom]]',
		help='mark the anchor in a photo the tracker lost, as a point or a box to search; '
		'keeps the zoom unless given one, relative to the first photo; repeatable',
	)
	parser.add_argument(
		'--pad',
		metavar='COLOUR',
		help='let the crop overhang photos, filled with transparent, black, white or #rrggbb; '
		'omit to shrink the crop until every photo fills it',
	)
	parser.add_argument('--fps', type=float, default=4)
	parser.add_argument('--out', type=Path, default=Path('out.mp4'))
	parser.add_argument(
		'--frames',
		type=Path,
		metavar='DIR',
		help='also save each aligned photo as DIR/0001.png, 0002.png, ... (with alpha when padding is transparent)',
	)
	parser.add_argument(
		'--vf',
		metavar='FILTERS',
		help='ffmpeg video filters for the clip only, e.g. deflicker or tpad=stop_mode=clone:stop_duration=2',
	)
	args = parser.parse_args()
	if args.frames:
		args.frames.mkdir(parents=True, exist_ok=True)
		assert not any(args.frames.glob('*.png')), f'--frames {args.frames}: already has PNGs; empty it first'

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

	render(frames, sources, size, fill, args.out, args.fps, args.frames, args.vf)
	write_rects(paths, sizes, sources, path, args.out.with_suffix('.json'))
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
	for marks in ({0: box}, {0: box, 1: (round(ux) - 40, round(uy) - 40, 80, 80, 0.5)}):
		path = track(frames, marks)
		for hit, (ex, ey, es) in zip(path, truth):
			assert hit.found and abs(hit.x - ex) <= 2 and abs(hit.y - ey) <= 2, (path, truth)
			assert abs(hit.scale / es - 1) < 0.03, (path, truth)
	assert [hit.source for hit in path] == ['mark', 'mark', 'tracked'], path

	# A point on a photo at the same zoom moves the anchor there and keeps the scale.
	same_zoom = [frames[0], np.dstack([world[400:700, 450:850]] * 3)]
	for mark in ((150, 200), (100, 150, 100, 100)):
		path = track(same_zoom, {0: box, 1: mark})
		assert path[1][:3] == (150, 200, 1.0) and path[1].note == 'your mark', path

	# A fit that spins or zooms 10x between two photos is a false match, not a camera move.
	assert not plausible(np.float64([[10, 0, 0], [0, 10, 0]]))[0]
	assert not plausible(np.float64([[0.7, -0.7, 0], [0.7, 0.7, 0]]))[0]
	assert plausible(np.float64([[2.5, 0.05, 0], [-0.05, 2.5, 0]]))[0]

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

	hidden = [Hit(0, 0, 1, 'tracked', ''), Hit(9, 9, 9, 'lost', ''), Hit(20, 40, 2, 'mark', '')]
	filled = fill_lost(hidden, [(100, 100)] * 3)
	assert filled[1][:4] == (10, 20, 1.5, 'lost'), 'an estimated photo must stay marked lost'
	print('self-check passed')


if __name__ == '__main__':
	if len(sys.argv) == 1:
		demo()
	else:
		main()
