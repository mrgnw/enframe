# pinframe in Rust: port spike

Spike date: 2026-10-05. Question: what does porting pinframe's pipeline from Python to Rust buy, and what do Rust's compile-time optimizations add?

Machine: Mac mini, Apple M4 Pro (8 performance + 4 efficiency cores), 24 GB, macOS 27.0.1. Rust 1.97.1. Python 3.14, OpenCV 5.0.0 (opencv-python wheel), Pillow 12.3 with libavif 1.4.2. The machine is shared: load averages ran from 3 to 23 during the runs, so every comparison is interleaved, and tables give the fastest and the median.

## Recommendation

**Port to Rust when pinframe moves into enframe; until then keep `anchor.py` as the CLI and the reference. Skip the PyO3 hybrid.**

## Persistent feature cache and sidecar-only tracking

The Rust CLI caches each photo's SIFT result under `~/Library/Caches/pinframe`. `--cache DIR` selects another root and `--no-cache` disables reads and writes. An entry is content addressed with BLAKE3: photo bytes, the global downscale factor and resulting size, `TRACK_EDGE`, the SIFT implementation identifier, and the cache format version are all inputs. BLAKE3 is fast enough that hashing AVIF/JPEG/PNG files is normally I/O-bound, while a content key cannot return stale features after a file replacement; path, size, and mtime are intentionally not trusted as cache identity.

Entries hold the downscaled dimensions, packed 24-byte keypoints, and the existing lossless 128-byte SIFT descriptors. The descriptor values are already integer 0--255 in this port, so this is byte-for-byte equivalent to the live representation and warm tracking produces identical rects. Each entry has a BLAKE3 checksum and exact-length validation. A malformed or truncated file is treated as a miss and atomically replaced. Writers use a unique temporary file, `fsync`, and rename; concurrent producers may race but write identical bytes, leaving one valid entry.

The cache uses file mtime as an LRU clock. Its capacity is 5% of free space on the cache filesystem, bounded to 512 MiB--8 GiB, and is enforced after writes by deleting oldest entries. Cached data replaces, rather than adds to, the per-photo features already held by the tracker; the planner additionally reserves the cache hasher's 1 MiB streaming buffer per prepare worker.

An `--out path.json` run is sidecar-only: it reads headers, loads (or creates) features, tracks and writes the JSON, but never renders or encodes. On a warm cache it never decodes pixels. The JSON is still published through a run-unique temporary name while holding the output-folder lock; `run` remains a fresh ID and `clip` is `null`, explicitly denoting that no clip was produced. Clip extensions retain the existing paired clip-plus-sidecar publication.

- **For the CLI alone, Rust is a modest win.** The Girona job takes 2.33 s against 2.69 s (median), starts in 4 ms against 135 ms, and peaks at 1.47 GB against 1.86 GB. On eight 48 MP photos it takes 3.4–3.6 s against 5.5–6.3 s. The gap is small because Python already spends its time in native code (OpenCV, dav1d, libaom), and the Rust version calls the same dav1d and libaom.
- **For enframe, Rust is the better core.** enframe is a Tauri (Rust) app that already ships a Rust server (`enframe-serve`) as its sidecar. A Rust pinframe links into it as a library: no Python runtime or 159 MB venv to bundle (OpenCV alone is 119 MB installed), and no process per click. It can keep every photo's features in memory between marks: a re-track of all 57 Girona photos then takes 145 ms instead of a 2.3 s run.
- **The hybrid buys little.** A PyO3 extension cannot remove Python's start-up, and OpenCV's SIFT and the codecs are already native. The one Rust-side gain that would carry over is feeding libavif ready YUV frames (render 1.04 s to about 0.77 s), and it would cost a second toolchain and maturin wheels.

Before the Rust version can replace `anchor.py`:

- **Choose the SIFT.** The Rust port in this prototype needs no extra dependencies but is not bit-exact with OpenCV: Girona rects differ by up to 3.0e-4 of the photo size (0.6 px) with the same LOST photos. OpenCV's own SIFT through the `opencv` crate matches `anchor.py` within 8.2e-8, but needs OpenCV 5 built for every platform enframe ships on.
- **Port what the prototype skips:** drawing the first box (`--box` is required), and inputs other than AVIF (`anchor.py` reads anything Pillow reads, with EXIF orientation).

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
- **Encoder settings** follow `anchor.py`'s Pillow call: speed 10, quality 75, 4:2:0, full range, alpha kept, 250 ms frames, run ID in the XMP. ffmpeg outputs (`.mov`, `.mp4`, `.webm`) pipe RGB(A) as before, with the run ID in the comment tag.
- **Alternatives considered:**
  - `libavif-sys` 0.17 vendors libavif 1.0.4 and builds dav1d with meson and ninja (not installed) and aom with cmake. It also pulls rav1e by default.
  - `image` with `avif` encodes still images through ravif only; decoding needs dav1d separately.
  - `ravif` 0.13 (rav1e) encodes single images, not animations.

## Crates and licenses

