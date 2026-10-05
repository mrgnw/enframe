import sys
from pathlib import Path

import cv2

sys.path.insert(0, str(Path(__file__).parent.parent))
from anchor import gray, load, resize  # noqa: E402

folder = Path('samples/girona clouds')
k = 0.5


def score(ref, box, scale, target):
	# box is the anchor box in ref, already at the tracked scale, full-res coords
	g0 = resize(gray(load(folder / f'{ref}.avif')), k)
	g1 = resize(gray(load(folder / f'{target}.avif')), k)
	cx, cy, w, h = (v * k for v in box)
	t = cv2.getRectSubPix(g0, (round(w * scale), round(h * scale)), (cx, cy))
	_, s, _, loc = cv2.minMaxLoc(cv2.matchTemplate(g1, t, cv2.TM_CCOEFF_NORMED))
	return s, (loc[0] + t.shape[1] / 2) / k, (loc[1] + t.shape[0] / 2) / k


# Run B: last good 4702, anchor 716,718, box 400x350 at scale ~1
for t in ['IMG_4703', 'IMG_4704', 'IMG_4705']:
	print('B real ', t, '%.2f at %.0f,%.0f' % score('IMG_4702', (716, 718, 400, 350), 0.98, t))
# Run C: last good before occlusions, box 560x170 at scale 2.54
print('C occl ', 'IMG_4721', '%.2f at %.0f,%.0f' % score('IMG_4720', (459, 909, 560, 170), 2.54, 'IMG_4721'))
print('C occl ', 'IMG_4725', '%.2f at %.0f,%.0f' % score('IMG_4724', (617, 874, 560, 170), 2.57, 'IMG_4725'))
