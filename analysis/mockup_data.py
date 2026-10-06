"""Turn the Girona sidecar into photos.js for the HTML mockups.

The sidecar gives each photo's reference points, scale and source. The run log adds what the sidecar leaves
out: each photo's pixel size and, for tracked photos, the feature count. Run after:
BIN=rust/target/release/pinframe PAD=transparent analysis/girona_all.sh /tmp/pin/all.json > /tmp/pin/all.log
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SIDECAR = Path('/tmp/pin/all.json')
LOG = Path('/tmp/pin/all.log')
RECTS = ROOT / 'mockups' / 'assets' / 'rects.json'
OUT = ROOT / 'mockups' / 'assets' / 'photos.js'
LINE = re.compile(r'^(IMG_\d+)\.avif\s+(\d+)x(\d+)\s+anchor (\d+),(\d+)\s+scale [\d.]+\s+(.*)$')
COUNT = 57
TOLERANCE_PX = 1.0


def read_log() -> dict[str, dict]:
	rows = {}
	for line in LOG.read_text().splitlines():
		m = LINE.match(line)
		if m:
			name, w, h, ax, ay, note = m.groups()
			feats = re.search(r'(\d+) features', note)
			rows[name] = {
				'w': int(w),
				'h': int(h),
				'ax': int(ax),
				'ay': int(ay),
				'feats': feats and int(feats.group(1)),
			}
	return rows


def main() -> None:
	log = read_log()
	rows = json.loads(SIDECAR.read_text())['photos']
	assert len(rows) == len(log) == COUNT, (len(rows), len(log))

	photos = []
	for row in rows:
		name = row['file'][:-5]
		fw, fh = log[name]['w'], log[name]['h']
		points = row['points']
		assert points, name
		x, y = points[0]
		assert (
			abs(x * fw - log[name]['ax']) <= TOLERANCE_PX and abs(y * fh - log[name]['ay']) <= TOLERANCE_PX
		), (
			name,
			x * fw,
			y * fh,
		)
		photos.append(
			{
				'name': name,
				'w': fw,
				'h': fh,
				'scale': row['scale'],
				'source': row['source'],
				'feats': log[name]['feats'],
				'pts': [[round(px, 4), round(py, 4)] for px, py in points],
			}
		)

	RECTS.write_text(json.dumps(rows, separators=(',', ':')))
	OUT.write_text('const PHOTOS = ' + json.dumps(photos, separators=(',', ':')) + ';\n')
	chains = 1 + sum(p['source'] != 'tracked' for p in photos[1:])
	print(
		f'{len(photos)} photos, points within {TOLERANCE_PX} px of the log, {chains} linked chains -> {OUT.name}'
	)


if __name__ == '__main__':
	sys.exit(main())
