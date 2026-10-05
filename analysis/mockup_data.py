"""Derive each photo's anchor box from the sidecar and the CLI marks, for the HTML mockups.

The sidecar has crop rects and scale but no anchor box. Both parts of the box are recoverable:

- Centre: the crop keeps the anchor at one spot of the clip, so it sits at the same fraction (u, v) of
  every crop rect. (u, v) comes from photo 0, whose anchor is the centre of --box.
- Size: track() follows a box rw,rh that grows with each tracked zoom step and is replaced by every --at
  box. So rw / scale is constant between box marks: size = last box / scale there * this photo's scale.

Checked against the anchor positions anchor.py prints. Run after:
PAD=transparent analysis/girona_all.sh /tmp/pin/all.avif > /tmp/pin/all.log
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / 'analysis' / 'girona_all.sh'
SIDECAR = ROOT / 'mockups' / 'assets' / 'rects.json'
LOG = Path('/tmp/pin/all.log')
OUT = ROOT / 'mockups' / 'assets' / 'photos.js'
LINE = re.compile(r'^(IMG_\d+)\.avif\s+(\d+)x(\d+)\s+anchor (\d+),(\d+)\s+scale [\d.]+\s+(.*)$')
TOLERANCE_PX = 1.5


def read_marks() -> tuple[tuple[float, ...], dict[str, tuple[float, ...]]]:
	text = SCRIPT.read_text()
	box = tuple(float(n) for n in re.search(r'--box ([\d,]+)', text).group(1).split(','))
	ats = {
		name: tuple(float(n) for n in nums.split(','))
		for name, nums in re.findall(r'--at (IMG_\d+)\.avif=([\d.,]+)', text)
	}
	return box, ats


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
	box, ats = read_marks()
	log = read_log()
	sidecar = json.loads(SIDECAR.read_text())
	assert len(sidecar) == len(log) == 57, (len(sidecar), len(log))

	first = sidecar[0]
	fw, fh = log[first['file'][:-5]]['w'], log[first['file'][:-5]]['h']
	u = (box[0] + box[2] / 2 - first['x'] * fw) / (first['w'] * fw)
	v = (box[1] + box[3] / 2 - first['y'] * fh) / (first['h'] * fh)
	per_scale = (box[2], box[3])

	photos = []
	for row in sidecar:
		name = row['file'][:-5]
		fw, fh = log[name]['w'], log[name]['h']
		cx = (row['x'] + u * row['w']) * fw
		cy = (row['y'] + v * row['h']) * fh
		assert abs(cx - log[name]['ax']) <= TOLERANCE_PX and abs(cy - log[name]['ay']) <= TOLERANCE_PX, (
			name,
			cx,
			cy,
		)
		mark = ats.get(name)
		if mark is not None and len(mark) >= 4:
			per_scale = (mark[2] / row['scale'], mark[3] / row['scale'])
		bw, bh = per_scale[0] * row['scale'], per_scale[1] * row['scale']
		photos.append(
			{
				'name': name,
				'w': fw,
				'h': fh,
				'scale': row['scale'],
				'source': row['source'],
				'feats': log[name]['feats'],
				'a': [round(cx / fw, 4), round(cy / fh, 4), round(bw / fw, 4), round(bh / fh, 4)],
			}
		)

	OUT.write_text('const PHOTOS = ' + json.dumps(photos, separators=(',', ':')) + ';\n')
	chains = 1 + sum(p['source'] != 'tracked' for p in photos[1:])
	print(
		f'{len(photos)} photos, anchors within {TOLERANCE_PX} px of the log, {chains} linked chains -> {OUT.name}'
	)


if __name__ == '__main__':
	sys.exit(main())
