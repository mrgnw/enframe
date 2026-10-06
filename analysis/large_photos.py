"""Run pinframe on 48 MP photos and compare peak memory with the planned budget.

uv run analysis/large_photos.py  (BIN=path/to/binary runs an alternate build)
Upscales the 8 turbine photos (IMG_4698-4705) to 8064x6048 AVIFs once, into out/large/, then runs them with
the turbine box and mark scaled to match.
"""

import os
import re
import subprocess
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).parent.parent
SOURCES = [ROOT / 'samples' / 'girona clouds' / f'IMG_{n}.avif' for n in range(4698, 4706)]
WORK = ROOT / 'out' / 'large'
SIZE = (8064, 6048)
SCALE = SIZE[0] / 2048


def upscaled(source: Path) -> Path:
	path = WORK / source.name
	if not path.exists():
		WORK.mkdir(parents=True, exist_ok=True)
		with Image.open(source) as im:
			im.convert('RGB').resize(SIZE, Image.Resampling.BICUBIC).save(path, quality=75, speed=10)
	return path


def box_points(x: int, y: int, w: int, h: int) -> str:
	"""A box's four quarter centres, scaled, as a --points/--at point list (A5b)."""
	corners = [
		(x + w / 4, y + h / 4),
		(x + 3 * w / 4, y + h / 4),
		(x + w / 4, y + 3 * h / 4),
		(x + 3 * w / 4, y + 3 * h / 4),
	]
	return '/'.join(f'{round(px * SCALE)},{round(py * SCALE)}' for px, py in corners)


if __name__ == '__main__':
	photos = [upscaled(s) for s in SOURCES]
	cmd = [
		'/usr/bin/time', '-l', *os.environ.get('BIN', 'rust/target/release/pinframe').split(), *map(str, photos),
		'--points', box_points(840, 550, 400, 350), '--at', f'IMG_4703.avif={box_points(746, 554, 400, 350)}',
		'--pad', 'transparent', '--out', str(WORK / 'clip.avif'),
	]  # fmt: skip
	done = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
	print(done.stdout.splitlines()[0] if done.stdout else done.stderr)
	if done.returncode == 0:
		seconds = float(re.search(r'([\d.]+) real', done.stderr).group(1))
		peak = int(re.search(r'(\d+)\s+maximum resident set size', done.stderr).group(1)) / 2**30
		print(f'{len(photos)} photos at {SIZE[0]}x{SIZE[1]}: {seconds:.1f} s, peak {peak:.2f} GB')
