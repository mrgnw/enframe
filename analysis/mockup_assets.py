"""Export downscaled JPEGs of the Girona samples for the HTML mockups.

Run after `PAD=transparent analysis/girona_all.sh /tmp/pin/all.avif --frames /tmp/pin/frames`.
"""

import json
import sys
from pathlib import Path

from PIL import Image, ImageOps

SAMPLES = Path('/Users/user/dev/pinframe/samples/girona clouds')
FRAMES = Path('/tmp/pin/frames')
SIDECAR = Path('/tmp/pin/all.json')
OUT = Path(__file__).resolve().parent.parent / 'mockups' / 'assets'

MAIN = ['IMG_4667', 'IMG_4675', 'IMG_4702', 'IMG_4703', 'IMG_4720', 'IMG_4721', 'IMG_4725']
THUMB_LONG, MAIN_LONG, FRAME_LONG = 200, 1400, 720
PATCH = ('IMG_4702', 716, 719, 400, 350)


def save(image: Image.Image, long_side: int, path: Path, quality: int = 72) -> None:
	image = ImageOps.exif_transpose(image)
	image.thumbnail((long_side, long_side))
	image.convert('RGB').save(path, quality=quality, optimize=True)


def main() -> None:
	OUT.mkdir(exist_ok=True)
	photos = sorted(SAMPLES.glob('*.avif'))
	assert len(photos) == 57, len(photos)
	for p in photos:
		with Image.open(p) as im:
			save(im, THUMB_LONG, OUT / f'{p.stem}-t.jpg', 65)
			if p.stem in MAIN:
				save(im, MAIN_LONG, OUT / f'{p.stem}.jpg')
	name, ax, ay, w, h = PATCH
	with Image.open(SAMPLES / f'{name}.avif') as im:
		patch = ImageOps.exif_transpose(im).crop((ax - w // 2, ay - h // 2, ax + w // 2, ay + h // 2))
		patch.convert('RGB').save(OUT / 'patch.jpg', quality=80)
	frames = sorted(FRAMES.glob('*.png'))
	assert len(frames) == 57, len(frames)
	for f in frames:
		with Image.open(f) as im:
			im = im.convert('RGBA')
			im.thumbnail((FRAME_LONG, FRAME_LONG))
			im.save(OUT / f'frame-{f.stem}.webp', quality=70, method=6)
	rows = json.loads(SIDECAR.read_text())
	(OUT / 'rects.json').write_text(json.dumps(rows, separators=(',', ':')))
	files = list(OUT.iterdir())
	total = sum(p.stat().st_size for p in files)
	print(f'{len(files)} files, {total / 2**20:.1f} MB')


if __name__ == '__main__':
	sys.exit(main())
