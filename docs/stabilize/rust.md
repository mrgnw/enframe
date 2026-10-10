# pinframe in Rust: port spike

Spike date: 2026-10-05. Question: what does porting pinframe's pipeline from Python to Rust buy, and what do Rust's compile-time optimizations add?

Machine: Mac mini, Apple M4 Pro (8 performance + 4 efficiency cores), 24 GB, macOS 27.0.1. Rust 1.97.1. Python 3.14, OpenCV 5.0.0 (opencv-python wheel), Pillow 12.3 with libavif 1.4.2. The machine is shared: load averages ran from 3 to 23 during the runs, so every comparison is interleaved, and tables give the fastest and the median.

## Recommendation

**Port to Rust when pinframe moves into enframe; until then keep `anchor.py` as the CLI and the reference. Skip the PyO3 hybrid.**

Superseded 2026-10-06 ([F1](../todos/done/1006-F1-python-reference.md)): the Rust CLI is now the only implementation, checked against the committed tracking baseline (`analysis/tracking_baseline.json`) instead of `anchor.py`, which has been deleted. The measurements below are historical.

## Persistent feature cache and sidecar-only tracking

The Rust CLI caches each photo's SIFT result under `~/Library/Caches/pinframe` on macOS and `$XDG_CACHE_HOME/pinframe`, default `~/.cache/pinframe`, elsewhere. `--cache DIR` selects another root and `--no-cache` disables reads and writes. An entry is content addressed with BLAKE3: photo bytes, the decoder and its version, the global downscale factor and resulting size, `TRACK_EDGE`, the SIFT implementation identifier, and the cache format version are all inputs. The decoder version is libavif's and dav1d's for AVIF, the macOS build (`kern.osversion`) for HEIC and JPEG XL through ImageIO, and the locked zune-jpeg or png crate version for JPEG and PNG; each is read once per run, so an OS update or a decoder bump starts a fresh cache. BLAKE3 is fast enough that hashing AVIF/JPEG/PNG files is normally I/O-bound, while a content key cannot return stale features after a file replacement; path, size, and mtime are intentionally not trusted as cache identity.

Entries hold the downscaled dimensions, packed 24-byte keypoints, and the existing lossless 128-byte SIFT descriptors. The descriptor values are already integer 0--255 in this port, so this is byte-for-byte equivalent to the live representation and warm tracking produces identical rects. Each entry has a BLAKE3 checksum and exact-length validation. A malformed or truncated file is treated as a miss and atomically replaced. Writers use a unique temporary file and rename; concurrent producers may race but write identical bytes, leaving one valid entry. A power loss before the filesystem persists a rename can leave a bad entry, which the next reader validates and recomputes.

The cache uses file mtime as an LRU clock. Its capacity is 5% of free space on the cache filesystem, bounded to 512 MiB--8 GiB, and is enforced after writes by deleting oldest entries. Cached data replaces, rather than adds to, the per-photo features already held by the tracker; the planner additionally reserves the cache hasher's 1 MiB streaming buffer per prepare worker.

An `--out path.json` run is sidecar-only: it reads headers, loads (or creates) features, tracks and writes the JSON, but never renders or encodes. On a warm cache it never decodes pixels. The JSON is still published through a run-unique temporary name while holding the output-folder lock; `run` remains a fresh ID and `clip` is `null`, explicitly denoting that no clip was produced. Clip extensions retain the existing paired clip-plus-sidecar publication.

Every photo's row also carries `points`: `[[x, y]]`, the anchor's position in that photo normalized 0–1 by the photo's oriented size, the same units as its crop rect. It is written for tracked, marked and lost (interpolated) photos alike, straight from `Hit.x`/`Hit.y`, which are already full-resolution pixels; `analysis/check_points.py` checks it against the pixel anchor `anchor.py` prints on all 57 Girona photos.

- **For the CLI alone, Rust is a modest win.** The Girona job takes 2.33 s against 2.69 s (median), starts in 4 ms against 135 ms, and peaks at 1.47 GB against 1.86 GB. On eight 48 MP photos it takes 3.4–3.6 s against 5.5–6.3 s. The gap is small because Python already spends its time in native code (OpenCV, dav1d, libaom), and the Rust version calls the same dav1d and libaom.
- **For enframe, Rust is the better core.** enframe is a Tauri (Rust) app that already ships a Rust server (`enframe-serve`) as its sidecar. A Rust pinframe links into it as a library: no Python runtime or 159 MB venv to bundle (OpenCV alone is 119 MB installed), and no process per click. It can keep every photo's features in memory between marks: a re-track of all 57 Girona photos then takes 145 ms instead of a 2.3 s run.
- **The hybrid buys little.** A PyO3 extension cannot remove Python's start-up, and OpenCV's SIFT and the codecs are already native. The one Rust-side gain that would carry over is feeding libavif ready YUV frames (render 1.04 s to about 0.77 s), and it would cost a second toolchain and maturin wheels.

Before the Rust version can replace `anchor.py`:

- **Choose the SIFT.** The Rust port in this prototype needs no extra dependencies but is not bit-exact with OpenCV: Girona rects differ by up to 3.0e-4 of the photo size (0.6 px) with the same LOST photos. OpenCV's own SIFT through the `opencv` crate matches `anchor.py` within 8.2e-8, but needs OpenCV 5 built for every platform enframe ships on.
- **Port what the prototype skips:** drawing the first box (`--points` is required), and inputs other than AVIF (`anchor.py` reads anything Pillow reads, with EXIF orientation).

## Search window

