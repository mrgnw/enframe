---
branch: b1-colour
---

# B1 · Keep Display P3 and label the clip

Depends on: — · Model: Opus · Size: M
Touches: `rust/src/decode.rs`, `rust/src/imageio.rs`, `rust/src/avif.rs`, `rust/src/main.rs` (ffmpeg arguments, PNG frames), README, `docs/rust.md`

## Why

The train clouds photos are 10-bit Display P3 AVIF (`color_primaries=smpte432`), but every clip is labelled sRGB (`bt709`, set in `avif.rs`). Colour-managed viewers such as Safari and Photos show the clips duller than the photos. Tracking is unaffected.

## Decision (2026-10-06)

Keep the pixel values and copy the photos' colour label onto the clip. A set whose photos share one label needs no conversion.

## Goal

- Read each photo's label: AVIF's colour primaries and transfer (libavif), the colour space ImageIO reports for HEIC and JXL, the ICC profile in JPEG and PNG.
- Write it to the AVIF clip (nclx), the `.mp4` (`-color_primaries`, `-color_trc`, `-colorspace`) and the `--frames` PNGs (cICP or iCCP).
- Mixed sets: convert the sRGB photos to Display P3 (linearize, fixed 3x3 matrix, re-encode). sRGB fits inside P3, so nothing clips. If the conversion runs before tracking, the feature cache key covers it.
- Any other profile (Adobe RGB, ProPhoto, an unknown ICC): stop with an error that names the photo.

## Done when

- [x] `avifdec --info` on a train clouds clip shows P3 primaries and sRGB transfer; `ffprobe` shows the same on the `.mp4`.
- [x] An all-sRGB set (the JPEG conversions in `check.sh`) still gives an sRGB clip with unchanged pixels.
- [x] A mixed set (P3 AVIF and sRGB JPEG) gives a P3 clip, and a test checks the sRGB to P3 conversion on known values.
- [x] A browser screenshot of a train clouds photo and of its frame in the clip match in colour (Playwright, same browser for both).
- [x] Tracking output is unchanged: the train clouds comparison in `check.sh` still passes.
- [x] `bash analysis/check.sh` passes.

## Result

Clips, ffmpeg outputs and `--frames` PNGs carry the photos' profile (Display P3 for train clouds). Mixed sets convert sRGB photos to P3 at render time, so tracking and the cache are unchanged. In Chromium a photo and its clip frame now differ by 1.07 levels on average (was 4.19). Not verified: real Safari; Playwright's WebKit ignores the label on animated AVIF.
