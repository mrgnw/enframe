# pinframe

Turn a series of photos into a short clip where one chosen feature, like a cloud seen from a train window, stays in the same place in frame while everything else changes.

> [!NOTE]
> This project was built with heavy LLM assistance and should be considered proof-of-concept. Contributions are welcome, but I can't guarantee the accuracy of the code or that I will continue to maintain it. If there are any mistakes in attribution or anything else, please let me know.

## Usage

Build the Rust CLI, `pinframe`:

```sh
brew install cmake meson ninja
```

```sh
cd rust
cargo build --release
```

This needs Rust stable, cmake, meson and ninja, libclang for bindgen (Xcode or the Command Line Tools), and network access on the first build. The build downloads pinned releases of libavif, dav1d and libaom and links them in statically, so the binary runs without Homebrew. The first build takes about 40 s; later builds take a couple of seconds. See [docs/rust.md](docs/rust.md) for the full build notes and the licences that come with distributing it.

```sh
rust/target/release/pinframe "samples/girona clouds"/IMG_479{1,2,3,4,5}.avif --box 520,220,620,500 --out out/heart.avif
```

Unlike the Python reference implementation below, there's no box-drawing window: `--box x,y,w,h` is required, in first-photo pixels, and gives the feature to hold still.

Any photo the tracker reports as `LOST` can be fixed by marking the feature there, and tracking carries on from the mark:

- `--at PHOTO=x,y` puts the anchor at a point.
- `--at PHOTO=x,y,w,h` also says where to search, which keeps foreground clutter out around a small or distant cloud.
- `--at PHOTO=x,y,w,h,zoom` also sets the zoom relative to the first photo, for a lost photo taken on a different lens.

Otherwise a mark keeps the zoom of the photo before it.

```sh
rust/target/release/pinframe "samples/girona clouds"/IMG_{4698,4699,4700,4701,4702,4703,4704,4705}.avif --box 840,550,400,350 --at IMG_4703.avif=746,554,400,350 --out out/turbines.avif
```

By default the crop shrinks until every photo fills it. `--pad` lets it overhang photos instead, filled with `black`, `white`, a `#rrggbb` colour, or `transparent`:

```sh
rust/target/release/pinframe "samples/girona clouds"/IMG_47{18,19,20,21,22,23,24,25,26,27,28,29}.avif --box 400,940,540,180 --pad transparent --out out/portrait.avif
```

The extension of `--out` picks the format. The default, `.avif`, is an animated AVIF that keeps transparency and plays in browsers as an image. On the 57-photo Girona clip it encodes as fast as ProRes, and the file is 9 MB instead of 109 MB. Through ffmpeg, `.mp4` writes H.264, `.mov` writes ProRes 4444 when transparent (for video editors), and `.webm` writes VP9.

Writes the clip and a `.json` beside it: `{"run": ..., "clip": ..., "photos": [...]}`. Each entry in `photos` has the photo's crop rect, normalized to 0–1 like enframe's coordinates. With padding, rects can extend past 0 and 1. Each entry also has the photo's `scale` relative to the first photo and its `source`: `tracked`, `mark` (placed by `--box` or `--at`), or `lost` (position estimated from neighbours, the photos to mark next).

Both files are written under names unique to the run, then renamed into place together while holding a lock on the output folder. A failed run leaves the previous clip and `.json` untouched, and runs that share an output never mix their files. `run` is a random ID that is also stored in the clip's metadata (XMP for `.avif`, `comment` for ffmpeg outputs), so a reader can confirm a clip and its `.json` belong together. A reader can also take a shared lock on the folder while reading the pair.

`--frames DIR` also saves every aligned photo as `DIR/0001.png`, `0002.png`, …, so other tools can take over: re-encode at another speed with `ffmpeg -framerate 8 -i DIR/%04d.png`, stack them in Siril, or preview them in enframe. The directory must hold no PNGs yet.

`--vf` passes ffmpeg filters to a `.mp4`, `.mov` or `.webm` clip (not to `--frames`). `deflicker` evens exposure jumps between shots, `minterpolate=fps=24:mi_mode=blend` blends between photos instead of cutting, and `tpad=stop_mode=clone:stop_duration=2` holds the last photo for 2 seconds:

```sh
rust/target/release/pinframe "samples/girona clouds"/IMG_{4698,4699,4700,4701,4702,4703,4704,4705}.avif --box 840,550,400,350 --at IMG_4703.avif=746,554,400,350 --vf deflicker,tpad=stop_mode=clone:stop_duration=2 --out out/turbines.mp4
```

The whole sample folder, with every mark, is one script:

```sh
BIN=rust/target/release/pinframe bash analysis/girona_all.sh out/all.avif
```

Run the test suite as the self-check:

```sh
cd rust && cargo test --release
```

### Python reference implementation