On a warm cache a JSON-only re-run is almost all matching, and that re-run is what the marking UI waits on after each mark. The tracker therefore matches the reference features (today the box's, later A5's points) only against next-photo keypoints within a window: the reference features' bounds grown by `MOTION_MARGIN` (5) times the last tracked step's motion, the farthest any reference feature moved under that step's fit. The motion carries across marks and lost photos, since it describes how the series was shot; the first step after the first photo searches the whole photo.

When the window gives too few inliers, an implausible fit, or a fit that moves the reference past the window's edge, the search adds the keypoints outside the window to the nearest-neighbour state it already has. Ties go to the lower index, so the fallback finds exactly what a whole-photo search finds and costs no more than one. On Girona the window links 23 of 33 searches; 9 fall back (3 of them lost photos) and 1 has no motion yet. In the synthetic sequences 20 link in the window, 1 falls back and 5 are first steps.

Warm-cache Girona JSON-only re-run (`analysis/girona_all.sh` with `--cache`), 11 interleaved runs, M4 Pro:

| | match work (`PINFRAME_PROFILE`), median | process wall, median |
|-|-:|-:|
| whole photo (before) | 100.6 ms | 118 ms |
| window, factor 5 (after) | 47.7 ms | 65 ms |

Smaller factors were 5–10% faster again (factor 3: 43 ms) but moved the synthetic `rotate` sequence's results by 0.001 px and 0.01% scale either way; at 5 every synthetic frame matches the whole-photo tracker exactly. See `docs/tracking.md` for the accuracy and stability before and after.

## Rust against Python

Girona job (`analysis/girona_all.sh`: 57 AVIFs at 2048x1536, 24 marks, transparent padding, animated AVIF out), 9 interleaved runs each, through `/usr/bin/time -l`:

```sh
uv run analysis/bench_rust.py 9 python='uv run anchor.py' rust=rust/target/release/pinframe
```

| version | wall fastest | wall median | CPU-s median | peak memory median |
|-|-|-|-|-|
| `anchor.py` | 2.62 s | 2.69 s | 20.4 | 1.86 GB |
| Rust, own SIFT (default build) | 2.27 s | 2.33 s | 20.7 | 1.47 GB |
| Rust, OpenCV's SIFT (`--features opencv`) | 2.22 s | 2.26 s | 20.3 | 2.09 GB |

Per step. Python from `analysis/profile.sh 5`; Rust from the same runs as above, through `PINFRAME_PROFILE`. Peak memory is the highest resident size sampled every 5 ms during the step.

| step | Python wall | Rust wall | Python CPU-s | Rust CPU-s | Python peak | Rust peak |
|-|-|-|-|-|-|-|
| start, imports | 0.15 s | 0.004 s | | | 62 MB | |
| check input, plan resources | 0.01 s | 0.002 s | 0.01 | 0.00 | 66 MB | |
| decode, find features, track | 1.44 s | 1.52 s | 14.3 | 16.1 | 1861 MB | 1470 MB |
| render: place and encode | 1.04 s | 0.77 s | 5.6 | 4.6 | 1479 MB | 1240 MB |
| **whole run** | **2.65 s** | **2.33 s** | **20.0** | **20.7** | **1861 MB** | **1470 MB** |

| work, in thread-seconds | Python | Rust |
|-|-|-|
| decode photos (dav1d) | 2.99 | 4.24 |
| SIFT | 7.98 | 12.77 |
| match + RANSAC | 0.23 | 0.16 |
| place frames (Rust: and convert them to YUV) | 0.83 | 1.61 |
| encode AVIF (libaom, encoding thread) | 1.02 | 0.75 |

Thread-seconds are wall time summed over calls, not CPU: Rust runs 12 photos at once, so its calls also count time on the slower efficiency cores, and Python's decode and SIFT spread each call over OpenCV's and libavif's own threads.

- **Prepare is level.** Rust's SIFT costs 136 ms per photo on one core against OpenCV's 115 ms. Rust makes up for it by running on all 12 cores (1.73 s on 8 threads, 1.47 s on 12); Python ran slower on 12.
- **Render is faster** because the render threads convert each frame to YUV 4:2:0 and libavif encodes those planes directly. Homebrew's libavif has no libyuv, and its own conversion cost 8 ms a frame on the encoding thread (render 1.17 s before, 0.74 s after).
- **Start-up** is 3–4 ms against 135 ms for `uv run anchor.py` (both measured exiting at the input check).

Memory per task, each in a fresh process (`analysis/rust_task_memory.py`; Python from `analysis/task_memory.py` in `docs/performance.md`):

| photo | Python prepare | Rust prepare | Python render | Rust render |
|-|-|-|-|-|
| 2048x1536 (3.1 MP) | 208 MB | 139 MB | 80 MB | 38 MB |
| 4032x3024 (12.2 MP) | 325 MB | 174 MB | 309 MB | 123 MB |
| 8064x6048 (48.8 MP) | 1019 MB | 347 MB | 1203 MB | 480 MB |

The Rust planner uses its own costs: 130 MB + 6 B per pixel to prepare, 11 B per pixel to render. Eight 48 MP photos (`BIN=rust/target/release/pinframe uv run analysis/large_photos.py`), two runs each, interleaved:

| version | wall | peak | threads (prepare, render) |
|-|-|-|-|
| `anchor.py` | 6.3 s, 5.5 s | 4.76 GB, 4.94 GB | 4–6, 2–3 |
| Rust | 3.4 s, 3.6 s | 4.62 GB, 4.07 GB | 12, 5–6 |

## What made it fast, and what didn't

Changes to the code paid; compiler settings did not.

