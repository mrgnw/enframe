#!/usr/bin/env -S uv run --with playwright --script
import asyncio
from playwright.async_api import async_playwright

URL = 'http://127.0.0.1:5911/library'


async def main():
	async with async_playwright() as p:
		browser = await p.chromium.launch()
		page = await browser.new_page()
		await page.goto(URL)
		await page.get_by_role('button', name='Lightroom').click()
		await page.reload()
		await page.wait_for_timeout(1500)
		cls = await page.get_by_role('button', name='Lightroom').get_attribute('class')
		assert 'active' in (cls or ''), f'tab did not persist: {cls}'
		assert await page.locator('.rail').count() == 1, 'lightroom rail missing after reload'
		print('tab persistence ok')
		await browser.close()


asyncio.run(main())
