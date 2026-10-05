"""Check a run's crop rects against a baseline: uv run analysis/compare_rects.py out/base.json out/all.json"""

import json
import sys


def photos(path: str) -> list[dict]:
	"""The per-photo rows, from a sidecar or an older one that was a bare list."""
	data = json.loads(open(path).read())
	return data if isinstance(data, list) else data['photos']


base, new = (photos(p) for p in sys.argv[1:3])
assert len(base) == len(new), f'{len(base)} vs {len(new)} photos'
worst = 0.0
for a, b in zip(base, new):
	assert a['file'] == b['file'] and a['source'] == b['source'], (a, b)
	assert abs(a['scale'] - b['scale']) <= 1e-4, (a, b)
	worst = max(worst, *(abs(a[k] - b[k]) for k in 'xywh'))
lost = [row['file'] for row in new if row['source'] == 'lost']
assert worst <= 1e-4, f'rects differ by {worst:.2e}'
print(f'match: max rect diff {worst:.2e}, {len(lost)} lost: {", ".join(lost)}')
