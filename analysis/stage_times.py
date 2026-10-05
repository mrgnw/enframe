"""Run anchor.py with per-stage wall-clock totals: RUN=analysis/stage_times.py bash analysis/girona_all.sh"""

import atexit
import resource
import sys
import time
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))
import anchor  # noqa: E402

totals = defaultdict(float)
calls = defaultdict(int)


def timed(name, fn):
	def wrapper(*args, **kwargs):
		start = time.perf_counter()
		try:
			return fn(*args, **kwargs)
		finally:
			totals[name] += time.perf_counter() - start
			calls[name] += 1

	return wrapper


for name in (
	'load',
	'track',
	'gray',
	'resize',
	'similarity',
	'features_in',
	'place',
	'render',
	'write_rects',
):
	setattr(anchor, name, timed(name, getattr(anchor, name)))
anchor.Image.Image.save = timed('png save', anchor.Image.Image.save)


def report():
	print('\nstage            calls   seconds')
	for name, seconds in totals.items():
		print(f'{name:<16} {calls[name]:>5} {seconds:>9.2f}')
	print(f'{"total":<16} {"":>5} {time.perf_counter() - started:>9.2f}')
	peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 2**20
	print(f'peak RSS {peak:.0f} MB')


started = time.perf_counter()
atexit.register(report)
anchor.main()
