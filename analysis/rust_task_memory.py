"""Peak memory of one Rust prepare and render task per photo size, each in a fresh process.

uv run analysis/rust_task_memory.py
Makes 12 MP and 48 MP AVIF, JPEG and PNGs from IMG_4698 once (out/large/), runs the task_memory
check in the Rust test binary for each, and prints the cost over the process's starting memory with a
fixed + per-pixel fit.
"""

import re
import subprocess
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).parent.parent
SOURCE = ROOT / 'samples' / 'girona clouds' / 'IMG_4698.avif'


def upscaled(size: tuple[int, int], kind: str) -> Path:
	path = ROOT / 'out' / 'large' / f'IMG_4698_{size[0]}.{kind}'
	if not path.exists():
		path.parent.mkdir(parents=True, exist_ok=True)
		with Image.open(SOURCE) as im:
			image = im.convert('RGB').resize(size, Image.Resampling.BICUBIC)
			options = {'avif': {'quality': 75, 'speed': 10}, 'jpeg': {'quality': 75}, 'png': {}}[kind]
			image.save(path, format={'avif': 'AVIF', 'jpeg': 'JPEG', 'png': 'PNG'}[kind], **options)
	return path


build = subprocess.run(
	['cargo', 'test', '--release', '--no-run', '--message-format=short'],
	cwd=ROOT / 'rust',
	capture_output=True,
	text=True,
)
binary = re.search(r'Executable unittests src/main.rs \((.+)\)', build.stderr).group(1)
rows = []
for kind in ('avif', 'jpeg', 'png'):
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
