# Performance

Where pinframe spends its time on a real run, what made it faster, and what didn't.

All numbers are from `analysis/girona_all.sh`: 57 AVIF photos (2048x1536, some portrait), 24 marks, `--pad transparent` to ProRes 4444 `.mov`, clip 1442x1196. Machine: Apple M4 Pro, 12 cores, Python 3.14, OpenCV 5.0.0, ffmpeg 8.1. Runs are warm. The first `uv run` in a fresh checkout also builds the venv, which took the first run to 11.4 s.

## Measuring

```sh
bash analysis/bench.sh 3
```

`bench.sh` runs the benchmark through `analysis/stage_times.py`, which wraps `anchor.py`'s functions with timers and prints per-stage seconds and peak memory. It then checks the JSON against a baseline with `analysis/compare_rects.py`: same `source` per photo, `scale` within 1e-4, rects within 1e-4. Every kept change matched the baseline exactly (difference 0), with the same 3 LOST photos (IMG_4675, IMG_4721, IMG_4725). The clip's frames matched too (`ffmpeg -f framemd5`), and `--frames` PNGs are byte-identical.

## Baseline

| stage | calls | seconds |
|-|-|-|
| load (AVIF decode, `exif_transpose`) | 57 | 0.61 |
| track: whole-photo SIFT | 33 | ~0.9 |
| track: masked SIFT in the box (`features_in`) | 54 | 1.00 |
| track: matching + RANSAC (`similarity`) | 33 | 0.03 |
| track: gray + downscale | 57 | 0.03 |
| render: `place` (resize + warp) | 57 | 0.85 |
| render: ffmpeg ProRes encode (pipe back-pressure) | | ~0.55 |
| `--frames` PNG writes (only with `--frames`) | 57 | 4.52 |
| main() total | | 3.96 |
| wall clock, `uv run` + imports included | | 4.23 |
| peak RSS | | 1.04-1.09 GB |

Peak memory is mostly the 57 decoded photos: 57 x 9.4 MB of RGB is 540 MB, held from load to render.

## Kept

| change | before | after |
|-|-|-|
| Reuse the whole-photo SIFT for the box features of tracked photos | track 1.95 s | track 1.45 s |
| Place frames and write PNGs on 8 worker threads, in order, while ffmpeg encodes | `--frames` total 7.48 s, PNG 4.52 s | `--frames` total 3.8 s |
| **Wall clock, 5 runs, median** | **4.23 s** | **3.71 s (-12%)** |
| **Wall clock with `--frames`** | **8.1 s** | **4.2 s (-49%)** |

Peak RSS rose by about 45 MB, from the 8 frames in flight during render.

**SIFT reuse.** OpenCV's SIFT applies the mask only after it has detected keypoints on the whole image: `detectAndCompute(g, mask)` detects everywhere, then drops keypoints whose rounded position falls outside the mask. A tracked photo already ran whole-photo SIFT to match against. Filtering those keypoints by the box gives the same keypoints and descriptors as a second masked run. `analysis/mask_filter_check.py` checks this on 5 random boxes on every Girona photo. Marked photos still run masked SIFT. Masked SIFT costs about 19 ms against 29 ms for a whole-photo run, because it computes descriptors only for the keypoints it keeps. Running whole-photo SIFT on every photo and filtering everywhere saved only 0.16 s.

**Threaded render.** zlib and OpenCV release the GIL. `ThreadPoolExecutor.map(..., buffersize=8)` places and saves up to 8 frames at once, and the main thread pipes them to ffmpeg in order. Without `--frames` it gains little (render 1.4 s to 1.3 s), because ProRes is the bottleneck.

## Tried and rejected

