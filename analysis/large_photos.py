"""Run pinframe on 48 MP photos and compare peak memory with the planned budget.

uv run analysis/large_photos.py
Upscales the 8 turbine photos (IMG_4698-4705) to 8064x6048 AVIFs once, into out/large/, then runs them with
the turbine box and mark scaled to match.
"""

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


def scaled(numbers: tuple[int, ...]) -> str:
	return ','.join(str(round(n * SCALE)) for n in numbers)


if __name__ == '__main__':
	photos = [upscaled(s) for s in SOURCES]
	cmd = [
		'/usr/bin/time', '-l', 'uv', 'run', 'anchor.py', *map(str, photos),
		'--box', scaled((840, 550, 400, 350)), '--at', f'IMG_4703.avif={scaled((746, 554, 400, 350))}',
		'--pad', 'transparent', '--out', str(WORK / 'clip.avif'),
	]  # fmt: skip
	done = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
	print(done.stdout.splitlines()[0] if done.stdout else done.stderr)
	if done.returncode == 0:
		seconds = float(re.search(r'([\d.]+) real', done.stderr).group(1))
		peak = int(re.search(r'(\d+)\s+maximum resident set size', done.stderr).group(1)) / 2**30
		print(f'{len(photos)} photos at {SIZE[0]}x{SIZE[1]}: {seconds:.1f} s, peak {peak:.2f} GB')
