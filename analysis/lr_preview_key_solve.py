#!/usr/bin/env -S uv run --script
# /// script
# requires-python = '>=3.12'
# dependencies = ['msgpack']
# ///
"""Given confirmed assetId -> preview pairs, search for the digest that produces the filename."""

import hashlib
import itertools
import json
import os
import sqlite3
import zlib

import msgpack

CAT = os.path.expanduser('~/Pictures/Lightroom Library.lrlibrary/00000000000000000000000000000000')
CATALOG_ID = '00000000000000000000000000000000'
PAIRS = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'lr_preview_pairs.json')
ALPHABET = 'abcdefghjkmnpqrstuvwxyz123456789'
PROBE = 10


def b32(data: bytes, alphabet: str = ALPHABET) -> str:
	n = int.from_bytes(data, 'big')
	nbits = len(data) * 8
	nchars = (nbits + 4) // 5
	n <<= nchars * 5 - nbits
	return ''.join(alphabet[(n >> (5 * (nchars - 1 - i))) & 31] for i in range(nchars))


def b32_le(data: bytes, alphabet: str = ALPHABET) -> str:
	n = int.from_bytes(data, 'little')
	nchars = (len(data) * 8 + 4) // 5
	return ''.join(alphabet[(n >> (5 * i)) & 31] for i in range(nchars))


def annotation(mcat: sqlite3.Connection, aid: str) -> dict | None:
	row = mcat.execute(
		"SELECT annotation FROM docs WHERE fullDocId=? AND type='asset' AND deleted=0", (aid,)
	).fetchone()
	if not row or not row[0]:
		return None
	return msgpack.unpackb(row[0], raw=False, strict_map_key=False)


def string_atoms(aid: str, ann: dict) -> dict[str, str]:
	server = ann.get('_serverProvidedMetadata') or {}
	local = ann.get('_localOnly') or {}
	links = server.get('links') or {}
	files = local.get('files') or {}
	atoms = {'asset': aid, 'catalog': CATALOG_ID}
	for i, rev in enumerate(server.get('revision_ids') or []):
		atoms[f'rev{i}'] = rev
	for key, tag in (
		('/rels/xmp/develop', 'develop'),
		('/rels/proxy_type/2560', 'proxy2560'),
		('/rels/master', 'master'),
	):
		sha = (links.get(key) or {}).get('sha256')
		if sha:
			atoms[tag] = sha
	for kind in ('xmp_develop', 'proxy'):
		sha = (files.get(kind) or {}).get('sha256')
		if sha:
			atoms[f'f_{kind}'] = sha
	orig = (files.get('original') or {}).get('master') or {}
	if orig.get('sha256'):
		atoms['f_original'] = orig['sha256']
	prof = local.get('cameraProfileInfo') or {}
	for k in ('profileDigest', 'xmpCameraRawProfileDigest'):
		if prof.get(k):
			atoms[k] = prof[k]
	return atoms


def settings_blob(atoms: dict[str, str]) -> bytes | None:
	sha = atoms.get('f_xmp_develop') or atoms.get('develop')
	if not sha:
		return None
	path = f'{CAT}/settings/{sha}'
	try:
		return open(path, 'rb').read()
	except OSError:
		return None


def payloads(atoms: dict[str, str]) -> dict[str, bytes]:
	out: dict[str, bytes] = {}
	blob = settings_blob(atoms)
	if blob is not None:
		out['settings'] = blob
		out['asset+settings'] = atoms['asset'].encode() + blob
		out['settings+asset'] = blob + atoms['asset'].encode()
		out['assetBytes+settings'] = bytes.fromhex(atoms['asset']) + blob
	for name, val in atoms.items():
		out[name] = val.encode()
		out[f'{name}^upper'] = val.upper().encode()
		try:
			out[f'{name}^bytes'] = bytes.fromhex(val)
		except ValueError:
			pass
	for a, b in itertools.permutations([k for k in atoms if k != 'catalog'], 2):
		for sep in ('', '/', ':', '-'):
			out[f'{a}{sep}{b}'] = f'{atoms[a]}{sep}{atoms[b]}'.encode()
	for a in atoms:
		out[f'catalog/{a}'] = f'{CATALOG_ID}/{atoms[a]}'.encode()
		out[f'catalog{a}'] = f'{CATALOG_ID}{atoms[a]}'.encode()
	for a, b in itertools.permutations([k for k in atoms if k != 'catalog'], 2):
		try:
			out[f'{a}^b+{b}^b'] = bytes.fromhex(atoms[a]) + bytes.fromhex(atoms[b])
		except ValueError:
			pass
	return out


def encodings(digest: bytes) -> dict[str, str]:
	rev = digest[::-1]
	return {
		'be': b32(digest),
		'bitrev': b32(bytes(int(f'{b:08b}'[::-1], 2) for b in digest)),
		'be^rev': b32(rev),
		'le': b32_le(digest),
		'le^rev': b32_le(rev),
	}


def main() -> None:
	with open(PAIRS) as fh:
		pairs = json.load(fh)
	targets = []
	for aid, tiers in pairs.items():
		for _size, rel in tiers.items():
			targets.append((aid, rel.split('/')[1][:50]))
	print('ground-truth pairs:', len(targets))

	mcat = sqlite3.connect(f'file:{CAT}/Managed Catalog.mcat?mode=ro', uri=True)
	mcat.text_factory = bytes

	algs = {
		'md5': lambda b: hashlib.md5(b).digest(),
		'sha1': lambda b: hashlib.sha1(b).digest(),
		'sha256': lambda b: hashlib.sha256(b).digest(),
		'sha512': lambda b: hashlib.sha512(b).digest(),
		'blake2b32': lambda b: hashlib.blake2b(b, digest_size=32).digest(),
		'crc32': lambda b: zlib.crc32(b).to_bytes(4, 'big'),
	}

	hits: dict[str, int] = {}
	tried = 0
	for aid, stem in targets:
		ann = annotation(mcat, aid)
		if not ann:
			continue
		atoms = string_atoms(aid, ann)
		for label, payload in payloads(atoms).items():
			for alg, fn in algs.items():
				for enc_name, enc in encodings(fn(payload)).items():
					tried += 1
					for off in range(4):
						if enc[off : off + PROBE] and enc[off : off + PROBE] == stem[:PROBE]:
							key = f'{alg}({label}) {enc_name}[{off}:]'
							hits[key] = hits.get(key, 0) + 1
							print('HIT', key, aid, stem)
	print('combinations tried:', tried)
	print('hits:', hits or 'none')


main()
