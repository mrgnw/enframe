"""Export downscaled JPEGs of the train clouds samples for the HTML mockups.

Run after `PAD=transparent analysis/train_clouds_all.sh /tmp/pin/all.avif --frames /tmp/pin/frames`.
"""

import json
import sys
from pathlib import Path

from PIL import Image, ImageOps

SAMPLES = Path('/Users/user/dev/pinframe/samples/train clouds')
FRAMES = Path('/tmp/pin/frames')
SIDECAR = Path('/tmp/pin/all.json')
OUT = Path(__file__).resolve().parent.parent / 'mockups' / 'assets'

THUMB_LONG, MAIN_LONG, FRAME_LONG = 200, 1100, 720


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
			save(im, MAIN_LONG, OUT / f'{p.stem}.jpg', 62)
	frames = sorted(FRAMES.glob('*.png'))
	assert len(frames) == 57, len(frames)
	for f in frames:
		with Image.open(f) as im:
			im = im.convert('RGBA')
			im.thumbnail((FRAME_LONG, FRAME_LONG))
			im.save(OUT / f'frame-{f.stem}.webp', quality=70, method=6)
	rows = json.loads(SIDECAR.read_text())['photos']
	(OUT / 'rects.json').write_text(json.dumps(rows, separators=(',', ':')))
	files = list(OUT.iterdir())
	total = sum(p.stat().st_size for p in files)
	print(f'{len(files)} files, {total / 2**20:.1f} MB')


if __name__ == '__main__':
	sys.exit(main())
