#!/usr/bin/env -S uv run --with playwright --script
import asyncio
from pathlib import Path

from playwright.async_api import async_playwright

URL = 'http://127.0.0.1:5911/library'
FOLDER = str(Path.home() / 'Pictures/samples')
OUT = Path('/Users/user/dev/enframe/captures/folder_browser.png')


async def main():
	async with async_playwright() as p:
		browser = await p.chromium.launch()
		page = await browser.new_page(viewport={'width': 1440, 'height': 900}, device_scale_factor=2)
		await page.goto(URL, wait_until='networkidle')
		await page.evaluate(f'localStorage.setItem("enframe.folders", JSON.stringify(["{FOLDER}"]))')
		await page.evaluate('localStorage.setItem("enframe.library.tab", "folders")')
		await page.reload(wait_until='networkidle')
		await page.wait_for_timeout(2000)

		tiles = await page.locator('.photo-item').count()
		print(f'tiles: {tiles}')
		assert tiles > 0, 'no tiles rendered'

		await page.get_by_role('button', name='Select all').click()
		await page.wait_for_timeout(300)
		import_label = await page.locator('.import-btn').text_content()
		print(f'import button: {import_label!r}')
		assert import_label and f'Import {tiles}' in import_label

		await page.locator('.photo-item').first.click()
		await page.wait_for_timeout(300)
		after_toggle = await page.locator('.import-btn').text_content()
		print(f'after deselect one: {after_toggle!r}')
		assert after_toggle and f'Import {tiles - 1}' in after_toggle

		photos_before = await page.evaluate(
			"fetch('/sql/select', {method:'POST', headers:{'content-type':'application/json'}, body: JSON.stringify({sql:'SELECT COUNT(*) AS n FROM photos', values:[]})}).then(r => r.json())"
		)
		print(f'photos table before import: {photos_before}')

		OUT.parent.mkdir(exist_ok=True)
		await page.screenshot(path=str(OUT))
		print(f'screenshot: {OUT}')
		await browser.close()


asyncio.run(main())
