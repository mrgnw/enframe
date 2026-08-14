#!/usr/bin/env -S uv run --with playwright --script
import asyncio
from pathlib import Path

from playwright.async_api import async_playwright

URL = 'http://127.0.0.1:5911/edit/1'
OUT = Path('/Users/user/dev/enframe/captures')


async def main():
	async with async_playwright() as p:
		browser = await p.chromium.launch()
		page = await browser.new_page(viewport={'width': 1280, 'height': 800}, device_scale_factor=2)
		await page.goto(URL, wait_until='networkidle')
		await page.wait_for_timeout(2500)

		await page.keyboard.press('a')
		await page.wait_for_timeout(800)
		btn = page.get_by_role('button', name='Generate')
		if await btn.count():
			await btn.last.click()
		await page.wait_for_timeout(6000)

		await page.keyboard.press('t')
		await page.wait_for_timeout(600)
		panel = page.get_by_role('region', name='Frames and points')
		await panel.hover()

		rows = page.locator('.panel-row:not(.point-row)')
		before = await rows.count()
		print('frames before', before)

		labels = page.locator('.panel-row:not(.point-row) .row-label')
		await labels.nth(0).click()
		for i in (1, 2):
			await labels.nth(i).click(modifiers=['Shift'])
		await page.wait_for_timeout(300)
		print('selected rows', await page.locator('.export-check.checked').count())
		print('header', await page.locator('.selection-count').inner_text())
		await page.screenshot(path=OUT / 'frame-multiselect.png')

		await labels.nth(1).click(button='right')
		await page.wait_for_timeout(300)
		print('menu items', await page.locator('.context-menu button').all_inner_texts())
		await page.screenshot(path=OUT / 'frame-actions.png')
		await page.keyboard.press('Escape')
		await page.locator('.menu-backdrop').click()
		await page.wait_for_timeout(200)

		await labels.nth(0).click()
		for i in (1, 2):
			await labels.nth(i).click(modifiers=['Shift'])
		await page.wait_for_timeout(200)
		await panel.hover()
		await page.keyboard.press('Backspace')
		await page.wait_for_timeout(1200)
		after = await rows.count()
		print(
			'frames after delete', after, 'toast', await page.locator('[data-sonner-toast]').all_inner_texts()
		)

		await page.keyboard.press('Meta+z')
		await page.wait_for_timeout(1500)
		await panel.hover()
		restored = await rows.count()
		print('frames after one undo', restored)
		print('PASS' if after == before - 3 and restored == before else 'FAIL')
		await browser.close()


asyncio.run(main())
