# Build and test pinframe on Linux

The Rust CLI is built and tested on macOS only. On Linux it does not compile yet.

## Make it build

- Gate `rust/src/imageio.rs` and the ImageIO, CoreGraphics and CoreFoundation links in `rust/build.rs` behind `cfg(target_os = "macos")`.
- On other platforms, refuse HEIC/HEIF and JPEG XL before decoding: `<file>: HEIC input needs macOS (ImageIO); not supported on this platform yet`.
- Skip `MACOSX_DEPLOYMENT_TARGET` handling off macOS.

## Test

Use a Linux container through OrbStack, which is installed and runs linux/arm64:

```sh
docker run --rm -it -v /Users/user/dev/pinframe:/src -w /src rust:latest bash
```

Inside, install cmake, meson, ninja and libclang, then build with `cd rust && cargo build --release`.

## Done when

- [ ] `cargo build --release`, `cargo test --release` and `cargo clippy --release --all-targets -- -D warnings` pass on Linux.
- [ ] The Girona job runs (`BIN=rust/target/release/pinframe bash analysis/girona_all.sh out/linux.avif`) and its rects match the macOS build exactly, since both decode AVIF with libavif + dav1d.
- [ ] The decode-parity tests against Python dumps pass (`cargo test --release -- --include-ignored --test-threads=1` after the `analysis/dump_*.py` scripts).
- [ ] HEIC and JPEG XL inputs fail with the clear message, before any decoding.
- [ ] `ldd rust/target/release/pinframe` lists only system libraries: libavif, dav1d and libaom are linked statically.

## Later

HEIC and JPEG XL on Linux need another decoder: `jxl-oxide` for JPEG XL (pure Rust, slower than ImageIO; see the speed table in `docs/rust.md`), and libheif + libde265 for HEIC (LGPL, so link dynamically or accept the distribution terms).
