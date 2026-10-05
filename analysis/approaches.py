"""The same Girona job through every milestone of the performance work, end to end.

uv run analysis/approaches.py
Checks each commit out into a temporary worktree, runs the job from analysis/girona_all.sh (57 photos,
24 marks, transparent padding) with the same native ffmpeg, and reports medians of RUNS runs after one
warm-up: wall clock, CPU (user + system, ffmpeg included), peak RSS and output size.
"""

import re
import shlex
import statistics
import subprocess
from pathlib import Path

ROOT = Path(__file__).parent.parent
PHOTOS = sorted(Path('/Users/user/dev/pinframe/samples/girona clouds').glob('*.avif'))
TREES = Path('/tmp/pinframe-approaches')
RUNS = 7
# (commit, what it added, outputs it supports)
VERSIONS = [
	('5bc670f', 'baseline: decode all, then SIFT photo by photo, then render', ('.mov',)),
	('b38d86b', '+ reuse whole-photo SIFT for box features', ('.mov',)),
	('1288081', '+ place frames on threads', ('.mov',)),
	('2568af7', '+ AVIF output', ('.mov', '.avif')),
	('eab0d77', '+ decode and SIFT per photo on 6 threads', ('.mov', '.avif')),
	('8f8ea5a', '+ bounded memory: stream photos, decode again at render', ('.mov', '.avif')),
	('87f68fc', '+ keep decoded photos within 1/4 of RAM', ('.mov', '.avif')),
	('086f502', '+ threads and memory planned from the machine (current)', ('.mov', '.avif')),
]


def job_args() -> list[str]:
	"""--box, the --at marks and --pad from girona_all.sh, so every version runs the same job."""
	script = (ROOT / 'analysis' / 'girona_all.sh').read_text()
	marks = re.findall(r'^\s*(--at \S+)$', script, re.MULTILINE)
	box = re.search(r'--box (\S+)', script).group(1)
	return ['--box', box, *(arg for mark in marks for arg in shlex.split(mark)), '--pad', 'transparent']


def checkout(commit: str) -> Path:
	tree = TREES / commit
	if not tree.exists():
		subprocess.run(
			['git', 'worktree', 'add', '--detach', str(tree), commit],
			cwd=ROOT,
			check=True,
			capture_output=True,
		)
	return tree


def run(tree: Path, out: Path) -> tuple[float, float, float]:
	cmd = ['/usr/bin/time', '-l', 'uv', 'run', 'anchor.py', *map(str, PHOTOS), *job_args(), '--out', str(out)]
	done = subprocess.run(cmd, cwd=tree, capture_output=True, text=True, check=True)
	wall = float(re.search(r'([\d.]+) real', done.stderr).group(1))
	cpu = sum(float(v) for v in re.search(r'([\d.]+) user\s+([\d.]+) sys', done.stderr).groups())
	peak = int(re.search(r'(\d+)\s+maximum resident set size', done.stderr).group(1)) / 2**20
	return wall, cpu, peak


if __name__ == '__main__':
	TREES.mkdir(parents=True, exist_ok=True)
	configs = [(commit, label, suffix) for commit, label, outputs in VERSIONS for suffix in outputs]
	results = {config: [] for config in configs}
	try:
		for commit, _, suffix in configs:
			run(checkout(commit), TREES / f'{commit}{suffix}')
		# Round-robin, so background load lands on every version alike.
		for _ in range(RUNS):
			for config in configs:
				commit, _, suffix = config
				results[config].append(run(TREES / commit, TREES / f'{commit}{suffix}'))
		print('| commit | approach | output | fastest s | median s | CPU s | peak MB | file MB |')
		print('|-|-|-|-|-|-|-|-|')
		for (commit, label, suffix), runs in results.items():
			fastest = min(wall for wall, _, _ in runs)
			wall, cpu, peak = (statistics.median(r[i] for r in runs) for i in range(3))
			size = (TREES / f'{commit}{suffix}').stat().st_size / 2**20
			cells = f'{fastest:.2f} | {wall:.2f} | {cpu:.1f} | {peak:.0f} | {size:.1f}'
			print(f'| `{commit}` | {label} | {suffix} | {cells} |')
	finally:
		for commit, _, _ in VERSIONS:
			subprocess.run(
				['git', 'worktree', 'remove', '--force', str(TREES / commit)], cwd=ROOT, capture_output=True
			)
