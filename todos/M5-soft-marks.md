# M5 · Soft marks

Depends on: M4 (shares `web/src/lib/marking/`) · Model: Opus · Size: M
Touches: `rust/src/lib.rs`, `rust/src/main.rs`, `rust/src/track.rs`, `README.md`, `docs/rust.md`, `web/src/lib/marking/`, `web/e2e/`, `analysis/check.sh` (one new README command if needed)

## Problem

A mark today does two jobs: it places that photo, and it becomes the tracker's new reference (its points, scale and the features near them carry on to the next photos). On a lost photo the user can often see roughly where the feature is but cannot place it precisely: haze, a half-hidden cloud, a blurred frame. A rough mark then re-seeds tracking from a poor reference and can lose the photos after it. Skipping the photo throws the rough position away.

## Goal

A soft mark places its photo but is not a tracking reference.

- The photo is positioned at the soft mark's points and zoom, and counts as a found photo for estimating lost and skipped neighbours (`fill_lost`).
- Tracking of the next photos continues from the last hard reference (tracked photo or mark), exactly as if the soft-marked photo were lost: `points`, `radius`, `scale`, `reference` and `reach` in `track::track` are unchanged by it.
- The CLI gets a syntax for it alongside `--at` and `--skip` (for example `--near PHOTO=x,y/x,y@zoom`); pick one and say why. Same validation as `--at`: points inside the photo, not also marked or skipped, not the first photo. The sidecar reports its source as `soft`.
- The UI offers it on a lost photo next to Mark and Skip, and in M4's "first track failed" recovery next to Clear and Skip. A soft-marked photo shows its own label and trail style, distinct from a mark.

## Done when

- [ ] A unit test in `track.rs`: a soft mark on photo i leaves the hits after i identical to the run with photo i lost, and photo i's hit is at the soft mark.
- [ ] The README documents the flag; `bash analysis/check.sh` passes with tracking unchanged on the baseline.
- [ ] In the UI, a soft mark can be placed, cleared and reloaded from the document; `bash analysis/check.sh --browser` passes.
