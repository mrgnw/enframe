---
branch: a4-search-window
---

# A4 · Look for the feature near where it should be

Depends on: — · Blocks: A5 · Model: Opus · Size: M
Touches: `rust/src/track.rs`, `analysis/check.sh` expectations, `docs/rust.md`

## Why

The tracker matches the feature's keypoints against every keypoint in the next photo. The last photo already says roughly where the feature is. With features cached, matching is the whole cost of a re-run: 0.098 s of 0.102 s on Girona, which is what the marking screen waits on after each mark. Matching only near the predicted spot is faster and cannot pick a look-alike elsewhere in the photo.

## Goal

- Predict where the reference points land in the next photo, from the last find.
- Match only against next-photo keypoints within a window around that prediction. Derive the window size from the motion A1 measures between photos, not a fixed number.
- When too few keypoints agree inside the window, retry on the whole photo, so lens switches and big jumps still link.

## Done when

- [x] A1 shows accuracy equal or better, and no more LOST photos.
- [x] The warm-cache Girona re-run is faster; before and after go in `docs/rust.md`.
- [x] `bash analysis/check.sh` passes, with its expected values updated in this branch if tracking changes.

## Result

Matching looks within the reference features' bounds grown by `MOTION_MARGIN` (5) times the last step's motion, and falls back to the rest of the photo when that gives no plausible fit. Warm Girona re-run: match 100.6 to 47.7 ms, whole process 118 to 65 ms. Accuracy and every LOST set unchanged. IMG_4795 moved 32.6 px to its other near-equal fit, which shifts the stability numbers; see A2.
