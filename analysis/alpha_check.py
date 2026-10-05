import subprocess
import sys

import numpy as np

clip, width, height = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
# libvpx is the only VP9 decoder that reads the alpha side-channel; ffmpeg's native one drops it.
decoder = ['-c:v', 'libvpx-vp9'] if clip.endswith('.webm') else []
raw = subprocess.run(
	['ffmpeg', '-v', 'error', *decoder, '-i', clip, '-f', 'rawvideo', '-pix_fmt', 'rgba', '-'],
	capture_output=True,
	check=True,
).stdout
frames = np.frombuffer(raw, np.uint8).reshape(-1, height, width, 4)
for i, f in enumerate(frames):
	a = f[..., 3]
	print(f'frame {i:2}: {(a < 8).mean():5.1%} transparent, {(a > 247).mean():5.1%} opaque')