| idea | result | why rejected |
|-|-|-|
| Crop to the box plus a margin before masked SIFT | `features_in` 1.00 s to 0.38-0.88 s | Rects drift by 1e-3 to 1e-2 even with a 2x box margin. SIFT's octave count and blur borders depend on the image size. |
| Parallel decode (thread pool) | total 3.96 s to 3.89 s | libavif already decodes on all cores (8.5 s of thread time with 12 workers). With 12 workers peak RSS rose from 1.04 to 1.5 GB. |
| Pipeline `place` with the encoder (4 threads) | render unchanged at 1.45 s | ProRes encoding is the bottleneck. Kept only for `--frames`, as above. |
| Whole-photo SIFT for upcoming photos on worker threads (`analysis/sift_threads.py`) | track 1.39 s to 1.17 s (8 workers) or 1.22 s (4) | Peak RSS rose from 1.08 GB to 1.9 GB (8 workers) or 1.55 GB (4). Each SIFT holds a ~100 MB pyramid. |
| FLANN matcher | not tried | All matching plus RANSAC takes 0.03 s. |
| PNG `compress_level=1` | PNG 4.52 s to 2.11 s, 55% larger files | Threads at the default level take 0.51 s with identical files (`analysis/png_levels.py`). |
| Faster ProRes | prores_ks: 1.22 s for 57 frames, using 10 cores | `-threads 12`: 1.24 s. `prores_videotoolbox`: 1.16 s (less CPU, same wall clock). RGBA to 10-bit YUV conversion costs 0.09 s. No setting makes ProRes 4444 faster. |
| Smaller `TRACK_EDGE` | 768: track 0.94 s, total 2.97 s. 512: track 0.67 s, total 2.66 s | Rects change beyond 1e-4. At 512 one more photo (IMG_4696) is LOST. Accuracy is untested against ground truth. |

## Where the time went before threading (ProRes output)

Measured with an earlier version of `analysis/profile_stages.py`, which charged each second of the main thread to exactly one stage. The current breakdown is under "Where the time goes now". Medians of 9 runs, with other apps running, so totals sit above the 3.71 s quiet-machine figure; the shares are what matter.

**Clip only, 3.90 s:**

| stage | seconds | share |
|-|-|-|
| render: ffmpeg ProRes encode (blocked writing 1.03 + finishing 0.52) | 1.55 | 40% |
| track: SIFT, whole photo (33 tracked photos) | 0.96 | 25% |
| decode 57 AVIFs | 0.67 | 17% |
| track: SIFT in box (24 marked photos) | 0.46 | 12% |
| `uv run`, Python start, imports | 0.16 | 4% |
| everything else (matching, RANSAC, plan, placing frames, JSON) | 0.10 | 3% |

**With `--frames`, 4.46 s:** render grows from 1.58 s to 2.09 s. The main thread now waits on the PNG workers (1.25 s) more than on ffmpeg (0.82 s). PNG compression is the largest CPU cost of the run: 9.6 thread-seconds across 8 threads.

**Cores busy per phase:**

| phase | wall s | CPU s | cores busy |
|-|-|-|-|
| decode | 0.67 | 3.75 | 5.6 |
| track | 1.50 | 9.03 | 6.0 |
| render, clip only (Python side) | 1.58 | 1.41 | 0.9, plus ffmpeg at 11 CPU-s ≈ 7 cores |
| render, with `--frames` | 2.09 | 7.19 | 3.4, plus ffmpeg |

The three phases run one after another, and each already keeps 6–7 of 12 cores busy. That is why more threading gave little: the next gains have to come from doing less work.

The crop size depends on every photo's anchor (`crop_rects` takes min or median margins over all of them). So no frame can be rendered or encoded until tracking has finished. Encoding cannot overlap tracking. Only decode and tracking can overlap.

## Output encoding on the Mac mini (M4 Pro, 24 GB, macOS 27.0.1)

```sh
FFMPEG=/opt/homebrew/bin/ffmpeg uv run analysis/codecs.py
```

