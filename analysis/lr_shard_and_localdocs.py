#!/usr/bin/env -S uv run --script
# /// script
# requires-python = '>=3.12'
# dependencies = ['msgpack']
# ///
"""Two probes: (a) what function of a preview name yields its shard dir, (b) what mcat.localDocs holds."""

import hashlib
import os
import sqlite3
import zlib

CAT = os.path.expanduser(os.environ.get('ENFRAME_LR_CATALOG') or exit('set ENFRAME_LR_CATALOG to the Lightroom catalog directory'))
ALPHABET = 'abcdefghjkmnpqrstuvwxyz123456789'


def decode(name: str) -> bytes:
	n = 0
	for ch in name:
		n = (n << 5) | ALPHABET.index(ch)
	nbits = len(name) * 5
	return (n >> (nbits % 8)).to_bytes(nbits // 8, 'big')


def shard_byte(shard: str) -> int:
	return ALPHABET.index(shard[0]) * 32 + ALPHABET.index(shard[1])


def sample(count: int) -> list[tuple[str, str]]:
	root = f'{CAT}/previews'
	out = []
	for shard in sorted(os.listdir(root)):
		d = f'{root}/{shard}'
		if not os.path.isdir(d):
			continue
		for name in sorted(os.listdir(d))[:4]:
			out.append((shard, name))
			if len(out) >= count:
				return out
	return out


def shard_probe() -> None:
	files = sample(600)
	raw = {n: decode(n) for _, n in files}
	stem = {n: decode(n[:50]) for _, n in files}
	fns = {
		'md5(name)[0]': lambda s, n: hashlib.md5(n.encode()).digest()[0],
		'md5(name)[-1]': lambda s, n: hashlib.md5(n.encode()).digest()[-1],
		'sha1(name)[0]': lambda s, n: hashlib.sha1(n.encode()).digest()[0],
		'sha256(name)[0]': lambda s, n: hashlib.sha256(n.encode()).digest()[0],
		'crc32(name)&255': lambda s, n: zlib.crc32(n.encode()) & 255,
		'crc32(name)>>24': lambda s, n: (zlib.crc32(n.encode()) >> 24) & 255,
		'sum(chars)&255': lambda s, n: sum(n.encode()) & 255,
		'raw[0]': lambda s, n: raw[n][0],
		'raw[-1]': lambda s, n: raw[n][-1],
		'stem[0]': lambda s, n: stem[n][0],
		'stem[-1]': lambda s, n: stem[n][-1],
		'md5(stembytes)[0]': lambda s, n: hashlib.md5(stem[n]).digest()[0],
		'idx(c0,c1)': lambda s, n: ALPHABET.index(n[0]) * 32 + ALPHABET.index(n[1]),
		'idx(c48,c49)': lambda s, n: ALPHABET.index(n[48]) * 32 + ALPHABET.index(n[49]),
	}
	print('=== shard-derivation probe over', len(files), 'files')
	for label, fn in fns.items():
		ok = sum(1 for s, n in files if fn(s, n) == shard_byte(s))
		print(f'  {label:22s} {ok}/{len(files)}')
	s, n = files[0]
	print(f'  sample {s}/{n} shardByte={shard_byte(s)} raw={raw[n].hex()}')

	def bits(name: str) -> int:
		v = 0
		for ch in name:
			v = (v << 5) | ALPHABET.index(ch)
		return v

	total_bits = 53 * 5
	for label, target in (
		('big-endian shard', shard_byte),
		('little-endian shard', lambda s: ALPHABET.index(s[1]) * 8 + ALPHABET.index(s[0])),
	):
		best = []
		for off in range(total_bits - 7):
			ok = 0
			for s, n in files:
				window = (bits(n) >> (total_bits - 8 - off)) & 0xFF
				if window == target(s):
					ok += 1
			if ok > len(files) // 10:
				best.append((ok, off))
		print(f'  {label}: bit-window matches {best or "none"}')


def localdocs_probe() -> None:
	c = sqlite3.connect(f'file:{CAT}/Managed Catalog.mcat?mode=ro', uri=True)
	c.text_factory = bytes
	print('\n=== mcat aux tables')
	for t in ('localDocs', 'labels', 'revs'):
		cols = [r[1] for r in c.execute(f'PRAGMA table_info({t})')]
		n = c.execute(f'SELECT count(*) FROM {t}').fetchone()[0]
		print(f'  {t}: rows={n} cols={cols}')
		for row in c.execute(f'SELECT * FROM {t} LIMIT 2'):
			print('    ', str(row)[:300])


shard_probe()
localdocs_probe()
