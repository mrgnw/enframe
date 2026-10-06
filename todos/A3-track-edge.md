# A3 · Smaller tracking size, if accuracy holds

Depends on: — · Model: Terra · Size: S
Touches: `rust/src/track.rs` (`TRACK_EDGE`), `analysis/check.sh` expectations, docs

## Goal

Find the smallest `TRACK_EDGE` (now 1024) that keeps the A1 benchmark within tolerance, and take it if it saves real time. The Python spike measured 768 at about 0.5 s faster on Girona (`docs/performance.md`), but rects changed and accuracy was never measured.

## Done when

- [ ] A1 scores for 1024, 896 and 768, with Girona wall time, interleaved.
- [ ] The change is kept only if accuracy and stability stay within the A1 baseline's noise. Otherwise the numbers go in `docs/` and nothing changes.
- [ ] If kept: the cache version is bumped (features depend on `TRACK_EDGE`), and `check.sh`'s Python comparison is updated deliberately in the same branch.
- [ ] `bash analysis/check.sh` passes.
