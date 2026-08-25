#!/usr/bin/env -S uv run --with httpx --script
"""Check the serve exposure policy from a non-loopback address."""

import subprocess
from pathlib import Path

import httpx

TAILNET = subprocess.run(['tailscale', 'ip', '-4'], capture_output=True, text=True).stdout.strip()
PORT = 5911
FOLDER = str(Path.home() / 'Pictures/samples')
OUTSIDE = '/etc'


def post(host: str, command: str, body: dict) -> httpx.Response:
	return httpx.post(f'http://{host}:{PORT}/api/{command}', json=body, timeout=20)


def get(host: str, path: str) -> httpx.Response:
	return httpx.get(f'http://{host}:{PORT}{path}', timeout=20)


checks = []

r = post(TAILNET, 'scan_inbox', {'directory': FOLDER})
checks.append(('remote scan_inbox inside a read root', r.status_code, 200))
if r.status_code == 200:
	print(f'  scanned {len(r.json())} images remotely')

r = post(TAILNET, 'scan_inbox', {'directory': OUTSIDE})
checks.append(('remote scan_inbox outside the roots', r.status_code, 403))

r = get(TAILNET, '/api/ls')
checks.append(('remote file browser', r.status_code, 403))

r = post(TAILNET, 'save_image', {'path': f'{FOLDER}/probe.png', 'data': ''})
checks.append(('remote caller-chosen write', r.status_code, 403))

r = httpx.post(
	f'http://{TAILNET}:{PORT}/sql/select', json={'sql': 'SELECT 1 AS one', 'values': []}, timeout=20
)
checks.append(('remote database read', r.status_code, 200))

r = post('127.0.0.1', 'scan_inbox', {'directory': FOLDER})
checks.append(('loopback scan_inbox', r.status_code, 200))

r = get('127.0.0.1', '/api/ls')
checks.append(('loopback file browser', r.status_code, 200))

failed = 0
for label, got, want in checks:
	ok = got == want
	failed += not ok
	print(f'{"ok  " if ok else "FAIL"} {label}: {got} (want {want})')

print(f'\n{TAILNET} — {len(checks) - failed}/{len(checks)} as expected')
assert failed == 0, f'{failed} checks failed'
assert not Path(f'{FOLDER}/probe.png').exists(), 'refused write touched the disk'