**The `ffmpeg` on `PATH` was an Intel build.** `/Users/user/.local/bin/ffmpeg` is the x86_64 static build from evermeet.cx (8.1). It runs under Rosetta and shadows Homebrew's native arm64 ffmpeg 9.0.1 at `/opt/homebrew/bin/ffmpeg`. The Intel build can't open VideoToolbox encoders (`Cannot create compression session: -12908`) and lacks SVT-AV1. It also encodes slower: the Girona ProRes run takes 3.75 s through it against 3.42 s through the native build. Every ProRes number above was measured through it. **Fixed on 2026-10-05:** Homebrew's ffmpeg was upgraded to 9.0.2, and the Intel `ffmpeg` and `ffprobe` were moved to `/Users/user/.local/share/ffmpeg-x86_64-evermeet/`, so `PATH` now finds the native build. Its one gap: it has no libaom, and SVT-AV1 can't encode the one-plane alpha image that ffmpeg's AVIF muxer requires, so ffmpeg can't write AVIF with alpha. Pillow's libavif can.

**No hardware AV1 encoder.** `VTCopyVideoEncoderList` on this machine lists hardware H.264, HEVC, HEVC with alpha ("Apple Muxed Alpha"), ProRes and JPEG encoders, but no `av01`. Apple's spec for this Mac lists "AV1 decode" only ([Mac mini (2024) tech specs](https://support.apple.com/en-us/121555)). AV1 and AVIF encoding run on the CPU.

57 Girona frames at 1442x1196, native ffmpeg, one run each. PSNR is against the placed frames.

**Transparent** (`--pad transparent`, the Girona default):

