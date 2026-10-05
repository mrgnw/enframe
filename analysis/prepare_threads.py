"""Decode + whole-photo SIFT for every Girona photo: serial phases vs threads, wall clock and peak memory.

uv run analysis/prepare_threads.py            runs every variant, each in a fresh process
uv run analysis/prepare_threads.py VARIANT N  runs one: phases | sift-pool | pipeline
"""

import hashlib
import resource
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import cv2
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent.parent))
from anchor import TRACK_EDGE, gray, load, resize  # noqa: E402

PHOTOS = sorted(Path('samples/girona clouds').glob('*.avif'))
VARIANTS = [
	('phases', 1),
	('sift-pool', 4),
	('sift-pool', 8),
	('pipeline', 4),
	('pipeline', 6),
	('pipeline', 8),
]


def track_factor() -> float:
	# Headers only: the long edge does not change under EXIF rotation.
	sizes = []
	for p in PHOTOS:
		with Image.open(p) as im:
			sizes.append(max(im.size))
	return min(1.0, TRACK_EDGE / max(sizes))


def sift(g):
	return cv2.SIFT_create().detectAndCompute(g, None)


def prepare(path: Path, k: float):
	rgb = load(path)
	return rgb, sift(resize(gray(rgb), k))


def run(variant: str, workers: int) -> list:
	k = track_factor()
	if variant == 'phases':
		frames = [load(p) for p in PHOTOS]
		return [sift(resize(gray(f), k)) for f in frames]
	with ThreadPoolExecutor(workers) as pool:
		if variant == 'sift-pool':
			frames = [load(p) for p in PHOTOS]
			return list(pool.map(sift, [resize(gray(f), k) for f in frames]))
		return [features for _, features in pool.map(prepare, PHOTOS, [k] * len(PHOTOS))]


def fingerprint(results) -> str:
	digest = hashlib.sha256()
	for kp, des in results:
		digest.update(repr([p.pt for p in kp]).encode())
		digest.update(des.tobytes())
	return digest.hexdigest()[:12]


if __name__ == '__main__':
	if len(sys.argv) == 3:
		start = time.perf_counter()
		results = run(sys.argv[1], int(sys.argv[2]))
		seconds = time.perf_counter() - start
		peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 2**20
		print(f'| {sys.argv[1]} | {sys.argv[2]} | {seconds:.2f} | {peak:.0f} | {fingerprint(results)} |')
	else:
		print('| variant | threads | seconds | peak MB | features |')
		print('|-|-|-|-|-|', flush=True)
		for _ in range(3):
			for variant, workers in VARIANTS:
				subprocess.run(['uv', 'run', __file__, variant, str(workers)], check=True)
