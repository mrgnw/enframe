# /// script
# requires-python = '>=3.14'
# dependencies = ['numpy', 'pillow>=11.3']
# ///
"""Pixel difference between ImageIO (via sips) and libavif (via Pillow) for chosen train clouds photos.

uv run analysis/stabilize/decoder_diff.py IMG_4698 IMG_4700 ...
"""

import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent.parent
TRAIN_CLOUDS = ROOT / 'samples' / 'train clouds'


def imageio_rgb(path: Path, tmp: Path) -> np.ndarray:
	out = tmp / f'{path.stem}.png'
	subprocess.run(
		['sips', '-s', 'format', 'png', str(path), '--out', str(out)], check=True, capture_output=True
	)
	return np.asarray(Image.open(out).convert('RGB'), dtype=np.int16)


def libavif_rgb(path: Path) -> np.ndarray:
	return np.asarray(Image.open(path).convert('RGB'), dtype=np.int16)


def gray(rgb: np.ndarray) -> np.ndarray:
	return rgb @ np.array([0.299, 0.587, 0.114])


def main():
	with tempfile.TemporaryDirectory() as tmp:
		for stem in sys.argv[1:]:
			path = TRAIN_CLOUDS / f'{stem}.avif'
			a, b = imageio_rgb(path, Path(tmp)), libavif_rgb(path)
			assert a.shape == b.shape, (stem, a.shape, b.shape)
			d = np.abs(a - b)
			g = np.abs(gray(a) - gray(b))
			print(
				f'{stem}: rgb max {d.max()} mean {d.mean():.3f} share>0 {(d > 0).mean():.2%}; '
				f'gray max {g.max():.2f} mean {g.mean():.3f}; mean signed {(a - b).mean():+.3f}'
			)


main()
