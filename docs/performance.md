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

## Where the time goes now

```sh
bash analysis/profile.sh 9
bash analysis/profile.sh 9 --frames
```

`analysis/profile_stages.py` charges each second of the main thread to exactly one stage (self-time, children subtracted), so the rows add up to the wall clock. It also times the ffmpeg pipe, and per phase it records process CPU time, which shows how many cores the phase keeps busy. Medians of 9 runs, with other apps running, so totals sit above the 3.71 s quiet-machine figure; the shares are what matter.

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

**The `ffmpeg` on `PATH` was an Intel build.** `/Users/user/.local/bin/ffmpeg` is the x86_64 static build from evermeet.cx (8.1). It runs under Rosetta and shadows Homebrew's native arm64 ffmpeg 9.0.1 at `/opt/homebrew/bin/ffmpeg`. The Intel build can't open VideoToolbox encoders (`Cannot create compression session: -12908`) and lacks SVT-AV1. It also encodes slower: the Girona ProRes run takes 3.75 s through it against 3.42 s through the native build. Every ProRes number above was measured through it.

**No hardware AV1 encoder.** `VTCopyVideoEncoderList` on this machine lists hardware H.264, HEVC, HEVC with alpha ("Apple Muxed Alpha"), ProRes and JPEG encoders, but no `av01`. Apple's spec for this Mac lists "AV1 decode" only ([Mac mini (2024) tech specs](https://support.apple.com/en-us/121555)). AV1 and AVIF encoding run on the CPU.

57 Girona frames at 1442x1196, native ffmpeg, one run each. PSNR is against the placed frames.

**Transparent** (`--pad transparent`, the Girona default):

| encoding | wall s | CPU s | MB | PSNR RGB dB | PSNR alpha dB |
|-|-|-|-|-|-|
| ProRes 4444 (`.mov`) | 1.03 | 7.3 | 109.3 | 63.0 | lossless |
| VP9 alpha crf 30 (`.webm`) | 8.89 | 26.1 | 6.2 | 38.9 | 70.7 |
| HEVC + alpha, VideoToolbox HW, q 80 | 0.85 | 0.2 | 12.9 | 38.1 | 71.7 |
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

- **AVIF** is the default because it is the one format that is small, keeps alpha, and needs no ffmpeg. At 40 dB it is a twelfth of ProRes and encodes as fast. In Playwright's WebKit and Chromium, the animated transparent AVIF decodes at full size, animates, and shows the page background through its transparent areas. Real Safari was not tested.
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

## Further ideas, ranked

0. **For click-to-mark, skip the clip and cache features between runs.**
   - Each new mark re-runs everything: decode 0.67 s, SIFT 1.42 s, encode 1.55 s.
   - Marking needs only the JSON, so a JSON-only run drops the 1.55 s encode.
   - Masked SIFT equals whole-photo SIFT filtered to the box (see "SIFT reuse"). Each photo's whole-photo features therefore depend only on the file, not on the marks. They can be cached on disk, keyed by path and mtime.
   - A re-run would then load features and match. Estimated, not measured: about 0.3–0.5 s instead of about 4 s.
   - Output is unchanged by construction. This matters most for the enframe integration in `docs/wasm.md` (`spike-wasm`), which re-runs `anchor.py` on every click.
1. **Smaller `TRACK_EDGE` (768)** would save about 0.5 s with the same LOST set on Girona. It needs an accuracy check first, for example `analysis/grids.py` contact sheets at lens switches, since 1024 was chosen for them.
2. **Faster encode for previews.** Done differently: the default output is now animated AVIF. See "Output encoding" above.
3. **Overlap decode with tracking.** Pillow reads image sizes without decoding, so `k` can be known up front. Tracking photo i could then run while photo i+1 decodes. Both stages already use several cores, so expect 0.2-0.3 s at most.
4. **Hold less in memory.** Decoding twice (downscaled gray for tracking, full RGB in render) would bring peak RSS from ~1.1 GB to ~200 MB. It costs a second 0.6 s decode, unless render decodes on its worker threads.
5. **Threaded whole-photo SIFT** (rejected above). Worth it only where memory is plentiful: 0.2 s for 500-800 MB. The 24 GB Mac mini has the room.
