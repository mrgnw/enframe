import sys
from pathlib import Path

import cv2
import numpy as np

sys.path.insert(0, str(Path(__file__).parent.parent))
from anchor import gray, load, resize  # noqa: E402

folder = Path('samples/girona clouds')
names = sys.argv[1].split(',')
x, y, w, h = (int(v) for v in sys.argv[2].split(','))
k = 1.0
sift = cv2.SIFT_create()
matcher = cv2.BFMatcher(cv2.NORM_L2)

frames = [resize(gray(load(folder / f'{n}.avif')), k) for n in names]
box = np.array([x, y, w, h]) * k
cx, cy, bw, bh = box[0] + box[2] / 2, box[1] + box[3] / 2, box[2], box[3]
mask = np.zeros_like(frames[0])
mask[int(cy - bh / 2) : int(cy + bh / 2), int(cx - bw / 2) : int(cx + bw / 2)] = 255
kp, des = sift.detectAndCompute(frames[0], mask)
scale = 1.0
print(f'{names[0]}: {len(kp)} keypoints in box')
for n, g in zip(names[1:], frames[1:]):
	kp2, des2 = sift.detectAndCompute(g, None)
	pairs = [m for m, m2 in matcher.knnMatch(des, des2, k=2) if m.distance < 0.8 * m2.distance]
	if len(pairs) < 4:
		print(f'{n}: {len(pairs)} matches, LOST')
		continue
	src = np.float32([kp[m.queryIdx].pt for m in pairs])
	dst = np.float32([kp2[m.trainIdx].pt for m in pairs])
	M, inl = cv2.estimateAffinePartial2D(src, dst, method=cv2.RANSAC, ransacReprojThreshold=0.03 * bw * scale)
	inliers = int(inl.sum()) if inl is not None else 0
	if M is None or inliers < 8:
		print(f'{n}: {len(pairs)} matches, {inliers} inliers, LOST')
		continue
	s = float(np.hypot(M[0, 0], M[1, 0]))
	rot = float(np.degrees(np.arctan2(M[1, 0], M[0, 0])))
	ncx, ncy = M @ np.array([cx, cy, 1])
	print(f'{n}: {len(pairs)} matches, {inliers} inliers, step scale {s:.2f}, rot {rot:+.1f}, anchor {ncx / k:.0f},{ncy / k:.0f}')
	scale *= s
	cx, cy, bw, bh = ncx, ncy, bw * s, bh * s
	mask = np.zeros_like(g)
	mask[max(0, int(cy - bh / 2)) : int(cy + bh / 2), max(0, int(cx - bw / 2)) : int(cx + bw / 2)] = 255
	kp, des = sift.detectAndCompute(g, mask)
print('total scale', round(scale, 2))
