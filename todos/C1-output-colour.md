# C1 · Colour profile in the output clip

Depends on: B1 · Model: Terra · Size: S
Touches: `rust/src/avif.rs` (encoder), render

## Goal

Write the colour profile that B1 settles on into the AVIF clip (ICC or nclx), so browsers and editors show the colours as intended. For ffmpeg outputs, set the matching colour tags.

## Done when

- [ ] `avifdec --info` (or ffprobe for ffmpeg outputs) shows the profile.
- [ ] A P3 test clip displays correctly in Safari and Chrome (checked with Playwright screenshots against a reference).
- [ ] `bash analysis/check.sh` passes.
