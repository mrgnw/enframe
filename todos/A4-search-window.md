# A4 · Look for the feature near where it should be

Depends on: F1 · Blocks: A5 · Model: Opus · Size: M
Touches: `rust/src/track.rs`, `analysis/check.sh` expectations, `docs/rust.md`

## Why

The tracker matches the feature's keypoints against every keypoint in the next photo. The last photo already says roughly where the feature is. With features cached, matching is the whole cost of a re-run: 0.098 s of 0.102 s on Girona, which is what the marking screen waits on after each mark. Matching only near the predicted spot is faster and cannot pick a look-alike elsewhere in the photo.

## Goal

- Predict where the reference points land in the next photo, from the last find.
- Match only against next-photo keypoints within a window around that prediction. Derive the window size from the motion A1 measures between photos, not a fixed number.
- When too few keypoints agree inside the window, retry on the whole photo, so lens switches and big jumps still link.

## Done when

- [ ] A1 shows accuracy equal or better, and no more LOST photos.
- [ ] The warm-cache Girona re-run is faster; before and after go in `docs/rust.md`.
- [ ] `bash analysis/check.sh` passes, with its expected values updated in this branch if tracking changes.
