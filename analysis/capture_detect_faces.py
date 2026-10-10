# /// script
# requires-python = ">=3.14"
# dependencies = ["playwright"]
# ///
import sys
from playwright.sync_api import sync_playwright

url, photo, out_dir = sys.argv[1:4]
shell = """
(() => {
	let next = 0;
	window.__TAURI_INTERNALS__ = {
		metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main', windowLabel: 'main' } },
		convertFileSrc: (path) => `${location.origin}/img/${encodeURIComponent(path)}`,
		transformCallback: (callback) => {
			const id = ++next;
			window[`_cb${id}`] = callback;
			return id;
		},
		invoke: async (command, args) => {
			if (command === 'plugin:event|listen') {
				window[`__${args.event}`] = window[`_cb${args.handler}`];
				return 1;
			}
			if (command.startsWith('plugin:')) return null;
			const response = await fetch(`/api/${command}`, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify(args ?? {})
			});
			return response.json();
		}
	};
	localStorage.setItem('enframe.connection', location.origin);
})()
"""
with sync_playwright() as p:
	browser = p.chromium.launch()
	page = browser.new_page(viewport={'width': 1280, 'height': 800})
	page.on('console', lambda message: print('console:', message.text))
	page.on('pageerror', lambda error: print('pageerror:', error))
	page.add_init_script(shell)
	page.goto(url)
	page.wait_for_load_state('networkidle')
	page.wait_for_timeout(1000)
	print('listener', page.evaluate("typeof window['__tauri://drag-drop']"))
	page.evaluate(
		"(path) => window['__tauri://drag-drop']({ event: 'tauri://drag-drop', id: 1, payload: { paths: [path], position: { x: 0, y: 0 } } })",
		photo,
	)
	page.wait_for_timeout(2500)
	page.screenshot(path=f'{out_dir}/detect-faces-1-before.png')
	page.get_by_title('Detect faces').click()
	page.wait_for_timeout(2500)
	page.screenshot(path=f'{out_dir}/detect-faces-2-after.png')
	page.keyboard.press('Meta+z')
	page.wait_for_timeout(800)
	page.screenshot(path=f'{out_dir}/detect-faces-3-undone.png')
	browser.close()
