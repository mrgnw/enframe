# Make tracking less sensitive to tiny pixel differences

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
- [ ] A fix keeps the Python reference parity check meaningful, and brings the ImageIO-against-libavif rect difference under 2e-2 on Girona without changing the LOST set.
