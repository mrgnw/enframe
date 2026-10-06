---
branch: a5a-points-tracker
---

# A5a · Track a set of points instead of a box

Depends on: — · Blocks: A5b · Model: Opus · Size: M
Touches: `rust/src/track.rs`, `rust/src/main.rs` (marks to point sets, sidecar `points`), `analysis/tracking_baseline.json`, `docs/tracking.md`

## Goal

The tracker follows a set of points with the area rule from A5's design. The CLI stays as it is: `--box` and box marks become their four quarter centres internally, a point mark `--at PHOTO=x,y` becomes a one-point set that replaces the current set (decision 3). The sidecar's `points` lists every point.

## Done when

- [x] Accuracy max within 1.5 px, LOST 0 on synthetic sequences and IMG_4675/4721/4725 on Girona.
- [x] The baseline is regenerated and every move is explained in `docs/tracking.md`.
- [x] `bash analysis/check.sh` passes.

## Result

Accuracy median 0.213 to 0.096 px, max 0.999 to 1.092 px; LOST unchanged everywhere. IMG_4689 under ImageIO decoding moves 0.9 px instead of 7.8; the largest crop change in any variant fell from 1.60e-1 to 2.15e-2. IMG_4795 still has two fits (A2). On Girona 41 of 57 photos moved 1 px or less; each larger move is explained in `docs/tracking.md`. `check.sh` and the benchmark now take the anchor as the mean of `points`.
