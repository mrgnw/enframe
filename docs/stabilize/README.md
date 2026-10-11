# Stabilize

Turn a series of photos into a short clip where one chosen feature, like a cloud seen from a train window, stays in the same place in frame while everything else changes.

> [!NOTE]
> This project was built with heavy LLM assistance and should be considered proof-of-concept. Contributions are welcome, but I can't guarantee the accuracy of the code or that I will continue to maintain it. If there are any mistakes in attribution or anything else, please let me know.

## Usage

Build the Rust CLI, `stabilize`, from the enframe root:

```sh
brew install cmake meson ninja                                # macOS
sudo apt install cmake meson ninja-build nasm libclang-dev    # Debian/Ubuntu
```

```sh
cargo build --release --manifest-path src-tauri/Cargo.toml -p stabilize
```

This needs Rust stable, cmake, meson and ninja, libclang for bindgen (Xcode or the Command Line Tools on macOS, `libclang-dev` on Debian/Ubuntu), nasm on x86, and network access on the first build. The build downloads pinned releases of libavif, dav1d and libaom and links them in statically, so the binary runs without Homebrew. The first build takes about 40 s; later builds take a couple of seconds. See [docs/rust.md](rust.md) for the full build notes and the licences that come with distributing it.

```sh
src-tauri/target/release/stabilize "samples/train clouds"/IMG_479{1,2,3,4,5}.avif --points 675,345/985,345/675,595/985,595 --out out/heart.mp4
```