| encoding | wall s | CPU s | MB | PSNR RGB dB | PSNR alpha dB |
|-|-|-|-|-|-|
| ProRes 4444 (`.mov`) | 1.03 | 7.3 | 109.3 | 63.0 | lossless |
| VP9 alpha crf 30 (`.webm`) | 8.89 | 26.1 | 6.2 | 38.9 | 70.7 |
| HEVC + alpha, VideoToolbox HW, q 80 | 0.85 | 0.2 | 12.9 | 38.1 | 71.7 |
| ProRes 4444, VideoToolbox HW | 0.39 | 0.2 | 67.0 | 50.7 | lossless |
| animated AVIF, aom speed 6 (Pillow's default) | 8.18 | 37.2 | 10.1 | 41.0 | lossless |
| **animated AVIF, aom speed 10, q 75 (new default)** | 1.01 | 4.2 | 9.3 | 40.0 | 71.9 |
| animated AVIF, aom speed 10, q 85 | 1.06 | 4.5 | 12.7 | 41.5 | 73.0 |

**Opaque** (`--pad black`). The default crop without `--pad` is only 392x368 on Girona, where every encoder takes about 0.3 s.

| encoding | wall s | CPU s | MB | PSNR RGB dB |
|-|-|-|-|-|
| H.264 x264 crf 18 (`.mp4`) | 1.29 | 8.6 | 17.3 | 40.9 |
| HEVC VideoToolbox HW, q 85 | 0.49 | 0.3 | 16.4 | 41.0 |
| HEVC VideoToolbox HW, q 75 | 0.43 | 0.3 | 9.2 | 39.1 |
| AV1 SVT-AV1 preset 10, crf 23 | 0.44 | 2.1 | 8.1 | 39.4 |
| animated AVIF, aom speed 10, q 75 | 0.84 | 3.6 | 9.2 | 40.0 |

- **AVIF** was the default until #33 removed animated AVIF output with libaom (`.mp4`, native H.264, is the default now), because it was the one format that is small, keeps alpha, and needs no ffmpeg. At 40 dB it is a twelfth of ProRes and encodes as fast. In Playwright's WebKit and Chromium, the animated transparent AVIF decodes at full size, animates, and shows the page background through its transparent areas. Real Safari was not tested.
- **The hardware HEVC encoder** uses almost no CPU, but at equal PSNR its files are as large as x264's. With alpha it is only as fast as AVIF, because feeding frames is the limit, and its colour is worse per megabyte.
- **SVT-AV1** is the fastest small opaque encoder: 0.44 s, 8 MB. It has no alpha and needs the native ffmpeg.
- **VP9 with alpha** takes 8.9 s, the slowest by far.

End to end, transparent Girona, 9 runs each:

| `--out` | median wall | peak RSS | file |
|-|-|-|-|
| `.mov` through the Intel ffmpeg | 3.75 s | 1.07 GB | 109 MB |
| `.mov` through the native ffmpeg | 3.42 s | 1.07 GB | 109 MB |
| `.avif` | 3.42–3.57 s | 1.5 GB | 9.3 MB |

AVIF holds every placed frame for Pillow's encoder, which adds about 450 MB of peak memory.

## Threaded decode + SIFT

```sh
uv run analysis/prepare_threads.py
```

Before this change, all 57 photos were decoded and then SIFT ran photo by photo. Each phase kept only about 6 of 12 cores busy. Whole-photo SIFT depends only on the photo: a marked photo's box features are the whole-photo features filtered to the box (see "SIFT reuse"). So decode + SIFT can run as one parallel map over photos, and the tracking loop only filters and matches.

Decode + SIFT for all 57 Girona photos, each variant in a fresh process, 3 runs; features are identical in every variant:

| variant | threads | seconds | peak MB |
|-|-|-|-|
| decode all, then SIFT one photo at a time | 1 | 2.22–2.39 | 1019–1137 |
| decode all, then SIFT on threads | 4 | 1.94–2.10 | 1364–1609 |
| decode all, then SIFT on threads | 8 | 1.86–2.09 | 1748–2024 |
| **decode + SIFT per photo on threads** | 4 | 1.55–1.67 | 1029–1047 |
| **decode + SIFT per photo on threads (kept)** | 6 | 1.48–1.54 | 1243–1297 |
| decode + SIFT per photo on threads | 8 | 1.55–1.71 | 1431–1594 |

Threading SIFT alone gains little, because OpenCV already parallelizes each SIFT. Overlapping one photo's AVIF decode with another's SIFT fills the idle cores. `PREPARE_WORKERS = 6` is the best on the M4 Pro; at 8 the efficiency cores and memory traffic cost more than they add.

End to end, Girona to `.avif`, 9 runs: **2.86 s** (from 3.42–3.57 s), with decode + SIFT 1.45 s on about 9.4 cores and the tracking loop 0.07 s. Peak memory is 1.86 GB, up from 1.5 GB. Rects are identical to the baseline.

**Other ways to reuse earlier keypoints, and why they were not taken:**

- **Search only near the last find** (crop the next photo to a window around the predicted anchor): about half the SIFT work on paper. But it makes each photo's features depend on the previous result. That serializes SIFT again, so it loses the threading above, and it can't be cached. It needs a window-size rule and a whole-photo fallback for lens switches and big jumps, and it changes the output (see the cropped-SIFT row under "Tried and rejected"). More moving parts for less gain than the threaded map.
- **Optical flow from the previous keypoints** (pyramidal Lucas-Kanade): fast, but it assumes small motion between photos. These photos are seconds apart from a moving train and switch lenses, so it would need SIFT as a fallback anyway.
- **Cache features across runs**: this is where earlier knowledge pays off most. Features are now a pure function of the photo, so a cache keyed by file path, mtime and `TRACK_EDGE` would make re-runs after a new mark skip SIFT entirely. See idea 0.

## Bounded memory

```sh
uv run analysis/memory_scaling.py
```

Peak memory used to grow with the photo count, because every decoded photo was held from load to render and the AVIF encoder took the whole clip at once. Peak RSS (the whole process, from `/usr/bin/time -l`), first N Girona photos, transparent padding, AVIF out:

| photos | before | streaming, no keep | streaming + keep (current) |
|-|-|-|-|
| 10 | 1107 MB | 1136 MB | about 1.1 GB |
| 20 | 1173 MB | 1144 MB | |
| 40 | 1666 MB | 1192 MB | |
| 57 | 1926 MB | 1158 MB | 1560–1570 MB |

The current version:

- **Tracking consumes photos as the threads finish them** (`load_photos` yields in order). It keeps only each photo's features, never its pixels, so the tracking pass holds about `PREPARE_AHEAD` small results, whatever the photo count.
- **Decoded photos are kept for render only within `KEEP_SHARE` (a quarter) of physical RAM**: 6 GB on this Mac. Photos past the budget are decoded again at render. With a 25 MB budget, 55 of 57 Girona photos were decoded twice and the rects were still identical.
- **The AVIF encoder pulls one frame at a time** through `StreamedClip`, a multi-frame `Image` whose `tobytes()` takes the next placed frame. ffmpeg outputs were already streamed.
- **Result queues are 3x the thread count deep.** Results come back in order, so a queue only as deep as the workers left threads idle behind one slow photo. That version took 3.74 s; the deeper queue took it back to 3.23 s, and the keep budget to 2.80 s.

What still scales: a few numbers per photo (sizes, hits, crop rects), and the per-photo working set times the threads. On paper, for 48 MP photos (146 MB of RGB each), the worst case is about 6 GB of kept photos, plus 6 decode + SIFT threads at about 0.4 GB each, plus 8 render threads and 24 queued clip frames at photo size: about 14 GB. That fits in 24 GB, but it is an estimate, not a measurement. Pillow refuses images over 179 MP (`DecompressionBombError`), and pinframe reports that as a message.

## Reliability

- Input is checked against the photo headers before any decoding: at least two photos with distinct names, readable headers, `--box` with 4 numbers, every `--at` naming a photo once (and not the first), points and boxes inside their photo, a positive zoom and `--fps`, a valid `--pad` colour, a supported `--out` extension in an existing folder, `--vf` only with ffmpeg outputs, and ffmpeg on `PATH` when needed. Bad input stops in well under a second.
- Errors the user can fix exit with `pinframe: <what to do>` and code 1 through `fail()`, even under `python -O`. Tracking failures say what to do next: draw a bigger first box, mark more photos, or use `--pad`. Asserts remain only for invariants.
- The clip and its `.json` are written under hidden `.<name>.partial.<ext>` names and renamed together. A failed encode, tested with `--vf nosuchfilter`, leaves the previous clip and `.json` byte-identical, with no partial files.

## Resource planning

Threads and memory are planned at start by `plan_resources`, from what the machine has and what the work costs, instead of fixed numbers:

- **Threads = performance cores** (`sysctl hw.perflevel0.physicalcpu`; physical cores elsewhere). Results come back in order, so a thread on a slower efficiency core holds up the others. Girona sweep (`analysis/threads_sweep.py`), 3 runs each:

  | prepare threads | 2 | 4 | 6 | 8 | 12 |
  |-|-|-|-|-|-|
  | seconds | 3.07 | 2.93 | 2.90 | **2.83** | 3.18 |
  | peak MB | 1343 | 1448 | 1572 | 1892 | 2181 |

  Render threads: 4 → 2.97 s, **8 → 2.83 s**, 12 → 2.89 s. Queue depth per thread: 1x → 3.03 s, 2x → 3.04 s, **3x → 2.83 s**, 4x → 2.94 s.
- **Capped by memory.** The budget is 3/4 of what psutil reports as available at start: memory free now or reclaimable without swapping. The remaining quarter is left for other apps during the run. Each thread costs its measured working set plus its queued results (`analysis/task_memory.py`, each task in a fresh process):

  | photo | prepare task | render task |
  |-|-|-|
  | 2048x1536 (3.1 MP) | 208 MB | 80 MB |
  | 4032x3024 (12.2 MP) | 325 MB | 309 MB |
  | 8064x6048 (48.8 MP) | 1019 MB | 1203 MB |

  Prepare is modeled as 150 MB (SIFT always runs at `TRACK_EDGE`) + 18 B per photo pixel. Render is modeled as 25 B per pixel. One photo's queued features take at most about 4 MB (Girona's largest is 3.6 MB).
