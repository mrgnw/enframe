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

About 3.7 s wall clock: ~0.4 s `uv run` and imports, 0.6 s decode, 1.45 s track (57 SIFT runs at 1024 px), 1.3 s render (ProRes-bound).

## Further ideas, ranked

1. **Smaller `TRACK_EDGE` (768)** would save about 0.5 s with the same LOST set on Girona. It needs an accuracy check first, for example `analysis/grids.py` contact sheets at lens switches, since 1024 was chosen for them.
2. **Faster encode for previews.** ProRes 4444 sets a ~1.2 s floor. A draft mode that writes VP9 or x264 with `-preset ultrafast`, or `--frames` alone, would skip it. This changes the output, so it belongs behind a flag.
3. **Overlap decode with tracking.** Pillow reads image sizes without decoding, so `k` can be known up front. Tracking photo i could then run while photo i+1 decodes. Both stages already use several cores, so expect 0.2-0.3 s at most.
4. **Hold less in memory.** Decoding twice (downscaled gray for tracking, full RGB in render) would bring peak RSS from ~1.1 GB to ~200 MB. It costs a second 0.6 s decode, unless render decodes on its worker threads.
5. **Threaded whole-photo SIFT** (rejected above). Worth it only where memory is plentiful: 0.2 s for 500-800 MB.
