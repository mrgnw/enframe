---
branch: f1-retire-python
---

# F1 · Retire the Python reference

Depends on: — · Blocks: A2, A3, A4, A5 (they change tracking on purpose) · Model: Sonnet · Size: S
Touches: `anchor.py`, `analysis/check.sh`, the analysis scripts that only run or compare `anchor.py`, README, `docs/`. Not `analysis/mockup_data.py` or `docs/ui-plan.md` (E5 owns them).

## Decision (2026-10-06)

Retire `anchor.py` once A1 merges. A1's committed baseline replaces the Rust-against-Python comparison as the check that tracking is right. The cards that change tracking come after this one, so Python never has to follow them.

What goes with it: the mouse window for drawing a box. Until the marking screen (E3), marks are typed as numbers.

## Goal

- `check.sh` compares the train clouds run against A1's committed baseline instead of `anchor.py`.
- Delete `anchor.py` and the scripts that exist only to run or compare it, in one commit. Keep the fixtures the Rust tests use.
- README and docs stop pointing users to the Python version. History notes such as `docs/performance.md` may still mention it.

## Done when

- [x] `check.sh` runs no Python tracker and still fails when train clouds tracking moves (try a deliberate change to `RANSAC_TOLERANCE`, then revert it).
- [x] `bash analysis/check.sh` passes.

## Result

`anchor.py` and 12 scripts that only ran or compared it are gone. `check.sh` compares the train clouds run against `analysis/tracking_baseline.json` exactly (rect 1e-9, scale 5e-5, point 6e-4 px, each set by storage precision); a `RANSAC_TOLERANCE` change fails it. Comments in `rust/src` that cite `anchor.py` as the origin of a behaviour remain.
