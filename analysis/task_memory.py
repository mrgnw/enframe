"""Peak memory of one prepare task (decode + SIFT) and one render task (decode + place) by photo size.

uv run analysis/task_memory.py
Each measurement runs in a fresh process: peak RSS after the task minus RSS after imports and a warm-up
on a tiny image. Test photos are a Girona photo upscaled to iPhone sizes and saved as AVIF.
"""

import resource
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
SOURCE = ROOT / 'samples' / 'girona clouds' / 'IMG_4791.avif'
WORK = ROOT / 'out' / 'task-memory'
SIZES = [(2048, 1536), (4032, 3024), (8064, 6048)]


def test_photo(w: int, h: int) -> Path:
	from PIL import Image

	path = WORK / f'{w}x{h}.avif'
	if not path.exists():
		WORK.mkdir(parents=True, exist_ok=True)
		with Image.open(SOURCE) as im:
			im.convert('RGB').resize((w, h), Image.Resampling.BICUBIC).save(path, quality=75, speed=10)
	return path


def measure(task: str, path: Path) -> float:
	sys.path.insert(0, str(ROOT))
	import numpy as np

	import anchor

	tiny = np.zeros((64, 64, 3), np.uint8)
	anchor.prepare(tiny, 1.0)
	anchor.place(tiny, (8, 8, 32, 32), (32, 32), (0, 0, 0, 0))
	before = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
	size = anchor.oriented_size(path)
	if task == 'prepare':
		anchor.prepare_file(path, size, anchor.track_factor([size]), anchor.Keep(0))
	else:
		w, h = size
		# A padded clip at 70% of the photo, the largest Girona's crops get relative to their photos.
		clip = (round(w * 0.7) // 2 * 2, round(h * 0.7) // 2 * 2)
		anchor.place(anchor.load(path), (w * 0.15, h * 0.15, w * 0.7, h * 0.7), clip, (0, 0, 0, 0))
	return (resource.getrusage(resource.RUSAGE_SELF).ru_maxrss - before) / 2**20


if __name__ == '__main__':
	if len(sys.argv) == 3:
		print(measure(sys.argv[1], Path(sys.argv[2])))
	else:
		print('| photo | megapixels | RGB MB | prepare task MB | render task MB |')
		print('|-|-|-|-|-|', flush=True)
		for w, h in SIZES:
			path = test_photo(w, h)
			mb = {}
			for task in ('prepare', 'render'):
				out = subprocess.run(
					['uv', 'run', __file__, task, str(path)], capture_output=True, text=True, check=True
				)
				mb[task] = float(out.stdout)
			rgb = w * h * 3 / 2**20
			print(
				f'| {w}x{h} | {w * h / 1e6:.1f} | {rgb:.0f} | {mb["prepare"]:.0f} | {mb["render"]:.0f} |',
				flush=True,
			)
