---
branch: a2-tracker-sensitivity
---

# A2 · Make tracking less sensitive to tiny pixel differences

Depends on: — · Model: Opus · Size: M
Touches: `rust/src/track.rs`, `analysis/check.sh` expectations, docs

Decoding the same AVIF photos through macOS ImageIO instead of libavif changes pixels by at most 2–3 levels (PSNR about 50 dB). Yet on the Girona job the rects at IMG_4689/4690 moved by up to 1.60e-1 and the anchor's scale by 1.7e-1. Lossy re-encodes show the same effect at lens switches: JPEG 1.7e-2, HEIC 4.5e-2. Real photos arrive in many formats, so this matters for the core feature. Details: `docs/rust.md`, "Decode speed".

A1 (`docs/tracking.md`) measured it: IMG_4689/4690 move the rect by 1.60e-1 under ImageIO decoding, and IMG_4795 moves 29–45 px under every re-encode (PNG through `sips`, HEIC, JPEG q90). It also found that pinframe reduces 16-bit PNG to 8 bits by dropping the low byte instead of rounding; fix that here if it matters.

After A4 (search window), IMG_4795 has two fits about 30–45 px apart, and the original run switched to the other one (32.6 px, scale 1.076 to 1.089). Neither could be confirmed right by eye. Decide which is correct, then make the tracker pick it reliably.

## Find out

- At IMG_4689/4690, compare the two runs' matches, inlier counts and fitted transforms. Is RANSAC choosing between two near-equal solutions, are there few inliers, or is one photo's fit propagating down the chain?
- Is the RANSAC sampling deterministic in both the Python and Rust implementations, and does a different seed alone move the result?

## Possible fixes, to measure

- Refine the RANSAC fit on all inliers (least squares) after selection.
- Require a minimum inlier count or inlier ratio before accepting a scale change at a lens switch.
- Re-anchor from the last marked photo instead of chaining photo to photo.

## Done when

- [x] The cause at IMG_4689/4690 is explained with numbers.
- [x] A fix brings A1's stability score (and the ImageIO-against-libavif rect difference on Girona) under 2e-2, without making A1's accuracy worse or changing the LOST set.
- [x] If the fix changes tracking, `analysis/tracking_baseline.json` and `check.sh`'s expected values are updated deliberately in the same branch, with the change explained.
- [x] `bash analysis/check.sh` passes.

## Result

IMG_4795 had two fits with near-equal support (14 to 18 inliers), one turning +0.3° and one +8.9°, after a 9.2 s gap in which the cloud changed shape; RANSAC stopped after about 20 draws. The +0.3° fit is right (cloud overlap, whole-photo rotation, no other step turns over 1.4°). Fix: always draw all 2000 samples, score fits by inliers minus a turn penalty (3° spread), refit on inliers until stable. Worst move in any variant 44.4 to 2.8 px, largest crop change 2.15e-2 to 1.49e-2, accuracy and LOST unchanged, warm Girona re-run time unchanged (157 ms both). 16-bit PNG still truncates; rounding did not improve stability.