| change | measured on | before | after |
|-|-|-|-|
| First port of OpenCV's SIFT | one photo, one core | | 193 ms |
| Static dispatch in the 26-neighbour extremum test (was `&dyn Fn`) | same | 193 ms | 176 ms |
| Descriptor samples only from each row's span that can land in the grid | same | 176 ms | 170 ms |
| Blur copies each padded row instead of reflecting every pixel | same | 170 ms | 160 ms |
| One sample buffer per photo; branchless `fastAtan2`; `exp32f` table lookup in its own loop, so the rest vectorizes | same | 160 ms | 149 ms |
| Branch-free in-layer prefilter before the full extremum test | same | 149 ms | 138 ms |
| DoG read from the Gaussian layers, not stored (5 fewer planes per octave) | same | 138 ms | 135 ms |
| Frames converted to YUV on the render threads | Girona, render | 1.17 s | 0.74 s |
| Every core instead of the performance cores | Girona, whole run | 2.52 s | 2.29 s |
| `place` keeps only the rows its vertical taps read | Girona, peak | 1.79 GB | 1.49 GB |

Every SIFT change kept the output checksum unchanged (`cargo test --release -- --ignored sift_bench`).

**Compile-time options**, cumulative, built by `analysis/rust_builds.sh`. SIFT is the fastest of 7 single-threaded runs on IMG_4667. Whole runs are 6 interleaved rounds.

| build | SIFT, one core | Girona wall median (fastest) | binary |
|-|-|-|-|
| release defaults (opt-level 3, thin local LTO, 16 codegen units) | 136 ms | 2.39 s (2.25) | 1.20 MB |
| + `lto = "fat"` | 146 ms | 2.35 s (2.23) | 0.95 MB |
| + `codegen-units = 1` | 144 ms | 2.40 s (2.24) | 0.95 MB |
| + `panic = "abort"` | 144 ms | 2.31 s (2.18) | 0.86 MB |
| + `-C target-cpu=native` (apple-m4) | 144 ms | 2.29 s (2.26) | 0.87 MB |
| + PGO (`cargo pgo`, trained on two Girona runs) | | 2.33 s (2.26) | 0.86 MB |

- **None of them helps beyond noise.** The whole-run medians span 2.29–2.40 s. An earlier round on older code gave the same picture (2.66–2.76 s).
- **Fat LTO makes SIFT 7% slower** on one core, in two separate rounds (136 → 146 ms, 149 → 160 ms). It changes inlining in the hot loops.
- **`target-cpu=native` adds nothing:** the aarch64 baseline already has NEON and FMA, and the M4's extra features (SME) are not what these loops use.
- **PGO adds nothing:** the hot code is a few loops with fixed trip counts, and a third of the time is in C libraries it cannot touch.
- The crate keeps the release defaults (plus `debug = 1` for symbols when profiling).

Other code-generation questions from the brief:

- **Explicit NEON intrinsics** for both blur passes: SIFT 137 → 130 ms (−5%), the 11-tap blur 3.4 → 2.5 ms. Not kept: it needs `unsafe` and an aarch64-only path beside a portable one, for about 2% of a whole run. A const-generic, register-blocked version in plain Rust gained nothing (159 vs 160 ms); the compiler already vectorized the simple loops (`fmla.4s`).
- **Portable SIMD** (`std::simd`) needs nightly Rust and was not tried.
- **Descriptor matching** uses byte descriptors and integer squared distances. OpenCV's float descriptors are whole numbers, so this is exact. Matching and RANSAC take 0.16 thread-seconds for the whole job, so nothing more was spent there.
- **rayon is not used.** Both pools must hand results to a sequential consumer in order, with bounded memory. `par_iter().collect()` holds every result, and `par_bridge` loses the order. A 50-line ordered map on `std::thread::scope` does it (`render::ordered`).

## Correctness

| comparison | result |
|-|-|
| AVIF decode to RGB, IMG_4667 and IMG_4791 | bit-exact with Pillow (see "AVIF" below) |
| gray + INTER_AREA to 1024 px | bit-exact with OpenCV |
| SIFT, first-octave Gaussian pyramid | bit-exact with OpenCV |
| SIFT keypoints, IMG_4667 / IMG_4791 | same counts (5415 / 5053), positions within 0.002 px, 98.5% of descriptors identical |
| rects, Rust's own SIFT | max difference 2.95e-4 (IMG_4705), scales within 4e-4, same 3 LOST (IMG_4675, IMG_4721, IMG_4725), same sources |
| rects, kornia-imgproc's SIFT (`--features kornia`) | identical to Rust's own SIFT (difference 0) |
| rects, OpenCV's SIFT (`--features opencv`) | max difference 8.2e-8, same inlier count on all 57 photos |
| inlier counts, Rust's own SIFT | 56 of 57 equal; IMG_4699 has 21 against 20 |
| placed frames (`--frames` PNGs), OpenCV's SIFT | 59.2 dB median; the 18 shrunk zoom shots 32–39 dB |
| clips (both encoded) | 42.1 dB median RGB PSNR, 9.7 MB against 9.3 MB |

```sh
uv run analysis/rect_diff.py out/base.json out/rust.json
cd rust && cargo test --release -- --ignored --nocapture
```

