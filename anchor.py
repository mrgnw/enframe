import argparse
import json
import math
import os
import shutil
import subprocess
import sys
from collections.abc import Iterable, Iterator
from concurrent.futures import ThreadPoolExecutor
from itertools import repeat
import tempfile
import threading
from pathlib import Path
from typing import NamedTuple, NoReturn

import cv2
import numpy as np
from PIL import ExifTags, Image, ImageOps, ImageSequence

MIN_INLIERS = 8
MIN_CROP = 64
TRACK_EDGE = 1024
RATIO_TEST = 0.8
RANSAC_TOLERANCE = 0.03
MAX_STEP_ZOOM = 6
MAX_STEP_TURN = 15
RENDER_WORKERS = 8
# On the M4 Pro (8 performance + 4 efficiency cores), Girona decode + SIFT: 4 threads 1.6 s,
# 6 threads 1.5 s, 8 threads 1.6 s with more memory; one photo at a time 2.3 s.
PREPARE_WORKERS = 6
# Results queued ahead of the consumer: results come back in order, so a queue no deeper than the
# workers idles them behind one slow photo. Queued results are small (features, or one clip frame).
PREPARE_AHEAD = 3 * PREPARE_WORKERS
RENDER_AHEAD = 3 * RENDER_WORKERS
# Share of physical RAM for decoded photos kept from the first pass, so render need not decode them again.
KEEP_SHARE = 0.25
# libaom's fastest preset; on the Girona clip it encodes as fast as ProRes at 1/12 the size, 40 dB PSNR.
AVIF_SPEED = 10
AVIF_QUALITY = 75
CLIP_SUFFIXES = ('.avif', '.mp4', '.mov', '.webm')


def fail(message: str) -> NoReturn:
	"""Stop on input or photos pinframe cannot use: a message and exit code 1, not a traceback.
	Asserts stay for invariants; checks of what the user gave must survive python -O."""
	raise SystemExit(f'pinframe: {message}')


def load(path: Path) -> np.ndarray:
	with Image.open(path) as im:
		return np.asarray(ImageOps.exif_transpose(im).convert('RGB'))


def oriented_size(path: Path) -> tuple[int, int]:
	"""Width and height after EXIF rotation, as load() returns it, read from the header alone."""
	with Image.open(path) as im:
		w, h = im.size
		turned = im.getexif().get(ExifTags.Base.Orientation, 1) in (5, 6, 7, 8)
	return (h, w) if turned else (w, h)


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


class Photo(NamedTuple):
	"""What tracking needs from a photo. The pixels are dropped; render decodes the photo again."""

	small_shape: tuple[int, ...]  # the downscaled gray image SIFT ran on
	keypoints: tuple
	descriptors: np.ndarray | None


def track_factor(sizes) -> float:
	"""One downscale for every photo, so differing resolutions show up as scale, like a lens change."""
	return min(1.0, TRACK_EDGE / max(max(size) for size in sizes))


def prepare(rgb: np.ndarray, k: float) -> Photo:
	small = resize(gray(rgb), k)
	# A fresh SIFT per call, because photos are prepared on several threads at once.
	kp, des = cv2.SIFT_create().detectAndCompute(small, None)
	return Photo(small.shape, kp, des)


class Keep:
	"""Decoded photos held from the first pass for render, within a byte budget shared by the threads.
	A photo that does not fit is decoded again at render, so memory stays bounded for any photo count."""

	def __init__(self, budget: int):
		self.budget = budget
		self.photos: dict[Path, np.ndarray] = {}
		self.lock = threading.Lock()

	def offer(self, path: Path, rgb: np.ndarray):
		with self.lock:
			if rgb.nbytes <= self.budget:
				self.budget -= rgb.nbytes
				self.photos[path] = rgb

	def take(self, path: Path) -> np.ndarray:
		with self.lock:
			rgb = self.photos.pop(path, None)
		return load(path) if rgb is None else rgb


def physical_memory() -> int:
	return os.sysconf('SC_PAGE_SIZE') * os.sysconf('SC_PHYS_PAGES')


