# pinframe in the browser: WASM spike

Spike date: 2026-10-05. Question: can enframe offer pinframe as a click-to-mark UI, and where should the tracking run?

Everything below is either measured here, read from a primary source (linked), or marked **unverified**.

## Recommendation

Keep tracking in Python on a server. Run `anchor.py` behind `enframe-serve`, and let the browser do marking only.

Superseded 2026-10-06 ([F1](../todos/F1-python-reference.md)): `anchor.py` has been deleted; the Rust CLI (`rust/target/release/pinframe`) is the tracker to run server-side instead. The measurements below are historical.

- The tracking step runs in WASM and gives the same answer as CPython (proof of concept below), so a browser port is possible, but it is 12x slower than native and costs a 26 MB download before the first photo is matched.
- The clip can't be written the way `anchor.py` writes it today. The browser has no ffmpeg; WebCodecs can't encode alpha in Chromium (it throws `NotSupportedError`), so `--pad transparent` would need ffmpeg.wasm (32 MB, GPL, and multithreading needs the cross-origin isolation enframe's static site deliberately turns off).
- enframe already has a server with the same UI as the desktop app (`enframe-serve`, Rust/axum, also shipped as the Tauri sidecar), and it already confines file access to read/write roots. pinframe plugs in there as one subprocess call with no new runtime.

First integration step: a `POST /pinframe` route in `enframe-serve` that runs `uv run anchor.py <files> --box … --at … --out <write-root>/clip.mov` and returns the sidecar JSON. The framer UI draws the first box with the crop tool it already has, lists rows with `"source": "lost"`, and each click on one adds an `--at` and re-runs. Nothing in `anchor.py` changes for that step.

## Measured: one tracking step, WASM vs native

`wasm/match.mjs` decodes IMG_4791 and IMG_4792 (1536x2048 AVIF) with `@jsquash/avif`, runs `wasm/pair.py` (the tracking step lifted from `anchor.py`) under Pyodide's `opencv-python` in Node 24, then runs the same file under CPython and prints both next to `anchor.py`'s own answer. Box 520,220,620,500 from `analysis/girona_all.sh`.

| | Pyodide 314.0.7, OpenCV 4.11 | CPython 3.14, OpenCV 5.0 | anchor.py |
|-|-|-|-|
| features in box / whole photo | 161 / 3466 | 158 / 3479 | |
| ratio-test pairs | 121 | 116 | |
| RANSAC inliers | 116 | 112 | 112 |
| anchor in photo B | 816.1, 500.9 | 816.0, 500.9 | 816.0, 500.9 |
| scale step | 0.9984 | 0.9989 | 0.9989 |
| turn | 0.05° | 0.05° | |
| gray + resize to 1024 | 6–11 ms | 7 ms | |
| SIFT inside box (full pyramid still built) | 256–430 ms | 35–45 ms | |
| SIFT whole photo | 312–483 ms | 24 ms | |
| knnMatch + estimateAffinePartial2D | 31–40 ms | 4–5 ms | |
| decode two AVIFs | 683 ms (jsquash, Node) | | |
| Pyodide boot + load numpy, opencv | 850 + 360 ms (wheels cached on disk) | | |

Same answer within 0.1 px and 0.0005 in scale; the inlier counts differ by 4 because the OpenCV versions differ. Per photo, tracking costs about 0.8 s in WASM versus 0.06 s native, single-threaded in both cases. Extrapolated, not measured: 57 photos ≈ 45 s of WASM tracking. Measured: the whole native pipeline on all 57 photos, marks, render and ProRes encode, is 4.4 s wall (`analysis/girona_all.sh`).

Why 12x: Pyodide's recipe builds OpenCV with `CV_ENABLE_INTRINSICS=OFF`, `CPU_BASELINE=''` and `WITH_PTHREADS_PF=OFF` ([build_args.sh](https://github.com/pyodide/pyodide-recipes/blob/main/packages/opencv-python/extras/build_args.sh)), so no SIMD and no threads. Browser timing would be about the same as Node (same V8); not measured.

Run it (Node 24; the first run downloads the two wheels, 13.6 MB, into `node_modules`):

```sh
cd wasm && npm install && node match.mjs
```

## Options

| Approach | What runs where | SIFT + similarity fit | AVIF decode | Encode / with alpha | Bundle | Speed | License |
|-|-|-|-|-|-|-|-|
| opencv.js official | browser | **No.** Whitelist exports ORB, BFMatcher, `estimateAffine2D` only; no SIFT, no `estimateAffinePartial2D` ([4.x config](https://github.com/opencv/opencv/blob/4.x/platforms/js/opencv_js.config.py), [5.x config](https://github.com/opencv/opencv/blob/main/platforms/js/opencv_js.config.py)). 4.x also has KAZE/AKAZE; 5.x drops them. | browser-native | WebCodecs, no alpha | 10.96 MB (docs.opencv.org/4.13.0/opencv.js, measured) | not measured; SIMD/threads builds exist | Apache-2.0 |
| opencv.js custom build | browser | Yes with `build_js.py --config <whitelist>` ([build_js.py](https://github.com/opencv/opencv/blob/4.x/platforms/js/build_js.py)); needs emsdk and an OpenCV build. `@techstark/opencv-js` 5.0.0-build9 is this stock whitelist, 13.3 MB, no SIFT. | browser-native | WebCodecs, no alpha | **unverified**, likely ~10 MB | **unverified**; `--simd` available | Apache-2.0 |
| Pyodide + opencv-python | browser (web worker) | **Yes, measured above.** `anchor.py`'s track/fit/crop code runs unchanged; `load()` and `render()` don't (no AVIF in Pyodide's Pillow, no ffmpeg). | not in Pyodide: Pillow 12.2 built without libavif ([recipe](https://github.com/pyodide/pyodide-recipes/tree/main/packages/Pillow)); decode in JS and pass pixels | WebCodecs or ffmpeg.wasm | core wasm 9.6 MB (3.6 MB brotli) + stdlib 2.5 MB + numpy 3.0 MB + opencv wheel 10.7 MB (served uncompressed by jsdelivr): ~26 MB | 0.8 s/photo, 12x native | MPL-2.0 (Pyodide), Apache-2.0 (OpenCV) |
| Rust → WASM | browser | [sift-features](https://github.com/tnibler/sift-features) 0.1.0 (2024-09, MIT, 8 stars, "aims to be compatible with OpenCV's feature2d"), [sift-wgpu](https://crates.io/crates/sift-wgpu) 0.1.0 (2026-01, MIT on crates.io, no LICENSE file in repo, WASM + WebGPU backends), [arrsac](https://crates.io/crates/arrsac) 0.11 (MIT) for RANSAC; similarity model must be written. None tested here. | `image` crate (enframe already uses it with `avif`) | WebCodecs or ffmpeg.wasm | **unverified**, likely < 2 MB | **unverified** | MIT |
| ORB / AKAZE in opencv.js | browser | Available in the stock build (ORB in 5.x; ORB, KAZE, AKAZE in 4.x). Not an option: the README's "What didn't work" chose SIFT for scale invariance across lens switches (measured 2.55x); ORB's binary descriptors are weaker there. **unverified** for this data set. | browser-native | WebCodecs | 11 MB | | Apache-2.0 |
| Tauri sidecar, Python | desktop | Yes, native OpenCV | Pillow | ffmpeg (must be bundled or on PATH) | PyInstaller bundle, **unverified** size (opencv-python wheel alone is ~60 MB) | native | MIT, Apache-2.0 |
| Tauri sidecar, Rust | desktop | `opencv` crate needs a native OpenCV build per platform; or the pure-Rust crates above | `image` | ffmpeg | **unverified** | native | |
| Server: `enframe-serve` + `anchor.py` | server (Mac mini / Hetzner), browser marks | Yes, native | Pillow | ffmpeg, alpha works today | none in the browser | 4.4 s for 57 photos (measured) | MIT |
| Server: Cloudflare Python Worker | edge | Pyodide packages are supported ([docs](https://developers.cloudflare.com/workers/languages/python/packages/)), so opencv-python should load; **unverified**. 128 MB per isolate, 30 s CPU default, 5 min max on paid ([limits](https://developers.cloudflare.com/workers/platform/limits/)). Same 0.8 s/photo WASM speed. No ffmpeg; a Container would be needed for the clip. | jsquash in the Worker, or send 1024-px gray from the browser | none | none in the browser | 0.8 s/photo | |

## Feature matching detail

- **opencv.js official build.** The whitelist is the full list of what the JS build exports. 5.x `features` has ORB, MSER, FAST, GFTT, SimpleBlob, BFMatcher, and `_3d` has `estimateAffine2D` (6-parameter affine) but not `estimateAffinePartial2D` (the 4-parameter similarity pinframe fits). The 4.x whitelist adds KAZE and AKAZE and is otherwise the same. SIFT needs a custom build, and the size with SIFT is **unverified**.
- **Pyodide.** `opencv-python` 4.11.0.86 is in the Pyodide 314.0.7 lockfile, built with `features2d`, `calib3d` and `flann` on. `cv2.SIFT_create`, `BFMatcher.knnMatch` and `estimateAffinePartial2D` all ran. The Pyodide `pillow` 12.2.0 recipe has libwebp as its only host codec library and no libavif, so `anchor.load()` can't open AVIF there; `pair.py` takes RGB arrays instead. Threads: none (`WITH_PTHREADS_PF=OFF`). SIMD: off for OpenCV (`CV_ENABLE_INTRINSICS=OFF`); whether Pyodide's core is built with `-msimd128` is **unverified**.
- **Rust.** No crate was built here. `sift-features` is the only one claiming OpenCV-compatible descriptors; `sift-wgpu` claims WASM support and a WebGPU backend in its README but is a 0.1.0 with 5 stars. [rust-cv](https://github.com/rust-cv/rust-cv/issues/8) has AKAZE (last push 2020) and ARRSAC. A port is a week of work plus validation against the 57-photo run, not a spike.
- **ORB/AKAZE.** They would remove the custom-build problem, but the README chose SIFT after template matching failed at the lens switch, and ORB has no scale-space descriptor. Not tried here.

## Decode

- **Browser-native AVIF**: Chrome 85+, Firefox 93+, Safari 16.4+ ([caniuse](https://caniuse.com/avif)). `createImageBitmap` applies EXIF orientation by default (`imageOrientation: "from-image"`, [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Window/createImageBitmap)), which matches `ImageOps.exif_transpose` in `anchor.load()`. A `<canvas>` `drawImage` + `getImageData` then yields RGBA for the tracker.
- **HEIC**: Safari only, natively; elsewhere a WASM libheif. **unverified** which libheif build enframe would need; not in scope for the AVIF samples.
- **WASM decode**: `@jsquash/avif` (what enframe uses, Apache-2.0, libavif inside) decoder wasm is 1.17 MB and took ~340 ms per 1536x2048 photo in Node. Its JS has no orientation handling; whether libavif applies `irot`/`imir` is **unverified**. The Girona samples carry no EXIF orientation tag, so this didn't matter for the measurement.
- Decoding is only needed at full resolution for the render; tracking works on a 1024-px grayscale copy, which is 1.5 MB per photo instead of 12.6 MB RGBA.

## Render and encode

- **Warps**: `OffscreenCanvas.drawImage` with a source rect and a destination rect is the same operation as `place()` (`cv2.resize` + `cv2.warpAffine` with a fill colour). Browser resampling is bilinear, not area/cubic; visible difference **unverified**.
- **WebCodecs `VideoEncoder`**: Chrome 94+, Firefox 130+, Safari 26+ full, Safari 16.4–18.7 partial ([caniuse](https://caniuse.com/webcodecs)). It produces H.264/VP9/AV1 chunks; a muxer is needed: [mediabunny](https://mediabunny.dev/guide/supported-formats-and-codecs) (MPL-2.0, writes MP4 and WebM) or webm-muxer (MIT). No cross-origin isolation needed.
- **Alpha**: Chromium's `video_encoder.cc` on `main` rejects `alpha: "keep"` with "Alpha encoding is not currently supported." ([source](https://chromium.googlesource.com/chromium/src/+/main/third_party/blink/renderer/modules/webcodecs/video_encoder.cc)); the spec issues [#200](https://github.com/w3c/webcodecs/issues/200) and [#672](https://github.com/w3c/webcodecs/issues/672) are open. So transparent WebM (VP9 with the alpha plane as a BlockAdditional stream) can't come from WebCodecs. Workarounds: encode the alpha plane as a second VP9 stream and mux it as BlockAdditional by hand (**unverified** that mediabunny or webm-muxer expose that), or ffmpeg.wasm.
- **ffmpeg.wasm**: `@ffmpeg/core` 0.12.10 wasm is 32.2 MB, GPL-2.0-or-later per npm (presumably because it links libx264; **unverified**). The `core-mt` build needs `SharedArrayBuffer`, so COOP/COEP headers. Whether its libvpx build accepts `yuva420p` is **unverified**. ProRes 4444 output would work if the build includes `prores_ks` (**unverified**).
- **Playback** of VP9-alpha WebM in Chrome is fine; it is the encoding that is missing.

## Limits

- **Memory**: wasm32 tops out at 4 GiB; Memory64 is in Chrome 133+ and Firefox 134+, Safari only in Technology Preview 251/252 ([caniuse](https://caniuse.com/wf-wasm-memory64)). 57 photos at 2048x1536 RGBA is 720 MB, which fits wasm32 but not comfortably in a tab next to Pyodide's own heap; keep only the 1024-px gray copies (86 MB) during tracking and decode full-resolution photos one at a time for the render, as `render()` already streams them.
- **Threads and SIMD**: threads need COOP/COEP, which `wrangler.jsonc` in enframe deliberately omits so `@jsquash/avif` can use its single-threaded build. Pyodide has no threads anyway. SIMD needs no headers, but Pyodide's OpenCV is built without it.
- **Bundle and first load**: Pyodide path ~26 MB before matching starts (3.6 MB of the 9.6 MB core compresses; the 10.7 MB opencv wheel does not). opencv.js custom ~10 MB (**unverified**). Rust likely under 2 MB (**unverified**). A service worker could precache either, as enframe already does for its codecs.
- **Time to answer**: ~45 s for 57 photos in WASM (extrapolated) against 4.4 s native, and every re-run after a new mark repeats the tracking from that mark on.

## Alternatives weighed

- **Tauri sidecar (desktop only).** Python: bundling opencv-python with PyInstaller works but adds tens of MB to the app and a second toolchain to the build (`scripts/sidecar.sh` builds Rust only). Rust: the `opencv` crate needs a native OpenCV per target; the pure-Rust SIFT crates are unproven. Neither reaches the headless-serve or static-web modes.
- **Server-side via `enframe-serve`.** The server already exists, runs on the Mac mini, serves the same UI the desktop app uses, confines paths to roots, and the desktop app ships it as the sidecar. Adding one route that spawns `uv run anchor.py` covers desktop and serve modes with native speed and real ffmpeg output. The static web build stays without pinframe, which matches its "no backend" contract.
- **Split: tracking on a server, marking in the browser.** This is the same as the row above once the server returns the sidecar JSON; the browser never needs OpenCV. The only extra would be returning the 1024-px gray frames or the aligned PNGs (`--frames`) for preview, which `anchor.py` can already write.
- **Cloudflare Python Worker.** Possible for the match step on paid plans, but it is the slow WASM OpenCV again, can't write the clip, and enframe's web target is a static SPA with no Worker today.

## Sources not linked above

- Pyodide package sizes: `https://cdn.jsdelivr.net/pyodide/v314.0.7/full/` (`pyodide-lock.json`, measured with curl).
- `@techstark/opencv-js`: [TechStark/opencv-js](https://github.com/TechStark/opencv-js), `dist/opencv.js` 13,298,869 bytes, release `opencv-js-5.0.0-build9` (2026-06-26), built with the stock `build_js.py`.
- Memory64: [SpiderMonkey blog](https://spidermonkey.dev/blog/2025/01/15/is-memory64-actually-worth-using.html), [Chrome intent to ship](https://groups.google.com/a/chromium.org/g/blink-dev/c/5vTbd1dttwc/m/Z4UFehJBAgAJ).
- ffmpeg.wasm: `npm view @ffmpeg/core` (license, version), `https://unpkg.com/@ffmpeg/core@0.12.10/dist/esm/ffmpeg-core.wasm` (size, measured).
