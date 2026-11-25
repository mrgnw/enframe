# /// script
# requires-python = '>=3.14'
# dependencies = ['pillow==12.0.0']
# ///
from pathlib import Path

from PIL import Image, ImageDraw

WIDTH, HEIGHT = 1200, 800
OUT = Path(__file__).parent.parent / 'photo.jpg'

image = Image.new('RGB', (WIDTH, HEIGHT))
draw = ImageDraw.Draw(image)
for x in range(WIDTH):
	for y in range(0, HEIGHT, 4):
		draw.line([(x, y), (x, y + 3)], fill=(int(255 * x / WIDTH), int(255 * y / HEIGHT), 140))
for x in range(0, WIDTH, 100):
	draw.line([(x, 0), (x, HEIGHT)], fill=(255, 255, 255), width=2)
for y in range(0, HEIGHT, 100):
	draw.line([(0, y), (WIDTH, y)], fill=(255, 255, 255), width=2)
for col in range(12):
	for row in range(8):
		draw.text((col * 100 + 8, row * 100 + 8), f'{chr(65 + row)}{col}', fill=(0, 0, 0), font_size=28)
draw.ellipse([880, 160, 1080, 360], fill=(250, 220, 40), outline=(0, 0, 0), width=6)
image.save(OUT, quality=90)
print(OUT, image.size)
