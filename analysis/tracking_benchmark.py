# /// script
# requires-python = '>=3.14'
# dependencies = ['numpy', 'pillow']
# ///
"""Score tracking: accuracy against known truth, and stability under tiny pixel changes.

uv run analysis/tracking_benchmark.py [REPORT]

Accuracy: synthetic sequences warped from Girona photos with known shifts, crops, zooms (2-3x jumps like
lens switches) and small rotations, so the true anchor in every frame is exact. Reports anchor error in
frame pixels, scale error in percent, and LOST frames.

Stability: the real Girona job (analysis/girona_all.sh marks) on the AVIF originals and on copies with
slightly different pixels. Reports how far each photo's anchor moves, in original pixels, and whether the
LOST set changes.

Runs the Rust CLI in JSON-only mode with a temporary --cache. Prints one summary line and writes the
report (default out/tracking_benchmark.json). analysis/tracking_baseline.json is the committed report for
main.
"""

import json
import math
import re
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np
from PIL import Image, ImageOps

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / 'rust' / 'target' / 'release' / 'pinframe'
GIRONA = ROOT / 'samples' / 'girona clouds'
GIRONA_JOB = ROOT / 'analysis' / 'girona_all.sh'
DEFAULT_REPORT = ROOT / 'out' / 'tracking_benchmark.json'

# Synthetic frames: 1024 px on the long edge, so the tracker runs SIFT at full frame size (TRACK_EDGE).
FRAME = (1024, 768)
# Noise variant: uniform integer noise in [-2, 2] levels, the size of the ImageIO-vs-libavif difference
# documented in docs/rust.md. Fixed seed so the baseline is reproducible.
NOISE_LEVELS = 2
NOISE_SEED = 0
TOP_MOVERS = 5

# Each sequence: a Girona photo, the anchor box in it (from girona_all.sh or check.sh marks), and frames as
# (zoom from the photo, rotation in degrees, anchor offset from the frame centre in frame px). Frames that
# would leave the photo are shifted back inside; the truth follows the actual frame.
SEQUENCES = {
	'shift': (
		'IMG_4698.avif',
		(840, 550, 400, 350),
		[
			(1, 0, (0, 0)),
			(1, 0, (150, 40)),
			(1, 0, (-120, 90)),
			(1, 0, (200, -120)),
			(1, 0, (-60, -150)),
			(1, 0, (30, 60)),
		],
	),
	'crop': (
		'IMG_4713.avif',
		(1080, 570, 320, 300),
		[
			(1, 0, (0, 0), (1024, 768)),
			(1, 0, (40, -30), (800, 800)),
			(1, 0, (-80, 20), (640, 480)),
			(1, 0, (60, 50), (1024, 600)),
			(1, 0, (-20, -40), (560, 720)),
		],
	),
	'zoom': (
		'IMG_4791.avif',
		(520, 220, 620, 500),
		[
			(0.5, 0, (0, 0)),
			(1.0, 0, (40, 30)),
			(1.0, 0, (-30, 10)),
			(0.5, 0, (0, 0)),
			(1.5, 0, (20, -20)),
			(0.5, 0, (0, 0)),
			(0.75, 0, (-50, 40)),
			(1.5, 0, (0, 0)),
		],
	),
	'rotate': (
		'IMG_4716.avif',
		(650, 670, 400, 150),
		[
			(1, 0, (0, 0)),
			(1, 3, (30, 20)),
			(1, -2, (-40, 10)),
			(1, 5, (20, -30)),
			(1, -5, (0, 40)),
			(1, 8, (-30, -20)),
		],
	),
	'mixed': (
		'IMG_4698.avif',
		(840, 550, 400, 350),
		[
			(0.6, 0, (0, 0)),
			(1.2, 4, (60, -40)),
			(1.8, -3, (-30, 30)),
			(0.7, 2, (40, 20)),
			(2.0, -4, (0, 0)),
			(0.8, 0, (-50, -30)),
		],
	),
}


def box_points(box: tuple[float, float, float, float]) -> str:
	"""A box's four quarter centres as a --points/--at point list (A5b), mean unchanged at the box centre."""
	x, y, w, h = box
	corners = [
		(x + w / 4, y + h / 4),
		(x + 3 * w / 4, y + h / 4),
		(x + w / 4, y + 3 * h / 4),
		(x + 3 * w / 4, y + 3 * h / 4),
	]
	return '/'.join(f'{px:.3f},{py:.3f}' for px, py in corners)


