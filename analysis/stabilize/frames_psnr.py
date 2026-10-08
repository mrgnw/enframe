"""Compare two --frames folders of placed PNGs (lossless, before encoding):
uv run analysis/frames_psnr.py out/fmt/pyframes out/fmt/frames"""

import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image


def psnr(a: np.ndarray, b: np.ndarray) -> float:
	mse = np.mean((a - b) ** 2)
	return math.inf if mse == 0 else 10 * math.log10(255**2 / mse)


base, new = (Path(p) for p in sys.argv[1:3])
rows = []
for path in sorted(base.glob('*.png')):
	a = np.asarray(Image.open(path)).astype(np.float64)
	b = np.asarray(Image.open(new / path.name)).astype(np.float64)
	rows.append(
		(psnr(a[..., :3], b[..., :3]), psnr(a[..., 3], b[..., 3]) if a.shape[2] == 4 else math.inf, path.name)
	)
rgb = [r[0] for r in rows]
print(f'{len(rows)} frames: RGB PSNR median {np.median(rgb):.1f} dB, worst {min(rgb):.1f} dB')
print(f'alpha PSNR median {np.median([r[1] for r in rows]):.1f} dB, worst {min(r[1] for r in rows):.1f} dB')
for row in sorted(rows)[:3]:
	print('  %.1f dB RGB, %.1f dB alpha: %s' % row)
