"""Check that filtering whole-photo SIFT matches SIFT run with the box as mask, on every Girona photo."""

import sys
from pathlib import Path

import cv2
import numpy as np

sys.path.insert(0, str(Path(__file__).parent.parent))
from anchor import box_mask, features_in, gray, load, resize  # noqa: E402

rng = np.random.default_rng(0)
sift = cv2.SIFT_create()
for path in sorted(Path('samples/girona clouds').glob('*.avif')):
	g = resize(gray(load(path)), 0.5)
	kp, des = sift.detectAndCompute(g, None)
	for _ in range(5):
		h, w = g.shape
		bw, bh = rng.uniform(40, w / 2), rng.uniform(40, h / 2)
		cx, cy = rng.uniform(0, w), rng.uniform(0, h)
		mask = box_mask(g.shape, cx, cy, bw, bh)
		want_kp, want_des = sift.detectAndCompute(g, mask)
		got_kp, got_des = features_in(kp, des, mask)
		assert [p.pt for p in want_kp] == [p.pt for p in got_kp], path
		assert (want_des is None and len(got_kp) == 0) or np.array_equal(want_des, got_des), path
print('masked SIFT and filtered whole-photo SIFT agree')
