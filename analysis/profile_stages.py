"""Time, CPU and memory for each step of a pinframe run, through analysis/profile.sh (medians of N runs).

Steps run one after another on the main thread, so their wall times add up to the run. CPU is the whole
process (every thread) during the step; cores busy is CPU over wall. Peak memory is the highest RSS
sampled every 5 ms during the step. The work table splits the threaded parts by what the threads did.
"""

import json
import os
import resource
import statistics
import sys
import threading
import time
from collections import defaultdict
from pathlib import Path

import psutil

started = time.perf_counter()
sys.path.insert(0, str(Path(__file__).parent.parent))
import anchor  # noqa: E402

imported = time.perf_counter()

# Each step is the main-thread calls that make it up.
STEPS = {
	'check input and plan resources': ('check_output', 'read_sizes', 'read_marks', 'plan_resources'),
	'decode, find features, track': ('track',),
	'plan the crop': ('fill_lost', 'crop_rects'),
	'render: place and encode': ('render',),
	'write the JSON': ('write_rects',),
}
# Thread-seconds of the work inside the steps, wherever it ran.
WORK = {
	'load': 'decode photos (AVIF, dav1d)',
	'sift': 'find features (SIFT)',
	'similarity': 'match + RANSAC',
	'place': 'place frames (resize + warp)',
	'avif encode': 'encode AVIF (libaom, in-process)',
	'frame wait': 'encoder waiting for a placed frame',
	'png save': 'save --frames PNGs',
	'ffmpeg': 'ffmpeg (separate process, CPU)',
}

windows = defaultdict(list)  # step -> [(start, end, cpu)]
work = defaultdict(float)
samples: list[tuple[float, int]] = []


def sample_memory(stop: threading.Event):
	process = psutil.Process()
	while not stop.is_set():
		samples.append((time.perf_counter(), process.memory_info().rss))
		stop.wait(0.005)


def step_timer(step: str, fn):
	def wrapper(*args, **kwargs):
		start, cpu = time.perf_counter(), time.process_time()
		try:
			return fn(*args, **kwargs)
		finally:
			windows[step].append((start, time.perf_counter(), time.process_time() - cpu))

	return wrapper


def work_timer(name: str, fn):
	def wrapper(*args, **kwargs):
		start = time.perf_counter()
		try:
			return fn(*args, **kwargs)
		finally:
			work[name] += time.perf_counter() - start

	return wrapper


class TimedSift:
	def __init__(self, sift):
		self.sift = sift

	def detectAndCompute(self, image, mask):
		return work_timer('sift', self.sift.detectAndCompute)(image, mask)


def instrument():
	for step, names in STEPS.items():
		for name in names:
			setattr(anchor, name, step_timer(step, getattr(anchor, name)))
	for name in ('load', 'similarity', 'place'):
		setattr(anchor, name, work_timer(name, getattr(anchor, name)))
	sift_create = anchor.cv2.SIFT_create
	anchor.cv2.SIFT_create = lambda: TimedSift(sift_create())

	# Pillow's save runs the AVIF encoder; inside it, StreamedClip.tobytes waits for the next placed frame.
	save, pull = anchor.Image.Image.save, anchor.StreamedClip.tobytes

	def timed_save(image, fp, format=None, **params):
		name = 'avif encode' if isinstance(image, anchor.StreamedClip) else 'png save'
		waited, start = work['frame wait'], time.perf_counter()
		try:
			return save(image, fp, format, **params)
		finally:
			work[name] += time.perf_counter() - start - (work['frame wait'] - waited)

	anchor.Image.Image.save = timed_save
	anchor.StreamedClip.tobytes = work_timer('frame wait', pull)


def peak_between(start: float, end: float) -> int:
	return max((rss for t, rss in samples if start <= t <= end), default=0)


def profile(log: Path):
	stop = threading.Event()
	sampler = threading.Thread(target=sample_memory, args=(stop,), daemon=True)
	sampler.start()
	instrument()
	main_started = time.perf_counter()
	anchor.main()
	ended = time.perf_counter()
	stop.set()
	sampler.join()
	me = resource.getrusage(resource.RUSAGE_SELF)
	kids = resource.getrusage(resource.RUSAGE_CHILDREN)
	steps = {
		step: {
			'wall': sum(end - start for start, end, _ in spans),
			'cpu': sum(cpu for _, _, cpu in spans),
			'peak': max(peak_between(start, end) for start, end, _ in spans),
		}
		for step, spans in windows.items()
	}
	row = {
		'imports': imported - started,
		'main': ended - main_started,
		'script': ended - started,
		'steps': steps,
		'work': dict(work) | {'ffmpeg': kids.ru_utime + kids.ru_stime},
		'cpu': me.ru_utime + me.ru_stime + kids.ru_utime + kids.ru_stime,
		'peak': me.ru_maxrss,
		'import peak': peak_between(0, main_started),
	}
	with log.open('a') as f:
		f.write(json.dumps(row) + '\n')


def summarize(log: Path, walls: Path):
	runs = [json.loads(line) for line in log.read_text().splitlines()]
	assert runs, f'no runs in {log}'
	wall = statistics.median(float(v) for v in walls.read_text().split())

	def median(get) -> float:
		return statistics.median(get(run) for run in runs)

	mb = 2**20
	startup = wall - median(lambda r: r['script']) + median(lambda r: r['imports'])
	rows = [
		('start Python (uv), import numpy, OpenCV, Pillow', startup, None, median(lambda r: r['import peak']))
	]
	for step in STEPS:
		if all(step in r['steps'] for r in runs):
			rows.append(
				(
					step,
					median(lambda r: r['steps'][step]['wall']),
					median(lambda r: r['steps'][step]['cpu']),
					median(lambda r: r['steps'][step]['peak']),
				)
			)
	stepped = sum(seconds for _, seconds, _, _ in rows[1:])
	rows.append(('other: printing, renaming the outputs', median(lambda r: r['main']) - stepped, None, None))

	print(f'{len(runs)} runs, medians\n')
	print('| step | wall s | share | CPU s | cores busy | peak memory |')
	print('|-|-|-|-|-|-|')
	for label, seconds, cpu, peak in rows:
		cpu_cells = f'{cpu:.2f} | {cpu / seconds:.1f}' if cpu is not None and seconds > 0.005 else ' | '
		peak_cell = f'{peak / mb:.0f} MB' if peak else ''
		print(f'| {label} | {seconds:.2f} | {seconds / wall:.0%} | {cpu_cells} | {peak_cell} |')
	cpu = median(lambda r: r['cpu'])
	peak = median(lambda r: r['peak']) / mb
	print(
		f'| **whole run** | **{wall:.2f}** | | **{cpu:.1f}** | **{cpu / wall:.1f}** | **{peak:.0f} MB** |\n'
	)

	print('| work inside the steps | thread-seconds |')
	print('|-|-|')
	for name, label in WORK.items():
		seconds = median(lambda r: r['work'].get(name, 0.0))
		if seconds >= 0.005:
			print(f'| {label} | {seconds:.2f} |')


if __name__ == '__main__':
	if sys.argv[1:2] == ['--summarize']:
		summarize(Path(sys.argv[2]), Path(sys.argv[3]))
	else:
		profile(Path(os.environ['PROFILE_LOG']))
