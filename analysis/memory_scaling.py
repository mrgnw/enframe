"""Peak memory of a pinframe run against the number of photos.

uv run analysis/memory_scaling.py
Runs pinframe on the first N Girona photos (transparent padding, AVIF out) and reads the peak RSS that
/usr/bin/time -l reports for the whole process.
"""

import re
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).parent.parent
PHOTOS = sorted((ROOT / 'samples' / 'girona clouds').glob('*.avif'))
COUNTS = (10, 20, 40, 57)


def peak_mb(count: int) -> tuple[float, float]:
	out = ROOT / 'out' / f'memory-{count}.avif'
	cmd = [
		'/usr/bin/time', '-l', 'rust/target/release/pinframe', *map(str, PHOTOS[:count]),
		'--box', '1010,410,1030,400', '--pad', 'transparent', '--out', str(out),
	]  # fmt: skip
	start = time.perf_counter()
	run = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, check=True)
	seconds = time.perf_counter() - start
	peak = int(re.search(r'(\d+)\s+maximum resident set size', run.stderr).group(1))
	return peak / 2**20, seconds


if __name__ == '__main__':
	print('| photos | peak MB | MB per photo | seconds |')
	print('|-|-|-|-|', flush=True)
	for count in COUNTS:
		mb, seconds = peak_mb(count)
		print(f'| {count} | {mb:.0f} | {mb / count:.1f} | {seconds:.2f} |', flush=True)
