# RapidRAW integration research

Researched 2026-10-31 for issue #54 by reading RapidRAW's source on GitHub (`CyberTimon/RapidRAW`,
`main`, v1.6.7), then checked headless against the v1.6.7 macOS build (see "Spike results"). The
RapidRAW window itself was never opened.

## Why look at it

The Lightroom integration reads Lightroom's private SQLite catalog read-only. It cannot write crops
back, cloud-only photos have no local file, and Adobe's API needs an enterprise contract. RapidRAW is
an open-source RAW editor (Tauri + Rust, like enframe) whose edits are plain JSON files next to each
photo, and which has command-line entry points for opening and rendering photos.

## What RapidRAW exposes

| Surface | What it does | Status |
|-|-|-|
| Sidecar `IMG.CR3.rrdata` | JSON edit file next to each photo: rating, flag, tags, `adjustments` (crop, rotation, colour, masks) | Shipped |
| Virtual copy `IMG.CR3.<6 hex>.rrdata` | A second edit of the same photo; RapidRAW lists every `*.rrdata` it finds when it scans a folder | Shipped |
| XMP `IMG.xmp` | Rating, colour label and tags, synced both ways (on by default) | Shipped |
| `RapidRAW <file>` | Opens a photo; a running instance receives it through the single-instance plugin | Shipped |
| `RapidRAW --edit <src> --output <out>` | Opens the photo for editing; when the user clicks Done it writes the rendered result to `<out>` and quits | Shipped (#1325) |
| `RapidRAW export <file\|dir> --output <path> [--format --quality --tiff-bit-depth --adjustments adj.json]` | Headless render through the full GPU pipeline; a folder export includes virtual copies | Shipped (#1418) |
| "Edit In" | Sends a photo, or a rendered 16-bit TIFF `<stem>_<App>.tif`, to an external app the user configures | Shipped (#2082) |
| Localhost control API (`127.0.0.1/v1`, MCP) | Remote control of the running app | Open PR #1810/#1811, not merged |
| Plugin system | | Open issue #539 |

Missing: no URL scheme, no HTTP server, no file watcher. RapidRAW re-reads sidecars only when a folder
loads or the user refreshes it.

Licence is AGPL-3.0. Calling RapidRAW as a separate program is fine; linking its crate into enframe
would make enframe AGPL. Its render code is also tied to its app state, so it is not usable as a
library anyway.

## The sidecar format

Rust type `ImageMetadata` in `src-tauri/src/image_processing.rs`; the adjustments are typed in
`src/utils/adjustments.ts`.

```json
{
	"version": 1,
	"rating": 4,
	"flag": "pick",
	"tags": ["travel", "color:red"],
	"adjustments": {
		"crop": { "x": 412, "y": 0, "width": 3000, "height": 3750, "unit": "px" },
		"aspectRatio": 0.8,
		"rotation": 0,
		"orientationSteps": 0,
		"flipHorizontal": false,
		"flipVertical": false,
		"masks": []
	}
}
```

- `crop` is in full-resolution pixels, not 0–1 fractions. It is measured on the photo after its EXIF
  orientation, then `orientationSteps` quarter turns, then the flips, then `rotation`.
- `rotation` straightens within the same canvas, so a crop has to stay inside the rotated area.
- `aspectRatio` is a number or null; 0 means "original".
- `version` defaults to 1 and nothing reads it. A RapidRAW update can change the format with no signal.
- The virtual copy id is the first 6 hex characters of a random UUID. Inside RapidRAW the copy is
  addressed as `IMG.CR3?vc=<id>`.

Mapping to enframe: a frame is `{x, y, width, height}` in 0–1 of the EXIF-oriented photo. Multiply by
RapidRAW's oriented pixel size (width and height swapped after an odd number of `orientationSteps`).
For a Sony A7 III ARW both apps agree on 6000×4000: enframe's `rawler` 0.7 develop and RapidRAW's
export. The raw header says 6048×4024 before the sensor border is cropped, so scale by the developed
size, never the header.

## UX stories

The goal from #54 is making several versions of one photo faster. The split that fits both apps:
RapidRAW develops the photo (colour, exposure, masks), enframe plans the crops.

1. **Frames become virtual copies.** After framing a photo in enframe at 1:1, 4:5 and 9:16, "Send to
   RapidRAW" writes one `IMG.CR3.<id>.rrdata` per frame: the original's sidecar with `crop` and
   `aspectRatio` replaced. In RapidRAW, after a folder refresh, each crop shows up as its own copy,
   can get its own colour tweak. enframe then renders each frame with
   `RapidRAW export IMG.CR3 --adjustments <frame>.json --output <name>.jpg`, about 1.3 s each. A
   folder export also renders every copy but names them `_edited`, `_edited_VC01`, … in an order
   unrelated to the copy ids. This is the write-back the Lightroom integration never had.
2. **Developed pixels in the framer.** enframe currently shows a RAW with `rawler`'s default develop,
   not the user's edit. Running `RapidRAW export IMG.CR3 --output <cache>.tif` in the background gives
   enframe the edited photo to frame and export. Cost: one full GPU render per photo, and the whole
   RapidRAW app starts for each call: 5.5 s for a 24 MP ARW on the first run, about 1.3 s after.
3. **Open in RapidRAW.** A button in the framer runs `open -a RapidRAW <path>`. With
   `--edit <src> --output <tmp>`, enframe waits for the user to click Done and reloads the rendered
   result, so edit and frame become one loop.
4. **enframe as RapidRAW's "Edit In" target.** The user adds enframe under Edit In; RapidRAW hands over
   a rendered TIFF and enframe opens it in the framer. Needs enframe to accept a file path at launch,
   which it does not today (no argv handling, no single-instance plugin).
5. **RapidRAW folders as a library source.** Read ratings, picks and tags from `.rrdata` (or XMP) to
   filter the library, the way the Lightroom browser filters by rating and flag. No catalog to decode:
   one JSON file per photo.

Recommendation: 1 and 3 first. Story 1 is the only one that saves real time on many versions, and
story 3 is a one-line command.

## Risks

- No format version check: enframe must refuse sidecars with keys or a `version` it does not know,
  rather than write back a file RapidRAW then misreads.
- Writes are invisible until the user refreshes the folder in RapidRAW.
- Run RapidRAW by its real path. Tauri refuses an executable path that goes through a symlink on
  macOS, so `/tmp/…/RapidRAW` panics with `failed to resolve lensfun_db directory: UnknownPath` while
  `/private/tmp/…/RapidRAW` works.
- enframe writes only new virtual copy files and never rewrites the user's own `IMG.CR3.rrdata`, except
  to create it when missing (RapidRAW's own `create_virtual_copy` does the same).
- Open PRs #2038 and #2080 add a storage layer for metadata, with SQLite planned. If sidecars stop being
  the source of truth, stories 1 and 5 break.

## Spike results (2026-10-31)

RapidRAW v1.6.7 `macos-14_aarch64` build, run headless from a scratch folder on an Apple M4 Pro, on
one 24 MP Sony A7 III ARW (landscape, normal orientation).

| Check | Result |
|-|-|
| `RapidRAW export IMG.ARW --output full.tif --format tiff` | 6000×4000 16-bit TIFF, 144 MB. 5.5 s on the first run (4.2 s GPU) |
| Hand-written `IMG.ARW.a1b2c3.rrdata` holding only `crop` and `aspectRatio` | Accepted. Folder export rendered it at 3200×4000 |
| Folder export with copies but no `IMG.ARW.rrdata` | Only the copies render. Writing `{"version":1,"rating":0,"tags":[],"adjustments":{}}` as the original's sidecar makes the original render too |
| Folder export of the original plus two copies | 2.9 s for all three. Output names do not match copy ids |
| `--adjustments frame.json --output frame-4x5.jpg` | 3200×4000 at the chosen path, 1.3 s |
| Sidecars after export | Not rewritten |
| enframe `rawler` 0.7 size for the same file | 6000×4000 developed. Matches |

Not tested yet:
- whether the copies show up in RapidRAW's window after a folder refresh;
- portrait or rotated photos;
- DNG and other cameras;
- what happens when RapidRAW edits a copy enframe wrote.