def run_pinframe(photos: list[Path], marks: list[str], work: Path) -> list[dict]:
	out = work / 'track.json'
	cmd = [str(BIN), *map(str, photos), *marks, '--cache', str(work / 'cache'), '--out', str(out)]
	subprocess.run(cmd, check=True, capture_output=True, text=True)
	return json.loads(out.read_text())['photos']


def anchor_px(row: dict, fw: int, fh: int) -> tuple[float, float]:
	"""The anchor, the mean of the photo's points, in photo pixels."""
	xs, ys = zip(*row['points'], strict=True)
	return sum(xs) / len(xs) * fw, sum(ys) / len(ys) * fh


def oriented(path: Path) -> Image.Image:
	with Image.open(path) as im:
		return ImageOps.exif_transpose(im).convert('RGB')


# --- accuracy ---


def frame_transform(spec: tuple, base_size: tuple[int, int], anchor: tuple[float, float]):
	"""The frame's size and its map from photo to frame, continuous coordinates (pixel centres at +0.5)."""
	zoom, degrees, (dx, dy) = spec[:3]
	bw, bh = base_size
	# Default frames take the photo's orientation.
	w, h = spec[3] if len(spec) > 3 else (FRAME if bw >= bh else FRAME[::-1])
	t = math.radians(degrees)
	cos, sin = math.cos(t), math.sin(t)
	# Photo point at the frame centre, chosen so the anchor sits (dx, dy) from it, then kept inside the photo.
	ax, ay = anchor[0] + 0.5, anchor[1] + 0.5
	cx = ax - (cos * dx + sin * dy) / zoom
	cy = ay - (-sin * dx + cos * dy) / zoom
	hx = (w / 2 * abs(cos) + h / 2 * abs(sin)) / zoom
	hy = (w / 2 * abs(sin) + h / 2 * abs(cos)) / zoom
	assert 2 * hx <= bw and 2 * hy <= bh, f'frame {spec} does not fit a {bw}x{bh} photo'
	cx = min(max(cx, hx), bw - hx)
	cy = min(max(cy, hy), bh - hy)

	def forward(qx: float, qy: float) -> tuple[float, float]:
		ux, uy = qx - cx, qy - cy
		return w / 2 + zoom * (cos * ux - sin * uy), h / 2 + zoom * (sin * ux + cos * uy)

	return (w, h), zoom, cos, sin, (cx, cy), forward


def render_frame(base: Image.Image, half: Image.Image, spec: tuple, anchor: tuple[float, float]):
	"""One synthetic frame and its true anchor (pixel index coordinates, as the tracker reports)."""
	(w, h), zoom, cos, sin, (cx, cy), forward = frame_transform(spec, base.size, anchor)
	# Below 0.75x, sample a box-filtered half-size photo so the zoom-out does not alias.
	src, f = (half, 0.5) if zoom <= 0.75 else (base, 1.0)
	z = zoom / f
	# Inverse map, frame to photo: q = c + R^-1 (p - frame centre) / zoom, then into the source's scale.
	a, b = cos / z, sin / z
	d, e = -sin / z, cos / z
	c0 = cx * f - a * w / 2 - b * h / 2
	f0 = cy * f - d * w / 2 - e * h / 2
	frame = src.transform((w, h), Image.Transform.AFFINE, (a, b, c0, d, e, f0), Image.Resampling.BICUBIC)
	tx, ty = forward(anchor[0] + 0.5, anchor[1] + 0.5)
	return frame, (tx - 0.5, ty - 0.5), zoom


def accuracy_sequence(name: str, tmp: Path) -> dict:
	photo, (bx, by, bw, bh), specs = SEQUENCES[name]
	base = oriented(GIRONA / photo)
	half = base.reduce(2)
	anchor = (bx + bw / 2, by + bh / 2)
	work = tmp / f'seq-{name}'
	work.mkdir()
	paths, truths, zooms, sizes = [], [], [], []
	for i, spec in enumerate(specs):
		frame, truth, zoom = render_frame(base, half, spec, anchor)
		path = work / f'f{i:02}.png'
		frame.save(path, compress_level=1)
		paths.append(path)
		truths.append(truth)
		zooms.append(zoom)
		sizes.append(frame.size)
	z0 = zooms[0]
	box = (truths[0][0] - bw * z0 / 2, truths[0][1] - bh * z0 / 2, bw * z0, bh * z0)
	rows = run_pinframe(paths, ['--points', box_points(box)], work)
	frames = []
	for row, truth, zoom, (fw, fh) in zip(rows, truths, zooms, sizes, strict=True):
		found = anchor_px(row, fw, fh)
		frames.append(
			{
				'file': row['file'],
				'source': row['source'],
				'truth': [round(v, 3) for v in truth],
				'found': [round(v, 3) for v in found],
				'error_px': round(math.dist(found, truth), 3),
				'scale_true': round(zoom / z0, 4),
				'scale_found': row['scale'],
				'scale_error_pct': round(abs(row['scale'] / (zoom / z0) - 1) * 100, 3),
			}
		)
	return {'photo': photo, 'box': [bx, by, bw, bh], 'frames': frames}


