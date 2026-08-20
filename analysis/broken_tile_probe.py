#!/usr/bin/env -S uv run --script
# /// script
# requires-python = '>=3.12'
# dependencies = ['msgpack']
# ///
import os
import sqlite3

import msgpack

CAT = os.path.expanduser('~/Pictures/Lightroom Library.lrlibrary/00000000000000000000000000000000')
BROKEN = ['IMG_0001.ARW', 'IMG_0001.ARW', 'IMG_0001.ARW', 'IMG_0001.ARW', 'IMG_0001.ARW']
WORKING = ['IMG_0001.ARW', 'IMG_0001.ARW', 'IMG_0001.ARW', 'IMG_0001.ARW']

wf = sqlite3.connect(f'file:{CAT}/Managed Catalog.wfindex?mode=ro', uri=True)
c = sqlite3.connect(f'file:{CAT}/Managed Catalog.mcat?mode=ro', uri=True)
c.text_factory = bytes


def paths_for(filename: str):
	row = wf.execute(
		'SELECT a.assetId FROM assets a WHERE a.filename = ? LIMIT 1', (filename,)
	).fetchone()
	if not row:
		return None
	aid = row[0]
	doc = c.execute(
		"SELECT annotation FROM docs WHERE fullDocId = ? AND type = 'asset' AND deleted = 0", (aid,)
	).fetchone()
	if not doc or not doc[0]:
		return {'asset_id': aid, 'annotation': None}
	local = msgpack.unpackb(doc[0], raw=False, strict_map_key=False).get('_localOnly') or {}
	files = local.get('files') or {}
	master = (files.get('original') or {}).get('master') or {}
	proxy = (files.get('proxy') or {}).get('2560') or {}
	out = {'asset_id': aid}
	for label, rel in [('original', master.get('relativePath')), ('proxy', proxy.get('relativePath'))]:
		if rel:
			full = '/' + rel
			out[label] = (full, 'EXISTS' if os.path.exists(full) else 'MISSING')
		else:
			out[label] = None
	return out


for group, names in [('BROKEN', BROKEN), ('WORKING', WORKING)]:
	print(f'== {group} ==')
	for name in names:
		print(f'{name}: {paths_for(name)}')
