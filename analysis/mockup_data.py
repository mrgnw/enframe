"""Turn anchor.py's per-photo log lines into mockups/assets/photos.js for the HTML mockups."""

import json
import re
import sys
from pathlib import Path

LOG = Path('/Users/user/dev/pinframe/out/all.log')
OUT = Path(__file__).resolve().parent.parent / 'mockups' / 'assets' / 'photos.js'
LINE = re.compile(r'^(IMG_\d+)\.avif\s+(\d+)x(\d+)\s+anchor (\d+),(\d+)\s+scale ([\d.]+)\s+(.*)$')


def main() -> None:
	rows = []
	for line in LOG.read_text().splitlines():
		m = LINE.match(line)
		if not m:
			continue
		name, w, h, ax, ay, scale, note = m.groups()
		if note == 'your mark':
			source, feats = 'mark', None
		elif note.startswith('LOST'):
			source, feats = 'lost', int(re.search(r'\((\d+) features', note).group(1))
		else:
			source, feats = 'tracked', int(note.split()[0])
		rows.append(
			{
				'name': name,
				'w': int(w),
				'h': int(h),
				'ax': int(ax),
				'ay': int(ay),
				'scale': float(scale),
				'source': source,
				'feats': feats,
			}
		)
	assert len(rows) == 57, len(rows)
	OUT.write_text('const PHOTOS = ' + json.dumps(rows, separators=(',', ':')) + ';\n')
	print(f'{len(rows)} photos -> {OUT}')


if __name__ == '__main__':
	sys.exit(main())
