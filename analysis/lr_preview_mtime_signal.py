#!/usr/bin/env -S uv run --script
# /// script
# requires-python = '>=3.12'
# dependencies = ['msgpack']
# ///
"""Is the settings/<sha256> mtime a usable second discriminator for matching previews to assets?

Measured over the reference_size-confirmed pairs.
"""

import json
import os
import sqlite3
from collections import Counter

import msgpack

CAT = os.path.expanduser(os.environ.get('ENFRAME_LR_CATALOG') or exit('set ENFRAME_LR_CATALOG to the Lightroom catalog directory'))
PAIRS = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'lr_preview_pairs.json')


def develop_sha(mcat: sqlite3.Connection, aid: str) -> str | None:
	row = mcat.execute(
		"SELECT annotation FROM docs WHERE fullDocId=? AND type='asset' AND deleted=0", (aid,)
	).fetchone()
	if not row or not row[0]:
		return None
	ann = msgpack.unpackb(row[0], raw=False, strict_map_key=False)
	files = ((ann.get('_localOnly') or {}).get('files')) or {}
	return (files.get('xmp_develop') or {}).get('sha256')


def main() -> None:
	with open(PAIRS) as fh:
		pairs = json.load(fh)
	mcat = sqlite3.connect(f'file:{CAT}/Managed Catalog.mcat?mode=ro', uri=True)
	mcat.text_factory = bytes

	buckets = Counter()
	deltas = []
	for aid, tiers in pairs.items():
		sha = develop_sha(mcat, aid)
		if not sha:
			buckets['no settings file'] += 1
			continue
		try:
			s_mtime = os.path.getmtime(f'{CAT}/settings/{sha}')
		except OSError:
			buckets['settings missing on disk'] += 1
			continue
		best = min(abs(os.path.getmtime(f'{CAT}/previews/{rel}') - s_mtime) for rel in tiers.values())
		deltas.append(best)
		if best < 60:
			buckets['<1 min'] += 1
		elif best < 3600:
			buckets['<1 hour'] += 1
		elif best < 86400:
			buckets['<1 day'] += 1
		else:
			buckets['>=1 day'] += 1

	print('confirmed pairs examined:', len(pairs))
	print('closest preview-vs-settings mtime delta:', dict(buckets))
	if deltas:
		deltas.sort()
		q = [deltas[int(len(deltas) * p)] for p in (0.1, 0.25, 0.5, 0.75, 0.9)]
		print('delta percentiles (s) p10/p25/p50/p75/p90:', [round(x) for x in q])


main()
