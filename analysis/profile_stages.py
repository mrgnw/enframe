"""Where a pinframe run's wall clock goes, as self-time per stage that sums to the total.

Run through analysis/profile.sh, which repeats the Girona benchmark and prints medians. Main-thread stages
add up to the run; worker-thread time (place, PNG saves) overlaps the render wait and is listed apart.
"""

import json
import os
import resource
import statistics
import subprocess
import sys
import threading
import time
from collections import defaultdict
from pathlib import Path

started = time.perf_counter()
sys.path.insert(0, str(Path(__file__).parent.parent))
import anchor  # noqa: E402

imported = time.perf_counter()

LABELS = {
	'load': 'decode photos (Pillow AVIF + EXIF)',
	'load_photos': 'decode + SIFT on threads: wait',
	'prepare': 'prepare photo: other',
	'header_sizes': 'read photo headers',
	'gray': 'track: gray',
	'resize': 'track: downscale to 1024',
	'sift whole photo': 'track: SIFT on whole photo',
	'sift in box': 'track: SIFT in box (marked photos)',
	'features_in': 'track: filter features to box',
	'box_mask': 'track: box mask',
	'similarity': 'track: match + RANSAC',
	'plausible': 'track: plausibility check',
	'track': 'track: loop overhead',
	'fill_lost': 'plan: interpolate lost',
	'crop_rects': 'plan: crop rects',
	'plan': 'plan: other',
	'render': 'render: wait for placed frames',
	'write_avif': 'render: AVIF encode, incl. waiting for placed frames',
	'pipe_to_ffmpeg': 'render: feed ffmpeg, incl. waiting for placed frames',
	'pipe write': 'render: blocked writing to ffmpeg',
	'pipe close': 'render: close pipe',
	'ffmpeg wait': 'render: wait for ffmpeg to finish',
	'write_rects': 'write JSON',
	'place': 'place (resize + warp)',
	'pillow save': 'Pillow save: the AVIF clip, or --frames PNGs',
}

# Top-level phases get wall and process-wide CPU time, to show how many cores each one keeps busy.
PHASES = ('load_photos', 'track', 'render')

spent = defaultdict(float)
calls = defaultdict(int)
phase_wall = defaultdict(float)
phase_cpu = defaultdict(float)
local = threading.local()


def timed(name, fn):
	def wrapper(*args, **kwargs):
		stack = local.__dict__.setdefault('stack', [])
		stack.append(0.0)
		start, cpu_start = time.perf_counter(), time.process_time()
		try:
			return fn(*args, **kwargs)
		finally:
			total = time.perf_counter() - start
			children = stack.pop()
			thread = 'main' if threading.current_thread() is threading.main_thread() else 'worker'
			spent[f'{thread}:{name}'] += total - children
			calls[f'{thread}:{name}'] += 1
			if name in PHASES and thread == 'main':
				phase_wall[name] += total
				phase_cpu[name] += time.process_time() - cpu_start
			if stack:
				stack[-1] += total

	return wrapper


class TimedSift:
	def __init__(self, sift):
		self.sift = sift

	def detectAndCompute(self, image, mask):
		name = 'sift whole photo' if mask is None else 'sift in box'
		return timed(name, self.sift.detectAndCompute)(image, mask)


class TimedPipe:
	def __init__(self, pipe):
		self.pipe = pipe

	def write(self, data):
		return timed('pipe write', self.pipe.write)(data)

	def close(self):
		return timed('pipe close', self.pipe.close)()

	def __getattr__(self, name):
		return getattr(self.pipe, name)


class TimedPopen(subprocess.Popen):
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)
		if self.stdin is not None:
			self.stdin = TimedPipe(self.stdin)

	def wait(self, timeout=None):
		return timed('ffmpeg wait', super().wait)(timeout)


def instrument():
	for name in (
		'load',
		'header_sizes',
		'prepare',
		'load_photos',
		'gray',
		'resize',
		'features_in',
		'box_mask',
		'similarity',
		'plausible',
		'track',
		'fill_lost',
		'crop_rects',
		'plan',
		'place',
		'render',
		'write_avif',
		'pipe_to_ffmpeg',
		'write_rects',
	):
		setattr(anchor, name, timed(name, getattr(anchor, name)))
	anchor.Image.Image.save = timed('pillow save', anchor.Image.Image.save)
	sift_create = anchor.cv2.SIFT_create
	anchor.cv2.SIFT_create = lambda: TimedSift(sift_create())
	anchor.subprocess.Popen = TimedPopen


