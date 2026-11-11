---
branch: claude/amazing-mendel-xmtd5j
---

# D1 · Build and test pinframe on Linux

Depends on: — · Model: Opus · Size: M
Touches: `rust/build.rs`, `rust/src/decode.rs`, `rust/src/imageio.rs` (gating only), docs. Also needed: `rust/src/main.rs`, `cache.rs`, `profile.rs`, `colour.rs` (Mach memory calls, `arc4random_buf`, cache paths and the Display P3 ICC were macOS-only too), a stub `rust/src/imageio_unavailable.rs`, and `analysis/check.sh` (sips and cjxl).

The Rust CLI is built and tested on macOS only. On Linux it does not compile yet.

## Make it build

- Gate `rust/src/imageio.rs` and the ImageIO, CoreGraphics and CoreFoundation links in `rust/build.rs` behind `cfg(target_os = "macos")`.
- On other platforms, refuse HEIC/HEIF and JPEG XL before decoding: `<file>: HEIC input needs macOS (ImageIO); not supported on this platform yet`.
- Skip `MACOSX_DEPLOYMENT_TARGET` handling off macOS.

## Test

Use a Linux container through OrbStack, which is installed and runs linux/arm64:

```sh
docker run --rm -it -v $PINFRAME_DIR:/src -w /src rust:latest bash
```

Inside, install cmake, meson, ninja and libclang, then build with `cd rust && cargo build --release`.

## Done when

- [x] `cargo build --release`, `cargo test --release` and `cargo clippy --release --all-targets -- -D warnings` pass on Linux.
- [x] The train clouds job runs (`BIN=rust/target/release/pinframe bash analysis/train_clouds_all.sh out/linux.avif`) and its rects match the macOS build exactly, since both decode AVIF with libavif + dav1d.
- [x] The decode-parity tests against Python dumps pass: only `kernels_match_opencv` still has a producer (`dump_sift.py` and `dump_ransac.py` went with `anchor.py` in F1), and it passes on Linux.
- [x] HEIC and JPEG XL inputs fail with the clear message, before any decoding.
- [x] `ldd rust/target/release/pinframe` lists only system libraries: libavif, dav1d and libaom are linked statically.

## Later

HEIC and JPEG XL on Linux need another decoder: `jxl-oxide` for JPEG XL (pure Rust, slower than ImageIO; see the speed table in `docs/rust.md`), and libheif + libde265 for HEIC (LGPL, so link dynamically or accept the distribution terms).

## Note from D2

`rust/build.rs` caches source tarballs in `~/Library/Caches/pinframe-sources`. On Linux use `$XDG_CACHE_HOME` (or `~/.cache`) instead.

## Result

Done directly on a Linux host (x86_64, Ubuntu 24.04, 4 cores), not in OrbStack. Everything platform-specific sits behind `cfg(target_os = "macos")`, so the macOS build is byte-for-byte the same code path: `imageio.rs` and the three frameworks are macOS-only, `imageio_unavailable.rs` stands in elsewhere, and `decode::format` refuses HEIC and JPEG XL with `<file>: HEIC input needs macOS (ImageIO); not supported on this platform yet` before any entry point decodes. Linux reads free memory from `/proc/meminfo`, the resident set from `/proc/self/statm`, random bytes from `getrandom(2)`, caches under `$XDG_CACHE_HOME` or `~/.cache`, hashes tarballs with `sha256sum`, and labels `--frames` PNGs with a minimal Display P3 ICC profile built in `colour.rs` (recognised by `from_icc`, tested on both platforms). `PINFRAME_SOURCES=DIR` lets `build.rs` use unpacked sources on a network that blocks a source host (this host's proxy blocks github.com, so libavif came from `git clone`). `check.sh` converts the format and colour sets with Pillow where sips is missing and checks the HEIC and JPEG XL refusals instead of running them, so it stays at 19 checks on both platforms. On Linux: build 3 min 10 s wall, `check.sh` 19 of 19, the 57 train clouds photos match `analysis/tracking_baseline.json` to 1.8e-15 in rects, exactly in scale, 5e-4 px in points, same LOST set (IMG_4675, 4721, 4725); `ldd` shows libc, libm and libgcc_s only. Not verified here: the macOS build after these changes, which needs a Mac (`bash analysis/check.sh`). Later, as the card says: `jxl-oxide` for JPEG XL and libheif + libde265 for HEIC on Linux.
