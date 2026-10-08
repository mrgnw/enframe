# /// script
# requires-python = '>=3.12'
# dependencies = ['numpy', 'pillow', 'playwright']
# ///
"""Does a browser show a clip's first frame in the same colours as its photo?

Renders the first photo, cropped to its frame's source rectangle, and the clip itself in one browser, screenshots
both, and compares the mean colour of 16x16 blocks. Run once per browser engine:

	uv run analysis/stabilize/colour_screenshot.py CLIP.avif [CLIP.avif ...]

Each clip needs its .json beside it and the photos where the .json's run found them (pass --photos DIR). Make
clips with a long --fps (for example --fps 0.1) so the first frame is still showing when the screenshot is taken.
"""

import argparse
import json
from io import BytesIO
from pathlib import Path

import numpy as np
from PIL import Image
from playwright.sync_api import sync_playwright

BLOCK = 16


def page_html(clip: Path, photo: Path) -> str:
	sidecar = json.loads(clip.with_suffix('.json').read_text())
	first = sidecar['photos'][0]
	with Image.open(clip) as im:
		width, height = im.size
	with Image.open(photo) as im:
		photo_w, photo_h = im.size
	sx, sy = width / (first['w'] * photo_w), height / (first['h'] * photo_h)
	x, y = first['x'] * photo_w * sx, first['y'] * photo_h * sy
	return f"""<!doctype html><body style="margin:0;background:#000">
<div id="photo" style="width:{width}px;height:{height}px;background:url('{photo.resolve().as_uri()}') no-repeat;
background-size:{photo_w * sx}px {photo_h * sy}px;background-position:{-x}px {-y}px"></div>
<img id="clip" style="display:block" src="{clip.resolve().as_uri()}">
</body>"""


def blocks(png: bytes) -> np.ndarray:
	image = np.asarray(Image.open(BytesIO(png)).convert('RGB'), dtype=np.float64)
	h, w = image.shape[0] // BLOCK * BLOCK, image.shape[1] // BLOCK * BLOCK
	return image[:h, :w].reshape(h // BLOCK, BLOCK, w // BLOCK, BLOCK, 3).mean((1, 3))


def main():
	parser = argparse.ArgumentParser()
	parser.add_argument('clips', nargs='+', type=Path)
	parser.add_argument('--photos', type=Path, required=True)
	parser.add_argument('--out', type=Path, default=Path('out/colour-screenshot'))
	args = parser.parse_args()
	args.out.mkdir(parents=True, exist_ok=True)
	with sync_playwright() as p:
		for engine in (p.chromium, p.webkit):
			browser = engine.launch()
			page = browser.new_page(viewport={'width': 4000, 'height': 4000}, device_scale_factor=1)
			for clip in args.clips:
				photo = args.photos / json.loads(clip.with_suffix('.json').read_text())['photos'][0]['file']
				html = args.out / f'{clip.stem}.html'
				html.write_text(page_html(clip, photo))
				page.goto(html.resolve().as_uri())
				page.wait_for_load_state('load')
				shots = {name: page.locator(f'#{name}').screenshot() for name in ('photo', 'clip')}
				for name, png in shots.items():
					(args.out / f'{engine.name}-{clip.stem}-{name}.png').write_bytes(png)
				diff = np.abs(blocks(shots['photo']) - blocks(shots['clip']))
				print(
					f'{engine.name:8} {clip.name}: block colour difference mean {diff.mean():.2f},'
					f' red {diff[..., 0].mean():.2f}, 99th percentile {np.percentile(diff, 99):.2f}'
				)
			browser.close()


if __name__ == '__main__':
	main()
