# /// script
# dependencies = ["playwright"]
# ///
import asyncio
from playwright.async_api import async_playwright

URL = 'http://127.0.0.1:5911/edit/1'


async def main():
	async with async_playwright() as p:
		browser = await p.chromium.launch()
		page = await browser.new_page(viewport={'width': 1280, 'height': 860})
		await page.goto(URL, wait_until='domcontentloaded')
		await page.wait_for_selector('img.photo')
		await page.wait_for_timeout(2500)

		box = await page.locator('img.photo').bounding_box()
		await page.mouse.move(640, 430)
		for _ in range(12):
			await page.mouse.wheel(0, -120)
			await page.wait_for_timeout(60)
		await page.wait_for_timeout(600)

		result = await page.evaluate("""() => {
			const img = document.querySelector('img.photo');
			return {
				styleWidth: img.style.width,
				renderedWidth: img.getBoundingClientRect().width,
				maxWidth: getComputedStyle(img).maxWidth
			};
		}""")
		print(f'before zoom: {box["width"]:.0f}px')
		print(f'style says:  {result["styleWidth"]}')
		print(f'rendered:    {result["renderedWidth"]:.0f}px')
		print(f'max-width:   {result["maxWidth"]}')
		wanted = float(result['styleWidth'].replace('px', ''))
		clamped = abs(wanted - result['renderedWidth']) > 1
		print('CLAMPED — zoom is broken' if clamped else 'ok — no clamp')
		await browser.close()


asyncio.run(main())
