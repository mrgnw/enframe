"""Save the 1024-px gray of one Girona photo as PNG and OpenCV's SIFT keypoints + descriptors for comparison."""
import json, sys, time
from pathlib import Path
import cv2, numpy as np
sys.path.insert(0, str(Path(__file__).parent.parent))
import anchor

name = sys.argv[1] if len(sys.argv) > 1 else 'IMG_4667'
rgb = anchor.load(Path(f'samples/girona clouds/{name}.avif'))
small = anchor.resize(anchor.gray(rgb), 0.5)
cv2.imwrite(f'out/{name}.gray.png', small)
Path(f'out/{name}.rgb.raw').write_bytes(rgb.tobytes())
sift = cv2.SIFT_create()
t = time.perf_counter(); kp, des = sift.detectAndCompute(small, None); dt = time.perf_counter() - t
cv2.setNumThreads(1)
t = time.perf_counter(); kp1, des1 = sift.detectAndCompute(small, None); dt1 = time.perf_counter() - t
rows = [dict(x=p.pt[0], y=p.pt[1], size=p.size, angle=p.angle, response=p.response, octave=p.octave) for p in kp]
json.dump(rows, open(f'out/{name}.kp.json', 'w'))
np.save(f'out/{name}.des.npy', des)
print(f'{len(kp)} keypoints, {dt*1000:.1f} ms (threaded) {dt1*1000:.1f} ms (1 thread)')
