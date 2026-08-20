#!/usr/bin/env -S uv run --script
# /// script
# requires-python = '>=3.12'
# dependencies = []
# ///
"""List every JPEG marker segment in a previews/ file, plus any trailing bytes after EOI."""

import os
import sys

CAT = os.path.expanduser('~/Pictures/Lightroom Library.lrlibrary/00000000000000000000000000000000')
NAMES = {0xE0 + i: f'APP{i}' for i in range(16)} | {
	0xDB: 'DQT',
	0xC0: 'SOF0',
	0xC2: 'SOF2',
	0xC4: 'DHT',
	0xDA: 'SOS',
	0xFE: 'COM',
	0xD9: 'EOI',
}


def walk(path: str) -> None:
	data = open(path, 'rb').read()
	print(f'\n=== {path} ({len(data)} bytes)')
	i = 2
	while i < len(data) - 1:
		if data[i] != 0xFF:
			print(f'  {i}: desync {data[i]:02x}')
			break
		m = data[i + 1]
		if m == 0xDA:
			eoi = data.rfind(b'\xff\xd9')
			print(f'  {i}: SOS -> scan; EOI at {eoi}; trailing={len(data) - eoi - 2} bytes')
			if len(data) - eoi - 2 > 0:
				print('    trailer:', data[eoi + 2 : eoi + 130])
			return
		size = int.from_bytes(data[i + 2 : i + 4], 'big')
		body = data[i + 4 : i + 2 + size]
		label = NAMES.get(m, f'0x{m:02x}')
		note = ''
		if label.startswith('APP') or label == 'COM':
			note = repr(body[:96])
		print(f'  {i}: {label} len={size} {note}')
		i += 2 + size


def main() -> None:
	if len(sys.argv) > 1:
		targets = sys.argv[1:]
	else:
		root = f'{CAT}/previews'
		targets = []
		for shard in sorted(os.listdir(root))[:2]:
			d = f'{root}/{shard}'
			for name in sorted(os.listdir(d))[:2]:
				targets.append(f'{d}/{name}')
	for t in targets:
		walk(t if t.startswith('/') else f'{CAT}/previews/{t}')


main()
