---
branch: a5b-points-syntax
---

# A5b · Points syntax on the command line

Depends on: — · Blocks: E3 · Model: Sonnet · Size: S
Touches: `rust/src/main.rs` (CLI), `analysis/girona_all.sh`, `analysis/tracking_benchmark.py`, README, `docs/rust.md`

## Goal

`--points x,y/x,y` on the first photo and `--at PHOTO=x,y/x,y@zoom` on later ones, as in A5's design. `--box` and the `x,y,w,h` form are removed and fail with a message that gives the four-quarter-centres conversion. `girona_all.sh` is rewritten with points.

## Done when

- [x] The baseline is identical to A5a's: the same points go in, so any difference is a parsing bug.
- [x] Limits enforced with messages: 1 to 32 points per mark, inside the photo, zoom positive.
- [x] README documents the syntax.
- [x] `bash analysis/check.sh` passes.

## Result

`--points x,y/x,y` and `--at PHOTO=x,y/x,y@zoom`; `--box` and `x,y,w,h` marks fail with the four-quarter-centres conversion. Benchmark output identical to the A5a baseline. Every script, the README and current docs use points.
