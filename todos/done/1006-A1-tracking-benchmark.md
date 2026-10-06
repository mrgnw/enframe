---
branch: a1-tracking-benchmark
---

# A1 · Tracking accuracy and stability benchmark

Depends on: E1 · Blocks: A2, A3, B4 · Model: Opus · Size: M
Touches: `analysis/` (new script and baseline JSON), `docs/tracking.md` (new). No changes to `rust/src` or `anchor.py`.

## Goal

One command that scores tracking, so changes to the tracker can be judged on accuracy, not only "identical to before":

```sh
uv run analysis/tracking_benchmark.py
```

It prints a one-line summary and writes a JSON report.

## What it measures

- **Accuracy against known truth.** Build sequences from Girona photos with transforms you choose: crop, shift, scale (including 2–3x jumps like lens switches) and small rotations. The true anchor position is therefore known exactly. Report the anchor position error in pixels, the scale error in percent, and the LOST count.
- **Stability under tiny input changes.** Run the real Girona job on the AVIF originals and on re-encodes: AVIF through ImageIO-equivalent pixels, HEIC, JPEG q90, and ±2-level noise. Report how far each photo's anchor moves (max and median) and whether the LOST set changes.

## Done when

- [x] The benchmark runs in under 2 minutes through the Rust CLI in JSON-only mode.
- [x] Baseline numbers for `main` are recorded in `docs/` and in `out/`-independent form (committed JSON) for later comparison.
- [x] It reproduces the known sensitivity at IMG_4689/4690 (`docs/rust.md`, "Decode speed").
- [x] `bash analysis/check.sh` passes.

## Result

`uv run analysis/tracking_benchmark.py` runs in about 25 s and is deterministic. Accuracy: median 0.21 px, max 1.0 px, no LOST, across 2–3x zoom jumps. Stability: medians about 0.1 px, but IMG_4689/4690 (rect 1.60e-1, reproduced) and IMG_4795 (29–45 px under every re-encode) are sensitive. Baseline: `analysis/tracking_baseline.json`; details in `docs/tracking.md`.
