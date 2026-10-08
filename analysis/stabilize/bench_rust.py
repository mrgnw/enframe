"""Whole train clouds runs, interleaved across commands, through /usr/bin/time -l.

uv run analysis/stabilize/bench_rust.py ROUNDS NAME=COMMAND [NAME=COMMAND ...]

COMMAND runs the job from analysis/stabilize/train_clouds_all.sh, e.g. rust=src-tauri/target/release/stabilize. Each round runs
every command once, in a rotating order, so load on the shared machine falls on all of them alike. Prints
the fastest and median wall, CPU (user + sys) and peak memory per command. Rust runs also write per-step
profiles to out/bench-NAME.jsonl.
"""

import json
import os
import re
import statistics
import subprocess
import sys
from pathlib import Path

rounds = int(sys.argv[1])
commands = dict(arg.split('=', 1) for arg in sys.argv[2:])
results: dict[str, list[tuple[float, float, float]]] = {name: [] for name in commands}
for name in commands:
	Path(f'out/bench-{name}.jsonl').unlink(missing_ok=True)

for r in range(rounds):
	order = list(commands)[r % len(commands) :] + list(commands)[: r % len(commands)]
	for name in order:
		env = dict(
			os.environ, BIN=f'/usr/bin/time -l {commands[name]}', PINFRAME_PROFILE=f'out/bench-{name}.jsonl'
		)
		run = subprocess.run(
			['bash', 'analysis/stabilize/train_clouds_all.sh', f'out/bench-{name}.avif'],
			env=env,
			capture_output=True,
			text=True,
		)
		assert run.returncode == 0, run.stderr
		wall, user, system = (
			float(v) for v in re.search(r'([\d.]+) real\s+([\d.]+) user\s+([\d.]+) sys', run.stderr).groups()
		)
		peak = int(re.search(r'(\d+)\s+maximum resident set size', run.stderr).group(1))
		results[name].append((wall, user + system, peak / 2**30))
		print(
			f'round {r + 1} {name}: {wall:.2f} s wall, {user + system:.1f} CPU-s, {peak / 2**30:.2f} GB',
			file=sys.stderr,
		)

print('| command | wall fastest | wall median | CPU-s median | peak GB median |')
print('|-|-|-|-|-|')
for name, rows in results.items():
	walls, cpus, peaks = zip(*rows)
	print(
		f'| {name} | {min(walls):.2f} | {statistics.median(walls):.2f} | {statistics.median(cpus):.1f} '
		f'| {statistics.median(peaks):.2f} |'
	)

for name in commands:
	profile = Path(f'out/bench-{name}.jsonl')
	if not profile.exists():
		continue
	runs = [json.loads(line) for line in profile.read_text().splitlines()]
	steps = [s['step'] for s in runs[0]['steps']]
	print(f'\n{name}, per step (median of {len(runs)}):')
	print('| step | wall s | CPU s | peak GB |')
	print('|-|-|-|-|')
	for i, step in enumerate(steps):
		med = lambda key: statistics.median(run['steps'][i][key] for run in runs)  # noqa: E731
		print(f'| {step} | {med("wall"):.3f} | {med("cpu"):.2f} | {med("peak") / 2**30:.2f} |')
	print('| work | thread-s |')
	print('|-|-|')
	for kind in runs[0]['work']:
		print(f'| {kind} | {statistics.median(run["work"][kind] for run in runs):.2f} |')
