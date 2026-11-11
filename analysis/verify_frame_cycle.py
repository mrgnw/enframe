import asyncio

from playwright.async_api import async_playwright

URL = 'http://127.0.0.1:5911/edit/1'
CAPTURES = 'captures'


async def selected_label(page):
	return await page.evaluate("""() => {
		const el = document.querySelector('.frame-overlay.selected');
		if (!el) return null;
		const label = el.querySelector('.frame-label');
		return label ? label.textContent.trim() : '(unlabelled)';
	}""")


async def frame_count(page):
	return await page.evaluate("() => document.querySelectorAll('.frame-overlay').length")


async def main():
	async with async_playwright() as p:
		browser = await p.chromium.launch()
		page = await browser.new_page(viewport={'width': 1440, 'height': 900})
		await page.goto(URL, wait_until='networkidle')
		await page.wait_for_selector('.overlay-container', timeout=15000)

		# "a" opens the generate dialog; confirm it to place one frame per destination.
		await page.keyboard.press('a')
		await page.wait_for_selector('.gen-dialog', timeout=5000)
		await page.click('.gen-dialog button.primary')
		await page.wait_for_selector('.gen-dialog', state='detached', timeout=10000)
		await page.wait_for_timeout(2500)
		print('frames after generating:', await frame_count(page))

		# Pick the viewport point covered by the most frames while staying clear of any
		# focal point marker, so the click exercises the frame pile rather than a point.
		spot = await page.evaluate("""() => {
			const c = document.querySelector('.overlay-container').getBoundingClientRect();
			const frames = [...document.querySelectorAll('.frame-overlay')].map((e) => e.getBoundingClientRect());
			const points = [...document.querySelectorAll('.focal-point')].map((e) => {
				const r = e.getBoundingClientRect();
				return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
			});
			let best = null;
			for (let i = 1; i < 40; i++) {
				for (let j = 1; j < 40; j++) {
					const x = c.x + (c.width * i) / 40;
					const y = c.y + (c.height * j) / 40;
					if (points.some((p) => Math.hypot(p.x - x, p.y - y) < 28)) continue;
					const cover = frames.filter(
						(f) => x >= f.x && x <= f.right && y >= f.y && y <= f.bottom
					).length;
					if (!best || cover > best.cover) best = { x, y, cover };
				}
			}
			return best;
		}""")
		print('click point:', spot)
		assert spot['cover'] >= 3, f'need an overlapping pile, got {spot}'
		x, y = spot['x'], spot['y']

		seen = []
		for i in range(7):
			await page.mouse.click(x, y)
			await page.wait_for_timeout(450)
			label = await selected_label(page)
			seen.append(label)
			print(f'click {i + 1}: selected = {label}')
			if i == 0:
				await page.screenshot(path=f'{CAPTURES}/frame-cycle-1.png')
			if i == 1:
				await page.screenshot(path=f'{CAPTURES}/frame-cycle-2.png')

		assert all(s is not None for s in seen), f'no frame selected at some click: {seen}'
		assert seen[0] != seen[1], f'clicking the same spot did not change selection: {seen}'
		distinct = len(set(seen))
		assert distinct >= 3, f'expected to cycle several frames, only saw {distinct}: {seen}'
		# The cycle must wrap back round rather than dead-end.
		assert seen[0] in seen[1:], f'cycle never wrapped back to the first frame: {seen}'
		print('\nPASS: distinct frames from one click point =', distinct)
		print('sequence:', seen)

		await browser.close()


asyncio.run(main())