`anchor.py` was the original implementation. It's kept as the one place with the interactive box window, and as the reference the Rust output is checked against (`analysis/compare_rects.py`, `analysis/rect_diff.py`):

```sh
uv run anchor.py "samples/girona clouds"/IMG_479{1,2,3,4,5}.avif --out out/heart.avif
```

A window opens on the first photo. Drag a box around the feature to hold still, then press Enter. To skip the window, pass the box as `--box x,y,w,h`, same as the Rust CLI.

Run with no arguments for `anchor.py`'s own self-check:

```sh
uv run anchor.py
```

Every other flag and output above is the same between the two.

## How it works

1. Photos are taken in filename order.
2. SIFT keypoints inside the box are matched into the next photo, and a scale + rotation + shift is fitted to them with RANSAC. That follows the anchor across lens switches and resolution changes, and the number of agreeing keypoints is an honest confidence: an occluded cloud gets 0–2, not a wrong answer.
3. A fit that zooms more than 6x or turns more than 15° between two photos is a false match, not a camera move, and is rejected.
4. Each photo is matched against the last photo where the anchor was found, so the tracker follows a cloud as it slowly changes shape.
5. A photo with fewer than 8 agreeing keypoints is `LOST`. It stays in the clip, with the anchor's position and zoom interpolated from the photos either side, unless `--at` marks it.
6. Each photo is scaled so the anchor is the same size in all of them. The crop then keeps the anchor at one fixed spot in every photo. Portrait, landscape, 1x, 2x and 3x photos mix freely.
7. The clip is sized at the median photo's zoom, so a single zoomed-out photo doesn't shrink the rest. ffmpeg assembles the crops at `--fps` (default 4).

The Rust CLI and `anchor.py` implement this the same way; `docs/rust.md` has where their SIFT and AVIF handling differ by a few parts in ten thousand.

## Results on the Girona samples

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

- **Point picking in a UI, not the terminal.** [docs/ui-plan.md](docs/ui-plan.md) and the mockups in `mockups/` sketch a phone-first flow: draw a box on the first photo, then mark any photo flagged `LOST`. The mockups are previewable at [00-start.html](https://mox.xcc.es/pinframe/00-start.html), [01-marking.html](https://mox.xcc.es/pinframe/01-marking.html) and [02-preview.html](https://mox.xcc.es/pinframe/02-preview.html). Marking 24 photos by typing coordinates is the slow part today.
- **Zoom from EXIF.** Original iPhone files record the focal length, which gives each photo's lens directly. These AVIF exports had none, so lens switches were measured or marked by hand.
- **A choice between same-size and full-frame.** Keeping the cloud the same size shrinks zoomed photos. An option to keep each photo's own zoom would fill the frame instead, with the cloud changing size at lens switches.
- **Soft lock.** Today the anchor is pinned exactly. Letting it drift a little along a smoothed path may look more natural.

## Research

- [docs/prior-art.md](docs/prior-art.md) — tools that keep one feature fixed across a sequence, and how they name their options.
- [docs/wasm.md](docs/wasm.md) — can this run in the browser? Pyodide matches a pair correctly but 12x slower than native, SIFT is missing from opencv.js, and browsers can't encode transparent video. Recommendation: track on the server behind enframe-serve, mark in the browser. The proof of concept is in `wasm/`.
- [docs/rust.md](docs/rust.md) — the Rust port: why it exists, how it compares to `anchor.py` on speed, memory and correctness, and what it means for packaging pinframe into enframe.
- [docs/performance.md](docs/performance.md) — `anchor.py`'s own performance measurements that the Rust comparison builds on.
- [docs/ui-plan.md](docs/ui-plan.md) — the marking UI plan behind the `mockups/` sketches.

## No-code alternative

Blender's 2D Stabilization does the same job for free on an image sequence: put one tracking marker on the feature, enable location stabilization, and set an anchor frame. For soft features like clouds, set the marker's motion model to Affine, enable Normalize, and raise Correlation to 0.9. It needs manual marker fixes when the track drifts, and a sudden lens switch means re-placing the marker by hand.

The one-click stabilizers (ffmpeg `vidstab`, Warp Stabilizer, Resolve's automatic stabilizer, Hugin's `align_image_stack`) don't work here. They lock onto the dominant motion, which from a train is the landscape, not the cloud.

## License

This project is licensed under [MIT](LICENSE).

The Rust CLI depends on the crates and C libraries listed in [docs/rust.md](docs/rust.md#crates-and-licenses). `anchor.py`, the Python reference implementation, depends on these libraries, under their own licenses:

- [OpenCV](https://github.com/opencv/opencv-python) — Apache-2.0 (keypoint tracking, image resizing)
- [NumPy](https://numpy.org) — BSD-3-Clause (arrays)
- [Pillow](https://python-pillow.org) — MIT-CMU (AVIF decoding)
- [FFmpeg](https://ffmpeg.org) — LGPL/GPL depending on the build (video encoding; run as an external program, not bundled)
