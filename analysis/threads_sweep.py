"""Girona run time and peak memory across thread counts and queue depths.

uv run analysis/threads_sweep.py            sweeps, 3 runs per setting, medians
uv run analysis/threads_sweep.py run ...    one run (used by the sweep through girona_all.sh)
"""

import os
import re
import statistics
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
RUNS = 3
# (prepare threads, render threads, queue depth as a multiple of the threads)
SETTINGS = [
	(2, 8, 3),
	(4, 8, 3),
	(6, 8, 3),
	(8, 8, 3),
	(12, 8, 3),
	(8, 4, 3),
	(8, 12, 3),
	(8, 8, 1),
	(8, 8, 2),
	(8, 8, 4),
]


def run_once(prepare: int, render: int, depth: int) -> tuple[float, float]:
	env = os.environ | {'RUN': __file__, 'SWEEP': f'{prepare},{render},{depth}'}
	cmd = ['/usr/bin/time', '-l', 'bash', 'analysis/girona_all.sh', 'out/sweep.avif']
	done = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True, check=True)
	seconds = float(re.search(r'([\d.]+) real', done.stderr).group(1))
	peak = int(re.search(r'(\d+)\s+maximum resident set size', done.stderr).group(1)) / 2**20
	return seconds, peak


def sweep():
	print('| prepare threads | render threads | queue depth | seconds | peak MB |')
	print('|-|-|-|-|-|', flush=True)
	for prepare, render, depth in SETTINGS:
		runs = [run_once(prepare, render, depth) for _ in range(RUNS)]
		seconds = statistics.median(s for s, _ in runs)
		peak = statistics.median(p for _, p in runs)
		print(f'| {prepare} | {render} | {depth}x | {seconds:.2f} | {peak:.0f} |', flush=True)


def run_with_setting():
	sys.path.insert(0, str(ROOT))
	import anchor

	prepare, render, depth = (int(v) for v in os.environ['SWEEP'].split(','))
	planned = anchor.plan_resources
	anchor.QUEUE_DEPTH = depth
	anchor.plan_resources = lambda *args: planned(*args)._replace(
		prepare_threads=prepare, render_threads=render
	)
	anchor.main()


if __name__ == '__main__':
	if 'SWEEP' in os.environ:
		run_with_setting()
	else:
		sweep()
