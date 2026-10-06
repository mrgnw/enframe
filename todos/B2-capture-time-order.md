# B2 · Order photos by capture time

Depends on: — · Blocks: B4 · Model: Terra · Size: S
Touches: `rust/src/main.rs` (photo order), `rust/src/decode.rs` (EXIF date read), README

## Goal

Photos are ordered by filename today (`ponytail:` note in `main.rs`), which breaks when names don't follow shooting order, for example when mixing two cameras. Order by EXIF `DateTimeOriginal` (plus `SubSecTimeOriginal`) when every photo has it, and fall back to filename otherwise. Add `--order time|name`, defaulting to time-with-fallback, and print which order was used.

## Done when

- [ ] Works for AVIF, HEIC, JXL, JPEG and PNG, reading only headers.
- [ ] Girona (no capture times in these exports) keeps its filename order and identical output.
- [ ] A test with three files whose names and times disagree.
- [ ] `bash analysis/check.sh` passes.
