#!/usr/bin/env -S uv run --script
# /// script
# requires-python = '>=3.12'
# dependencies = ['pillow', 'numpy']
# ///
"""Validate the reference_size pairing: assets matched to >1 preview key should be the same photo.

If the two keys really are two render generations of one asset, their thumbnails are near-identical.
If the reference_size match produced a false positive, they are unrelated images.
"""

import json
import os

import numpy as np
from PIL import Image

CAT = os.path.expanduser('~/Pictures/Lightroom Library.lrlibrary/00000000000000000000000000000000')
PAIRS = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'lr_preview_pairs.json')


def dhash(path: str) -> np.ndarray:
	img = Image.open(path).convert('L').resize((9, 8), Image.BILINEAR)
	a = np.asarray(img, dtype=np.int16)
	return (a[:, 1:] > a[:, :-1]).flatten()


def main() -> None:
	with open(PAIRS) as fh:
		pairs = json.load(fh)

	by_key: dict[str, dict[str, list[str]]] = {}
	for aid, tiers in pairs.items():
		for rel in tiers.values():
			by_key.setdefault(aid, {}).setdefault(rel.split('/')[1][:50], []).append(rel)
	multi = {a: k for a, k in by_key.items() if len(k) > 1}
	print(f'assets with >1 key: {len(multi)} / {len(pairs)}')

	dists = []
	for aid, keys in multi.items():
		reps = [sorted(v)[0] for v in keys.values()]
		try:
			hs = [dhash(f'{CAT}/previews/{r}') for r in reps]
		except OSError:
			continue
		d = min(int(np.count_nonzero(hs[i] != hs[j])) for i in range(len(hs)) for j in range(i + 1, len(hs)))
		dists.append((d, aid, reps))

	dists.sort()
	same = sum(1 for d, _, _ in dists if d <= 8)
	print(f'compared {len(dists)} assets; hamming<=8 (same photo): {same}; >8: {len(dists) - same}')
	print('\nclosest:')
	for d, aid, reps in dists[:3]:
		print(f'  d={d:2d} {aid} {reps}')
	print('furthest:')
	for d, aid, reps in dists[-3:]:
		print(f'  d={d:2d} {aid} {reps}')


main()
