#!/usr/bin/env bash
# The check every task runs before it merges: bash analysis/check.sh
# Builds from scratch-safe state, lints, tests, runs every README command and each input format, checks the
# outputs, and compares the Girona job against the tracking baseline. Exits non-zero if anything fails.
set -uo pipefail
cd "$(dirname "$0")/.."

[[ -e samples ]] || ln -s /Users/user/dev/pinframe/samples samples
OUT=out/check
rm -rf "$OUT"
mkdir -p "$OUT"
LOG=$OUT/last.log
pass=0
fail=0
check() {
	local name=$1
	shift
	if "$@" > "$LOG" 2>&1; then
		echo "PASS  $name"
		pass=$((pass + 1))
	else
		echo "FAIL  $name: $(tail -2 "$LOG" | tr '\n' ' ')"
		fail=$((fail + 1))
	fi
}

P=rust/target/release/pinframe
G="samples/girona clouds"
TURBINES=("$G"/IMG_{4698,4699,4700,4701,4702,4703,4704,4705}.avif)
MARKS=(--box 840,550,400,350 --at IMG_4703.avif=746,554,400,350)

check 'release build' bash -c 'cd rust && cargo build --release'
check 'cargo fmt --check' bash -c 'cd rust && cargo fmt --check'
check 'clippy -D warnings' bash -c 'cd rust && cargo clippy --release --all-targets -- -D warnings'
check 'cargo test' bash -c 'cd rust && cargo test --release'

check 'README: heart.avif' $P "$G"/IMG_479{1,2,3,4,5}.avif --box 520,220,620,500 --out $OUT/heart.avif
check 'README: turbines.avif' $P "${TURBINES[@]}" "${MARKS[@]}" --out $OUT/turbines.avif
check 'README: padded portrait.avif' $P "$G"/IMG_47{18,19,20,21,22,23,24,25,26,27,28,29}.avif --box 400,940,540,180 --pad transparent --out $OUT/portrait.avif
check 'README: JSON only' $P "$G"/IMG_479{1,2,3,4,5}.avif --box 520,220,620,500 --out $OUT/track.json
check 'README: --cache' $P "$G"/IMG_479{1,2,3,4,5}.avif --box 520,220,620,500 --cache $OUT/cache --out $OUT/cached.json
check 'README: .mp4 with --vf' $P "${TURBINES[@]}" "${MARKS[@]}" --vf deflicker,tpad=stop_mode=clone:stop_duration=2 --out $OUT/turbines.mp4
check 'README: --frames' $P "${TURBINES[@]}" "${MARKS[@]}" --frames $OUT/frames --out $OUT/frames.avif
check 'Girona, all 57 photos' env BIN="$P --no-cache" bash analysis/girona_all.sh $OUT/all.avif

