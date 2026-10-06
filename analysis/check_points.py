"""Check the sidecar's `points` field against the anchor anchor.py prints, on all 57 Girona photos.

uv run analysis/check_points.py
Runs analysis/girona_all.sh twice, once with the Rust binary and once with anchor.py, and compares each
photo's point (converted to pixels by the photo's oriented size, read with PIL) against the pixel anchor
anchor.py prints. Fails if any photo differs by more than 1.5 px in x or y.
"""

import json
import os
import re
import subprocess
from pathlib import Path

from PIL import Image, ImageOps

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / 'analysis' / 'girona_all.sh'
SAMPLES = ROOT / 'samples' / 'girona clouds'
TOLERANCE = 1.5

ANCHOR_LINE = re.compile(r'^(\S+)  \d+x\d+  anchor (-?\d+),(-?\d+)  scale')


def oriented_size(path: Path) -> tuple[int, int]:
	with Image.open(path) as im:
		return ImageOps.exif_transpose(im).size


def run(out: Path, env: dict | None = None) -> str:
	result = subprocess.run(
		['bash', str(SCRIPT), str(out)], cwd=ROOT, env=env, capture_output=True, text=True, check=True
	)
	return result.stdout


def anchors(stdout: str) -> dict[str, tuple[float, float]]:
	found = {
		m.group(1): (float(m.group(2)), float(m.group(3)))
		for m in map(ANCHOR_LINE.match, stdout.splitlines())
		if m
	}
	assert len(found) == 57, f'expected 57 photos, found {len(found)}'
	return found


def main() -> None:
	out = ROOT / 'out' / 'check_points'
	out.mkdir(parents=True, exist_ok=True)

	rust_env = {**os.environ, 'BIN': str(ROOT / 'rust' / 'target' / 'release' / 'pinframe')}
	rust_anchors = anchors(run(out / 'rust.json', rust_env))
	# anchor.py only writes a sidecar alongside a rendered clip, not standalone.
	python_anchors = anchors(run(out / 'python.avif', os.environ.copy()))

	sidecar = json.loads((out / 'rust.json').read_text())
	assert len(sidecar['photos']) == 57
	worst = 0.0
	for row in sidecar['photos']:
		fw, fh = oriented_size(SAMPLES / row['file'])
		(px, py) = row['points'][0]
		point_px = (px * fw, py * fh)
		ax, ay = python_anchors[row['file']]
		rx, ry = rust_anchors[row['file']]
		# Rust's own stdout anchor is rounded to the nearest pixel; the sidecar point is not.
		assert abs(point_px[0] - rx) <= 0.5 and abs(point_px[1] - ry) <= 0.5, (
			f'{row["file"]}: sidecar point {point_px} disagrees with its own stdout anchor {(rx, ry)}'
		)
		diff = max(abs(point_px[0] - ax), abs(point_px[1] - ay))
		worst = max(worst, diff)
		assert diff <= TOLERANCE, (
			f'{row["file"]}: point {point_px} vs Python anchor {(ax, ay)}, diff {diff:.2f}px'
		)
	print(f'points match Python anchors on all 57 Girona photos, worst diff {worst:.3f}px')


if __name__ == '__main__':
	main()