def accuracy_summary(sequences: dict) -> dict:
	# Frame 0 is the mark itself; LOST frames are estimates, counted, not scored.
	tracked = [f for s in sequences.values() for f in s['frames'][1:] if f['source'] == 'tracked']
	scored = [f for s in sequences.values() for f in s['frames'][1:]]
	errors = [f['error_px'] for f in tracked]
	return {
		'frames': len(scored),
		'lost': sum(f['source'] == 'lost' for f in scored),
		'error_px_median': round(float(np.median(errors)), 3) if errors else None,
		'error_px_max': max(errors, default=None),
		'scale_error_pct_max': max((f['scale_error_pct'] for f in tracked), default=None),
	}


# --- stability ---


def girona_marks(ext: str) -> list[str]:
	"""--points, --at marks and --pad default from analysis/girona_all.sh, with the photo extension swapped."""
	text = GIRONA_JOB.read_text()
	points = re.search(r'--points (\S+)', text).group(1)
	ats = re.findall(r'^\s*--at (IMG_\d+)\.avif=(\S+)$', text, re.M)
	assert ats, 'no --at marks found in girona_all.sh'
	pad = re.search(r'PAD=\$\{PAD-(\w+)\}', text).group(1)
	marks = ['--points', points, '--pad', pad]
	for stem, value in ats:
		marks += ['--at', f'{stem}.{ext}={value}']
	return marks


def sips(fmt: str, options: list[str], ext: str):
	def convert(src: Path, dst_dir: Path) -> None:
		dst = dst_dir / f'{src.stem}.{ext}'
		subprocess.run(
			['sips', '-s', 'format', fmt, *options, str(src), '--out', str(dst)],
			check=True,
			capture_output=True,
		)

	return convert


def imageio_avif(src: Path, dst_dir: Path) -> None:
	"""The AVIF bytes with 'avif' brands relabelled 'mif1', so pinframe decodes them through ImageIO."""
	data = bytearray(src.read_bytes())
	head = data[:32].replace(b'avif', b'mif1')
	assert head != data[:32], f'{src.name}: no avif brand in ftyp'
	data[:32] = head
	(dst_dir / f'{src.stem}.heic').write_bytes(data)


def pillow_png(noise: bool):
	def convert(src: Path, dst_dir: Path) -> None:
		with Image.open(src) as original:
			exif = original.info.get('exif')
			pixels = np.asarray(ImageOps.exif_transpose(original).convert('RGB'), dtype=np.int16)
		if noise:
			rng = np.random.default_rng([NOISE_SEED, int(src.stem.removeprefix('IMG_'))])
			pixels = pixels + rng.integers(-NOISE_LEVELS, NOISE_LEVELS + 1, pixels.shape, dtype=np.int16)
		Image.fromarray(np.clip(pixels, 0, 255).astype(np.uint8)).save(
			dst_dir / f'{src.stem}.png', compress_level=1, exif=exif
		)

	return convert


# Name: (how to make one photo, extension). imageio_avif is the docs/rust.md trial: the same AVIF bytes,
# decoded by ImageIO with rounding. sips decodes through ImageIO too, then writes 16-bit PNG (pinframe
# truncates to 8 bits), HEIC or JPEG. Pillow decodes through libavif, bit-exact with pinframe's AVIF path.
VARIANTS = {
	'imageio_avif': (imageio_avif, 'heic'),
	'imageio_png': (sips('png', [], 'png'), 'png'),
	'heic': (sips('heic', [], 'heic'), 'heic'),
	'jpeg_q90': (sips('jpeg', ['-s', 'formatOptions', '90'], 'jpeg'), 'jpeg'),
	'pillow_png': (pillow_png(noise=False), 'png'),
	'noise_2': (pillow_png(noise=True), 'png'),
}


