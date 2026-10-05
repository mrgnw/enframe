"""One tracking step, lifted from anchor.py so the same code runs under CPython and Pyodide."""

import time

import cv2
import numpy as np

MIN_INLIERS = 8
TRACK_EDGE = 1024
RATIO_TEST = 0.8
RANSAC_TOLERANCE = 0.03


def fit(rgb_a: np.ndarray, rgb_b: np.ndarray, box: tuple[float, float, float, float]) -> dict:
	"""Anchor at the box centre of photo A, found in photo B. Returns position, scale step and timings."""
	t = {}
	t0 = time.perf_counter()
	k = min(1.0, TRACK_EDGE / max(rgb_a.shape[0], rgb_a.shape[1], rgb_b.shape[0], rgb_b.shape[1]))
	ga, gb = (cv2.resize(cv2.cvtColor(f, cv2.COLOR_RGB2GRAY), None, fx=k, fy=k, interpolation=cv2.INTER_AREA) for f in (rgb_a, rgb_b))
	t['gray_resize'] = time.perf_counter() - t0

	bx, by, bw, bh = box
	cx, cy, rw, rh = (bx + bw / 2) * k, (by + bh / 2) * k, bw * k, bh * k
	sift = cv2.SIFT_create()

	t0 = time.perf_counter()
	mask = np.zeros_like(ga)
	mask[round(cy - rh / 2) : round(cy + rh / 2), round(cx - rw / 2) : round(cx + rw / 2)] = 255
	kp, des = sift.detectAndCompute(ga, mask)
	t['sift_box'] = time.perf_counter() - t0

	t0 = time.perf_counter()
	kp2, des2 = sift.detectAndCompute(gb, None)
	t['sift_full'] = time.perf_counter() - t0

	t0 = time.perf_counter()
	matcher = cv2.BFMatcher(cv2.NORM_L2)
	pairs = [m for m, n in matcher.knnMatch(des, des2, k=2) if m.distance < RATIO_TEST * n.distance]
	src = np.float32([kp[m.queryIdx].pt for m in pairs])
	dst = np.float32([kp2[m.trainIdx].pt for m in pairs])
	M, inliers = cv2.estimateAffinePartial2D(src, dst, method=cv2.RANSAC, ransacReprojThreshold=RANSAC_TOLERANCE * rw)
	t['match_fit'] = time.perf_counter() - t0
	assert M is not None, 'no fit'

	step = float(np.hypot(M[0, 0], M[1, 0]))
	turn = float(np.degrees(np.arctan2(M[1, 0], M[0, 0])))
	nx, ny = M @ np.array([cx, cy, 1.0])
	return {
		'opencv': cv2.__version__,
		'features_box': len(kp),
		'features_full': len(kp2),
		'pairs': len(pairs),
		'inliers': int(inliers.sum()),
		'anchor_a': [cx / k, cy / k],
		'anchor_b': [float(nx / k), float(ny / k)],
		'scale': step,
		'turn_deg': turn,
		'ms': {name: round(s * 1000, 1) for name, s in t.items()},
	}
