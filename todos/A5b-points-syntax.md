# A5b · Points syntax on the command line

Depends on: — · Blocks: E3 · Model: Sonnet · Size: S
Touches: `rust/src/main.rs` (CLI), `analysis/girona_all.sh`, `analysis/tracking_benchmark.py`, README, `docs/rust.md`

## Goal

`--points x,y/x,y` on the first photo and `--at PHOTO=x,y/x,y@zoom` on later ones, as in A5's design. `--box` and the `x,y,w,h` form are removed and fail with a message that gives the four-quarter-centres conversion. `girona_all.sh` is rewritten with points.

## Done when

- [ ] The baseline is identical to A5a's: the same points go in, so any difference is a parsing bug.
- [ ] Limits enforced with messages: 1 to 32 points per mark, inside the photo, zoom positive.
- [ ] README documents the syntax.
- [ ] `bash analysis/check.sh` passes.
