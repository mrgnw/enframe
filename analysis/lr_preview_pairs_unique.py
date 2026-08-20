#!/usr/bin/env -S uv run --script
# /// script
# requires-python = '>=3.12'
# dependencies = []
# ///
"""Build confirmed (assetId -> preview file) pairs.

Every previews/ JPEG carries an APP1 segment 'com.adobe.carousel.reference_size' holding the
pixel dimensions of the image the render came from. For the largest tier that equals the asset's
own dimensions, so assets whose (width, height) is unique in the catalog can be matched 1:1.
"""

import json
import os
import sqlite3
import struct

CAT = os.path.expanduser('~/Pictures/Lightroom Library.lrlibrary/00000000000000000000000000000000')
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'lr_preview_pairs.json')
MARKER = b'com.adobe.carousel.reference_size\x00'
ALPHABET = 'abcdefghjkmnpqrstuvwxyz123456789'


def reference_size(path: str) -> tuple[int, int] | None:
	try:
		with open(path, 'rb') as fh:
			head = fh.read(256)
	except OSError:
		return None
	i = head.find(MARKER)
	if i < 0:
		return None
	return struct.unpack('>II', head[i + len(MARKER) : i + len(MARKER) + 8])


def long_edge(name: str) -> int:
	n = 0
	for ch in name[50:53]:
		n = n * 32 + ALPHABET.index(ch)
	return n * 2


def unique_dims() -> dict[tuple[int, int], str]:
	wf = sqlite3.connect(f'file:{CAT}/Managed Catalog.wfindex?mode=ro', uri=True)
	counts: dict[tuple[int, int], list[str]] = {}
	for aid, w, h in wf.execute('SELECT assetId, width, height FROM assets WHERE width IS NOT NULL'):
		counts.setdefault((int(w), int(h)), []).append(aid)
	return {k: v[0] for k, v in counts.items() if len(v) == 1 and max(k) > 2600}


def main() -> None:
	uniq = unique_dims()
	print('assets with globally unique dimensions:', len(uniq))

	root = f'{CAT}/previews'
	pairs: dict[str, dict[int, str]] = {}
	scanned = 0
	for shard in sorted(os.listdir(root)):
		d = f'{root}/{shard}'
		if not os.path.isdir(d):
			continue
		for name in os.listdir(d):
			if len(name) != 53:
				continue
			scanned += 1
			ref = reference_size(f'{d}/{name}')
			if not ref:
				continue
			aid = uniq.get(ref) or uniq.get((ref[1], ref[0]))
			if aid:
				pairs.setdefault(aid, {})[long_edge(name)] = f'{shard}/{name}'
	print('previews scanned:', scanned)
	print('assets matched  :', len(pairs))

	with open(OUT, 'w') as fh:
		json.dump(pairs, fh, indent=1, sort_keys=True)
	print('wrote', OUT)

	for aid, tiers in list(pairs.items())[:6]:
		stems = {v.split('/')[1][:50] for v in tiers.values()}
		print(f'\n{aid}')
		for size, rel in sorted(tiers.items()):
			print(f'   {size:5d}  {rel}')
		print('   stems:', stems)


main()