- **Kept photos get what the threads leave**, so render can skip the second decode. If even one thread cannot fit, pinframe stops with `a N MP photo needs about X GB free; Y GB is`.

Measured on eight 8064x6048 photos (`analysis/large_photos.py`):

| version | free at start | threads (prepare, render) | wall | system CPU | peak RSS |
|-|-|-|-|-|-|
| fixed numbers (`67fa831`) | about 5 GB | 6, 8 | 13.8 s | 6.4 s | 3.8 GB, with 1.47 M page reclaims |
| planned | 5.1 GB | 3, 2 | 7.9 s | | 3.9 GB (budget 3.8 GB) |
| planned | 7.5 GB | 5, 3 | 6.4 s | 3.7 s | 5.1 GB (budget 5.6 GB) |

The fixed version spent its extra time in the kernel reclaiming memory. Free memory moved between 3.6 and 7.5 GB across runs as other apps changed, and the plan followed it. On Girona with 3.6 GB free it chooses 8 and 8 threads and 1.0 GB for kept photos, which holds all 57.

## Where the time goes now

```sh
bash analysis/profile.sh 9
```

Girona, 57 photos, transparent AVIF, 9 runs, medians. Steps run one after another, so their wall times add up to the run. CPU covers every thread of the process. Peak memory is the highest RSS sampled every 5 ms during the step.

