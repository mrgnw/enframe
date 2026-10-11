# /// script
# requires-python = ">=3.14"
# dependencies = ["playwright"]
# ///
import base64
import pathlib
import sys

from playwright.sync_api import sync_playwright

url, out_dir, *photos = sys.argv[1:]
drop = """
async ([name, b64]) => {
	const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
	const data = new DataTransfer();
	data.items.add(new File([bytes], name, { type: 'image/jpeg' }));
	window.dispatchEvent(new DragEvent('drop', { dataTransfer: data, bubbles: true }));
}
"""
markers = 'document.querySelectorAll(\'[data-focal-id], .focal-point, [class*="focal"]\').length'
with sync_playwright() as p:
	browser = p.chromium.launch()
	for photo in photos:
		path = pathlib.Path(photo)
		page = browser.new_page(viewport={'width': 1280, 'height': 800})
		requests = []
		page.on('request', lambda request: requests.append(f'{request.method} {request.url}'))
		page.on('pageerror', lambda error: print('pageerror:', error))
		page.goto(url)
		page.wait_for_load_state('networkidle')
		page.evaluate(drop, [path.name, base64.b64encode(path.read_bytes()).decode()])
		page.wait_for_timeout(2000)
		page.get_by_title('Detect faces').click()
		page.wait_for_timeout(2500)
		toasts = page.locator('[data-sonner-toast]').all_inner_texts()
		page.screenshot(path=f'{out_dir}/browser-{path.stem}-after.png')
		page.keyboard.press('Meta+z')
		page.wait_for_timeout(800)
		page.screenshot(path=f'{out_dir}/browser-{path.stem}-undone.png')
		sent = [r for r in requests if 'detect_faces' in r or 'upload' in r]
		print(f'{path.name}: toasts={toasts} requests={sent}')
		page.close()
	browser.close()
