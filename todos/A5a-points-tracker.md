# A5a · Track a set of points instead of a box

Depends on: — · Blocks: A5b · Model: Opus · Size: M
Touches: `rust/src/track.rs`, `rust/src/main.rs` (marks to point sets, sidecar `points`), `analysis/tracking_baseline.json`, `docs/tracking.md`

## Goal

The tracker follows a set of points with the area rule from A5's design. The CLI stays as it is: `--box` and box marks become their four quarter centres internally, a point mark `--at PHOTO=x,y` becomes a one-point set that replaces the current set (decision 3). The sidecar's `points` lists every point.

## Done when

- [ ] Accuracy max within 1.5 px, LOST 0 on synthetic sequences and IMG_4675/4721/4725 on Girona.
- [ ] The baseline is regenerated and every move is explained in `docs/tracking.md`.
- [ ] `bash analysis/check.sh` passes.
