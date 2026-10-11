#!/usr/bin/env bash
# The check every task runs before it merges: bash analysis/stabilize/check.sh
# Builds from scratch-safe state, lints, tests, runs every README command and each input format, checks the
# outputs, and compares the train clouds job against the tracking baseline. Exits non-zero if anything fails.
set -uo pipefail
cd "$(dirname "$0")/../.."

OUT=out/stabilize/check
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

P=src-tauri/target/release/stabilize
G="samples/train clouds"
TURBINES=("$G"/IMG_{4698,4699,4700,4701,4702,4703,4704,4705}.avif)
MARKS=(--points 940,637.5/1140,637.5/940,812.5/1140,812.5 --at IMG_4703.avif=846,641.5/1046,641.5/846,816.5/1046,816.5)

check 'release build' cargo build --release --manifest-path src-tauri/Cargo.toml -p stabilize
check 'cargo fmt --check' cargo fmt --check --manifest-path src-tauri/Cargo.toml -p stabilize
check 'clippy -D warnings' cargo clippy --release --manifest-path src-tauri/Cargo.toml -p stabilize --all-targets -- -D warnings
check 'cargo test' cargo test --release --manifest-path src-tauri/Cargo.toml -p stabilize

check 'README: heart.mp4' $P "$G"/IMG_479{1,2,3,4,5}.avif --points 675,345/985,345/675,595/985,595 --out $OUT/heart.mp4
check 'README: turbines.mp4' $P "${TURBINES[@]}" "${MARKS[@]}" --out $OUT/turbines.mp4
check 'README: padded portrait.mov' $P "$G"/IMG_47{18,19,20,21,22,23,24,25,26,27,28,29}.avif --points 535,985/805,985/535,1075/805,1075 --pad transparent --out $OUT/portrait.mov
check 'README: --zoom and --canvas' $P "$G"/IMG_47{18,19,20,21,22,23,24,25,26,27,28,29}.avif --points 535,985/805,985/535,1075/805,1075 --zoom IMG_4720.avif=1.5 --canvas 1536x1536 --pad black --out $OUT/canvas.mp4
check 'README: JSON only' $P "$G"/IMG_479{1,2,3,4,5}.avif --points 675,345/985,345/675,595/985,595 --out $OUT/track.json
check 'README: --cache' $P "$G"/IMG_479{1,2,3,4,5}.avif --points 675,345/985,345/675,595/985,595 --cache $OUT/cache --out $OUT/cached.json
check 'README: .mp4 with --vf' $P "${TURBINES[@]}" "${MARKS[@]}" --vf deflicker,tpad=stop_mode=clone:stop_duration=2 --out $OUT/vf.mp4
check '.mp4 with ffmpeg off PATH' env PATH=/usr/bin:/bin $P "${TURBINES[@]}" "${MARKS[@]}" --out $OUT/native.mp4
check 'README: --frames' $P "${TURBINES[@]}" "${MARKS[@]}" --frames $OUT/frames --out $OUT/frames.mp4
check 'train clouds, all 57 photos' env BIN="$P --no-cache" bash analysis/stabilize/train_clouds_all.sh $OUT/all.mov

# Each input format: the 8 turbine photos converted, with the same marks. macOS converts with sips and cjxl.
# Elsewhere Pillow writes the JPEG and PNG sets, and HEIC and JPEG XL, which decode through ImageIO, must be
# refused with a clear message instead.
cat > "$OUT/convert.py" <<'PY'
"""convert.py SRC DST [--srgb]: SRC (AVIF) to the format DST's extension names, upright, keeping EXIF and the
colour profile. --srgb converts Display P3 pixels to sRGB and labels the file sRGB, like sips -m."""

import io
import sys

from PIL import Image, ImageCms, ImageOps

src, dst = sys.argv[1], sys.argv[2]
with Image.open(src) as im:
	im.load()
	icc = im.info.get('icc_profile')
	image = ImageOps.exif_transpose(im)
if '--srgb' in sys.argv[3:]:
	srgb = ImageCms.createProfile('sRGB')
	if icc:
		image = ImageCms.profileToProfile(image, ImageCms.ImageCmsProfile(io.BytesIO(icc)), srgb, outputMode='RGB')
	icc = ImageCms.ImageCmsProfile(srgb).tobytes()
