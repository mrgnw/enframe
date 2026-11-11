#!/usr/bin/env -S uv run --script
# /// script
# requires-python = '>=3.12'
# dependencies = []
# ///
import os
import sqlite3
import sys

CAT = os.path.expanduser(os.environ.get('ENFRAME_LR_CATALOG') or exit('set ENFRAME_LR_CATALOG to the Lightroom catalog directory'))
DBS = (
	'Managed Catalog.mru',
	'Managed Catalog.notifdb',
	'cullingScores.db',
	'Managed Catalog.wfindex',
)
WANT = ('purge', 'aux', 'path', 'preview', 'rendition', 'cache', 'local', 'index')


def main() -> None:
	only = sys.argv[1] if len(sys.argv) > 1 else None
	for db in DBS:
		c = sqlite3.connect(f'file:{CAT}/{db}?immutable=1', uri=True)
		print(f'\n########## {db}')
		for (sql,) in c.execute('SELECT sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name'):
			name = sql.split('(')[0]
			if only and only not in sql.lower():
				continue
			if not only and not any(w in name.lower() for w in WANT):
				continue
			print(sql.replace('\n', ' ')[:400])
			table = name.split()[-1].strip('"[]`')
			try:
				n = c.execute(f'SELECT count(*) FROM "{table}"').fetchone()[0]
				rows = c.execute(f'SELECT * FROM "{table}" LIMIT 2').fetchall()
				print(f'   rows={n} sample={str(rows)[:400]}')
			except sqlite3.Error as e:
				print('   (', e, ')')


main()
