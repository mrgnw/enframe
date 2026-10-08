# /// script
# requires-python = '>=3.14'
# dependencies = ['pillow>=12', 'numpy']
# ///
import sys

import numpy as np
from PIL import Image


def frames(path):
	image = Image.open(path)
	for i in range(image.n_frames):
		image.seek(i)
		yield np.asarray(image.convert('RGBA'), dtype=np.float64)


def main():
	a_path, b_path = sys.argv[1:3]
	count = 0
	worst = float('inf')
	for a, b in zip(frames(a_path), frames(b_path), strict=True):
		mse = np.mean((a - b) ** 2)
		worst = min(worst, float('inf') if mse == 0 else 10 * np.log10(255**2 / mse))
		count += 1
	print(f'{count} frames, lowest PSNR {worst} dB' + (' (identical)' if worst == float('inf') else ''))


main()
