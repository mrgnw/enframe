"""Peak memory of one Rust prepare and render task per photo size, each in a fresh process.

uv run analysis/rust_task_memory.py
Makes 12 MP and 48 MP AVIF, JPEG, PNG, HEIC and JPEG XL inputs from IMG_4698 once (out/large/), runs the task_memory
check in the Rust test binary for each, and prints the cost over the process's starting memory with a
fixed + per-pixel fit.
"""

import re
import subprocess
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).parent.parent
SOURCE = ROOT / 'samples' / 'train clouds' / 'IMG_4698.avif'


def upscaled(size: tuple[int, int], kind: str) -> Path:
	suffix = {'avif': 'avif', 'jpeg': 'jpeg', 'png': 'png', 'heic8': 'heic', 'heic10': 'heic', 'jxl_lossless': 'jxl', 'jxl_lossy': 'jxl'}[kind]
	path = ROOT / 'out' / 'large' / f'IMG_4698_{size[0]}_{kind}.{suffix}'
	if not path.exists():
		path.parent.mkdir(parents=True, exist_ok=True)
		with Image.open(SOURCE) as im:
			image = im.convert('RGB').resize(size, Image.Resampling.BICUBIC)
			if kind in ('avif', 'jpeg', 'png'):
				options = {'avif': {'quality': 75, 'speed': 10}, 'jpeg': {'quality': 75}, 'png': {}}[kind]
				image.save(path, format={'avif': 'AVIF', 'jpeg': 'JPEG', 'png': 'PNG'}[kind], **options)
			else:
				png = path.with_suffix('.png')
				image.save(png)
				if kind == 'heic10':
					subprocess.run(['/usr/bin/sips', '-s', 'format', 'heic', str(png), '--out', str(path)], check=True)
				elif kind == 'heic8':
					jpeg = path.with_suffix('.jpeg')
					subprocess.run(['/opt/homebrew/bin/ffmpeg', '-y', '-loglevel', 'error', '-i', str(png), '-q:v', '1', str(jpeg)], check=True)
					subprocess.run(['/opt/homebrew/bin/heif-enc', '-q', '100', '-o', str(path), str(jpeg)], check=True)
				else:
					subprocess.run(['/opt/local/bin/cjxl', str(png), str(path), '-d', '0' if kind == 'jxl_lossless' else '1'], check=True)
	return path


build = subprocess.run(
	['cargo', 'test', '--release', '--no-run', '--message-format=short'],
	cwd=ROOT / 'rust',
	capture_output=True,
	text=True,
)
binary = re.search(r'Executable unittests src/main.rs \((.+)\)', build.stderr).group(1)
rows = []
for kind in ('avif', 'jpeg', 'png', 'heic8', 'heic10', 'jxl_lossless', 'jxl_lossy'):
	photos = [SOURCE if kind == 'avif' else upscaled((2048, 1536), kind), upscaled((4032, 3024), kind), upscaled((8064, 6048), kind)]
	for task in ('prepare', 'render'):
		for photo in photos:
			run = subprocess.run(
				[binary, '--ignored', '--exact', 'checks::task_memory', '--nocapture'],
				cwd=ROOT / 'rust',
				env={'TASK_PHOTO': str(photo), 'TASK': task, 'PATH': '/usr/bin:/bin'},
				capture_output=True,
				text=True,
			)
			pixels, peak, before = map(
				int, re.search(r'(\d+) px, peak (\d+) bytes, before (\d+)', run.stdout).groups()
			)
			rows.append((kind, task, pixels, peak - before))
			print(f'{kind} {task} {pixels / 1e6:.1f} MP: {(peak - before) / 2**20:.0f} MB')
	for task in ('prepare', 'render'):
		(_, _, p0, m0), *_, (_, _, p1, m1) = [r for r in rows if r[:2] == (kind, task)]
		per_pixel = (m1 - m0) / (p1 - p0)
		print(f'{kind} {task}: about {(m0 - per_pixel * p0) / 2**20:.0f} MB + {per_pixel:.1f} B per pixel')