- **Where the 3e-4 comes from.** At IMG_4699 the box on IMG_4698 has 37 features in both versions, but three of their descriptors differ from OpenCV's by one level in one of 128 values. That lets a 21st pair pass the ratio test, which moves the least-squares fit and the scale (2.4923 against 2.492), and every later photo follows.
- **Why the descriptors differ.** The sub-scale offset of each keypoint comes from solving a near-singular 3x3 system, which amplifies one-ulp differences about 1e5 times. The Rust port follows OpenCV's float operation order, including where clang fuses multiplies and adds, but the sizes still differ by about 35 float steps from the opencv-python wheel (built with Apple clang 15). No variant of the solve matched (`analysis/solve_probe.py`).
- **kornia-imgproc** independently claims bit-exactness with `cv::SIFT` (presumably against a GCC build). On this Mac it is not bit-exact either, yet it gives byte-identical rects to the Rust port. So on this data it does not matter which of the two is used; only OpenCV's own build reproduces `anchor.py`'s rects.
- **OpenCV's own SIFT** (a source build with Apple clang 21) through the Rust pipeline matches `anchor.py` within 8.2e-8. This confirms that the Rust matching, RANSAC (it replays OpenCV's random generator) and least-squares refinement reproduce OpenCV's.
- **The frames** are placed in one separable resample per axis: bicubic when enlarging, an area filter when shrinking. `anchor.py` area-resizes, then warps bilinearly, which is a little softer on the shrunk frames (marked with a `ponytail:` comment in `render.rs`).

## SIFT in Rust

| option | features | speed, one photo | license | state |
|-|-|-|-|-|
| `opencv` crate + OpenCV 5 | OpenCV's, identical | 115 ms one core | MIT + Apache-2.0 | needs OpenCV built per platform; see "Build" |
| this crate's port (`rust/src/sift.rs`) | within 0.002 px; rects 3e-4 | 136 ms one core | derived from OpenCV, Apache-2.0 | 860 lines, no dependencies |
| kornia-imgproc 0.2 (Sep 2026, 722 stars) | not bit-exact here; same rects as the port | 94 ms one core, 18 ms on all | Apache-2.0 | 47 crates; end to end 2.27 s and 3.0 GB peak, against 2.21 s and 1.49 GB for the port |
| sift-features 0.1.0 (2024, 8 stars) | different keypoints: none within 0.5 px and size of OpenCV's | 485–520 ms one core | MIT | unmaintained |
| sift-wgpu 0.1.0 (Jan 2026, 5 stars) | not tried | | MIT on crates.io, no license file | early |
| lowe-sift 0.2.1 (May 2026, 0 stars) | not tried; follows Lowe's paper, not OpenCV | | MIT | early |

- **Homebrew `opencv` was not installed.** It pulls 26 formulae, including openvino, vtk, tesseract, ceres-solver and a second Python. Instead, a minimal OpenCV 5.0.0 (core, imgproc, geometry, flann, features) was built from source into the cargo cache volume: 45 s wall (276 CPU-s), 18 MB installed. The `opencv` crate's bindings for it take 12.6 s to generate and compile, and need libclang (Xcode's) and the crate's `clang-runtime` feature.
- **The port is derived from OpenCV's code** (Apache-2.0): same algorithm, constants and operation order. Shipping it means keeping OpenCV's copyright notice and Apache-2.0 text beside pinframe's MIT license.
- **GPU SIFT** (Metal through wgpu) would pay only if it existed and could be trusted. SIFT is 8 of Python's 20 CPU-seconds and 12.8 of Rust's 20.7 thread-seconds. If it cost nothing, prepare would be decode-bound (4.2 thread-seconds across 12 cores, about 0.4 s) and the whole run would drop from 2.3 s to about 1.2 s. But no Rust SIFT runs on Metal today: sift-wgpu is a 0.1.0, and kornia's GPU path is CUDA only. A GPU SIFT also cannot match OpenCV bit for bit (histogram updates through atomics come in no fixed order), so its rects would drift at least as much as the port's. For enframe's click-to-mark loop, keeping features in memory gives more (145 ms per re-track) for no GPU work.

## AVIF decode and encode

