"""Native CPython run of the same step, plus anchor.py's own answer for the pair."""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path[:0] = [str(ROOT), str(ROOT / 'wasm')]

import anchor  # noqa: E402
import pair  # noqa: E402

A = ROOT / 'samples/girona clouds/IMG_4791.avif'
B = ROOT / 'samples/girona clouds/IMG_4792.avif'
BOX = (520, 220, 620, 500)

frames = [anchor.load(A), anchor.load(B)]
result = pair.fit(frames[0], frames[1], BOX)
hit = anchor.track(frames, {0: BOX})[1]
result['anchor_py'] = {'anchor_b': [hit.x, hit.y], 'scale': hit.scale, 'note': hit.note}
print(json.dumps(result))