def girona_run(photos: list[Path], ext: str, work: Path) -> dict:
	work.mkdir()
	rows = run_pinframe(photos, girona_marks(ext), work)
	sizes = {p.stem: oriented_size(p) for p in GIRONA.glob('*.avif')}
	result = {}
	for row in rows:
		stem = Path(row['file']).stem
		fw, fh = sizes[stem]
		result[stem] = {
			'point_px': [round(v, 3) for v in anchor_px(row, fw, fh)],
			'scale': row['scale'],
			'source': row['source'],
			'rect': [row[k] for k in 'xywh'],
		}
	return result


def oriented_size(path: Path) -> tuple[int, int]:
	with Image.open(path) as im:
		w, h = im.size
		return (h, w) if im.getexif().get(0x0112, 1) in (5, 6, 7, 8) else (w, h)


def compare(original: dict, variant: dict) -> dict:
	moved = {s: round(math.dist(original[s]['point_px'], variant[s]['point_px']), 3) for s in original}
	rect = {s: max(abs(a - b) for a, b in zip(original[s]['rect'], variant[s]['rect'])) for s in original}
	scale = {s: abs(variant[s]['scale'] / original[s]['scale'] - 1) * 100 for s in original}
	lost_o = {s for s in original if original[s]['source'] == 'lost'}
	lost_v = {s for s in variant if variant[s]['source'] == 'lost'}
	top = sorted(moved, key=moved.get, reverse=True)[:TOP_MOVERS]
	return {
		'moved_px_max': max(moved.values()),
		'moved_px_median': round(float(np.median(list(moved.values()))), 3),
		'scale_change_pct_max': round(max(scale.values()), 3),
		# The crop rect, normalized 0-1: the measure check.sh and docs/rust.md use.
		'rect_moved_max': round(max(rect.values()), 5),
		'rect_moved_max_photo': max(rect, key=rect.get),
		'top_movers': {s: moved[s] for s in top},
		'lost_same': lost_o == lost_v,
		'lost_added': sorted(lost_v - lost_o),
		'lost_removed': sorted(lost_o - lost_v),
		'moved_px': moved,
	}


def stability(tmp: Path) -> dict:
	originals = sorted(GIRONA.glob('*.avif'))
	assert len(originals) == 57, f'expected 57 Girona photos, found {len(originals)}'
	dirs = {name: tmp / name for name in VARIANTS}
	for d in dirs.values():
		d.mkdir()
	with ThreadPoolExecutor() as pool:
		jobs = [
			pool.submit(convert, src, dirs[name])
			for name, (convert, _) in VARIANTS.items()
			for src in originals
		]
		for job in jobs:
			job.result()
	original = girona_run(originals, 'avif', tmp / 'run-original')
	variants = {}
	for name, (_, ext) in VARIANTS.items():
		photos = sorted(dirs[name].glob(f'*.{ext}'))
		variants[name] = compare(original, girona_run(photos, ext, tmp / f'run-{name}'))
	return {'original': original, 'variants': variants}


# --- report ---


def summary_line(report: dict) -> str:
	a = report['accuracy']['summary']
	parts = [
		f'accuracy: {a["frames"]} frames, error median {a["error_px_median"]} px max {a["error_px_max"]} px, '
		f'scale max {a["scale_error_pct_max"]}%, lost {a["lost"]}'
	]
	for name, v in report['stability']['variants'].items():
		worst = next(iter(v['top_movers']))
		lost = 'same lost' if v['lost_same'] else f'lost +{v["lost_added"]} -{v["lost_removed"]}'
		parts.append(
			f'{name} max {v["moved_px_max"]:.1f} px ({worst}) median {v["moved_px_median"]:.2f}, '
			f'rect {v["rect_moved_max"]:.2e}, {lost}'
		)
	return ' | '.join(parts) + f' | {report["seconds"]:.0f} s'


def main() -> None:
	report_path = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_REPORT
	start = time.monotonic()
	subprocess.run(['cargo', 'build', '--release', '-q'], cwd=ROOT / 'rust', check=True)
	with tempfile.TemporaryDirectory(prefix='pinframe-bench-') as tmp_name:
		tmp = Path(tmp_name)
		with ThreadPoolExecutor() as pool:
			futures = {name: pool.submit(accuracy_sequence, name, tmp) for name in SEQUENCES}
			sequences = {name: f.result() for name, f in futures.items()}
		report = {
			'accuracy': {'summary': accuracy_summary(sequences), 'sequences': sequences},
			'stability': stability(tmp),
		}
	report['seconds'] = round(time.monotonic() - start, 1)
	report_path.parent.mkdir(parents=True, exist_ok=True)
	report_path.write_text(json.dumps(report, indent='\t') + '\n')
	print(summary_line(report))


if __name__ == '__main__':
	main()
