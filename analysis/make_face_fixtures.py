# /// script
# requires-python = ">=3.14"
# dependencies = ["pillow"]
# ///
import sys
from PIL import Image

source, upright_out, orient6_out = sys.argv[1:4]
image = Image.open(source).convert('RGB').resize((1024, 1024), Image.LANCZOS)
image.save(upright_out, quality=90)

exif = Image.Exif()
exif[0x0112] = 6
image.rotate(90, expand=True).save(orient6_out, quality=90, exif=exif)
