"""Seconds and bytes to save the --frames PNGs at each zlib level, and with threads: uv run analysis/png_levels.py out/frames"""

import io
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from PIL import Image

frames = [Image.open(p).copy() for p in sorted(Path(sys.argv[1]).glob('*.png'))]


def save(im: Image.Image, level: int) -> int:
	buffer = io.BytesIO()
	im.save(buffer, 'png', compress_level=level)
	return buffer.tell()


for level in (1, 3, 6):
	start = time.perf_counter()
	size = sum(save(im, level) for im in frames)
	print(f'level {level}  serial   {time.perf_counter() - start:5.2f} s  {size / 2**20:5.0f} MB')
start = time.perf_counter()
with ThreadPoolExecutor() as pool:
	size = sum(pool.map(lambda im: save(im, 6), frames))
print(f'level 6  threads  {time.perf_counter() - start:5.2f} s  {size / 2**20:5.0f} MB')
