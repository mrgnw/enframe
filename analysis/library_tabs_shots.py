#!/usr/bin/env -S uv run --with playwright --script
import asyncio
from playwright.async_api import async_playwright

URL = 'http://127.0.0.1:5911/library'
OUT = '/tmp/lib-shots'


async def main():
	async with async_playwright() as p:
		browser = await p.chromium.launch()
		page = await browser.new_page(viewport={'width': 1280, 'height': 860})
		for tab, name in (('Lightroom', 'lightroom'), ('Watch folder', 'watch')):
			await page.goto(URL)
			await page.get_by_role('button', name=tab).click()
			await page.wait_for_timeout(3000)
			await page.screenshot(path=f'{OUT}/{name}.png')
		await browser.close()


asyncio.run(main())
