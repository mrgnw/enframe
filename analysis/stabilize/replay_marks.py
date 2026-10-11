# /// script
# requires-python = '>=3.14'
# ///
"""Track a saved Stabilize document's marks with one or more stabilize binaries and print each photo's outcome.

uv run analysis/stabilize/replay_marks.py DOCUMENT FOLDER BIN... [--unskip PHOTO]... [--only PHOTO]...

DOCUMENT is a folder's .stabilize.json (its first feature); --unskip drops a photo from the skip list so its
own tracking outcome shows; --only limits the printed photos.
"""

import argparse
import json
import subprocess
import tempfile
from pathlib import Path


def points(pts: list[list[float]]) -> str:
	return '/'.join(f'{x},{y}' for x, y in pts)


def args_for(marks: dict, unskip: set[str]) -> list[str]:
	args = ['--points', points(marks['points'])]
	for name, mark in sorted(marks.get('at', {}).items()):
		zoom = f'@{mark["zoom"]}' if 'zoom' in mark else ''
		args += ['--at', f'{name}={points(mark["points"])}{zoom}']
	for name in marks.get('skip', []):
		if name not in unskip:
			args += ['--skip', name]
	return args


def track(binary: str, photos: list[str], marks: list[str]) -> dict[str, str]:
	with tempfile.TemporaryDirectory() as tmp:
		out = Path(tmp) / 'track.json'
		run = subprocess.run(
			[binary, *photos, *marks, '--no-cache', '--out', str(out)], capture_output=True, text=True
		)
		assert run.returncode == 0, run.stderr
		notes = {}
		for line in run.stdout.splitlines():
			name, _, rest = line.partition('  ')
			if name.endswith('.avif'):
				notes[name] = rest.split('  ', 3)[-1].strip()
		return notes


def main():
	parser = argparse.ArgumentParser()
	parser.add_argument('document')
	parser.add_argument('folder')
	parser.add_argument('bins', nargs='+')
	parser.add_argument('--unskip', action='append', default=[])
	parser.add_argument('--only', action='append', default=[])
	a = parser.parse_args()
	marks = json.loads(Path(a.document).read_text())['features'][0]['marks']
	photos = sorted(str(p) for p in Path(a.folder).glob('*.avif'))
	cli = args_for(marks, set(a.unskip))
	results = [track(b, photos, cli) for b in a.bins]
	names = a.only or sorted(results[0])
	for name in names:
		print(name, *(f'| {r.get(name, "?")}' for r in results))


main()