def prepare_file(path: Path, size: tuple[int, int], k: float, keep: Keep) -> Photo:
	rgb = load(path)
	assert (rgb.shape[1], rgb.shape[0]) == size, f'{path.name}: decoded {rgb.shape} but header says {size}'
	keep.offer(path, rgb)
	return prepare(rgb, k)


def load_photos(paths: list[Path], sizes, k: float, keep: Keep) -> Iterator[Photo]:
	"""Decode and find features photo by photo across threads, yielding them in order. AVIF decoding and
	SIFT each keep only about half the cores busy, so one photo's decode beside another's SIFT fills the
	rest. buffersize caps the photos in flight, so memory does not grow with the number of photos."""
	with ThreadPoolExecutor(PREPARE_WORKERS) as pool:
		yield from pool.map(prepare_file, paths, sizes, repeat(k), repeat(keep), buffersize=PREPARE_AHEAD)


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


def track(photos: Iterable[Photo], sizes, k: float, marks: dict[int, tuple[float, ...]]) -> list[Hit]:
	"""Per photo: anchor position, its scale relative to the first box, and how it was found.

	Photos are consumed one at a time, in order. sizes are their full-resolution sizes and k the downscale
	their features were found at. marks maps a photo index to where the user put the anchor: x,y or a box
	x,y,w,h, plus an optional zoom relative to the first photo. Photo 0 needs a box; later marks fix
	photos the tracker lost and keep the zoom unless they give one.
	"""
	assert 0 in marks and len(marks[0]) == 4, 'the first photo needs a box'
	path: list[Hit] = []
	cx = cy = 0.0
	rw, rh = marks[0][2] * k, marks[0][3] * k
	scale = 1.0
	kp, des = None, None
	for i, (photo, (fw, fh)) in enumerate(zip(photos, sizes, strict=True)):
		kp2, des2 = photo.keypoints, photo.descriptors
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
		fresh = features_in(kp2, des2, box_mask(photo.small_shape, cx, cy, rw, rh))
		if i == 0 and len(fresh[0]) < MIN_INLIERS:
			fail(f'only {len(fresh[0])} features in the first box; draw a bigger one')
		if len(fresh[0]) >= MIN_INLIERS:
			kp, des = fresh
	return path