There's no point-drawing window: `--points x,y[/x,y]...` is required, in first-photo pixels, and gives the feature to hold still. A point is `x,y`; mark several with the same feature's spread, joined by `/` (`675,345/985,345/675,595/985,595` above, a cloud's four quarter points). The anchor that stays still in the clip is their mean.

Input photos may be AVIF, HEIC/HEIF, JPEG XL, JPEG, or PNG. Stabilize identifies them from their contents rather than their file extension; other formats are refused before decoding. HEIC and JPEG XL decode through macOS ImageIO, so on Linux those two are refused before decoding, with a message naming the platform. Stabilize is built and tested on macOS and Linux; the two track the train clouds set identically (see [docs/rust.md](rust.md#platforms)).

Photos are ordered by EXIF capture time (`DateTimeOriginal`, with `SubSecTimeOriginal` breaking ties), falling back to filename order if any photo lacks one, such as a mix of cameras or an export that stripped EXIF. `--order name` always orders by filename. Either way, the run prints which order it used and why.

Any photo the tracker reports as `LOST` can be fixed by marking the feature there, and tracking carries on from the mark:

- `--at PHOTO=x,y` puts the anchor at a point.
- `--at PHOTO=x,y/x,y...` marks several points, which keeps foreground clutter out around a small or distant cloud; a mark replaces the photo's whole point set.
- `--at PHOTO=x,y/x,y...@zoom` also sets the zoom relative to the first photo, for a lost photo taken on a different lens.

Otherwise a mark keeps the zoom of the photo before it, times the change in EXIF 35 mm focal length when both photos record one (AVIF, HEIC, JPEG and PNG). At a lens switch the tracker also expects that zoom, within 1.25x.

If a building or trees hide the feature entirely rather than just losing the tracker, mark it with `--skip PHOTO` instead: the photo is never matched and stays in the clip at an estimated position, like `LOST`, but without risking a wrong fit on whatever is in front of the feature. A skipped photo cannot also carry an `--at` mark, and the first photo cannot be skipped.

When the feature is visible but too hazy or half-hidden to place precisely, `--near PHOTO=x,y/x,y...@zoom` soft-marks it instead. The photo is placed at those points and zoom, like `--at`, but tracking carries on from the last tracked or marked photo as if this one were lost, so a rough position never becomes the reference for the photos after it. It takes the same points, zoom and checks as `--at`, and cannot share a photo with `--at` or `--skip`.

```sh
src-tauri/target/release/stabilize "samples/train clouds"/IMG_{4698,4699,4700,4701,4702,4703,4704,4705}.avif --points 940,637.5/1140,637.5/940,812.5/1140,812.5 --at IMG_4703.avif=846,641.5/1046,641.5/846,816.5/1046,816.5 --out out/turbines.mp4
```

By default the crop shrinks until every photo fills it. `--pad` lets it overhang photos instead, filled with `black`, `white`, a `#rrggbb` colour, or `transparent`:

```sh
src-tauri/target/release/stabilize "samples/train clouds"/IMG_47{18,19,20,21,22,23,24,25,26,27,28,29}.avif --points 535,985/805,985/535,1075/805,1075 --pad transparent --out out/portrait.mov
```

Every photo is scaled so the feature stays the same size, so a photo where the feature is small is magnified and shows only a patch of itself. `--zoom PHOTO=scale` shows that photo at `scale` (relative to the first photo, like a mark's `@zoom`) whatever tracking found; tracking and the estimates for lost photos are unchanged. `--canvas WxH` sets the clip's size in pixels, both even: photos keep their scale and the anchor keeps its place in the frame, so a larger canvas shows more around the feature. A canvas larger than the crop every photo fills needs `--pad`:

```sh
src-tauri/target/release/stabilize "samples/train clouds"/IMG_47{18,19,20,21,22,23,24,25,26,27,28,29}.avif --points 535,985/805,985/535,1075/805,1075 --zoom IMG_4720.avif=1.5 --canvas 1536x1536 --pad black --out out/canvas.mp4
```

The extension of `--out` picks the format. The default, `.mp4`, writes H.264 with a built-in encoder, no ffmpeg needed; it has no alpha channel. Through ffmpeg, `.mov` writes ProRes 4444 when transparent (for video editors), and `.webm` writes VP9; transparent padding needs one of these two.

Colour stays as the photos have it. Stabilize keeps their pixel values and labels the clip, the `--frames` PNGs and the video outputs with their colour profile, so the Display P3 photos from an iPhone stay Display P3. A set mixing sRGB and Display P3 photos becomes Display P3: the sRGB photos are converted, which loses nothing, since Display P3 holds every sRGB colour. Unlabelled photos count as sRGB. A photo with any other profile, such as Adobe RGB, stops the run with its name.

With `--out result.json`, the CLI writes the sidecar only: it tracks and plans rects but does not render, decode for rendering, or encode. Its `clip` field is `null`.

SIFT features are cached under `~/Library/Caches/pinframe` on macOS and `~/.cache/pinframe` (or `$XDG_CACHE_HOME/pinframe`) on Linux by default, so a mark-and-retrack loop does not repeat SIFT. Use `--cache DIR` for a project-specific cache or `--no-cache` to bypass it:

```sh
src-tauri/target/release/stabilize "samples/train clouds"/IMG_479{1,2,3,4,5}.avif --points 675,345/985,345/675,595/985,595 --out out/track.json
src-tauri/target/release/stabilize "samples/train clouds"/IMG_479{1,2,3,4,5}.avif --points 675,345/985,345/675,595/985,595 --cache /tmp/stabilize-cache --out out/track.json
```

Writes the clip and a `.json` beside it: `{"run": ..., "clip": ..., "photos": [...]}`. Each entry in `photos` has the photo's crop rect, normalized to 0–1 like enframe's coordinates. With padding, rects can extend past 0 and 1. Each entry also has the photo's `scale` relative to the first photo and its `source`: `tracked`, `mark` (placed by `--points` or `--at`), `lost` (position estimated from neighbours, the photos to mark next), `soft` (placed by `--near`), or `skipped` (position estimated the same way, for a photo marked `--skip`). `points` lists every point of that photo, `[[x, y], ...]` normalized the same way as the crop rect, in the order given (or carried forward by tracking); the anchor that stays still in the clip is their mean.

Both files are written under names unique to the run, then renamed into place together while holding a lock on the output folder. A failed run leaves the previous clip and `.json` untouched, and runs that share an output never mix their files. `run` is a random ID that is also stored in the clip's metadata (the `comment` tag), so a reader can confirm a clip and its `.json` belong together. A reader can also take a shared lock on the folder while reading the pair.

`--frames DIR` also saves every aligned photo as `DIR/0001.png`, `0002.png`, …, so other tools can take over: re-encode at another speed with `ffmpeg -framerate 8 -i DIR/%04d.png`, stack them in Siril, or preview them in enframe. The directory must hold no PNGs yet.

`--vf` passes ffmpeg filters to a `.mp4`, `.mov` or `.webm` clip (not to `--frames`); a `.mp4` with `--vf` is encoded by ffmpeg. `deflicker` evens exposure jumps between shots, `minterpolate=fps=24:mi_mode=blend` blends between photos instead of cutting, and `tpad=stop_mode=clone:stop_duration=2` holds the last photo for 2 seconds:

```sh
src-tauri/target/release/stabilize "samples/train clouds"/IMG_{4698,4699,4700,4701,4702,4703,4704,4705}.avif --points 940,637.5/1140,637.5/940,812.5/1140,812.5 --at IMG_4703.avif=846,641.5/1046,641.5/846,816.5/1046,816.5 --vf deflicker,tpad=stop_mode=clone:stop_duration=2 --out out/turbines.mp4
```

The whole sample folder, with every mark, is one script:

```sh
BIN=src-tauri/target/release/stabilize bash analysis/stabilize/train_clouds_all.sh out/all.mov
```

The marking UI is enframe's `/stabilize`; enframe's `bun run test:e2e` covers the walkthrough.

Run the test suite as the self-check:

```sh
cargo test --release --manifest-path src-tauri/Cargo.toml -p stabilize
```

## How it works

1. Photos are taken in capture-time order (or filename order, see above).
2. SIFT keypoints inside the box are matched into the next photo, and a scale + rotation + shift is fitted to them with RANSAC. That follows the anchor across lens switches and resolution changes, and the number of agreeing keypoints is an honest confidence: an occluded cloud gets 0–2, not a wrong answer.
3. A fit that zooms more than 6x or turns more than 15° between two photos is a false match, not a camera move, and is rejected.
4. Each photo is matched against the last photo where the anchor was found, so the tracker follows a cloud as it slowly changes shape.
5. A photo with fewer than 8 agreeing keypoints is `LOST`. It stays in the clip, with the anchor's position and zoom interpolated from the photos either side, unless `--at` marks it.
6. Each photo is scaled so the anchor is the same size in all of them. The crop then keeps the anchor at one fixed spot in every photo. Portrait, landscape, 1x, 2x and 3x photos mix freely.
7. The clip is sized at the median photo's zoom, so a single zoomed-out photo doesn't shrink the rest. The crops are encoded at `--fps` (default 4).

[rust.md](rust.md) has how the Rust SIFT and AVIF handling differed from the retired Python reference, by a few parts in ten thousand.

## Results on the train clouds samples

| Run | Photos | Marks | Clip | Result |
|-|-|-|-|-|
| Heart cloud, IMG_4791–4795 | 5 | 1 | 1420x1452 | Holds across a portrait-to-landscape switch |
| Cumulus over turbines, IMG_4698–4705 | 8 | 2 | 1674x1462 | Holds; IMG_4703 shows the cloud from a new angle and needed a mark |
| Portrait cumulus, IMG_4718–4729 | 12 | 1 | 1200x1940, or 1590x2032 padded | Holds across a 1x → 2x lens switch (measured 2.55x); the tree (4721) and building (4725) frames are kept at estimated positions |
| Whole folder, all 57 | 57 | 24 | 1442x1196 padded | 30 photos linked by tracking, 24 marked at scene changes, 3 estimated (cloud out of frame, tree, building) |

In the whole-folder clip, photos taken at 2.5–3x appear small inside the padding. Keeping the cloud the same size means shrinking those photos, and they cover less sky than the 1x photos around them.

Across scene changes, "the same cloud" is a judgement call. Where the shots allowed it, the mark follows the same cumulus tower: IMG_4698's tower links automatically to 4696's at 2.5x. In the mountain stretch (IMG_4766–4780) the marks hold the middle of a broad cloud deck.

## What didn't work

- **Template matching across scales.** Normalized correlation favours small templates. At the lens switch it picked a 17 px patch at 0.2x and then tracked that patch with 0.97 confidence.
- **Template matching as a fallback when keypoints fail.** It recovered IMG_4703, but across the whole folder it chained onto blank sky, which matches blank sky at 0.87–0.98. Matching on detail only (blur subtracted) removed those, and the real matches with them.

## Next steps, in order

- **Point picking in a UI, not the terminal.** [docs/ui-plan.md](ui-plan.md) and the mockups in `mockups/stabilize/` sketch a phone-first flow: draw a box on the first photo, then mark any photo flagged `LOST`. The mockups are previewable at [00-start.html](https://mox.xcc.es/pinframe/00-start.html), [01-marking.html](https://mox.xcc.es/pinframe/01-marking.html) and [02-preview.html](https://mox.xcc.es/pinframe/02-preview.html). Marking 24 photos by typing coordinates is the slow part today.
- **A choice between same-size and full-frame.** Keeping the cloud the same size shrinks zoomed photos. An option to keep each photo's own zoom would fill the frame instead, with the cloud changing size at lens switches.
- **Soft lock.** Today the anchor is pinned exactly. Letting it drift a little along a smoothed path may look more natural.

## Research

- [docs/prior-art.md](prior-art.md) — tools that keep one feature fixed across a sequence, and how they name their options.
- [docs/wasm.md](wasm.md) — can this run in the browser? Pyodide matches a pair correctly but 12x slower than native, SIFT is missing from opencv.js, and browsers can't encode transparent video. Recommendation: track on the server behind enframe-serve, mark in the browser. The proof of concept is in the archived pinframe repository, `wasm/`.
- [docs/rust.md](rust.md) — the Rust port: why it exists, how it compares to `anchor.py` on speed, memory and correctness, and what it meant for moving the tracker into enframe.
- [docs/performance.md](performance.md) — `anchor.py`'s own performance measurements that the Rust comparison builds on.
- [docs/ui-plan.md](ui-plan.md) — the marking UI plan behind the `mockups/stabilize/` sketches.

## No-code alternative

Blender's 2D Stabilization does the same job for free on an image sequence: put one tracking marker on the feature, enable location stabilization, and set an anchor frame. For soft features like clouds, set the marker's motion model to Affine, enable Normalize, and raise Correlation to 0.9. It needs manual marker fixes when the track drifts, and a sudden lens switch means re-placing the marker by hand.

The one-click stabilizers (ffmpeg `vidstab`, Warp Stabilizer, Resolve's automatic stabilizer, Hugin's `align_image_stack`) don't work here. They lock onto the dominant motion, which from a train is the landscape, not the cloud.

## License

This project is licensed under [MIT](../../LICENSE).

The Rust CLI depends on the crates and C libraries listed in [docs/rust.md](rust.md#crates-and-licenses), plus [FFmpeg](https://ffmpeg.org) for `.mov`, `.webm` and `--vf` (LGPL/GPL depending on the build; run as an external program, not bundled).
