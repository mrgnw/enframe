# /// script
# requires-python = ">=3.12"
# dependencies = ["pillow"]
# ///
import sys
from pathlib import Path
from PIL import Image

frames = sorted(Path(sys.argv[1]).glob('frame-*.webp'))
for f in frames:
	im = Image.open(f).convert('RGBA')
	a = im.getchannel('A')
	covered = sum(1 for p in a.getdata() if p > 128) / (im.width * im.height)
	bbox = a.getbbox()
	print(f'{f.name} {im.width}x{im.height} covered={covered:.2f} bbox={bbox}')