def fill_lost(path: list[Hit], sizes) -> list[Hit]:
	"""Estimate hidden anchors from the nearest frames where it was found."""
	good = [i for i, hit in enumerate(path) if hit.found]
	if len(good) < 2:
		fail('the anchor was found in fewer than two photos; mark more of them with --at')
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
	if cw < MIN_CROP or ch < MIN_CROP:
		fail(
			f'the anchor wanders too far: only a {cw}x{ch} crop fits every photo; mark lost photos or use --pad'
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
	assert not alpha, 'transparent padding needs a .avif, .mov (ProRes 4444) or .webm (VP9) output'
	return ['-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '18']


class StreamedClip(Image.Image):
	"""A multi-frame image whose frames are pulled from an iterator as the encoder asks for them.

	Pillow's save_all seeks frame by frame and calls tobytes() on each, so the AVIF encoder holds one
	frame at a time instead of the whole clip. seek() only moves the index, because Pillow also walks
	every frame once to check their modes before encoding.
	"""

	def __init__(self, frames: Iterable[np.ndarray], count: int, size: tuple[int, int], mode: str):
		super().__init__()
		self._frames = iter(frames)
		self._mode, self._size = mode, size
		self.n_frames, self.is_animated = count, count > 1
		self._index = self._pulled = 0

	def seek(self, frame: int) -> None:
		if frame >= self.n_frames:
			raise EOFError
		self._index = frame

	def tell(self) -> int:
		return self._index

	def tobytes(self, encoder_name: str = 'raw', *args) -> bytes:
		assert encoder_name == 'raw' and self._index == self._pulled, 'frames must be encoded once, in order'
		self._pulled += 1
		frame = next(self._frames)
		assert frame.shape[:2] == (self.size[1], self.size[0]) and frame.shape[2] == len(self.mode)
		return frame.tobytes()


def write_avif(
	frames: Iterable[np.ndarray], count: int, size: tuple[int, int], alpha: bool, out: Path, fps: float
):
	"""Animated AVIF through Pillow's libavif: small, keeps transparency, and needs no ffmpeg."""
	clip = StreamedClip(frames, count, size, 'RGBA' if alpha else 'RGB')
	clip.save(out, 'AVIF', save_all=True, duration=round(1000 / fps), speed=AVIF_SPEED, quality=AVIF_QUALITY)
	assert clip._pulled == count, f'encoded {clip._pulled} of {count} frames'


def pipe_to_ffmpeg(frames, size: tuple[int, int], alpha: bool, out: Path, fps: float, filters: str | None):
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
			for frame in frames:
				ffmpeg.stdin.write(frame.tobytes())
			ffmpeg.stdin.close()
		except BrokenPipeError:
			pass
	if ffmpeg.returncode != 0:
		fail(f'ffmpeg exited {ffmpeg.returncode}')


def render(
	paths: list[Path],
	sources,
	size: tuple[int, int],
	fill: tuple[int, ...] | None,
	out: Path,
	fps: float,
	keep: Keep,
	frames_dir: Path | None = None,
	filters: str | None = None,
):
	"""Place each photo, kept or decoded again, and hand it on in order, holding at most RENDER_AHEAD frames."""
	alpha = fill is not None and len(fill) == 4

	def finish(i: int, path: Path, source) -> np.ndarray:
		frame = np.ascontiguousarray(place(keep.take(path), source, size, fill))
		if frames_dir is not None:
			Image.fromarray(frame).save(frames_dir / f'{i:04d}.png')
		return frame

	# libavif, zlib and OpenCV release the GIL, so photos are decoded, placed and saved in parallel.
	with ThreadPoolExecutor(RENDER_WORKERS) as pool:
		placed = pool.map(finish, range(1, len(paths) + 1), paths, sources, buffersize=RENDER_AHEAD)
		if out.suffix == '.avif':
			write_avif(placed, len(paths), size, alpha, out, fps)
		else:
			pipe_to_ffmpeg(placed, size, alpha, out, fps, filters)


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
	if box[2] <= 0 or box[3] <= 0:
		fail('no box drawn')
	return tuple(int(v) for v in box)


def plan(photos: Iterable[Photo], sizes, k: float, boxes, pad: bool = False):
	path = fill_lost(track(photos, sizes, k, boxes), sizes)
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


def parse_numbers(flag: str, text: str) -> tuple[float, ...]:
	try:
		return tuple(float(v) for v in text.split(','))
	except ValueError:
		fail(f'{flag} {text}: expected comma-separated numbers')


def check_mark(name: str, mark: tuple[float, ...], size: tuple[int, int]):
	"""A mark is a point x,y or a box x,y,w,h with an optional zoom, inside its photo."""
	fw, fh = size
	if len(mark) not in (2, 4, 5):
		fail(f'{name}: a mark is x,y or x,y,w,h with an optional zoom, not {len(mark)} numbers')
	if len(mark) == 2:
		px, py = mark
	else:
		bx, by, bw, bh = mark[:4]
		if not (bw > 0 and bh > 0 and bx >= 0 and by >= 0 and bx + bw <= fw and by + bh <= fh):
			fail(f'{name}: box {bx:g},{by:g},{bw:g},{bh:g} is empty or outside the {fw}x{fh} photo')
		px, py = bx + bw / 2, by + bh / 2
	if not (0 <= px < fw and 0 <= py < fh):
		fail(f'{name}: point {px:g},{py:g} is outside the {fw}x{fh} photo')
	if len(mark) == 5 and not mark[4] > 0:
		fail(f'{name}: zoom must be positive')


def read_sizes(paths: list[Path]) -> list[tuple[int, int]]:
	"""Every photo's size from its header, so bad input stops before any decoding."""
	if len(paths) < 2:
		fail('need at least two photos')
	names = [p.name for p in paths]
	if len(set(names)) < len(names):
		fail('two photos share a file name, so --at and the .json could not tell them apart')
	sizes = []
	for path in paths:
		try:
			sizes.append(oriented_size(path))
		except (OSError, Image.DecompressionBombError) as error:
			fail(f'{path}: {error}')
	return sizes


def read_marks(box: str | None, ats: list[str], paths: list[Path], sizes) -> dict[int, tuple[float, ...]]:
	"""--box (or a drawn box) and every --at, by photo index, checked against the photo sizes."""
	names = [p.name for p in paths]
	first = parse_numbers('--box', box) if box else pick_box(load(paths[0]))
	if len(first) != 4:
		fail('--box is x,y,w,h')
	marks = {0: first}
	for at in ats:
		name, sep, value = at.partition('=')
		if not sep or name not in names:
			fail(f'--at {at}: expected PHOTO=x,y with PHOTO one of the photo file names')
		i = names.index(name)
		if i in marks:
			fail(f'--at {name}: ' + ('use --box for the first photo' if i == 0 else 'marked twice'))
		marks[i] = parse_numbers(f'--at {name}', value)
	for i, mark in marks.items():
		check_mark(names[i], mark, sizes[i])
	return marks


def check_output(out: Path, alpha: bool, filters: str | None):
	if out.suffix not in CLIP_SUFFIXES:
		fail(f'--out {out}: use one of ' + ', '.join(CLIP_SUFFIXES))
	if not out.parent.is_dir():
		fail(f'--out {out}: folder {out.parent} does not exist')
	if out.suffix == '.avif':
		if filters:
			fail('--vf runs through ffmpeg: use a .mp4, .mov or .webm --out')
		return
	if shutil.which('ffmpeg') is None:
		fail(f'--out {out.suffix} needs ffmpeg on PATH; .avif does not')
	if alpha and out.suffix == '.mp4':
		fail('transparent padding needs a .avif, .mov or .webm --out')


def partial(path: Path) -> Path:
	"""A hidden name beside path, keeping its extension for the encoder, to write and then rename."""
	return path.with_name(f'.{path.stem}.partial{path.suffix}')


def parse_fill(value: str) -> tuple[int, ...]:
	if value == 'transparent':
		return (0, 0, 0, 0)
	hex_value = {'black': '000000', 'white': 'ffffff'}.get(value, value.removeprefix('#'))
	if len(hex_value) == 6:
		try:
			return tuple(int(hex_value[i : i + 2], 16) for i in (0, 2, 4))
		except ValueError:
			pass
	fail(f'--pad {value}: use transparent, black, white or a #rrggbb colour')


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
	parser.add_argument(
		'--out',
		type=Path,
		default=Path('out.avif'),
		help='the clip; its extension picks the format: .avif (animated AVIF), or through ffmpeg .mp4 (H.264), '
		'.mov (ProRes 4444 when transparent) or .webm (VP9)',
	)
	parser.add_argument(
		'--frames',
		type=Path,
		metavar='DIR',
		help='also save each aligned photo as DIR/0001.png, 0002.png, ... (with alpha when padding is transparent)',
	)
	parser.add_argument(
		'--vf',
		metavar='FILTERS',
		help='ffmpeg video filters for a .mp4, .mov or .webm clip, e.g. deflicker or '
		'tpad=stop_mode=clone:stop_duration=2',
	)
	args = parser.parse_args()

	# ponytail: filename order; the exported AVIFs carry no capture time to sort by
	paths = sorted(args.photos)
	fill = parse_fill(args.pad) if args.pad else None
	check_output(args.out, fill is not None and len(fill) == 4, args.vf)
	if not args.fps > 0:
		fail('--fps must be positive')
	sizes = read_sizes(paths)
	marks = read_marks(args.box, args.at, paths, sizes)
	if args.frames:
		args.frames.mkdir(parents=True, exist_ok=True)
		if any(args.frames.glob('*.png')):
			fail(f'--frames {args.frames}: already has PNGs; empty it first')

	k = track_factor(sizes)
	keep = Keep(int(physical_memory() * KEEP_SHARE))
	path, sizes, sources, size = plan(
		load_photos(paths, sizes, k, keep), sizes, k, marks, pad=fill is not None
	)
	for p, (fw, fh), hit in zip(paths, sizes, path):
		print(f'{p.name}  {fw}x{fh}  anchor {hit.x:.0f},{hit.y:.0f}  scale {hit.scale:.2f}  {hit.note}')
	print(f'clip {size[0]}x{size[1]}')

	# Written under hidden names and renamed together, so a failed run leaves the previous clip and its
	# .json as they were instead of a partial clip beside a stale .json.
	rects = args.out.with_suffix('.json')
	clip_partial, rects_partial = partial(args.out), partial(rects)
	try:
		render(paths, sources, size, fill, clip_partial, args.fps, keep, args.frames, args.vf)
		write_rects(paths, sizes, sources, path, rects_partial)
		clip_partial.replace(args.out)
		rects_partial.replace(rects)
	finally:
		clip_partial.unlink(missing_ok=True)
		rects_partial.unlink(missing_ok=True)
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

	def prepared(images: list[np.ndarray]) -> tuple[list[Photo], list[tuple[int, int]], float]:
		sizes = [(image.shape[1], image.shape[0]) for image in images]
		k = track_factor(sizes)
		return [prepare(image, k) for image in images], sizes, k

	tx, ty, _ = truth[0]
	box = (round(tx) - 80, round(ty) - 80, 160, 160)
	ux, uy, _ = truth[1]
	# Tracked alone, then with the user boxing frame 1: its box size must carry the 0.5 scale onward.
	for marks in ({0: box}, {0: box, 1: (round(ux) - 40, round(uy) - 40, 80, 80, 0.5)}):
		path = track(*prepared(frames), marks)
		for hit, (ex, ey, es) in zip(path, truth):
			assert hit.found and abs(hit.x - ex) <= 2 and abs(hit.y - ey) <= 2, (path, truth)
			assert abs(hit.scale / es - 1) < 0.03, (path, truth)
	assert [hit.source for hit in path] == ['mark', 'mark', 'tracked'], path

	# A point on a photo at the same zoom moves the anchor there and keeps the scale.
	same_zoom = [frames[0], np.dstack([world[400:700, 450:850]] * 3)]
	for mark in ((150, 200), (100, 150, 100, 100)):
		path = track(*prepared(same_zoom), {0: box, 1: mark})
		assert path[1][:3] == (150, 200, 1.0) and path[1].note == 'your mark', path

	# A fit that spins or zooms 10x between two photos is a false match, not a camera move.
	assert not plausible(np.float64([[10, 0, 0], [0, 10, 0]]))[0]
	assert not plausible(np.float64([[0.7, -0.7, 0], [0.7, 0.7, 0]]))[0]
	assert plausible(np.float64([[2.5, 0.05, 0], [-0.05, 2.5, 0]]))[0]

	_, _, sources, size = plan(*prepared(frames), {0: box})
	crops = [place(f, src, size, None).astype(int) for f, src in zip(frames, sources)]
	for c in crops[1:]:
		assert np.abs(c - crops[0]).mean() < 8, 'locked crops of a static world must match'

	# Padded: a bigger crop that overhangs some photos; where every photo shows, they must still agree.
	_, _, sources, padded = plan(*prepared(frames), {0: box}, pad=True)
	assert padded[0] * padded[1] > size[0] * size[1]
	crops = [place(f, src, padded, (0, 0, 0, 0)).astype(int) for f, src in zip(frames, sources)]
	alpha = np.stack([c[..., 3] for c in crops])
	assert (alpha == 0).any(), 'some photo should fall short of the padded crop'
	solid = (alpha == 255).all(axis=0)
	assert solid.any()
	for c in crops[1:]:
		assert np.abs(c[solid] - crops[0][solid]).mean() < 8, 'padding must not move the anchor'

	with tempfile.TemporaryDirectory() as tmp:
		# Photos prepared across threads get exactly the features one thread would find.
		paths = [Path(tmp) / f'{i}.png' for i in range(len(frames))]
		for frame, p in zip(frames, paths):
			Image.fromarray(frame).save(p)
		sizes = [oriented_size(p) for p in paths]
		k = track_factor(sizes)
		pooled = load_photos(paths, sizes, k, Keep(0))
		for photo, frame in zip(pooled, frames):
			alone = prepare(frame, k)
			assert [p.pt for p in photo.keypoints] == [p.pt for p in alone.keypoints]
			assert np.array_equal(photo.descriptors, alone.descriptors)

		# Keep holds photos only within its budget; the rest are decoded again when taken.
		keep = Keep(frames[0].nbytes)
		keep.offer(paths[0], frames[0])
		keep.offer(paths[1], frames[1])
		assert keep.take(paths[0]) is frames[0] and np.array_equal(keep.take(paths[1]), frames[1])
		assert not keep.photos, 'taking a photo releases it'

		# The default output streams frames in, and keeps every frame, the frame rate and the transparency.
		clip = Path(tmp) / 'clip.avif'
		streamed = (place(f, src, padded, (0, 0, 0, 0)) for f, src in zip(frames, sources))
		write_avif(streamed, len(frames), padded, True, clip, 4)
		with Image.open(clip) as im:
			assert im.n_frames == len(frames) and im.mode == 'RGBA', (im.n_frames, im.mode)
			alpha = [np.asarray(frame)[..., 3] for frame in ImageSequence.Iterator(im)]
			assert im.info['duration'] == 250, im.info
			assert min(a.min() for a in alpha) < 16, 'transparent padding must survive encoding'

	assert parse_fill('transparent') == (0, 0, 0, 0) and parse_fill('#ff8000') == (255, 128, 0)

	# Bad input stops with a message, never a traceback, and good input passes.
	def stops(action) -> bool:
		try:
			action()
		except SystemExit as stop:
			return str(stop).startswith('pinframe: ')
		return False

	photo = (100, 80)
	for mark in ((1, 2, 3), (10, 10, 0, 5), (90, 70, 20, 20), (100, 40), (-1, 40), (10, 10, 20, 20, 0)):
		assert stops(lambda: check_mark('p.avif', mark, photo)), mark
	assert stops(lambda: check_mark('p.avif', (float('nan'), 40), photo))
	check_mark('p.avif', (99, 79), photo)
	check_mark('p.avif', (0, 0, 100, 80, 2.5), photo)
	assert stops(lambda: parse_numbers('--box', '1,2,x'))
	for colour in ('purple', '#12345', '#gggggg', '#1234567'):
		assert stops(lambda: parse_fill(colour)), colour
	assert stops(lambda: check_output(Path('clip.gif'), False, None))
	assert stops(lambda: check_output(Path('clip.avif'), False, 'deflicker'))
	assert stops(lambda: check_output(Path('no/such/folder/clip.avif'), False, None))
	assert stops(lambda: read_sizes([Path('only-one.avif')]))
	assert stops(lambda: read_sizes([Path('a/x.avif'), Path('b/x.avif')]))
	assert stops(lambda: read_sizes([Path('missing-1.avif'), Path('missing-2.avif')]))
	two = [Path('a.avif'), Path('b.avif')]
	for ats in (['b.avif'], ['c.avif=1,1'], ['a.avif=1,1'], ['b.avif=1,1', 'b.avif=2,2'], ['b.avif=500,1']):
		assert stops(lambda: read_marks('0,0,10,10', ats, two, [photo, photo])), ats
	assert read_marks('0,0,10,10', ['b.avif=5,5'], two, [photo, photo]) == {0: (0, 0, 10, 10), 1: (5, 5)}

	hidden = [Hit(0, 0, 1, 'tracked', ''), Hit(9, 9, 9, 'lost', ''), Hit(20, 40, 2, 'mark', '')]
	filled = fill_lost(hidden, [(100, 100)] * 3)
	assert filled[1][:4] == (10, 20, 1.5, 'lost'), 'an estimated photo must stay marked lost'
	print('self-check passed')


if __name__ == '__main__':
	if len(sys.argv) == 1:
		demo()
	else:
		main()