- **libavif through bindgen** (`rust/build.rs`), built from source and linked statically: libavif 1.4.2 with dav1d 1.5.4 for decode and libaom 3.15.0 for encode, the versions Homebrew ships. Animated AVIF needs libavif or something like it; the pure-Rust options cannot write it.
- **The libavif build has no libyuv** (Homebrew's has none either), while Pillow's does. Its YUV to RGB conversion differed from Pillow's by up to 3 levels, so the 4:4:4 full-range BT.601 case (the Girona photos) converts in Rust the way libyuv does: bit-exact with Pillow. Other layouts fall back to libavif's own conversion (`avif.rs`, marked `ponytail:`). Its RGB to YUV conversion for encoding was slow (above), so frames convert in Rust on the render threads.
- **Orientation** comes from the AVIF `irot` and `imir` boxes, as Pillow reports it. It is unit-tested on synthetic images only: no rotated AVIF was at hand.
- **Encoder settings** follow `anchor.py`'s Pillow call: speed 10, quality 75, 4:2:0, full range, alpha kept, 250 ms frames, run ID in the XMP. ffmpeg outputs (`.mov`, `.webm`, and `.mp4` with `--vf`) pipe RGB(A) as before, with the run ID in the comment tag.
- **Alternatives considered:**
  - `libavif-sys` 0.17 vendors libavif 1.0.4 and builds dav1d with meson and ninja (not installed) and aom with cmake. It also pulls rav1e by default.
  - `image` with `avif` encodes still images through ravif only; decoding needs dav1d separately.
  - `ravif` 0.13 (rav1e) encodes single images, not animations.

## MP4 encode

- **`.mp4` without `--vf` is encoded in-process** by filmcraft's pure-Rust H.264 encoder and MP4 writer, vendored in `src-tauri/vendor/filmcraft` (#30). High profile, 2 B-frames, CRF 15 with the Speed preset, 8-bit 4:2:0, BT.709 limited range; the colour tags go in both the SPS VUI and the `colr` nclx box, the run ID in `©cmt`, and an edit list starts the clip at frame 1. `--vf`, `.mov` and `.webm` still go through ffmpeg.
- **Against the old path** (ffmpeg, libx264 CRF 18 preset medium), all 57 train clouds photos, `PAD=black`, clip 1476x1172, M4 Pro. Encoding the same YUV: 20.0 MB at SSIM 0.9932 native against 17.3 MB at 0.9942 for x264. Whole runs: 20.1 MB against 17.3 MB. On a quiet machine, encoding alone took 2.7 s native and 2.1 s for x264. Under heavy load, interleaved, it took 7.5 s against 6.5 s, and whole runs took 7.8 s against 6.5 s. On 12 photos at 1920x1080 the Speed preset was faster than x264 (0.25 s against 0.41 s, #30). The 57 photos are nearly unrelated frame to frame, so the encoder's scene cut makes most frames IDR.
- **Preset.** Balanced at CRF 15 was slower (4.4 s), larger (20.2 MB) and slightly worse (SSIM 0.9930) on the 57 photos. CRF 14 with the Speed preset matches x264's SSIM at 21.6 MB.

## HEIC and JPEG XL input

HEIC/HEIF and JPEG XL decode through macOS ImageIO (`CGImageSource`), linked to ImageIO, CoreGraphics and CoreFoundation rather than a new Rust or C codec dependency. It is the practical local choice: macOS uses the M4 media path for HEVC, supports both formats without a build-time codec library, and supplies their orientation metadata. The decoder reads the `CGImage` provider directly, avoiding a bitmap-context colour conversion; it accepts 8-bit RGB(A), 16-bit RGB(A), and Apple's packed HEVC Main 10 `l10r` layout. 10- and 16-bit samples are rounded to the nearest 8-bit sample, and premultiplied alpha is undone before alpha is dropped.

On the M4 Pro, `analysis/rust_task_memory.py` measured HEIC Main 8 and Main 10 at 8 B/pixel to prepare and 11 B/pixel to render (both rounded up). ImageIO JXL cost 10/14 B/pixel; that is the conservative maximum of its lossless and lossy measurements. The planner uses those values rather than AVIF's 6/11 B/pixel.

The Girona acceptance run used converted 2048×1536 fixtures and the normal `girona_all.sh` marks. Against the Rust AVIF originals, Main 8 HEIC differed by 4.71e-2 (IMG_4798), lossless JXL by 3.89e-2 (IMG_4798), and lossy JXL by 2.29e-2 (IMG_4689); all retained AVIF's three LOST photos. Main 10 HEIC first differed by 9.04e-1 (IMG_4681) with four extra LOST photos, because the decoder read ImageIO's packed `l10r` words with red and blue swapped. After the fix (`caf1a5b`) it differs by 4.47e-2 (IMG_4798) and keeps AVIF's three LOST photos. The lossless-JXL result is not pixel-identical because its source was an ffmpeg RGB decode of the AVIF rather than the AVIF's original decoded RGB path; it is a conversion-pipeline comparison, not a JXL round-trip claim.

Rebuilding the lossless JXL inputs from Pillow's AVIF RGB (the same RGB as pinframe's libavif path) gives exactly zero rect and scale difference and the same three LOST photos.

### Decode speed

M4 Pro, ten 2048×1536 Girona photos, five round-robin rounds. Values are seconds per photo, `fastest / median`; CPU is process user + system time. ImageIO and libavif timings call the decoder directly, with no CLI start-up or output file. `jxl-oxide` 0.12.6 and `djxl` 0.11.2 are separate CLI processes writing a temporary PNG, so their rows include process start-up and PNG output and are not directly comparable to the in-process rows.

| input | decoder | wall | CPU |
|-|-|-:|-:|
| AVIF | libavif/dav1d | 0.0488 / 0.0492 | 0.0486 / 0.0491 |
| AVIF | ImageIO | 0.0225 / 0.0234 | 0.0123 / 0.0127 |
| HEIC Main 8 | ImageIO | 0.0250 / 0.0258 | 0.0123 / 0.0127 |
| HEIC Main 10 | ImageIO | 0.0384 / 0.0395 | 0.0360 / 0.0370 |
| JXL lossless | ImageIO | 0.0985 / 0.1001 | 0.3518 / 0.3535 |
| JXL lossy d1 | ImageIO | 0.0329 / 0.0335 | 0.0739 / 0.0751 |
| JXL lossless | jxl-oxide CLI | 0.3768 / 0.3795 | 0.7597 / 0.7631 |
| JXL lossless | djxl CLI | 0.1725 / 0.1738 | 0.6742 / 0.6860 |

The Main 10 row was 0.0407 / 0.0416 s CPU before `unpack_l10r`; interleaved runs on the shared machine vary by about 3 ms. Its fixtures are 16-bit PNGs of the AVIF originals converted with `sips -s format heic`, which writes a 4×3 grid of 512×512 tiles with an ICC profile, as an iPhone does. The `imageio::tests::heic10_split` test (`HEIC10_DIR=<dir>`) splits the CPU per photo: ImageIO's decode and pixel copy take 34–42 ms, pinframe's `l10r` unpacking took 3.0 ms and now 1.9 ms. Most of ImageIO's time is system time: `sample` shows CMPhoto decoding each tile through VideoToolbox, then a VideoToolbox pixel transfer on the GPU through Metal, and the GPU command submissions run in the kernel. Tiled Main 8 (8-bit PNG through `sips`) costs 12 ms in total, and the same photos written untiled by `heif-enc -b 10` cost 8 ms in ImageIO, so the cost comes from tiles and 10 bits together. `sips` run directly on the AVIF originals, as `analysis/check.sh` does, also writes untiled Main 10. Asking ImageIO for SDR (`kCGImageSourceDecodeRequest` = `kCGImageSourceDecodeToSDR`) still returns 10-bit `l10r`, with identical pixels and no CPU change; no ImageIO option tried returns 8-bit pixels from these files.

ImageIO makes AVIF roughly twice as fast here and consumes about one quarter of the CPU, plausibly using the media engine. It is not worth switching: libavif/dav1d is bit-exact with the Python reference while ImageIO's colour and conversion behavior is not that reference, and AVIF decoding is not the pipeline bottleneck.

A full-pipeline trial confirmed it. With AVIF routed through ImageIO, decoded pixels differ from libavif's by at most 2–3 levels (PSNR about 50 dB), yet the Girona rects moved by up to 1.60e-1 and the scale by 1.7e-1 at IMG_4689/4690, with 7 of 57 photos past 2e-2; the LOST set stayed the same. The whole run used about 3 CPU-seconds less but took longer: 2.37 / 2.45 s against 2.12 / 2.22 s (fastest / median, 9 interleaved rounds), so ImageIO does not scale across pinframe's decode threads the way its per-photo time suggests. AVIF therefore stays on libavif/dav1d on every platform. That such small pixel differences move a fit so far also shows the tracker is sensitive at some photos; see `todos/`.

`jxl-oxide` is a portable pure-Rust alternative for JXL, but it would add a separate decoder and cannot use ImageIO's integration on this target. `djxl` is useful as an external reference, not an embeddable CLI dependency. `libheif` plus `libde265` would add LGPL components and a static-build/distribution obligation; it also gives up the system HEVC path. Linux and Windows builds need a feature-gated decoder (for example jxl-oxide for JXL and dynamically linked libheif, or platform-native APIs) instead of this macOS module. AVIF deliberately stays on libavif/dav1d for its existing bit-exact reference behavior.

## Colour

Decoders keep each photo's own pixel values, and the clip carries the photos' label (`rust/src/colour.rs`). Each photo's profile is read from its header before decoding: AVIF's ICC profile or nclx code points, the colour space ImageIO reports for HEIC and JPEG XL, the ICC profile in JPEG, and cICP, then iCCP, then sRGB in PNG. ICC profiles are identified by their colorants and tone curves, not their description: sRGB and Display P3 are accepted, and anything else (Adobe RGB, ProPhoto, BT.709 or BT.2020 transfer, an unknown ICC) is an error naming the photo. Unlabelled photos count as sRGB.

The AVIF clip gets nclx primaries 1 (sRGB) or 12 (Display P3) with transfer 13 (sRGB). The native `.mp4` gets the same with matrix 1, limited range, in both the H.264 VUI and the `colr` box. ffmpeg outputs get the same through `setparams` on the frames, after an explicit BT.709 limited-range RGB to YUV step; ffmpeg 9 ignores `-color_primaries` and friends when the frames carry their own. `--frames` PNGs get the sRGB chunk or Apple's Display P3 ICC profile from CoreGraphics.

A set mixing sRGB and Display P3 photos becomes Display P3. The sRGB photos are linearized, multiplied by the fixed sRGB to Display P3 matrix (both D65), and re-encoded to 8 bits on the render threads, just before placing. Tracking and the feature cache see the decoded values as before, so neither changes.

In Playwright's Chromium, a Girona photo cropped to its clip frame and the clip itself now differ by 1.07 levels on average over 16×16 blocks (99th percentile 2.9); the `bt709`-labelled clip from before differed by 4.19 (99th percentile 24.9, red the worst). Playwright's WebKit renders animated AVIF without colour management, so old and new clips look identical there and it cannot judge this. Re-run with `analysis/colour_screenshot.py`.

## Platforms

| | macOS | Linux |
|-|-|-|
| status | built and tested (M4 Pro, macOS 27) | built and tested (x86_64, Ubuntu 24.04, 4 cores) |
| AVIF | libavif + dav1d (bit-exact with Python) | libavif + dav1d (rects identical to macOS) |
| HEIC/HEIF, JPEG XL | ImageIO | refused before decoding: `HEIC input needs macOS (ImageIO); not supported on this platform yet` |
| JPEG, PNG | zune-jpeg, png | zune-jpeg, png |
| feature cache | `~/Library/Caches/pinframe` | `$XDG_CACHE_HOME/pinframe`, default `~/.cache/pinframe` |

What differs by platform, all behind `cfg(target_os = "macos")` so the macOS build is unchanged ([D1](../todos/done/1006-D1-linux-build.md)):

- `src-tauri/stabilize/src/imageio.rs` and the ImageIO, CoreGraphics and CoreFoundation frameworks are macOS-only. Elsewhere `rust/src/imageio_unavailable.rs` stands in with the same signatures, and `decode::format` refuses HEIC and JPEG XL before any entry point reaches them, so `oriented_size`, `profile`, `exif`, `decoder_id`, `memory_cost` and `decode` all fail with the same message. Later, `jxl-oxide` could decode JPEG XL and libheif + libde265 HEIC on Linux.
- Free memory comes from `MemAvailable` in `/proc/meminfo` instead of Mach's `host_statistics64`; the profiler's resident set from `/proc/self/statm`; run IDs from `getrandom(2)` instead of `arc4random_buf`.
- The Display P3 ICC profile that labels `--frames` PNGs is Apple's on macOS and, elsewhere, a minimal v4 matrix/TRC profile built in `colour::minimal_p3_icc` with the same colorants and the sRGB parametric curve; `colour::from_icc` reads both back as Display P3.
- Tests that need ImageIO fixtures or ColorSync's profiles run on macOS only; a Linux-only test checks the refusal. `analysis/check.sh` converts the format and colour sets with sips and cjxl on macOS and with Pillow elsewhere, where the HEIC and JPEG XL runs become refusal checks, so it is 19 checks on both.

Measured on the Linux host above: a clean release build takes 3 min 10 s wall (10 CPU-min, nearly all the C libraries); the 57-photo train clouds job 56 s wall without the cache; `check.sh` passes 19 of 19, with the train clouds rects matching `analysis/tracking_baseline.json` to 1.8e-15 (float-parsing noise), scales exactly and anchor points to 5e-4 px, the same three LOST photos. The binary is 23 MB with debug info and links only libc, libm and libgcc_s.

## Crates and licenses

| crate | used for | license |
|-|-|-|
| libc 0.2 | memory statistics, `flock`, `arc4random_buf`, `getrusage` | MIT or Apache-2.0 |
| png 0.18 | `--frames` PNGs and test fixtures | MIT or Apache-2.0 |
| serde 1, serde_json 1 | the sidecar, profiles | MIT or Apache-2.0 |
| bindgen 0.72, cmake 0.1 (build) | libavif bindings and build | BSD-3-Clause; MIT or Apache-2.0 |
| kornia-imgproc 0.2 (optional) | alternative SIFT | Apache-2.0 |
| opencv 0.101 (optional) | OpenCV's SIFT | MIT (OpenCV itself Apache-2.0) |
| filmcraft-h264enc, -isobmff, -bitstream 0.5 (vendored in `src-tauri/vendor/filmcraft`) | `.mp4`: H.264 encode and MP4 mux | MIT or Apache-2.0 |
| thiserror 2, rayon 1 | filmcraft's errors and threads | MIT or Apache-2.0 |
| libavif, dav1d, libaom (C, linked statically) | AVIF | BSD-2-Clause (libaom also carries the AOM patent license); see Build and install |

A default build compiles 26 crates, 48 with the build dependencies. SIFT's patent expired in March 2020.

## Build and install

```sh
brew install cmake meson ninja                                # macOS
sudo apt install cmake meson ninja-build nasm libclang-dev    # Debian/Ubuntu
```

```sh
cd rust && cargo build --release
```

- **Requirements:** Rust stable (1.97 here), cmake, meson and ninja, libclang for bindgen (Xcode or the Command Line Tools on macOS, `libclang-dev` on Debian/Ubuntu), and network access on the first build. nasm is needed only on x86 (aom's and dav1d's assembly). Homebrew's libavif is not needed.
- **Sources on a restricted network.** Verified tarballs are cached in `~/Library/Caches/pinframe-sources` on macOS and `$XDG_CACHE_HOME/pinframe-sources` (default `~/.cache/pinframe-sources`) elsewhere. A machine that cannot download one of them (a proxy that blocks github.com, say) can set `PINFRAME_SOURCES=DIR`, where `DIR` holds the unpacked sources under their names (`libavif-1.4.2/`, from `git clone --branch v1.4.2`, for example); those are used as they are, without a checksum. Changing the variable rebuilds the C libraries.
- **The C libraries are built by `rust/build.rs`.** It downloads the release tarballs of libavif 1.4.2, dav1d 1.5.4 and libaom 3.15.0, checks their SHA-256 (the same as Homebrew's for libavif and dav1d; aom's tarball is identical to its git tag `v3.15.0`), and builds them as static libraries in cargo's build directory (`OUT_DIR`), aom and libavif with cmake and dav1d with meson. Tarballs rather than git submodules: a clean checkout needs no extra step, the pins and checksums sit in one file, and aom's history is not cloned. aom builds only its encoder, without apps, docs, tests or VMAF tuning; dav1d without tools or tests; libavif without apps and with aom for encoding only. On macOS the C code targets macOS 11.0, like rustc, unless `MACOSX_DEPLOYMENT_TARGET` says otherwise.
- **Build cost:** a clean release build without sccache takes 37 s wall (116 CPU-s), most of it the C libraries; the C build leaves 107 MB in `OUT_DIR` (73 MB of it unpacked sources). The C libraries are rebuilt only when `build.rs` or `MACOSX_DEPLOYMENT_TARGET` changes, so a build after a Rust edit still takes about 1.5 s.
- **The binary is 7.5 MB** (6.8 MB stripped; the dynamic build was 1.2 MB) and links only `/usr/lib/libSystem` and `/usr/lib/libiconv`. Its output is the same as the Homebrew-linked build: identical sidecar rects and decoded frames for the Girona job. The Girona job runs as fast (12 interleaved runs at load 35: fastest 2.19 s against 2.20 s, median 2.34 s against 2.36 s).
- **Licenses for distribution.** libavif, dav1d and libaom are BSD-2-Clause: a binary must reproduce each copyright notice, the conditions and the disclaimer in its documentation or other materials. Also bundled are libyuv-derived scaling code in libavif (BSD-3-Clause, in libavif's `LICENSE`), and fastfeat (BSD-3-Clause) and vector (MIT) in libaom. libaom also comes under the Alliance for Open Media Patent License 1.0 (`PATENTS`), a royalty-free patent grant that ends for anyone who sues over AV1 patents; ship its text with the others. The texts are in each source tree under `OUT_DIR/src`.
- **This spike installed** meson 1.12.1 and ninja 1.13.2 (Homebrew; meson pulled python@3.14 3.14.8), plus `cargo-pgo` 0.3.0 (4 MB, in `~/.cargo/bin`). The minimal OpenCV build lives in `/Volumes/disk/cache/opencv-spike` (375 MB with its source and build tree; safe to delete).

The `kornia` and `opencv` SIFT backends compared below (`--features kornia`, `--features opencv`) were spike-only, to measure against the crate's own SIFT. They're not in the crate: see "Correctness" and "SIFT in Rust" for what the comparison found.

Run the train clouds job, and the checks against Python's dumps (`dump_sift.py` and `dump_ransac.py`, and with them `dump_pyramid.py`'s input, went with `anchor.py` in F1, so of the ignored tests in `rust/src/checks.rs` only the kernel comparison still has its dump):

```sh
BIN=rust/target/release/pinframe bash analysis/train_clouds_all.sh out/rust.avif
```

```sh
uv run analysis/dump_kernels.py > out/kernels.jsonl
```

```sh
cd rust && cargo test --release -- --include-ignored --test-threads=1 --nocapture
```

## What it means for enframe and Tauri

- **Integration.** Today's plan (`docs/wasm.md` on `spike-wasm`) runs `uv run anchor.py` from `enframe-serve` per click. With a Rust core, `enframe-serve` and the Tauri app call a `pinframe` library crate in-process.
  - The first run decodes and finds features (about 1.5 s for Girona). Later marks re-track from features held in memory in 145 ms, and a preview clip encodes in about 0.8 s.
  - Nothing Python ships with the app, and there is no 135 ms start-up per call.
- **Dependencies.** enframe's Rust side has `image` with pure-Rust AVIF (ravif) and no C codec libraries. pinframe adds libavif, dav1d and libaom. `build.rs` builds them statically (cmake for libavif and aom, meson and ninja for dav1d, nasm on x86), so the binary needs no Homebrew.
- **Platforms.** The Rust code is portable (no intrinsics). Free-memory and core-count queries are macOS-only (`host_statistics64`) and need Linux and Windows versions. The OpenCV backend would need OpenCV built for every target, which is the main argument for the own port or kornia.
- **The web build** stays without pinframe, as `docs/wasm.md` concluded. The Rust core could compile to WASM later (no threads assumed in the algorithm), but libavif would need an Emscripten build, and nothing was measured here.

## Library

The crate is a library plus the `stabilize` CLI over it, so `enframe-serve` tracks in-process (`cargo doc --open -p stabilize` in `src-tauri/` has the details):

- `stabilize::track(&job, &mut log)` checks a `Job` (the photos, their `Marks` and the CLI's other options, typed), tracks and plans the crop, and returns a `Tracked`. A job with no `output` only tracks: nothing is written but feature cache entries.
- `Tracked::sidecar(run, clip)` gives the `.json` sidecar's data; `Sidecar::to_json()` its bytes. `Tracked::write(&mut log)` renders and publishes the job's clip and sidecar, as the CLI does.
- Errors are the CLI's messages, as `String`s; nothing exits the process. Progress lines go to `log`, which the CLI prints.
- `Job::cancel`, an `Arc<AtomicBool>`, stops a run: once set, `track` and `Tracked::write` end with `cancelled` before the next photo, and nothing is published. serve sets it when a newer run supersedes one.
- Marks are kept in the CLI's text form so they are checked in the same order with the same messages; `Marks::new(points).with_at(photo, &mark).with_near(photo, &mark).with_skip(photo)` builds them from points, and the text reads back to the same `f64`s.

Measured on a 4-core Linux container (2026-10-08): an in-process re-track of the 57 train clouds photos with a warm feature cache takes 1.05 s, 1.02 s of it matching; the same CLI run takes 1.1 to 1.3 s. The 145 ms above is the M4 Pro's track and plan from features in memory (`retrack_bench` in `rust/src/checks.rs`).

## Files

- `src-tauri/stabilize/src/lib.rs`: the library: input checks, resource planning, pipeline, outputs.
- `src-tauri/stabilize/src/main.rs`: the CLI (same flags as `anchor.py` except box drawing), a thin layer over the library.
- `src-tauri/stabilize/src/avif.rs`: libavif decode and encode, libyuv-exact YUV to RGB.
- `src-tauri/stabilize/src/yuv.rs`: RGB to YUV 4:2:0 for the AVIF (BT.601 full range) and MP4 (BT.709 limited range) encoders.
- `src-tauri/stabilize/src/mp4.rs`: `.mp4` clips through the vendored filmcraft H.264 encoder and MP4 writer.
- `src-tauri/stabilize/src/colour.rs`: colour profiles from ICC and nclx, sRGB to Display P3.
- `src-tauri/stabilize/src/pixels.rs`: orientation, OpenCV-exact gray and INTER_AREA.
- `src-tauri/stabilize/src/sift.rs`: the SIFT port.
- `src-tauri/stabilize/src/track.rs`: matching, the RANSAC replay, tracking, lost-photo fill, crop planning.
- `src-tauri/stabilize/src/render.rs`: the ordered thread pool, kept photos, frame placement.
- `src-tauri/stabilize/src/profile.rs`: per-step wall, CPU and peak memory when `PINFRAME_PROFILE` names a file.
- `src-tauri/stabilize/src/checks.rs`: comparisons with Python's dumps and benchmarks, run with `--ignored`.
- `analysis/bench_rust.py`, `rust_builds.sh`, `rust_task_memory.py`, `rect_diff.py`, `clip_psnr.py`, `frames_psnr.py`, `dump_*.py`, `solve_probe.py`: the measurements above.