def profile(log: Path):
	instrument()
	main_started = time.perf_counter()
	anchor.main()
	ended = time.perf_counter()
	me = resource.getrusage(resource.RUSAGE_SELF)
	kids = resource.getrusage(resource.RUSAGE_CHILDREN)
	row = {
		'seconds': dict(spent),
		'calls': dict(calls),
		'phase wall': dict(phase_wall),
		'phase cpu': dict(phase_cpu),
		'imports': imported - started,
		'main': ended - main_started,
		'script': ended - started,
		'python cpu': me.ru_utime + me.ru_stime,
		'ffmpeg cpu': kids.ru_utime + kids.ru_stime,
		'peak MB': me.ru_maxrss / 2**20,
	}
	with log.open('a') as f:
		f.write(json.dumps(row) + '\n')


def summarize(log: Path, walls: Path):
	runs = [json.loads(line) for line in log.read_text().splitlines()]
	wall = statistics.median(float(v) for v in walls.read_text().split())
	assert runs, f'no runs in {log}'

	def median(get) -> float:
		return statistics.median(get(run) for run in runs)

	keys = sorted({k for run in runs for k in run['seconds']})
	main_keys = [k for k in keys if k.startswith('main:')]
	worker_keys = [k for k in keys if k.startswith('worker:')]
	script = median(lambda r: r['script'])
	rows = [
		('uv run + Python start', wall - script, ''),
		('import numpy, cv2, PIL', median(lambda r: r['imports']), ''),
	]
	for k in main_keys:
		name = k.removeprefix('main:')
		rows.append(
			(LABELS.get(name, name), median(lambda r: r['seconds'].get(k, 0)), runs[0]['calls'].get(k, ''))
		)
	rows.append(('other main()', median(lambda r: r['main'] - sum(r['seconds'][k] for k in main_keys)), ''))
	small = [row for row in rows if row[1] < 0.01]
	rows = [row for row in rows if row[1] >= 0.01]
	rows.append((f'{len(small)} stages under 0.01 s each', sum(seconds for _, seconds, _ in small), ''))

	print(
		f'{len(runs)} runs, median wall clock {wall:.2f} s, peak RSS {median(lambda r: r["peak MB"]):.0f} MB\n'
	)
	print('| stage (main thread, medians; sums to about the wall clock) | calls | seconds | share |')
	print('|-|-|-|-|')
	for label, seconds, n in sorted(rows, key=lambda row: -row[1]):
		print(f'| {label} | {n} | {seconds:.2f} | {seconds / wall:.0%} |')
	print(f'| **total** | | **{wall:.2f}** | |\n')
	print('| on worker threads (overlaps the waits above) | calls | thread-seconds |')
	print('|-|-|-|')
	for k in worker_keys:
		name = k.removeprefix('worker:')
		print(
			f'| {LABELS.get(name, name)} | {runs[0]["calls"][k]} | {median(lambda r: r["seconds"].get(k, 0)):.2f} |'
		)
	print(f'| ffmpeg encode (CPU, all cores) | | {median(lambda r: r["ffmpeg cpu"]):.2f} |\n')
	print('| phase | wall s | Python CPU s | cores busy |')
	print('|-|-|-|-|')
	for name in PHASES:
		w = median(lambda r: r['phase wall'][name])
		c = median(lambda r: r['phase cpu'][name])
		print(f'| {name} | {w:.2f} | {c:.2f} | {c / w:.1f} |')
	print(f'\nPython process CPU {median(lambda r: r["python cpu"]):.2f} s over {wall:.2f} s wall')


if __name__ == '__main__':
	if sys.argv[1:2] == ['--summarize']:
		summarize(Path(sys.argv[2]), Path(sys.argv[3]))
	else:
		profile(Path(os.environ['PROFILE_LOG']))
