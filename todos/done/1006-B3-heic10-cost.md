---
branch: b3-heic10-cost
---

# B3 · Cut 10-bit HEIC decode CPU

Depends on: — · Model: Terra · Size: S
Touches: `rust/src/imageio.rs`

## Goal

10-bit HEIC costs 41 ms CPU per photo against 12 ms for 8-bit (`docs/rust.md`, "Decode speed"). iPhones shoot 10-bit, so this is the common case. Find out whether the time is ImageIO's or pinframe's own packed `l10r` unpacking, and cut what is ours.

## Done when

- [x] A profile shows where the 10-bit time goes.
- [x] If it's ours: the unpacking is faster, with output bit-identical to before (compare decoded bytes on the 10-bit fixtures).
- [x] The speed table in `docs/rust.md` is updated.
- [x] `bash analysis/check.sh` passes.

## Result

ImageIO takes 34–42 ms of the time on tiled 10-bit files (iPhone layout); pinframe's unpacking went from 3.0 to 1.9 ms with identical bytes. No ImageIO option returns 8-bit pixels. Details in `docs/rust.md`, "Decode speed".
