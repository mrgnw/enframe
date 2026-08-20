#!/usr/bin/env -S uv run --script
# /// script
# requires-python = '>=3.12'
# dependencies = []
# ///
"""Resolve a Lightroom assetId to its local previews/ JPEGs.

Filename format (proven):
	previews/<shard 2 chars>/<24-char headfield><26-char tailfield><3-char size>
  - alphabet is base32 'abcdefghjkmnpqrstuvwxyz123456789' (a=0 .. 9=31; no i/l/o/0)
  - the trailing 3 chars are a big-endian base32 integer equal to longEdge / 2
	(afa=160 -> 320px, ama=320 -> 640px, axa=640 -> 1280px, baa=1024 -> 2048px)
  - the 50-char key identifies one render generation of one asset; every tier of a
	generation shares the key and the shard dir
  - the key splits cleanly at 24/26 chars (120 + 130 bits): the 26-char tail is shared by up
	to 1956 renders, the 24-char head is unique per render. Both change when develop settings
	change, so neither is a stable asset identifier
  - the key is NOT derivable from the assetId: lr_preview_key_solve.py tried 90M
	hash x payload x encoding combinations against 2003 confirmed pairs with zero hits, and
	the shard is not any bit window of the name (lr_shard_and_localdocs.py)

So the assetId -> file mapping is recovered by scanning instead: every preview JPEG starts with
an APP1 segment 'com.adobe.carousel.reference_size' holding the WxH of the image it was rendered
from, which for the top tier equals the asset's own dimensions. That is only decisive for assets
whose dimensions are unique in the catalog (1627 of 24887 here).

Usage:  uv run analysis/lr_preview_lookup.py [assetId ...]
"""

import os
import struct
import sqlite3
import sys

CAT = os.path.expanduser('~/Pictures/Lightroom Library.lrlibrary/00000000000000000000000000000000')
MARKER = b'com.adobe.carousel.reference_size\x00'
ALPHABET = 'abcdefghjkmnpqrstuvwxyz123456789'
DEFAULT_IDS = (
	'1b3e1705cc114c40b4beae8e0690bcff',
	'00a0493cc09f42b48043b22c97647d15',
)


def long_edge(name: str) -> int:
	n = 0
	for ch in name[50:53]:
		n = n * 32 + ALPHABET.index(ch)
	return n * 2


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


def jpeg_size(path: str) -> tuple[int, int] | None:
	data = open(path, 'rb').read(4096)
	i = 2
	while i < len(data) - 9:
		if data[i] != 0xFF:
			return None
		m = data[i + 1]
		size = int.from_bytes(data[i + 2 : i + 4], 'big')
		if m in (0xC0, 0xC1, 0xC2):
			h, w = struct.unpack('>HH', data[i + 5 : i + 9])
			return w, h
		i += 2 + size
	return None


def scan_previews() -> dict[tuple[int, int], list[str]]:
	root = f'{CAT}/previews'
	index: dict[tuple[int, int], list[str]] = {}
	for shard in sorted(os.listdir(root)):
		d = f'{root}/{shard}'
		if not os.path.isdir(d):
			continue
		for name in os.listdir(d):
			if len(name) != 53:
				continue
			ref = reference_size(f'{d}/{name}')
			if ref:
				index.setdefault(ref, []).append(f'{shard}/{name}')
	return index


def main() -> None:
	ids = sys.argv[1:] or list(DEFAULT_IDS)
	wf = sqlite3.connect(f'file:{CAT}/Managed Catalog.wfindex?mode=ro', uri=True)

	dim_counts: dict[tuple[int, int], int] = {}
	for w, h in wf.execute('SELECT width, height FROM assets WHERE width IS NOT NULL'):
		dim_counts[(int(w), int(h))] = dim_counts.get((int(w), int(h)), 0) + 1

	index = scan_previews()
	print(f'indexed {sum(len(v) for v in index.values())} previews over {len(index)} distinct source sizes\n')

	for aid in ids:
		row = wf.execute(
			'SELECT width, height, filename, subtype FROM assets WHERE assetId=?', (aid,)
		).fetchone()
		if not row:
			print(f'{aid}: not in wfindex.assets')
			continue
		w, h, fn, subtype = int(row[0]), int(row[1]), row[2], row[3]
		hits = index.get((w, h), []) + index.get((h, w), [])
		share = dim_counts.get((w, h), 0) + (dim_counts.get((h, w), 0) if (h, w) != (w, h) else 0)
		verdict = 'UNAMBIGUOUS' if share == 1 else f'ambiguous ({share} assets share {w}x{h})'
		print(f'{aid}  {fn}  {subtype}  {w}x{h}  -> {len(hits)} preview(s), {verdict}')
		for rel in sorted(hits, key=lambda r: long_edge(r.split('/')[1]))[:8]:
			path = f'{CAT}/previews/{rel}'
			print(
				f'    {long_edge(rel.split("/")[1]):5d}px tier  {jpeg_size(path)}  '
				f'{os.path.getsize(path):>8} bytes  {rel}'
			)
		print()


main()