image.save(dst, exif=image.info.get('exif'), icc_profile=icc, quality=95)
PY
convert() {
	# convert SRC DST [--srgb]: DST's extension names the format.
	if [[ $(uname) == Darwin ]]; then
		if [[ ${3-} == --srgb ]]; then
			sips -s format jpeg -m "/System/Library/ColorSync/Profiles/sRGB Profile.icc" "$1" --out "$2" > /dev/null
		else
			sips -s format "${2##*.}" "$1" --out "$2" > /dev/null
		fi
	else
		uv run --project analysis/stabilize "$OUT/convert.py" "$1" "$2" "${3-}"
	fi
}
refused() {
	# The command fails, naming the platform.
	local output
	output=$("$@" 2>&1) && return 1
	grep -q 'needs macOS (ImageIO)' <<< "$output"
}
if [[ $(uname) == Darwin ]]; then
	formats=(heic jpeg png jxl)
else
	formats=(jpeg png)
fi
for fmt in "${formats[@]}"; do
	mkdir -p "$OUT/$fmt"
	for f in "${TURBINES[@]}"; do
		stem=$(basename "$f" .avif)
		if [[ $fmt == jxl ]]; then
			sips -s format png "$f" --out "$OUT/$fmt/$stem.png" > /dev/null
			cjxl -q 90 "$OUT/$fmt/$stem.png" "$OUT/$fmt/$stem.jxl" > /dev/null 2>&1
			rm "${OUT:?}/${fmt:?}/${stem:?}.png"
		else
			convert "$f" "$OUT/$fmt/$stem.$fmt"
		fi
	done
	check "input: $fmt" $P "$OUT/$fmt"/*.$fmt --points 940,637.5/1140,637.5/940,812.5/1140,812.5 --at "IMG_4703.$fmt=846,641.5/1046,641.5/846,816.5/1046,816.5" --out "$OUT/input-$fmt.json"
done
if [[ $(uname) != Darwin ]]; then
	check 'input: heic refused off macOS' refused $P src-tauri/stabilize/testdata/orientation-6.heic --points 1,1 --out "$OUT/heic.json"
	check 'input: jxl refused off macOS' refused $P src-tauri/stabilize/testdata/orientation-6.jxl --points 1,1 --out "$OUT/jxl.json"
fi

# Colour: the conversions above keep Display P3. An sRGB set stays sRGB; a mixed set becomes Display P3.
mkdir -p "$OUT/srgb" "$OUT/mixed"
for f in "${TURBINES[@]}"; do
	convert "$f" "$OUT/srgb/$(basename "$f" .avif).jpeg" --srgb
done
cp "$G"/IMG_{4698,4700,4702,4704}.avif "$OUT"/srgb/IMG_{4699,4701,4703,4705}.jpeg "$OUT/mixed/"
check 'colour: sRGB set' $P "$OUT/srgb"/*.jpeg --points 940,637.5/1140,637.5/940,812.5/1140,812.5 --at IMG_4703.jpeg=846,641.5/1046,641.5/846,816.5/1046,816.5 --out $OUT/srgb.mp4
check 'colour: mixed set' $P "$OUT/mixed"/* --points 940,637.5/1140,637.5/940,812.5/1140,812.5 --at IMG_4703.jpeg=846,641.5/1046,641.5/846,816.5/1046,816.5 --out $OUT/mixed.mp4

cat > "$OUT/verify.py" <<'PY'
"""The outputs are real, and the train clouds job still matches the committed tracking baseline."""

import json
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageOps

out = Path(sys.argv[1])


def video(path):
	probe = ['ffprobe', '-v', 'error', '-count_frames']
	probe += ['-show_entries', 'stream=codec_name,nb_read_frames,pix_fmt']
	return json.loads(
		subprocess.run([*probe, '-of', 'json', str(path)], capture_output=True, check=True).stdout
	)['streams'][0]


for name, frames in (('heart.mp4', 5), ('turbines.mp4', 8), ('portrait.mov', 12), ('all.mov', 57)):
	assert video(out / name)['nb_read_frames'] == str(frames), (name, video(out / name))
	sidecar = json.loads((out / name).with_suffix('.json').read_text())
	assert len(sidecar['photos']) == frames and sidecar['run'], name
assert video(out / 'portrait.mov')['pix_fmt'].startswith('yuva'), 'a transparent-padded clip keeps alpha'
for name in ('turbines.mp4', 'vf.mp4', 'native.mp4'):
	assert video(out / name)['codec_name'] == 'h264', name
assert video(out / 'native.mp4')['nb_read_frames'] == '8'
assert len(list((out / 'frames').glob('*.png'))) == 8
assert json.loads((out / 'track.json').read_text())['clip'] is None


def colour(path):
	probe = ['ffprobe', '-v', 'error', '-show_entries', 'stream=color_primaries,color_transfer', '-of', 'json']
	stream = json.loads(subprocess.run([*probe, str(path)], capture_output=True, check=True).stdout)['streams'][0]
	return stream.get('color_primaries'), stream.get('color_transfer')


p3, srgb = ('smpte432', 'iec61966-2-1'), ('bt709', 'iec61966-2-1')
expected_colours = (
	('turbines.mp4', p3),
	('vf.mp4', p3),
	('native.mp4', p3),
	('srgb.mp4', srgb),
	('mixed.mp4', p3),
)
for name, expected in expected_colours:
	assert colour(out / name) == expected, (name, colour(out / name))
with Image.open(out / 'frames/0001.png') as im:
	assert im.info.get('icc_profile'), 'a Display P3 frame carries its ICC profile'

# Tracking: Rust against the committed baseline (analysis/stabilize/tracking_baseline.json) on all 57 train clouds photos.
baseline = json.loads(Path('analysis/stabilize/tracking_baseline.json').read_text())['stability']['original']
rust = {p['file']: p for p in json.loads((out / 'all.json').read_text())['photos']}
samples = Path('samples/train clouds')


def oriented_size(path):
	with Image.open(path) as im:
		return ImageOps.exif_transpose(im).size


# x,y,w,h go into the sidecar as full f64 precision (no rounding in lib.rs), and both the baseline and
# this run's JSON round-trip a double through text exactly (serde_json and Python's json both use a
# shortest round-trippable float format). A bit-exact rerun gives 0; this just leaves float-parsing noise.
RECT_TOLERANCE = 1e-9
# lib.rs writes scale as `format!("{:.4}", hit.scale)`, so both sides are already quantized to 4 decimal
# places; half that step covers the same float-parsing noise as above.
SCALE_TOLERANCE = 5e-5
# tracking_benchmark.py's train_clouds_run() rounds point_px to 3 decimal places when it writes the baseline;
# this run's point is read at full precision, so up to half that step is baseline rounding, not drift.
POINT_TOLERANCE_PX = 6e-4

worst_rect = 0.0
worst_scale = 0.0
worst_point_px = 0.0
for file, row in rust.items():
	base = baseline[Path(file).stem]
	assert row['source'] == base['source'], (
		f'{file}: source {row["source"]!r}, baseline (analysis/stabilize/tracking_baseline.json) has {base["source"]!r}'
	)
	rect_diff = max(abs(row[k] - base['rect'][i]) for i, k in enumerate('xywh'))
	assert rect_diff <= RECT_TOLERANCE, f'{file}: rect differs from baseline by {rect_diff:.2e}'
	worst_rect = max(worst_rect, rect_diff)
	scale_diff = abs(row['scale'] - base['scale'])
	assert scale_diff <= SCALE_TOLERANCE, f'{file}: scale differs from baseline by {scale_diff:.2e}'
	worst_scale = max(worst_scale, scale_diff)
	fw, fh = oriented_size(samples / file)
	# The anchor is the mean of the photo's points.
	px = sum(x for x, _ in row['points']) / len(row['points'])
	py = sum(y for _, y in row['points']) / len(row['points'])
	point_diff = max(abs(px * fw - base['point_px'][0]), abs(py * fh - base['point_px'][1]))
	assert point_diff <= POINT_TOLERANCE_PX, f'{file}: point differs from baseline by {point_diff:.2e}px'
	worst_point_px = max(worst_point_px, point_diff)
lost = sorted(f for f in rust if rust[f]['source'] == 'lost')
print(
	f'train clouds vs baseline (analysis/stabilize/tracking_baseline.json): max rect diff {worst_rect:.2e}, '
	f'max scale diff {worst_scale:.1e}, max point diff {worst_point_px:.2e}px, lost {lost}'
)
PY
check 'outputs valid, train clouds matches baseline' uv run --project analysis/stabilize "$OUT/verify.py" "$OUT"
tail -1 "$LOG"

echo "--- $pass passed, $fail failed"
[[ $fail -eq 0 ]]
