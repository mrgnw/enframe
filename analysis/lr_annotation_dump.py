#!/usr/bin/env -S uv run --script
# /// script
# requires-python = '>=3.12'
# dependencies = ['msgpack']
# ///
import json
import os
import sqlite3

import msgpack

CAT = os.path.expanduser('~/Pictures/Lightroom Library.lrlibrary/00000000000000000000000000000000')
CLOUD_ONLY = '1b3e1705cc114c40b4beae8e0690bcff'


def shorten(v, depth=0):
	if isinstance(v, dict):
		return {k: shorten(x, depth + 1) for k, x in v.items()}
	if isinstance(v, list):
		return [shorten(x, depth + 1) for x in v[:3]]
	if isinstance(v, bytes):
		return f'<bytes {len(v)}> {v[:24].hex()}'
	if isinstance(v, str) and len(v) > 120:
		return v[:120] + '...'
	return v


def main() -> None:
	wf = sqlite3.connect(f'file:{CAT}/Managed Catalog.wfindex?mode=ro', uri=True)
	ids = [
		r[0]
		for r in wf.execute(
			"SELECT assetId FROM assets WHERE subtype='image' ORDER BY captureDate DESC LIMIT 2"
		)
	]
	ids.append(CLOUD_ONLY)

	c = sqlite3.connect(f'file:{CAT}/Managed Catalog.mcat?mode=ro', uri=True)
	c.text_factory = bytes
	print('mcat tables:', [r[0] for r in c.execute("SELECT name FROM sqlite_master WHERE type='table'")])
	print('docs cols:', [r[1] for r in c.execute('PRAGMA table_info(docs)')])
	print('doc types:', c.execute('SELECT type, count(*) FROM docs GROUP BY type').fetchall())

	for aid in ids:
		row = c.execute(
			"SELECT annotation FROM docs WHERE fullDocId=? AND type='asset' AND deleted=0", (aid,)
		).fetchone()
		print(f'\n===== {aid} =====')
		if not row or not row[0]:
			print('  no annotation')
			continue
		ann = msgpack.unpackb(row[0], raw=False, strict_map_key=False)
		print(json.dumps(shorten(ann), indent=1, default=str)[:6000])


main()