| step | wall s | share | CPU s | cores busy | peak memory |
|-|-|-|-|-|-|
| start Python (uv), import numpy, OpenCV, Pillow | 0.16 | 6% | | | 62 MB |
| check input and plan resources | 0.01 | 0% | 0.01 | 1.0 | 66 MB |
| decode, extract features, track | 1.58 | 56% | 14.22 | 9.0 | 1802 MB |
| plan the crop | 0.00 | 0% | | | 1344 MB |
| render: place and encode | 1.06 | 38% | 5.51 | 5.2 | 1474 MB |
| write the JSON | 0.00 | 0% | | | |
| **whole run** | **2.81** | | **19.8** | **7.1** | **1802 MB** |

| work inside the steps | thread-seconds |
|-|-|
| decode photos (AVIF, dav1d) | 3.50 |
| SIFT features | 8.52 |
| match + RANSAC | 0.24 |
| place frames (resize + warp) | 0.89 |
| encode AVIF (libaom; main-thread time, its own threads add the rest of render's CPU) | 1.05 |
| encoder waiting for a placed frame | 0.01 |

SIFT is the largest cost: 8.5 of the run's 19.8 CPU-seconds. The AVIF encoder is the next longest stretch of wall time, about 1 s, and it is never kept waiting for a frame. Peak memory is reached during tracking: the kept photos (540 MB) plus 8 threads' decode buffers and SIFT pyramids.

## Further ideas, ranked

0. **For click-to-mark, skip the clip and cache features between runs.**
   - Each new mark re-runs everything: decode 0.67 s, SIFT 1.42 s, encode 1.55 s.
   - Marking needs only the JSON, so a JSON-only run drops the 1.55 s encode.
   - Masked SIFT equals whole-photo SIFT filtered to the box (see "SIFT reuse"). Each photo's whole-photo features therefore depend only on the file, not on the marks. They can be cached on disk, keyed by path and mtime.
   - A re-run would then load features and match. Estimated, not measured: about 0.3–0.5 s instead of about 4 s.
   - Output is unchanged by construction. This matters most for the enframe integration in `docs/wasm.md` (`spike-wasm`), which re-runs `anchor.py` on every click.
1. **Smaller `TRACK_EDGE` (768)** would save about 0.5 s with the same LOST set on Girona. It needs an accuracy check first, for example `analysis/grids.py` contact sheets at lens switches, since 1024 was chosen for them.
2. **Faster encode for previews.** Done differently: the default output became animated AVIF (see "Output encoding" above), and since #33 it is native `.mp4`; animated AVIF output was removed with libaom.
3. **Overlap decode with tracking.** Done, with a bigger gain than expected (0.7 s): see "Threaded decode + SIFT".
4. **Hold less in memory.** Decoding twice (downscaled gray for tracking, full RGB in render) would bring peak RSS from ~1.1 GB to ~200 MB. It costs a second 0.6 s decode, unless render decodes on its worker threads.
5. **Threaded whole-photo SIFT.** Superseded by threading decode + SIFT together, which gains more for less memory.
