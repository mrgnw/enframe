#!/usr/bin/env -S uv run --with playwright --script
"""Drive a browser client of a serve instance and report what actually rendered.

ENFRAME_CHECK_PORT=5923 ENFRAME_CHECK_FOLDER=/tmp/enframe_p2/photos uv run --with playwright analysis/browser_connection_check.py
"""

import os

from playwright.sync_api import sync_playwright

PORT = os.environ.get('ENFRAME_CHECK_PORT', '5911')
FOLDER = os.environ.get('ENFRAME_CHECK_FOLDER', '')
BASE = f'http://127.0.0.1:{PORT}'

with sync_playwright() as p:
	browser = p.chromium.launch()
	page = browser.new_page(viewport={'width': 1400, 'height': 900})
	errors: list[str] = []
	page.on('console', lambda m: errors.append(m.text) if m.type == 'error' else None)
	page.on('pageerror', lambda e: errors.append(str(e)))

	page.goto(f'{BASE}/library', wait_until='networkidle')
	page.wait_for_timeout(1500)
	print(f'library: {page.inner_text("body")[:70].replace(chr(10), " / ")!r}')
	print(f'tabs: {[t.strip() for t in page.locator("nav button").all_inner_texts()][:6]}')

	# A browser client must never see the remote pill: it is served by its own instance.
	print(f'remote pill present: {page.locator(".remote-pill").count() > 0}')

	if FOLDER:
		page.evaluate('folder => localStorage.setItem("enframe.folders", JSON.stringify([folder]))', FOLDER)
		page.evaluate('localStorage.setItem("enframe.library.tab", "folders")')
		page.reload(wait_until='networkidle')
		page.wait_for_timeout(4000)
		tiles = page.locator('.photo-item')
		print(f'folder tiles: {tiles.count()}')
		print(f'  rendered images: {page.locator(".photo-item img").count()}')
		print(f'  RAW placeholders: {page.locator(".photo-item .placeholder").count()}')
		for i in range(min(tiles.count(), 4)):
			img = tiles.nth(i).locator('img')
			if img.count() == 0:
				print(f'  [{i}] placeholder — {tiles.nth(i).inner_text().strip()}')
				continue
			natural = img.evaluate('el => el.naturalWidth')
			print(f'  [{i}] {tiles.nth(i).inner_text().strip()} naturalWidth={natural}')

	page.screenshot(path='/tmp/enframe_p2/browser.png', full_page=False)
	print(f'\nconsole errors: {errors[:5] if errors else "none"}')
	browser.close()
