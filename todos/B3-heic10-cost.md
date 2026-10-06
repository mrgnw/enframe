# B3 · Cut 10-bit HEIC decode CPU

Depends on: — · Model: Terra · Size: S
Touches: `rust/src/imageio.rs`

## Goal

10-bit HEIC costs 41 ms CPU per photo against 12 ms for 8-bit (`docs/rust.md`, "Decode speed"). iPhones shoot 10-bit, so this is the common case. Find out whether the time is ImageIO's or pinframe's own packed `l10r` unpacking, and cut what is ours.

## Done when

- [ ] A profile shows where the 10-bit time goes.
- [ ] If it's ours: the unpacking is faster, with output bit-identical to before (compare decoded bytes on the 10-bit fixtures).
- [ ] The speed table in `docs/rust.md` is updated.
- [ ] `bash analysis/check.sh` passes.
