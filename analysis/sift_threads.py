"""Whole-photo SIFT at TRACK_EDGE: serial with OpenCV's own threads vs one photo per Python thread."""

import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import cv2

sys.path.insert(0, str(Path(__file__).parent.parent))
from anchor import TRACK_EDGE, gray, load, resize  # noqa: E402

photos = [load(p) for p in sorted(Path('samples/girona clouds').glob('*.avif'))]
k = TRACK_EDGE / max(max(f.shape[:2]) for f in photos)
small = [resize(gray(f), k) for f in photos]


def sift(g):
	return cv2.SIFT_create().detectAndCompute(g, None)


def keys(results):
	return [([p.pt for p in kp], des.tobytes()) for kp, des in results]


default = cv2.getNumThreads()
for threads in (1, default):
	cv2.setNumThreads(threads)
	start = time.perf_counter()
	serial = [sift(g) for g in small]
	print(f'serial, cv2 threads {threads}: {time.perf_counter() - start:.2f} s')
for workers, threads in ((4, 1), (8, 1), (12, 1), (8, default), (12, default)):
	cv2.setNumThreads(threads)
	start = time.perf_counter()
	with ThreadPoolExecutor(workers) as pool:
		pooled = list(pool.map(sift, small))
	print(f'{workers} python threads, cv2 threads {threads}: {time.perf_counter() - start:.2f} s')
	assert keys(pooled) == keys(serial), 'threaded SIFT must match serial SIFT'
print('identical keypoints and descriptors')
