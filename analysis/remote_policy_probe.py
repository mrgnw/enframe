#!/usr/bin/env -S uv run --with httpx --script
"""Check what a non-loopback caller can reach on a running serve instance.

Roots are the only boundary: every route answers any caller, and where the path lands is
what bounds it. Point this at a sandbox instance rather than the live one when the folder
is going to be written to.

	ENFRAME_PROBE_PORT=5923 ENFRAME_PROBE_FOLDER=/tmp/enframe_p2/photos uv run analysis/remote_policy_probe.py
"""

import os
import subprocess
from pathlib import Path

import httpx

TAILNET = subprocess.run(['tailscale', 'ip', '-4'], capture_output=True, text=True).stdout.strip()
PORT = int(os.environ.get('ENFRAME_PROBE_PORT', '5911'))
# A directory that is both a read and a write root on the instance being probed.
FOLDER = os.environ.get('ENFRAME_PROBE_FOLDER', str(Path.home() / 'Pictures'))
OUTSIDE = '/etc'
PROBE_FILE = f'{FOLDER}/enframe_probe.png'


def post(host: str, path: str, body: dict) -> httpx.Response:
	return httpx.post(f'http://{host}:{PORT}{path}', json=body, timeout=20)


def get(host: str, path: str) -> httpx.Response:
	return httpx.get(f'http://{host}:{PORT}{path}', timeout=20)


checks = []


def check(label: str, response: httpx.Response, want: int) -> httpx.Response:
	checks.append((label, response.status_code, want))
	return response


r = check(
	'remote folder scan, inside a read root', post(TAILNET, '/api/scan_inbox', {'directory': FOLDER}), 200
)
if r.status_code == 200:
	print(f'  scanned {len(r.json())} images remotely')

check('remote folder scan, outside the roots', post(TAILNET, '/api/scan_inbox', {'directory': OUTSIDE}), 403)

# The file browser used to be loopback-only, which left a connected app unable to pick
# anything on the instance it was connected to.
check('remote file browser, default directory', get(TAILNET, '/api/ls'), 200)
check('remote file browser, inside a read root', get(TAILNET, f'/api/ls?dir={FOLDER}'), 200)
check('remote file browser, outside the roots', get(TAILNET, f'/api/ls?dir={OUTSIDE}'), 403)

check('remote thumbnail, outside the roots', post(TAILNET, '/api/raw_thumbnail', {'path': '/etc/hosts'}), 403)

# The trade the policy change makes: a remote write lands, but only in a write root.
check(
	'remote write, inside a write root',
	post(TAILNET, '/api/save_image', {'path': PROBE_FILE, 'data': ''}),
	200,
)
landed = Path(PROBE_FILE).exists()
Path(PROBE_FILE).unlink(missing_ok=True)

escape = '/etc/enframe_probe.png'
check(
	'remote write, outside the write roots',
	post(TAILNET, '/api/save_image', {'path': escape, 'data': ''}),
	403,
)

check('remote database read', post(TAILNET, '/sql/select', {'sql': 'SELECT 1 AS one', 'values': []}), 200)
check('loopback still works', get('127.0.0.1', '/api/ls'), 200)

failed = 0
for label, got, want in checks:
	ok = got == want
	failed += not ok
	print(f'{"ok  " if ok else "FAIL"} {label}: {got} (want {want})')

print(f'\n{TAILNET}:{PORT} — {len(checks) - failed}/{len(checks)} as expected')
assert failed == 0, f'{failed} checks failed'
assert landed, 'an allowed write never reached the disk'
assert not Path(escape).exists(), 'refused write touched the disk'
