# B1 ⚑ · Colour policy for Display P3 input

Depends on: a decision from the user · Blocks: C1 · Model: Opus · Size: M
Touches: `rust/src/decode.rs`, `rust/src/imageio.rs`, `rust/src/avif.rs`

## Decision needed

Colour profiles are ignored today: a Display P3 iPhone HEIC is treated as if its numbers were sRGB, so saturated colours in the clip can look wrong. Tracking is unaffected. Options:

1. **Keep ignoring** (today). Simplest. Colours in the clip may be off for wide-gamut photos.
2. **Convert every photo to sRGB on decode.** The clip looks right everywhere, but loses wide gamut. ImageIO can convert; libavif input would need a colour library (lcms2 or qcms).
3. **Keep native values and tag the clip with the input's profile** (C1). The colours stay right on wide-gamut displays, but mixed-profile inputs need a choice.

## Done when (after the decision)

- [ ] The chosen policy is implemented for every input format, with a test on a P3 photo.
- [ ] Tracking output is unchanged, or the change is measured with A1.
- [ ] `bash analysis/check.sh` passes.
