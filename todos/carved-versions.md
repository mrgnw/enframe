---
branch: explore-claire-duplicate-photo-crops
---
# Carved versions: content-aware resizing before the crop

Explored 2026-10-17. Not scheduled.

## Why

enframe only crops. A landscape photo framed for mobile (9:16) keeps a narrow slice and loses whatever sits outside it. [esimov/caire](https://github.com/esimov/caire) (Go, MIT) resizes photos by seam carving: it removes connected paths of pixels ("seams") that run top to bottom through the parts of the photo with the least detail, one at a time, until the photo is the target width. Sky and empty field close up; the subjects keep their shape.

The idea for enframe: carving makes a **version**, a separate copy of the original photo at a new aspect ratio. A version is never one of the crops. It is a photo of its own, gets its own frames, and exports like any photo. Switching between the original and its versions animates.

## Mockup

`mockups/carve/01-versions.html`: the original and two carved versions of IMG_4667 (train clouds). "+ Carve" makes a 1:1 or 4:5 version; clicking a version, or `V`, morphs between them. Every column slides from its place in one version to its place in the other, and carved columns close up. The morph and the frames are computed from the spike's real masks.

`mockups/carve/03-playground.html`: every knob on ten samples. The carver is ported to JavaScript and runs in a web worker. It draws each seam as it is removed, the way caire's progress window does, and carves once to 60% smaller so the size slider (40–140%) is instant afterwards. Widening repeats the first seams. Knobs: direction, size, protect and remove points (size, softness), a region band, a lean to one side, a straightness penalty, forward or gradient energy, blur, and working size. In the browser a 60% carve takes 0.4 s at 512 px, 1.0–1.3 s at 768 px and 2.9 s at 1024 px.

## Spike results

A forward-energy seam carver in Rust (about 150 lines; dependencies `image` and `rayon`), written in /tmp and not committed. It carves a downscaled working copy, then stretches the resulting map of kept columns over the full-size photo. Apple M4 Pro; the carve runs on one core. Each case ran twice, with and without focal points protected; a range covers both runs, and which one was faster varied.

| Photo | Size | Change | Working size | Carve | Full-size render |
|-|-|-|-|-|-|
| IMG_4667 (train clouds) | 2048×1536 | 4:3 → 1:1, 25% removed | 1024 | 0.8–1.3 s | under 10 ms |
| IMG_4667 | 2048×1536 | 4:3 → 1:1, 25% removed | full size, exact | 5.6 s | none needed |
| IMG_4667 | 2048×1536 | 4:3 → 4:5, 40% removed | 1024 | 1.1–2.0 s | under 10 ms |
| IMG_8115 (fl clouds) | 3024×4032 | 3:4 → 1:1, rows, 25% removed | 1024 / 1536 | 0.9–1.4 s / 2.8–3.3 s | 10 ms |
| IMG_8126-Pano (fl clouds) | 7766×7865, 61 MP | → 9:16, 43% removed | 1024 / 1536 | 1.9–2.1 s / 5.3–6.1 s | 30–60 ms |
| portrait-upright-ref.jpg (face fixture) | 1024×1024 | 1:1 → 9:16, 44% removed | full size | 1.5–1.6 s | none needed |

What each hypothesis came to:

1. **Fast enough to wait for.** Yes, with a working size. The cost of a carve grows with working-size pixels times seams removed, not with the photo's megapixels: 0.3–2.1 s at 1024 px for all 15 photos tried, up to 77 MP. Carving at full size is not viable: 5.6 s at 3 MP, so minutes at 24 MP and up.
2. **A working-size carve looks as good as a full-size one.** Yes. At 2× (IMG_4667) the result matched exact full-size carving. At 7.7× (the 61 MP pano) a gate's edges and fence posts stay straight with no steps, because the map is interpolated into a smooth squeeze instead of cutting blocky seams. 1536 px costs three times as long with no visible gain.
3. **The result looks natural.** Up to about 40%. Landscapes and skies at 25% look untouched. At 40%, anything without a focal point narrows: the cloud in IMG_4667 got narrower until it was protected. The portrait at 44% kept the face but turned the round helmet into an oval and thinned the torso. Cap the carve at about 40% and let a frame crop the rest, which is what version-then-frame does anyway. Two kinds of photo carve badly at any level: detail everywhere (water drops on a leaf, where round drops turn oval) and regular architecture (a row of arches, where one window narrows and the symmetry breaks). The morph shows the result before any frame is drawn, so the person framing can delete a bad version; no automatic check is proposed.
4. **Focal points can protect subjects.** Yes. A disc around each focal point (radius 14% of the short side) makes crossing it so expensive that no seam does; at 4:5 the cloud kept its width. Detected faces can already be turned into focal points (#31), so faces get the same protection. A disc covers a face, not a person: unprotected, the couple in IMG_0001 got thin even at 25%, while three points down their bodies kept them natural at 40%. Big Ben needed three points up the tower the same way.
5. **The switch animation is cheap.** Yes. It needs only each version's mask of kept columns per row: 100 KB as PNG at 1024 px, 220–270 KB at 1536 px. The mockup paints a 1536×1152 morph frame in 10–12 ms with plain canvas 2D; no WebGL.

### The eleven samples added on 2026-10-17

Each carved 25% and 40% narrower (the moon, a tall photo, shorter) at a 1024 px working size, with no focal points, and compared with a plain crop of the same size. Times are for the 40% carve. Side-by-side sheets: https://mox.xcc.es/_preview/enframe/carve/02-samples.html (the page and its images live only in `~/dev/_preview/enframe/mockups/carve/`, not in the repo).

| Photo | Size | Carve | Against a crop |
|-|-|-|-|
| 0-6, autumn mountains | 4928×2772 | 1.0 s | better |
| IMG_0001, lake panorama | 10820×7133, 77 MP | 0.9 s | better: both shores stay in |
| DJI_0145, drone panorama | 8405×2292, 3.7:1 | 0.4 s | better: islands keep their shape |
| DJI_0386, aerial house and river | 4000×2250 | 0.8 s | better |
| elchalten, mountain panorama | 6917×3541 | 1.4 s | better |
| moon, carved by rows | 2250×4000 | 0.7 s | better: the moon stays round, empty sky goes |
| untitled, tower, plane, storm cloud | 4898×2861 | 0.8 s | better |
| 0, Big Ben at night | 4928×3264 | 1.0 s | fine at 25%; at 40% only with the tower protected |
| IMG_0001, couple in Lisbon | 3342×2089 | 0.9 s | worse until the couple is protected, then better |
| 0-7, water drops (macro) | 4435×2495 | 1.5 s | worse, protected or not |
| IMG_6500, arches | 3977×2983 | 1.1 s | worse |

Seven of eleven are better with no input, two more once focal points cover the subject, two are worse. Panoramas are the strongest case. The 3.7:1 drone panorama carves from a working copy only 279 px tall and still looks clean at 40%; a minimum working height for very wide photos is untested.

Sizes for one version file, IMG_8115 carved to 3024×3024: 0.5 MB as AVIF (sips default quality), 7.1 MB as PNG.

Not measured: memory at full size. Rendering a 61 MP version holds the source and the output in RGB8, about 180 MB plus 105 MB; the working-size carve itself needs about 20 MB.

Considered and not used: the `seamcarving` crate (last release 2020, on `image` 0.23, no licence in its repository). The carver is short enough to own, and owning it gives the kept-column map the animation needs.

## Proposed design

A version is a photo row that points at its source, with its own file.

- **Schema.** `photos.source_id` (references `photos.id`, cascade delete) and `photos.carve` (JSON: target aspect, axis, the focal points used, working size). Store the kept-column mask too (`photos.carve_mask`, PNG bytes) so the animation needs no second carve.
- **Pixels.** Carve once and write the version as a file next to the source, for example `IMG_4667.carved-1x1.avif`, with the export encoder (on macOS, ImageIO AVIF keeping the source's colour profile). The framer, export, face detection and library thumbnails all open photos by file path, so they work on versions unchanged.
- **Rust.** A `carve` module in `src-tauri` and a `carve_version` command. First split `crop_and_export` into its decode half (JXL, RAW, HEIF, EXIF orientation, colour profile) and its encode half, so carving reuses both. Serve forwards the command like the others; the output needs a write root.
- **Library.** Versions sit under their source photo (a "2 versions" badge), not as separate cards.
- **Framer.** Version chips in an island, a "Carve" menu offering the destination ratios with anything over the cap disabled, and a progress note while it carves (1–2 s).
- **Animation.** Draw the original at preview size into a canvas and morph with the two masks, as the mockup does; at the end, show the version's own image. Frames fade out during the morph and the target version's frames fade in. Focal points ride along through the mask.

Alternative: no version file; render pixels from source plus mask every time they are needed (10–60 ms at full size). That avoids a second lossy encode and a file per version, but every place that opens a photo by path (Tauri asset protocol, serve's `/img` route, export, faces, thumbnails) would have to learn about versions. Start with the file and revisit if version files pile up or go stale after the source is edited.

Out of scope: the static web build, which has no backend to carve with; enlarging by inserting seams (caire does it; crops never need a wider photo).

## Phases

1. `carve` module with tests: exact output size, a protected disc is never crossed, a flat image loses columns evenly. Includes the decode/encode split of `crop_and_export`.
2. Schema migration, `db.ts`, and library grouping.
3. Framer: version chips, the Carve menu, progress.
4. The morph.

## Open questions

- Where version files live: next to the source (they then show in folder browsing, as exports already do) or in an enframe folder.
- People need several focal points to stay whole. Is placing them by hand acceptable, or should protection come from a drawn box, or from a detected face extended down to a body?
- Is 40% the right cap? A slider for any width instead of fixed ratios?
- When the source changes (a Lightroom edit), re-carve the version automatically, or mark it stale?
