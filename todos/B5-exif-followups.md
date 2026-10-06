# B5 · Finish B4: EXIF in benchmark variants, JXL focal length

Depends on: — · Model: Sonnet · Size: S
Touches: `analysis/tracking_benchmark.py`, `analysis/tracking_baseline.json`, `rust/src/imageio.rs`, `rust/src/decode.rs`, `docs/tracking.md`

## Goal

- The benchmark's `pillow_png` and `noise_2` variants are written without EXIF, so since B4 they compare a run without focal lengths to one with them (crop change 1.11). Copy the EXIF into every variant so only pixels differ; regenerate the baseline's stability section.
- JXL photos get their focal length through ImageIO, as their capture time already does.
- Fix the doc comment in `imageio.rs` that names the removed `decode::exif_capture_time`.

## Done when

- [ ] Every stability variant compares like with like; `pillow_png` and `noise_2` back near 0 and 1.7e-2.
- [ ] A JXL converted from a Girona photo reports the same focal length as the AVIF.
- [ ] `bash analysis/check.sh` passes.
