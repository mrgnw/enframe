# A2 · Make tracking less sensitive to tiny pixel differences

Depends on: A1 (to measure fixes; the diagnosis can start before) · Model: Opus · Size: M
Touches: `rust/src/track.rs`, `analysis/check.sh` expectations, docs

Decoding the same AVIF photos through macOS ImageIO instead of libavif changes pixels by at most 2–3 levels (PSNR about 50 dB). Yet on the Girona job the rects at IMG_4689/4690 moved by up to 1.60e-1 and the anchor's scale by 1.7e-1. Lossy re-encodes show the same effect at lens switches: JPEG 1.7e-2, HEIC 4.5e-2. Real photos arrive in many formats, so this matters for the core feature. Details: `docs/rust.md`, "Decode speed".

## Find out

- At IMG_4689/4690, compare the two runs' matches, inlier counts and fitted transforms. Is RANSAC choosing between two near-equal solutions, are there few inliers, or is one photo's fit propagating down the chain?
- Is the RANSAC sampling deterministic in both the Python and Rust implementations, and does a different seed alone move the result?

## Possible fixes, to measure

- Refine the RANSAC fit on all inliers (least squares) after selection.
- Require a minimum inlier count or inlier ratio before accepting a scale change at a lens switch.
- Re-anchor from the last marked photo instead of chaining photo to photo.

## Done when

- [ ] The cause at IMG_4689/4690 is explained with numbers.
- [ ] A fix brings A1's stability score (and the ImageIO-against-libavif rect difference on Girona) under 2e-2, without making A1's accuracy worse or changing the LOST set.
- [ ] If the fix changes tracking, `check.sh`'s Python comparison is updated deliberately in the same branch, with the new expected difference explained (or the Python reference gets the same fix, per F1).
- [ ] `bash analysis/check.sh` passes.
