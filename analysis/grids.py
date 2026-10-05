import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

folder = Path('samples/girona clouds')
out = Path(sys.argv[1])
names = sys.argv[2:]
tile = 900
font = ImageFont.load_default(size=44)
tiles = []
for name in names:
	im = Image.open(folder / f'{name}.avif').convert('RGB')
	d = ImageDraw.Draw(im)
	for x in range(0, im.width, 100):
		d.line(
			[(x, 0), (x, im.height)], fill='red' if x % 500 == 0 else 'orange', width=4 if x % 500 == 0 else 2
		)
		if x % 200 == 0:
			d.text((x + 6, 6), str(x), fill='red', font=font)
	for y in range(0, im.height, 100):
		d.line(
			[(0, y), (im.width, y)], fill='red' if y % 500 == 0 else 'orange', width=4 if y % 500 == 0 else 2
		)
		if y % 200 == 0:
			d.text((6, y + 6), str(y), fill='red', font=font)
	d.text((im.width // 2 - 120, im.height - 70), name, fill='yellow', font=font)
	im.thumbnail((tile, tile))
	tiles.append(im)
cols = 2 if len(tiles) > 1 else 1
rows = -(-len(tiles) // cols)
sheet = Image.new('RGB', (cols * (tile + 6), rows * (tile + 6)), 'black')
for i, t in enumerate(tiles):
	sheet.paste(t, ((i % cols) * (tile + 6), (i // cols) * (tile + 6)))
sheet.save(out)
