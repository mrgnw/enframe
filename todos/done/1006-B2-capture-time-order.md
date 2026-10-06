---
branch: b2-capture-time-order
---

# B2 · Order photos by capture time

Depends on: — · Blocks: B4 · Model: Terra · Size: S
Touches: `rust/src/main.rs` (photo order), `rust/src/decode.rs` (EXIF date read), README

## Goal

Photos are ordered by filename today (`ponytail:` note in `main.rs`), which breaks when names don't follow shooting order, for example when mixing two cameras. Order by EXIF `DateTimeOriginal` (plus `SubSecTimeOriginal`) when every photo has it, and fall back to filename otherwise. Add `--order time|name`, defaulting to time-with-fallback, and print which order was used.

## Done when

- [x] Works for AVIF, HEIC, JXL, JPEG and PNG, reading only headers.
- [x] Girona (no capture times in these exports) keeps its filename order and identical output.
- [x] A test with three files whose names and times disagree.
- [x] `bash analysis/check.sh` passes.

## Result

`--order time|name`, time by default, falling back to filename order when any photo lacks `DateTimeOriginal`. EXIF comes from kamadak-exif for AVIF, HEIC, JPEG and PNG, and from ImageIO for JXL. The Girona photos do carry capture times, and their time order equals filename order, so the output is unchanged. Reading capture times adds about 15 ms to a warm Girona re-run (0.188 s against 0.174 s with `--order name`, medians of 9).
