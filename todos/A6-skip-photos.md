# A6 · Skip photos where the feature is hidden

Depends on: — · Model: Sonnet · Size: S
Touches: `rust/src/track.rs`, `rust/src/main.rs` (CLI, sidecar), README, `analysis/tracking_benchmark.py` if needed

## Why

Sometimes the feature is simply not in the photo: a building or trees block it. Marking points there is impossible, and letting the tracker try risks a wrong fit on the building that every later photo then follows.

## Goal

- `--skip PHOTO` (repeatable) marks a photo where the feature is hidden. The tracker does not match it, and the next photo is matched against the last photo where the feature was found, as lost photos already are.
- The sidecar gives skipped photos `source: "skipped"` and the estimated position, like lost photos, so the UI can show them as handled, not as "to fix".
- A skipped photo stays in the clip at the estimated position, as lost photos do (decided 2026-10-06).

## Done when

- [ ] A test where a skipped photo would otherwise produce a wrong fit, and the photos after it still track.
- [ ] README documents `--skip`.
- [ ] `bash analysis/check.sh` passes.