# Each input format: the 8 turbine photos converted, with the same marks.
for fmt in heic jpeg png jxl; do
	mkdir -p "$OUT/$fmt"
	for f in "${TURBINES[@]}"; do
		stem=$(basename "$f" .avif)
		if [[ $fmt == jxl ]]; then
			sips -s format png "$f" --out "$OUT/$fmt/$stem.png" > /dev/null
			cjxl -q 90 "$OUT/$fmt/$stem.png" "$OUT/$fmt/$stem.jxl" > /dev/null 2>&1
			rm "$OUT/$fmt/$stem.png"
		else
			sips -s format "$fmt" "$f" --out "$OUT/$fmt/$stem.$fmt" > /dev/null
		fi
	done
	check "input: $fmt" $P "$OUT/$fmt"/*.$fmt --box 840,550,400,350 --at "IMG_4703.$fmt=746,554,400,350" --out "$OUT/input-$fmt.json"
done

# Colour: the conversions above keep Display P3. An sRGB set stays sRGB; a mixed set becomes Display P3.
mkdir -p "$OUT/srgb" "$OUT/mixed"
for f in "${TURBINES[@]}"; do
	sips -s format jpeg -m "/System/Library/ColorSync/Profiles/sRGB Profile.icc" "$f" --out "$OUT/srgb/$(basename "$f" .avif).jpeg" > /dev/null
done
cp "$G"/IMG_{4698,4700,4702,4704}.avif "$OUT"/srgb/IMG_{4699,4701,4703,4705}.jpeg "$OUT/mixed/"
check 'colour: sRGB set' $P "$OUT/srgb"/*.jpeg --box 840,550,400,350 --at IMG_4703.jpeg=746,554,400,350 --out $OUT/srgb.avif
check 'colour: mixed set' $P "$OUT/mixed"/* --box 840,550,400,350 --at IMG_4703.jpeg=746,554,400,350 --out $OUT/mixed.avif

cat > "$OUT/verify.py" <<'PY'
"""The outputs are real, and the Girona job still matches the committed tracking baseline."""

import json
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageOps

out = Path(sys.argv[1])
for name, frames in (('heart', 5), ('turbines', 8), ('portrait', 12), ('all', 57)):
	with Image.open(out / f'{name}.avif') as im:
		assert im.n_frames == frames, (name, im.n_frames)
	sidecar = json.loads((out / f'{name}.json').read_text())
	assert len(sidecar['photos']) == frames and sidecar['run'], name
with Image.open(out / 'portrait.avif') as im:
	assert im.mode == 'RGBA', 'a transparent-padded clip keeps alpha'
codec = subprocess.run(
	['ffprobe', '-v', 'error', '-show_entries', 'stream=codec_name', '-of', 'csv=p=0', str(out / 'turbines.mp4')],
	capture_output=True,
	text=True,
	check=True,
).stdout.strip()
assert codec == 'h264', codec
assert len(list((out / 'frames').glob('*.png'))) == 8
assert json.loads((out / 'track.json').read_text())['clip'] is None


def colour(path):
	probe = ['ffprobe', '-v', 'error', '-show_entries', 'stream=color_primaries,color_transfer', '-of', 'json']
	stream = json.loads(subprocess.run([*probe, str(path)], capture_output=True, check=True).stdout)['streams'][0]
	return stream.get('color_primaries'), stream.get('color_transfer')


p3, srgb = ('smpte432', 'iec61966-2-1'), ('bt709', 'iec61966-2-1')
for name, expected in (('turbines.avif', p3), ('turbines.mp4', p3), ('srgb.avif', srgb), ('mixed.avif', p3)):
	assert colour(out / name) == expected, (name, colour(out / name))
with Image.open(out / 'frames/0001.png') as im:
	assert im.info.get('icc_profile'), 'a Display P3 frame carries its ICC profile'

# Tracking: Rust against the committed baseline (analysis/tracking_baseline.json) on all 57 Girona photos.
baseline = json.loads(Path('analysis/tracking_baseline.json').read_text())['stability']['original']
rust = {p['file']: p for p in json.loads((out / 'all.json').read_text())['photos']}
samples = Path('samples/girona clouds')


def oriented_size(path):
	with Image.open(path) as im:
		return ImageOps.exif_transpose(im).size


# x,y,w,h go into the sidecar as full f64 precision (no rounding in main.rs), and both the baseline and
# this run's JSON round-trip a double through text exactly (serde_json and Python's json both use a
# shortest round-trippable float format). A bit-exact rerun gives 0; this just leaves float-parsing noise.
RECT_TOLERANCE = 1e-9
# main.rs writes scale as `format!("{:.4}", hit.scale)`, so both sides are already quantized to 4 decimal
# places; half that step covers the same float-parsing noise as above.
SCALE_TOLERANCE = 5e-5
# tracking_benchmark.py's girona_run() rounds point_px to 3 decimal places when it writes the baseline;
# this run's point is read at full precision, so up to half that step is baseline rounding, not drift.
POINT_TOLERANCE_PX = 6e-4

worst_rect = 0.0
worst_scale = 0.0
worst_point_px = 0.0
for file, row in rust.items():
	base = baseline[Path(file).stem]
	assert row['source'] == base['source'], (
		f'{file}: source {row["source"]!r}, baseline (analysis/tracking_baseline.json) has {base["source"]!r}'
	)
	rect_diff = max(abs(row[k] - base['rect'][i]) for i, k in enumerate('xywh'))
	assert rect_diff <= RECT_TOLERANCE, f'{file}: rect differs from baseline by {rect_diff:.2e}'
	worst_rect = max(worst_rect, rect_diff)
	scale_diff = abs(row['scale'] - base['scale'])
	assert scale_diff <= SCALE_TOLERANCE, f'{file}: scale differs from baseline by {scale_diff:.2e}'
	worst_scale = max(worst_scale, scale_diff)
	fw, fh = oriented_size(samples / file)
	px, py = row['points'][0]
	point_diff = max(abs(px * fw - base['point_px'][0]), abs(py * fh - base['point_px'][1]))
	assert point_diff <= POINT_TOLERANCE_PX, f'{file}: point differs from baseline by {point_diff:.2e}px'
	worst_point_px = max(worst_point_px, point_diff)
lost = sorted(f for f in rust if rust[f]['source'] == 'lost')
print(
	f'Girona vs baseline (analysis/tracking_baseline.json): max rect diff {worst_rect:.2e}, '
	f'max scale diff {worst_scale:.1e}, max point diff {worst_point_px:.2e}px, lost {lost}'
)
PY
check 'outputs valid, Girona matches baseline' uv run "$OUT/verify.py" "$OUT"
tail -1 "$LOG"

echo "--- $pass passed, $fail failed"
[[ $fail -eq 0 ]]