| crate | used for | license |
|-|-|-|
| libc 0.2 | memory statistics, `flock`, `arc4random_buf`, `getrusage` | MIT or Apache-2.0 |
| png 0.18 | `--frames` PNGs and test fixtures | MIT or Apache-2.0 |
| serde 1, serde_json 1 | the sidecar, profiles | MIT or Apache-2.0 |
| bindgen 0.72, cmake 0.1 (build) | libavif bindings and build | BSD-3-Clause; MIT or Apache-2.0 |
| kornia-imgproc 0.2 (optional) | alternative SIFT | Apache-2.0 |
| opencv 0.101 (optional) | OpenCV's SIFT | MIT (OpenCV itself Apache-2.0) |
| libavif, dav1d, libaom (C, linked statically) | AVIF | BSD-2-Clause (libaom also carries the AOM patent license); see Build and install |

A default build compiles 26 crates, 48 with the build dependencies. SIFT's patent expired in March 2020.

## Build and install

```sh
brew install cmake meson ninja
```

```sh
cd rust && cargo build --release
```

- **Requirements:** Rust stable (1.97 here), cmake, meson and ninja, libclang for bindgen (Xcode or the Command Line Tools), and network access on the first build. nasm is needed only on x86 (aom's and dav1d's assembly). Homebrew's libavif is not needed.
- **The C libraries are built by `rust/build.rs`.** It downloads the release tarballs of libavif 1.4.2, dav1d 1.5.4 and libaom 3.15.0, checks their SHA-256 (the same as Homebrew's for libavif and dav1d; aom's tarball is identical to its git tag `v3.15.0`), and builds them as static libraries in cargo's build directory (`OUT_DIR`), aom and libavif with cmake and dav1d with meson. Tarballs rather than git submodules: a clean checkout needs no extra step, the pins and checksums sit in one file, and aom's history is not cloned. aom builds only its encoder, without apps, docs, tests or VMAF tuning; dav1d without tools or tests; libavif without apps and with aom for encoding only. The C code targets macOS 11.0, like rustc, unless `MACOSX_DEPLOYMENT_TARGET` says otherwise.
- **Build cost:** a clean release build without sccache takes 37 s wall (116 CPU-s), most of it the C libraries; the C build leaves 107 MB in `OUT_DIR` (73 MB of it unpacked sources). The C libraries are rebuilt only when `build.rs` or `MACOSX_DEPLOYMENT_TARGET` changes, so a build after a Rust edit still takes about 1.5 s.
- **The binary is 7.5 MB** (6.8 MB stripped; the dynamic build was 1.2 MB) and links only `/usr/lib/libSystem` and `/usr/lib/libiconv`. Its output is the same as the Homebrew-linked build: identical sidecar rects and decoded frames for the Girona job. The Girona job runs as fast (12 interleaved runs at load 35: fastest 2.19 s against 2.20 s, median 2.34 s against 2.36 s).
- **Licenses for distribution.** libavif, dav1d and libaom are BSD-2-Clause: a binary must reproduce each copyright notice, the conditions and the disclaimer in its documentation or other materials. Also bundled are libyuv-derived scaling code in libavif (BSD-3-Clause, in libavif's `LICENSE`), and fastfeat (BSD-3-Clause) and vector (MIT) in libaom. libaom also comes under the Alliance for Open Media Patent License 1.0 (`PATENTS`), a royalty-free patent grant that ends for anyone who sues over AV1 patents; ship its text with the others. The texts are in each source tree under `OUT_DIR/src`.
- **This spike installed** meson 1.12.1 and ninja 1.13.2 (Homebrew; meson pulled python@3.14 3.14.8), plus `cargo-pgo` 0.3.0 (4 MB, in `~/.cargo/bin`). The minimal OpenCV build lives in `/Volumes/disk/cache/opencv-spike` (375 MB with its source and build tree; safe to delete).

The `kornia` and `opencv` SIFT backends compared below (`--features kornia`, `--features opencv`) were spike-only, to measure against the crate's own SIFT. They're not in the crate: see "Correctness" and "SIFT in Rust" for what the comparison found.

Run the Girona job, and the checks against Python's dumps:

```sh
BIN=rust/target/release/pinframe bash analysis/girona_all.sh out/rust.avif
```

```sh
uv run analysis/dump_sift.py IMG_4667 && uv run analysis/dump_sift.py IMG_4791 && uv run analysis/dump_kernels.py > out/kernels.jsonl && uv run analysis/dump_pyramid.py && uv run analysis/dump_ransac.py
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

## Files

- `rust/src/main.rs`: CLI (same flags as `anchor.py` except box drawing), input checks, resource planning, pipeline, outputs.
- `rust/src/avif.rs`: libavif decode and encode, libyuv-exact YUV to RGB, RGB to YUV for the encoder.
- `rust/src/pixels.rs`: orientation, OpenCV-exact gray and INTER_AREA.
- `rust/src/sift.rs`: the SIFT port.
- `rust/src/track.rs`: matching, the RANSAC replay, tracking, lost-photo fill, crop planning.
- `rust/src/render.rs`: the ordered thread pool, kept photos, frame placement.
- `rust/src/profile.rs`: per-step wall, CPU and peak memory when `PINFRAME_PROFILE` names a file.
- `rust/src/checks.rs`: comparisons with Python's dumps and benchmarks, run with `--ignored`.
- `analysis/bench_rust.py`, `rust_builds.sh`, `rust_task_memory.py`, `rect_diff.py`, `clip_psnr.py`, `frames_psnr.py`, `dump_*.py`, `solve_probe.py`: the measurements above.
