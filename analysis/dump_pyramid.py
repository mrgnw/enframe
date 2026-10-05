"""OpenCV's first SIFT octave for one Girona photo, rebuilt from public calls: the doubled float image
blurred to each layer, saved as raw float32 (out/NAME.g0..g5.f32)."""
import math
import sys
from pathlib import Path

import cv2
import numpy as np

name = sys.argv[1] if len(sys.argv) > 1 else 'IMG_4667'
small = cv2.imread(f'out/{name}.gray.png', cv2.IMREAD_GRAYSCALE)
fpt = small.astype(np.float32)
dbl = cv2.resize(fpt, (fpt.shape[1] * 2, fpt.shape[0] * 2), interpolation=cv2.INTER_LINEAR)
sig_diff = float(np.sqrt(np.float32(max(np.float32(1.6) * np.float32(1.6) - np.float32(1.0), np.float32(0.01)))))
k = 2 ** (1 / 3)
layer = cv2.GaussianBlur(dbl, (0, 0), sig_diff, sigmaY=sig_diff)
layer.tofile(f'out/{name}.g0.f32')
for i in range(1, 6):
	prev = k ** (i - 1) * 1.6
	s = math.sqrt((prev * k) ** 2 - prev**2)
	layer = cv2.GaussianBlur(layer, (0, 0), s, sigmaY=s)
	layer.tofile(f'out/{name}.g{i}.f32')
print(dbl.shape)
