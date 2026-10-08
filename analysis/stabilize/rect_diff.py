"""Report how far a run's crop rects are from a baseline, without a pass/fail threshold:
uv run analysis/stabilize/rect_diff.py out/base.json out/rust.json"""

import json
import sys


def photos(path: str) -> list[dict]:
	data = json.loads(open(path).read())
	return data if isinstance(data, list) else data['photos']


base, new = (photos(p) for p in sys.argv[1:3])
assert [a['file'] for a in base] == [b['file'] for b in new], 'different photos'
rect = max(abs(a[k] - b[k]) for a, b in zip(base, new) for k in 'xywh')
scale = max(abs(a['scale'] - b['scale']) for a, b in zip(base, new))
worst = max(zip(base, new), key=lambda ab: max(abs(ab[0][k] - ab[1][k]) for k in 'xywh'))[0]['file']
sources = [a['file'] for a, b in zip(base, new) if a['source'] != b['source']]
lost = [b['file'] for b in new if b['source'] == 'lost']
print(
	f'max rect diff {rect:.2e} (at {worst}), max scale diff {scale:.1e}, sources differ: {sources or "none"}'
)
print(f'{len(lost)} lost: {", ".join(lost)}')
