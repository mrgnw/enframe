#!/usr/bin/env -S uv run --script
# /// script
# requires-python = '>=3.12'
# dependencies = []
# ///
"""Find the internal field boundary in the 50-char preview key."""

import json
import os
from collections import Counter

CAT = os.path.expanduser(os.environ.get('ENFRAME_LR_CATALOG') or exit('set ENFRAME_LR_CATALOG to the Lightroom catalog directory'))
PAIRS = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'lr_preview_pairs.json')


def all_names() -> list[str]:
	root = f'{CAT}/previews'
	out = []
	for shard in sorted(os.listdir(root)):
		d = f'{root}/{shard}'
		if not os.path.isdir(d):
			continue
		out.extend(n for n in os.listdir(d) if len(n) == 53)
	return out


def main() -> None:
	names = all_names()
	keys = sorted({n[:50] for n in names})
	print('preview files:', len(names), 'distinct 50-char keys:', len(keys))

	print('\nlen  distinctSuffixes  distinctPrefixes  largestSuffixGroup')
	for L in range(18, 34):
		suf = Counter(k[-L:] for k in keys)
		pre = Counter(k[:-L] for k in keys)
		print(f'{L:3d}  {len(suf):16d}  {len(pre):16d}  {suf.most_common(1)[0][1]:18d}')

	for split in (24, 25, 26):
		heads = Counter(k[:split] for k in keys)
		tails = Counter(k[split:] for k in keys)
		print(
			f'\nsplit at {split}: distinct heads={len(heads)} (max group {heads.most_common(1)[0][1]}), '
			f'distinct tails={len(tails)} (max group {tails.most_common(1)[0][1]})'
		)

	with open(PAIRS) as fh:
		pairs = json.load(fh)
	multi = {a: t for a, t in pairs.items() if len({v.split('/')[1][:50] for v in t.values()}) > 1}
	print(f'\nconfirmed assets with >1 distinct key: {len(multi)} of {len(pairs)}')
	for aid, tiers in list(multi.items())[:5]:
		ks = sorted({v.split('/')[1][:50] for v in tiers.values()})
		print(f'  {aid}')
		for k in ks:
			print(f'    head24={k[:24]}  tail26={k[24:]}')


main()
