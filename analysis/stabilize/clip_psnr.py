"""Compare two animated AVIF clips frame by frame: uv run analysis/stabilize/clip_psnr.py out/base.avif out/rust.avif

Prints frame count, frame duration and mode of each, the PSNR of the RGB and alpha channels of the second
against the first, and whether each clip's XMP carries the run ID of its .json sidecar."""

import json
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageSequence


def psnr(a: np.ndarray, b: np.ndarray) -> float:
	mse = np.mean((a.astype(np.float64) - b.astype(np.float64)) ** 2)
	return math.inf if mse == 0 else 10 * math.log10(255**2 / mse)


def frames(path: str) -> tuple[list[np.ndarray], dict]:
	with Image.open(path) as im:
		info = {'frames': im.n_frames, 'mode': im.mode, 'duration': im.info.get('duration'), 'size': im.size}
		sidecar = json.loads(Path(path).with_suffix('.json').read_text())
		run = sidecar.get('run', '') if isinstance(sidecar, dict) else ''
		info['xmp has run'] = bool(run) and run.encode() in im.info.get('xmp', b'')
		return [np.asarray(f.convert('RGBA')) for f in ImageSequence.Iterator(im)], info


(base, base_info), (new, new_info) = frames(sys.argv[1]), frames(sys.argv[2])
print(sys.argv[1], base_info)
print(sys.argv[2], new_info)
rgb = [psnr(a[..., :3], b[..., :3]) for a, b in zip(base, new)]
alpha = [psnr(a[..., 3], b[..., 3]) for a, b in zip(base, new)]
print(f'RGB PSNR: median {np.median(rgb):.1f} dB, worst {min(rgb):.1f} dB')
print(f'alpha PSNR: median {np.median(alpha):.1f} dB, worst {min(alpha):.1f} dB')
