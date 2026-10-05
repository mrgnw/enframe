"""The IMG_4698 -> IMG_4699 tracking step through anchor.py's own functions: dumps the matched point pairs,
OpenCV's RANSAC inlier mask and its fitted matrix to out/ransac_4699.json for the Rust check."""

import json
import sys
from pathlib import Path

import cv2
import numpy as np

sys.path.insert(0, str(Path(__file__).parent.parent))
import anchor  # noqa: E402

folder = Path('samples/girona clouds')
a, b = (anchor.load(folder / f'IMG_{n}.avif') for n in (4698, 4699))
k = 0.5
pa, pb = anchor.prepare(a, k), anchor.prepare(b, k)
bx, by, bw, bh = 840, 550, 400, 350
cx, cy, rw, rh = (bx + bw / 2) * k, (by + bh / 2) * k, bw * k, bh * k
kp, des = anchor.features_in(pa.keypoints, pa.descriptors, anchor.box_mask(pa.small_shape, cx, cy, rw, rh))
matcher = cv2.BFMatcher(cv2.NORM_L2)
pairs = [
	m for m, n in matcher.knnMatch(des, pb.descriptors, k=2) if m.distance < anchor.RATIO_TEST * n.distance
]
src = np.float32([kp[m.queryIdx].pt for m in pairs])
dst = np.float32([pb.keypoints[m.trainIdx].pt for m in pairs])
tolerance = anchor.RANSAC_TOLERANCE * rw
M, inliers = cv2.estimateAffinePartial2D(src, dst, method=cv2.RANSAC, ransacReprojThreshold=tolerance)
Mn, inliers_norefine = cv2.estimateAffinePartial2D(
	src, dst, method=cv2.RANSAC, ransacReprojThreshold=tolerance, refineIters=0
)
out = {
	'tolerance': tolerance,
	'src': src.tolist(),
	'dst': dst.tolist(),
	'mask': inliers.ravel().tolist(),
	'M': M.tolist(),
	'M_norefine': Mn.tolist(),
	'box_features': len(kp),
	'box_descriptors': des.tolist(),
	'box_keypoints': [[p.pt[0], p.pt[1], p.size, p.angle, p.response, p.octave] for p in kp],
	'descriptors': pb.descriptors.tolist(),
	'knn': [
		[m.queryIdx, m.trainIdx, float(m.distance), n.trainIdx, float(n.distance)]
		for m, n in matcher.knnMatch(des, pb.descriptors, k=2)
	],
}
Path('out/ransac_4699.json').write_text(json.dumps(out))
print(f'{len(kp)} box features, {len(pairs)} pairs, {int(inliers.sum())} inliers, tolerance {tolerance}')
print('M', M.tolist())
print('M without refine', Mn.tolist())
