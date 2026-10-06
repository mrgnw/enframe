# A5 · Several reference points instead of a box

Depends on: — · Blocks: E3 · Model: Opus · Size: L (tracker first, then CLI, if it grows)
Touches: `rust/src/track.rs`, `rust/src/main.rs` (CLI, sidecar), README, `docs/rust.md`, `analysis/check.sh`, `analysis/girona_all.sh`

## Decision (2026-10-06)

The user marks the feature with one or more points instead of a box, and sees all of them in every photo. The tracker passes where the points were to the next photo as the hint for where to look (A4). Still one feature per clip.

## Goal

- The first photo and each mark take one or more points. The anchor that stays still in the clip is their centre.
- The tracker follows the keypoints near the points. The area around each point comes from the points' spacing and the keypoint density, not a fixed number; one point must still work.
- The fitted transform moves every point, and the sidecar's `points` (E1) lists them all.

## ⚑ Design first

Before building, the thread writes into this card and stops for the user's OK:

- the CLI syntax for points on the first photo and in `--at` marks, including zoom
- what happens to `--box` (kept as a shorthand, or removed)
- the rule for the area around each point

## Done when

- [ ] The Girona job runs with points only, and A1 shows accuracy and LOST count no worse than with boxes.
- [ ] README describes the new marks.
- [ ] `bash analysis/check.sh` passes.
