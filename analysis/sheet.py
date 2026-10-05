import subprocess
import sys

import cv2
import numpy as np

clip, out = sys.argv[1], sys.argv[2]
cols, tile_h = int(sys.argv[3]) if len(sys.argv) > 3 else 6, 260
probe = subprocess.run(
	[
		'ffprobe',
		'-v',
		'error',
		'-select_streams',
		'v:0',
		'-show_entries',
		'stream=width,height',
		'-of',
		'csv=p=0',
		clip,
	],
	capture_output=True,
	text=True,
	check=True,
).stdout
width, height = (int(v) for v in probe.strip().split(','))
decoder = ['-c:v', 'libvpx-vp9'] if clip.endswith('.webm') else []
raw = subprocess.run(
	['ffmpeg', '-v', 'error', *decoder, '-i', clip, '-f', 'rawvideo', '-pix_fmt', 'rgba', '-'],
	capture_output=True,
	check=True,
).stdout
frames = np.frombuffer(raw, np.uint8).reshape(-1, height, width, 4)

tile_w = round(width * tile_h / height)
yy, xx = np.mgrid[:tile_h, :tile_w]
checker = np.where((yy // 12 + xx // 12) % 2, 200, 120).astype(np.float32)[..., None]
tiles = []
for f in frames:
	small = cv2.resize(f, (tile_w, tile_h), interpolation=cv2.INTER_AREA).astype(np.float32)
	a = small[..., 3:] / 255
	tiles.append((small[..., :3] * a + checker * (1 - a)).astype(np.uint8))
rows = -(-len(tiles) // cols)
sheet = np.zeros((rows * (tile_h + 4), cols * (tile_w + 4), 3), np.uint8)
for i, t in enumerate(tiles):
	y, x = (i // cols) * (tile_h + 4), (i % cols) * (tile_w + 4)
	sheet[y : y + tile_h, x : x + tile_w] = t
cv2.imwrite(out, cv2.cvtColor(sheet, cv2.COLOR_RGB2BGR))
print(len(frames), 'frames ->', out)
