import argparse
import json
import subprocess
import sys
from pathlib import Path

import cv2
import numpy as np
from PIL import Image, ImageOps

MIN_SCORE = 0.5
MIN_CROP = 64


def load(path: Path) -> np.ndarray:
	with Image.open(path) as im:
		return np.asarray(ImageOps.exif_transpose(im).convert('RGB'))


def gray(rgb: np.ndarray) -> np.ndarray:
	return cv2.cvtColor(rgb, cv2.COLOR_RGB2GRAY)


def track(frames: list[np.ndarray], box: tuple[int, int, int, int]) -> list[tuple[int, int, float]]:
	x, y, w, h = box
	first = gray(frames[0])
	assert w > 0 and h > 0, 'empty box'
	assert x >= 0 and y >= 0 and x + w <= first.shape[1] and y + h <= first.shape[0], (
		'box outside first frame'
	)

	template = first[y : y + h, x : x + w]
	path = [(x + w // 2, y + h // 2, 1.0)]
	for rgb in frames[1:]:
		g = gray(rgb)
		assert g.shape[0] >= h and g.shape[1] >= w, 'anchor box larger than a frame'
		# ponytail: full-frame search; photos are seconds apart so the anchor can jump anywhere
		scores = cv2.matchTemplate(g, template, cv2.TM_CCOEFF_NORMED)
		_, score, _, (x, y) = cv2.minMaxLoc(scores)
		path.append((x + w // 2, y + h // 2, score))
		# Clouds deform, so match each frame against the last good one, never a lost one.
		if score >= MIN_SCORE:
			template = g[y : y + h, x : x + w]
	return path


def crop_rects(
	sizes: list[tuple[int, int]], anchors: list[tuple[int, int]]
) -> list[tuple[int, int, int, int]]:
	left = min(x for x, _ in anchors)
	top = min(y for _, y in anchors)
	right = min(fw - x for (fw, _), (x, _) in zip(sizes, anchors))
	bottom = min(fh - y for (_, fh), (_, y) in zip(sizes, anchors))
	# Even sizes keep yuv420p happy; flooring only shrinks the crop, so bounds still hold.
	cw = (left + right) // 2 * 2
	ch = (top + bottom) // 2 * 2
	assert cw >= MIN_CROP and ch >= MIN_CROP, (
		f'anchor wanders too far: only a {cw}x{ch} crop fits every frame'
	)

	rects = [(x - left, y - top, cw, ch) for x, y in anchors]
	for (fw, fh), (rx, ry, rw, rh) in zip(sizes, rects):
		assert 0 <= rx and 0 <= ry and rx + rw <= fw and ry + rh <= fh
	return rects


def render(frames: list[np.ndarray], rects: list[tuple[int, int, int, int]], out: Path, fps: float):
	_, _, w, h = rects[0]
	cmd = [
		'ffmpeg', '-y', '-loglevel', 'error',
		'-f', 'rawvideo', '-pix_fmt', 'rgb24', '-s', f'{w}x{h}', '-r', str(fps), '-i', '-',
		'-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '18', str(out),
	]  # fmt: skip
	with subprocess.Popen(cmd, stdin=subprocess.PIPE) as ffmpeg:
		assert ffmpeg.stdin is not None
		for rgb, (x, y, _, _) in zip(frames, rects):
			ffmpeg.stdin.write(np.ascontiguousarray(rgb[y : y + h, x : x + w]).tobytes())
		ffmpeg.stdin.close()
	assert ffmpeg.returncode == 0, f'ffmpeg exited {ffmpeg.returncode}'


def write_rects(paths: list[Path], sizes, rects, out: Path):
	rows = [
		{'file': p.name, 'x': x / fw, 'y': y / fh, 'w': w / fw, 'h': h / fh}
		for p, (fw, fh), (x, y, w, h) in zip(paths, sizes, rects)
	]
	out.write_text(json.dumps(rows, indent='\t') + '\n')


def pick_box(rgb: np.ndarray) -> tuple[int, int, int, int]:
	box = cv2.selectROI(
		'Drag a box around the anchor, then press Enter', cv2.cvtColor(rgb, cv2.COLOR_RGB2BGR)
	)
	cv2.destroyAllWindows()
	assert box[2] > 0 and box[3] > 0, 'no box drawn'
	return tuple(int(v) for v in box)


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
	sizes = [(f.shape[1], f.shape[0]) for f in frames]
	box = tuple(int(v) for v in args.box.split(',')) if args.box else pick_box(frames[0])
	assert len(box) == 4, '--box needs x,y,w,h'

	path = track(frames, box)
	for p, (fw, fh), (x, y, score) in zip(paths, sizes, path):
		flag = '' if score >= MIN_SCORE else '  LOST'
		print(f'{p.name}  {fw}x{fh}  anchor {x},{y}  score {score:.2f}{flag}')

	# A frame where the anchor is hidden can't hold it still, and its guess would shrink every crop.
	kept = [i for i, (_, _, score) in enumerate(path) if score >= MIN_SCORE]
	assert len(kept) >= 2, 'anchor found in fewer than two frames'
	if len(kept) < len(paths):
		print(f'dropped {len(paths) - len(kept)} lost frame(s)')
	paths = [paths[i] for i in kept]
	frames = [frames[i] for i in kept]
	sizes = [sizes[i] for i in kept]
	anchors = [path[i][:2] for i in kept]

	rects = crop_rects(sizes, anchors)
	_, _, cw, ch = rects[0]
	smallest = min(fw * fh for fw, fh in sizes)
	print(f'crop {cw}x{ch}, {cw * ch / smallest:.0%} of the smallest frame')

	render(frames, rects, args.out, args.fps)
	write_rects(paths, sizes, rects, args.out.with_suffix('.json'))
	print(f'wrote {args.out}')


def demo():
	rng = np.random.default_rng(0)
	world = (rng.random((1000, 1000)) * 255).astype(np.uint8)
	anchor = (500, 500)
	# Mixed orientations, anchor pushed toward opposite edges, like real handheld frames.
	windows = [(300, 350, 400, 300), (440, 200, 300, 400), (150, 380, 400, 300)]
	frames = [np.dstack([world[y : y + h, x : x + w]] * 3) for x, y, w, h in windows]
	truth = [(anchor[0] - x, anchor[1] - y) for x, y, _, _ in windows]

	box = (truth[0][0] - 20, truth[0][1] - 20, 40, 40)
	path = track(frames, box)
	assert [(x, y) for x, y, _ in path] == truth, path

	sizes = [(w, h) for _, _, w, h in windows]
	rects = crop_rects(sizes, truth)
	crops = [f[y : y + h, x : x + w] for f, (x, y, w, h) in zip(frames, rects)]
	assert all(np.array_equal(crops[0], c) for c in crops), 'locked crops of a static world must match'
	print('self-check passed')


if __name__ == '__main__':
	if len(sys.argv) == 1:
		demo()
	else:
		main()
