---
branch: b4-zoom-from-exif
---

# B4 · Seed lens-switch zoom from EXIF focal length

Depends on: — · Model: Opus · Size: M
Touches: `rust/src/track.rs`, `rust/src/decode.rs` (EXIF read), docs

## Goal

Original iPhone files record the focal length (35 mm equivalent), which gives each photo's lens directly (README "Next steps"). Use the ratio between consecutive photos to seed or check the zoom at lens switches, where most manual `--at ...,zoom` marks are needed today.

## Done when

- [x] On a sequence with focal lengths, fewer manual zoom marks are needed, measured.
- [x] A1 accuracy is the same or better. Photos without focal lengths behave exactly as before.
- [x] Uses the Girona photos: they carry `FocalLengthIn35mmFormat` (IMG_4697 48 mm, IMG_4698 120 mm, ratio 2.5, which matches the `@2.5` mark).
- [x] `bash analysis/check.sh` passes.

## Result

`FocalLengthIn35mmFormat` predicts the lens-switch zoom (measured 2.55x against 2.5x). A mark without `@zoom` takes the previous scale times the lens ratio (decided 2026-10-06: follow the lens; an explicit `@zoom` wins). Crossing a switch, the tracker searches the whole photo and prefers fits near the lens ratio. Girona drops 3 of 11 `@zoom` marks with identical output; no whole mark can go (1 to 7 matches at switches, 8 needed). Three zoomless marks now follow the lens (IMG_4680, 4773, 4796), which keeps magnification steady but softens IMG_4680/4681. Largest crop change 2.05e-2, accepted: no point moved, the crop plan did. Follow-ups in [B5](../B5-exif-followups.md).
