# B4 · Seed lens-switch zoom from EXIF focal length

Depends on: — · Model: Opus · Size: M
Touches: `rust/src/track.rs`, `rust/src/decode.rs` (EXIF read), docs

## Goal

Original iPhone files record the focal length (35 mm equivalent), which gives each photo's lens directly (README "Next steps"). Use the ratio between consecutive photos to seed or check the zoom at lens switches, where most manual `--at ...,zoom` marks are needed today.

## Done when

- [ ] On a sequence with focal lengths, fewer manual zoom marks are needed, measured.
- [ ] A1 accuracy is the same or better. Photos without focal lengths behave exactly as before.
- [ ] Uses the Girona photos: they carry `FocalLengthIn35mmFormat` (IMG_4697 48 mm, IMG_4698 120 mm, ratio 2.5, which matches the `@2.5` mark).
- [ ] `bash analysis/check.sh` passes.
