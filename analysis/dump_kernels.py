"""OpenCV's float Gaussian kernels for the sigmas SIFT blurs with, as JSON lines: [sigma, ksize, [values]]."""
import json
import math

import cv2
import numpy as np

sig_diff = float(np.sqrt(np.float32(max(np.float32(1.6) * np.float32(1.6) - np.float32(1.0), np.float32(0.01)))))
k = 2 ** (1 / 3)
sigmas = [sig_diff] + [math.sqrt((k**i * 1.6 * k) ** 2 - (k**i * 1.6) ** 2) for i in range(5)]
for s in sigmas:
	n = round(s * 8 + 1) | 1
	print(json.dumps([s, n, [float(v) for v in cv2.getGaussianKernel(n, s, cv2.CV_32F).ravel()]]))
