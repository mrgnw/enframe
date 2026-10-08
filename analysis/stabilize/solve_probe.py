"""Which 3x3 solve reproduces OpenCV's keypoint sizes? For octave-0 keypoints of IMG_4667, rebuild the DoG
derivatives at the keypoint from the first-octave pyramid (out/IMG_4667.g*.f32, bit-exact with OpenCV's,
from dump_pyramid.py) and count how many sizes each candidate solve reproduces exactly.

uv run analysis/stabilize/solve_probe.py"""

import json

import cv2
import numpy as np

f32 = np.float32
W, H = 2048, 1536
gauss = [np.fromfile(f'out/IMG_4667.g{i}.f32', dtype=np.float32).reshape(H, W) for i in range(6)]
dog = [gauss[i + 1] - gauss[i] for i in range(5)]
kps = json.load(open('out/IMG_4667.kp.json'))


def fma(a, b, c):
	"""Single-rounding a*b + c for float32 inputs: the double product is exact, the sum rarely double-rounds."""
	return f32(np.float64(a) * np.float64(b) + np.float64(c))


def derivatives(layer, r, c):
	img, prev, nxt = dog[layer], dog[layer - 1], dog[layer + 1]
	s = f32(1) / f32(255)
	deriv, second, cross = s * f32(0.5), s, s * f32(0.25)
	d = [
		(img[r, c + 1] - img[r, c - 1]) * deriv,
		(img[r + 1, c] - img[r - 1, c]) * deriv,
		(nxt[r, c] - prev[r, c]) * deriv,
	]
	v2 = img[r, c] * f32(2)
	dxx = (img[r, c + 1] + img[r, c - 1] - v2) * second
	dyy = (img[r + 1, c] + img[r - 1, c] - v2) * second
	dss = (nxt[r, c] + prev[r, c] - v2) * second
	dxy = (img[r + 1, c + 1] - img[r + 1, c - 1] - img[r - 1, c + 1] + img[r - 1, c - 1]) * cross
	dxs = (nxt[r, c + 1] - nxt[r, c - 1] - prev[r, c + 1] + prev[r, c - 1]) * cross
	dys = (nxt[r + 1, c] - nxt[r - 1, c] - prev[r + 1, c] + prev[r - 1, c]) * cross
	return np.array([[dxx, dxy, dxs], [dxy, dyy, dys], [dxs, dys, dss]], f32), np.array(d, f32)


def cramer(a, b, fused):
	m = (lambda p, q, x, y: fma(p, q, -(x * y))) if fused else (lambda p, q, x, y: f32(p * q - x * y))
	o3 = (
		(lambda p0, p1, q0, q1, r0, r1: fma(r0, r1, fma(p0, p1, -(q0 * q1))))
		if fused
		else (lambda p0, p1, q0, q1, r0, r1: f32(f32(p0 * p1 - q0 * q1) + r0 * r1))
	)
	det = o3(
		a[0, 0],
		m(a[1, 1], a[2, 2], a[2, 1], a[1, 2]),
		a[0, 1],
		m(a[1, 0], a[2, 2], a[2, 0], a[1, 2]),
		a[0, 2],
		m(a[1, 0], a[2, 1], a[2, 0], a[1, 1]),
	)
	d = f32(1) / det
	x0 = o3(
		b[0],
		m(a[1, 1], a[2, 2], a[1, 2], a[2, 1]),
		a[0, 1],
		m(b[1], a[2, 2], a[1, 2], b[2]),
		a[0, 2],
		m(b[1], a[2, 1], a[1, 1], b[2]),
	)
	x1 = o3(
		a[0, 0],
		m(b[1], a[2, 2], a[1, 2], b[2]),
		b[0],
		m(a[1, 0], a[2, 2], a[1, 2], a[2, 0]),
		a[0, 2],
		m(a[1, 0], b[2], b[1], a[2, 0]),
	)
	x2 = o3(
		a[0, 0],
		m(a[1, 1], b[2], b[1], a[2, 1]),
		a[0, 1],
		m(a[1, 0], b[2], b[1], a[2, 0]),
		b[0],
		m(a[1, 0], a[2, 1], a[1, 1], a[2, 0]),
	)
	return np.array([d * x0, d * x1, d * x2], f32)


def lu(a, b):
	ok, x = cv2.solve(a, b.reshape(3, 1), flags=cv2.DECOMP_LU)
	return x.ravel().astype(f32) if ok else np.zeros(3, f32)


def size(layer, xi, exp2):
	y = f32(f32(layer) + xi) / f32(3)
	p = f32(np.exp2(np.float64(y))) if exp2 else f32(np.power(np.float32(2), y))
	return f32(f32(f32(1.6) * p) * f32(1) * f32(2)) * f32(0.5)


solvers = {
	'cramer fused': lambda a, b: cramer(a, b, True),
	'cramer plain': lambda a, b: cramer(a, b, False),
	'cv2.solve LU': lu,
}
hits = {name: 0 for name in solvers}
total = 0
for kp in kps:
	if kp['octave'] & 255 != 255:
		continue
	layer = (kp['octave'] >> 8) & 255
	c, r = round(kp['x'] * 2), round(kp['y'] * 2)
	a, b = derivatives(layer, r, c)
	total += 1
	for name, solve in solvers.items():
		xi = -solve(a, b)[2]
		hits[name] += size(layer, xi, True) == f32(kp['size'])
print(f'{total} octave-0 keypoints; sizes reproduced exactly:', hits)

diffs = []
for kp in kps:
	if kp['octave'] & 255 != 255:
		continue
	layer = (kp['octave'] >> 8) & 255
	c, r = round(kp['x'] * 2), round(kp['y'] * 2)
	a, b = derivatives(layer, r, c)
	x = lu(a, b)
	xi_cv = 3 * np.log2(kp['size'] / 1.6) - layer
	xc_cv = kp['x'] * 2 - c
	diffs.append((xi_cv - float(-x[2]), xc_cv - float(-x[0])))
d = np.array(diffs)
print('xi: median |diff|', np.median(abs(d[:, 0])), 'max', abs(d[:, 0]).max())
print('xc: median |diff|', np.median(abs(d[:, 1])), 'max', abs(d[:, 1]).max())
print('xi diff percentiles', np.percentile(d[:, 0], [1, 25, 50, 75, 99]))

ulps = []
for kp in kps[:4151]:
	if kp['octave'] & 255 != 255:
		continue
	layer = (kp['octave'] >> 8) & 255
	c, r = round(kp['x'] * 2), round(kp['y'] * 2)
	a, b = derivatives(layer, r, c)
	xi = -cramer(a, b, True)[2]
	y = f32(f32(layer) + xi) / f32(3)
	want = f32(kp['size'])
	found = None
	for step in range(0, 400):
		for sign in (1, -1):
			cand = (np.array([y], f32).view(np.int32) + sign * step).view(np.float32)[0]
			if f32(f32(f32(1.6) * f32(np.exp2(np.float64(cand)))) * f32(2)) * f32(0.5) == want:
				found = sign * step
				break
		if found is not None:
			break
	ulps.append(found)
known = [u for u in ulps if u is not None]
print(f'y steps to OpenCV size: {len(known)} of {len(ulps)} found within 400; |steps| percentiles', np.percentile(np.abs(known), [25, 50, 75, 90]))
