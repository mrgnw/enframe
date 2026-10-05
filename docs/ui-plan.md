# pinframe UI plan

Written 2026-10-05 against `anchor.py` at commit e030dd8. Mockups: `mockups/00-start.html`, `mockups/01-marking.html`, `mockups/02-preview.html`.

## Who it is for, and the one job

One person, with a folder of photos shot from a moving viewpoint (a train window, a walk, a flight), who wants a short clip where one feature stays put. They already know which feature. They are not editing video; they are answering, photo by photo, the single question "where is the cloud in this one?"

The one job: **get from a folder to a pinned clip with as few clicks as there are scene changes.** On the 57-photo Girona set the tracker links 30 photos on its own and 24 need a human. Today each of those 24 is a hand-typed `--at`. The UI exists so each one is a click.

Everything in the design is judged by the cost of one mark. A mark should cost one click, one re-track, and no reading.

## Data model

The UI edits one document and shows one result.

The document is the CLI's input: the ordered photo list, the first-photo box, and the marks (`photo → point | box | box+zoom`). The result is the sidecar JSON `anchor.py` already writes: per photo `file`, `x,y,w,h` (normalized, may overhang 0–1 with padding), `scale`, `source` (`tracked`, `mark`, `lost`). The UI never computes a crop; it draws what the sidecar says.

The marking screen also needs each photo's anchor box, which the sidecar does not have yet. "Anchor box data" below gives the field and a derivation that works until it is added.

Both are small. The document is a few hundred bytes and is what gets saved beside the folder (`<folder>.pin.json`, proposed), so a session can be reopened and the exact CLI call reproduced.

## Screens and states

One window, one photo at a time, a filmstrip below it, controls floating over the photo. No sidebar, no settings page, no project list. Three stages, shown as a three-step strip in the top island: **Box → Fix → Export**. Stage changes are the only navigation.

| State | What the user sees | What advances it |
|-|-|-|
| Empty | A dashed drop zone filling the window, one sentence, a "Choose folder" button. The sentence says what the tool does, in the user's terms. | Drop a folder or files, or pick one. |
| Photos loaded | The first photo fills the stage. Filmstrip shows all photos in filename order, all neutral. Island: "Drag a box around the feature to hold still." The cursor is a crosshair. | Drag a box. |
| Box drawn | Box with corner handles on the first photo. Island gains a primary "Track" button and shows the box size. | Press Track or Enter. Resizing the box before tracking is free. |
| Tracking | Filmstrip fills left to right as results arrive: each thumbnail gets its dot. The stage keeps the first photo. Island shows a thin progress bar and "Tracking 23/57". With cached keypoints this lasts under a second, so there is no cancel button. | Tracking ends. |
| Lost photos to fix | Stage jumps to the first lost photo. Filmstrip shows tracked, marked and lost states (next section). Island: "3 to fix" with a next-lost button. Reference patch in the corner. | Click the feature. Each click re-tracks and advances. |
| All linked | Same screen; island reads "All 57 linked" with a primary "Preview" button. Lost count is zero but the user may still re-mark any photo. | Preview. |
| Preview | The clip plays in a loop at the chosen fps, drawn by the browser from the sidecar (no encode). Pad and fps controls. Scrubbing the filmstrip scrubs the clip. | Export. |
| Export | A panel with format, destination, optional per-photo PNGs. Progress is the encoder's; ~1 s on the sample. Done state shows the file and a "Reveal" link. | Done; or back to Fix. |
| Error | Inline where the error is, never a modal: a photo that fails to decode is greyed in the filmstrip with a tooltip; a mark outside the photo is refused by the cursor (the stage does not accept clicks outside the image); "ffmpeg not found" or "out path not writable" appear in the Export panel in place of the Export button. Tracker refusals (fewer than 2 photos, mixed folders) appear on the drop zone. | Fixing the cause. |

Tracking after a mark re-runs in JSON-only mode (planned: no encode, cached keypoints, ~0.3–0.5 s). During it the filmstrip keeps the old dots but dims them; dots are repainted when the new sidecar arrives. The stage is already on the next lost photo, so the user can look at it while the re-track runs. If a re-track takes more than one second, the island shows a progress bar; under that, nothing, to avoid flicker.

## Marking a lost photo

The whole product is this loop, so it is specified to the keystroke.

**What the user needs to know before marking:** what the feature looked like in the last photo where it was found. The island shows that as a **reference patch**: the anchor box cropped from the previous linked photo, at thumbnail size. On IMG_4703 that is the cloud from IMG_4702. Without this, the user has to flip back and forth; with it, they just look at the stage and find the same shape.

**What the tracker needs from the user:** a point, optionally a search box, optionally a zoom. The three CLI forms map to three gestures on the same stage, in increasing effort:

| Gesture | CLI form | When |
|-|-|-|
| Click | `--at P=x,y` | The usual case. The feature is at the previous zoom and there is nothing confusable around it. |
| Drag a box | `--at P=x,y,w,h` | A small or distant feature with clutter near it. The box is the search window; the anchor is its centre. |
| Scroll or `[` `]` before clicking, or pinch | `--at P=x,y,w,h,zoom` | The photo was shot on a different lens. The ghost box grows or shrinks; match it to the feature and click. |

The **ghost** makes the zoom visible: a dashed outline the size of the anchor box at the current zoom follows the cursor on a lost photo. Scrolling scales it in steps (1x, 1.5x, 2x, 2.5x, 3x, 5x; `[` and `]` step through the same list; `1` resets). When the ghost is the size of the cloud, the zoom is right. This replaces typing `2.5` and avoids the main cause of wrong marks: a zoomed-in photo marked at 1x.

The tracker's own estimate for a lost photo (interpolated from neighbours) is drawn as a faint box too. Often it is close, and a click near it confirms the estimate with no searching.

**Keyboard, complete list:**

| Key | Does |
|-|-|
| `→` `←` | Step one photo. Stops at the ends. |
| `Shift+→` `Shift+←` | Step ten photos. Shift is the 10-frame step in Premiere and Resolve. |
| `Home` `End` | First, last photo |
| `Tab`, `L` | Next lost photo (wraps) |
| `Shift+Tab` | Previous lost photo |
| `T` | Trail on or off; `−` `+` in the island set how many neighbours |
| `Enter` | Accept the tracker's estimate for this photo as a mark (zero mouse) |
| `Backspace`, `Delete` | Clear the mark on this photo, the same as the "Clear mark" button. The photo becomes lost again. |
| `⌘Z`, `⌘⇧Z` | Undo or redo the last mark, move or clear, and go to that photo. History holds 100 steps. |
| `[` `]` `1` | Zoom down, up, reset |
| `Space` | Play or pause the clip in Preview; in Fix, flip to the previous linked photo while held |
| `Esc` | Cancel a drag in progress |

**Mouse, complete list:** click = point mark; drag = search box; scroll = zoom. On a lost photo these place a mark. On a marked photo they move it. Tracked photos ignore clicks, so scrubbing past them can't create a mark by accident. Press and drag on the filmstrip = scrub; a vertical wheel over the filmstrip scrolls it sideways; click on the next-lost arrow = `Tab`.

**After a mark** the new box pops in place for 450 ms, about the time the re-track takes, and then the stage moves to the next lost photo. The pause is long enough to see where the mark landed, and short enough that 24 marks stay fast. A mark that fixes a scene change usually links several following photos too, so the next lost photo may be far away; the filmstrip scrolls to it.

**Fixing a mistake.** Every mark, move and clear can be undone, in three ways:

- **Undo toast.** An island reads "Marked IMG_4721 · Undo" for 6 s after each change. Undo puts the photo back as it was, goes back to it, and re-tracks. It is the only undo on a phone, where there is no ⌘Z.
- **⌘Z** does the same at any time, back through the last 100 changes.
- **Go back and redo it.** `←`, Shift+Tab or the filmstrip reach any photo. On a marked photo the island reads "Your mark" with a "Clear mark" button, and a new click or drag moves the mark without advancing, so the fix can be checked in place.

**Reaching the 24 marks on the sample:** 24 clicks, 24 re-tracks of under a second each, plus 12 scrolls where the lens changed. Around a minute. The same set costs about 40 minutes with the CLI.

## Showing tracked, marked and lost photos

The filmstrip is the only list. Every photo is a thumbnail, 56 px tall, in one scrollable row. State is one dot under each thumbnail and the thumbnail's opacity:

| Source | Dot | Thumbnail | Meaning |
|-|-|-|-|
| `tracked` | none | full | Linked by the tracker. Nothing to do. |
| `mark` | accent (amber) ring | full | Placed by the user. Click to revisit. |
| `lost` | red, filled | dimmed to 55% | Needs a mark. The count in the island is the number of these. |
| decode failed | grey X | 30% | Not in the clip. |

No labels, no text per thumbnail. Hovering shows the filename and, for tracked photos, the feature count ("38 features"); for lost ones, "lost, 7 features, estimated".

**Moving through the photos.** The user can go anywhere, not only to the next lost photo, to see where the anchor has been. Press and drag along the filmstrip to scrub: the stage switches to each photo the pointer crosses. Near either end of the strip it scrolls on its own, so one drag reaches all 57. A vertical wheel or a trackpad scrolls the strip without changing the photo. The keys are in the table above. Steps and scrubbing keep the current thumbnail in view. Jumps (`Tab`, `Home`, `End`) centre it. A re-track running in the background never blocks scrubbing.

**Which photo is current** is shown three ways: a 2 px amber outline, the thumbnail lifted 4 px above its row, and the filename with its position ("IMG_4721 · 30 / 57") in the top-left island.

A thin line runs above the dots showing which photos are **linked** to each other: a continuous bar across a tracked run, a break at every mark or lost photo. This is the derivative the user actually cares about: how many separate chains the clip has. One chain is the goal.

On the stage, state is the box:

- `mark` and `tracked`: a solid accent box at the anchor, with the anchor point marked at its centre. Tracked boxes are a thinner stroke.
- `lost`: a dashed grey box at the estimated position, the ghost following the cursor, the reference patch in the island.

Confidence (the inlier count) is shown on hover only. It is the tracker's honesty, not a decision input; the decision is binary (lost or not) and the dots show it.

## Neighbour trail

Over the current photo, the trail draws the anchor's **path** through up to N photos before it and N after it. A line joins the anchor centres in photo order and passes through the current anchor, with a dot at each neighbour. Box corners are drawn only on the nearest neighbour each side. On a lost photo, the path shows where the anchor was and where it goes next, so the click usually lands where the violet line meets the teal one. On a tracked photo, a drift the tracker missed shows up as a kink.

**Why a path, not boxes.** The first version drew corner ticks for every neighbour. At 2.5x the followed box covers most of the photo, so twelve sets of corners landed near the photo edges, far from each other. They read as scattered marks, not a sequence. The phone screenshot of IMG_4725 showed this. Centres stay close together whatever the box size, and a line between them shows order and connection directly.

| Property | Choice | Why |
|-|-|-|
| Colour | Before: violet `#b49cff`. After: teal `#3fd8c4`. | Distinct from amber, which is the current box and marks, and from red, which is lost. A dark halo under every line and tick keeps both readable on sky, cloud and foliage. |
| Line | 2.5 px on a 5.5 px dark halo. Each segment takes the opacity of its farther end. | The line is the strongest element of the trail, because the connection is what the user reads. |
| Opacity | Linear, from 0.95 at the nearest neighbour to 0.35 at the Nth. | The nearest neighbour is the one the click depends on, and the far end must stay visible. |
| Corners | Nearest neighbour each side only. Ticks are 18% of the box's shorter side, clamped to 5–14 px. | Box size matters most next to the current photo, which is where a lens switch shows. More boxes turn back into scatter. |
| Default N | 6 per side, adjustable from 5 to 10 with `−` `+`. `T` turns the trail off. | The Girona set's 57 photos form 27 linked chains, so most chains are one to three photos long. At 6, the path crosses a scene change or two on each side, which is where lost photos are. |
| Cost | One SVG over the photo: at most 20 line segments, 12 dots and 2 sets of corners. It is redrawn on photo change and on resize. | No images and no blur, so scrubbing stays instant. |

How the source changes the trail:

| Source | Dot | Line into it | Meaning |
|-|-|-|-|
| `tracked` | Filled | Solid | Seen by the tracker |
| `mark` | Large ring | Solid | Placed by the user. A long segment ending in a ring is a scene change. |
| `lost` | Small, hollow, dashed | Dashed | Interpolated from neighbours, never seen |

**Coordinates.** Each neighbour's centre and box are drawn at their own position in their own photo, normalized 0–1, so photos of different resolutions line up. The trail is then the anchor's path through the frame. A lens switch shows up as a jump in box size, which is useful information. In the mockup, the box goes from 0.35 to 0.90 of the photo width between IMG_4719 and IMG_4720, where the lens switches to 2.55x. Two side effects follow from this choice:

- A neighbour of the other orientation, such as a landscape photo next to a portrait one, is drawn stretched. Its position is still right.
- At large zoom a box can extend past the photo. Its ticks are drawn outside the photo edge rather than clipped, because the size is the information.

### Anchor box data

Per photo, the UI needs the anchor centre and the box the tracker followed, both normalized 0–1 to that photo. The sidecar has neither. The proposed addition to `anchor.py` is one field per row, with the centre x, centre y, width and height in the same units as the crop rect:

```json
{"file": "IMG_4720.avif", "x": -0.8613, "y": 0.0092, "w": 2.368, "h": 1.4717, "scale": 2.5543, "source": "tracked", "anchor": [0.3107, 0.4512, 0.898, 0.2245]}
```

The change is small. `track()` already holds the box as `rw, rh`, so it needs adding to `Hit` and writing out in `write_rects`.

The box is **not** "the first box × scale". `track()` replaces `rw, rh` with each `--at` box and multiplies them by every tracked zoom step. So the followed box equals the first box × scale only until the first box mark. For the trail, the followed box is the right one: it is what the user last drew and what the tracker matched.

**Derivation until the field exists.** `analysis/mockup_data.py` does this, and its centres match the anchors `anchor.py` prints to within 1.5 px on all 57 photos.

1. **Centre.** The crop holds the anchor at one spot of the clip, so the anchor sits at the same fraction (u, v) of every crop rect. Photo 0's anchor is the centre of `--box bx,by,bw,bh`. With W0 and H0 as photo 0's pixel size:
   - u = (bx + bw/2 − x0·W0) / (w0·W0)
   - v = (by + bh/2 − y0·H0) / (h0·H0)
   - For photo i: cx = xᵢ + u·wᵢ and cy = yᵢ + v·hᵢ.
2. **Size.** The followed box divided by scale stays constant between box marks. Start with (bw, bh) / 1. At each `--at P=x,y,w,h[,zoom]`, it becomes (w, h) / scale_P. Photo i's box is that ratio × scaleᵢ, divided by photo i's pixel size.

The derivation needs the marks and the photo sizes. The UI has both, because they are its own document. It is a stopgap, not the design, for two reasons:

- It holds only while the crop is a pure scale and shift of the anchor. The README's "soft lock" next step would let the anchor drift inside the crop and break step 1.
- Lost photos use the sidecar's interpolated scale, so their size is an estimate, like their position.

## Phone layout

Below 700 px wide, the islands stop floating over the photo. They stack in a dock above it and a dock below it, so nothing covers the photo or another island. The first mockup overlapped the wordmark, the steps and the status on a phone, and the trail control sat on top of the reference patch.

- **Top dock:** the position ("30 / 57"), the lost count with the next-lost arrow, and Preview. The wordmark, the folder name and the Box/Fix/Export steps are dropped; Preview is the next step.
- **Bottom dock:** a compact reference patch with "Looking for IMG_4720 · 2.55×", prev and next buttons in place of the arrow keys, then the trail and zoom steppers. Zoom uses `−` `+` buttons because there is no scroll wheel.
- **Photo:** sized to the space left between the docks, so it is never cut off. A tap marks a point and a one-finger drag marks a search box. The undo toast sits at the top of the photo, clear of the thumb.
- **Filmstrip:** 48 px thumbnails, scrubbed by dragging, with bottom padding for the iPhone home indicator so Safari's toolbar doesn't cover it.
- **Touch targets:** buttons are 36 px inside 44 px islands.
- **Preview screen:** the clip, the transport controls and the export panel stack in one scrolling column.

## Preview and export

Preview draws the clip in the browser: each photo is drawn into a canvas at the sidecar's crop, the same operation as `place()`. No encode, so preview is live the moment the sidecar returns, and changing pad colour or fps re-draws instantly. The sidecar is the contract; if the browser and ffmpeg ever disagree, the sidecar is wrong.

Controls, all in one island:

- **Pad**: none, black, white, transparent, custom colour. "None" shrinks the crop (the CLI default); any other value overhangs. Transparent is drawn on a checkerboard.
- **fps**: 2, 4, 8, 12, or a number field. Default 4.
- **Format**: `.avif` (default; keeps transparency, plays in a browser as an image), `.mp4`, `.mov` (ProRes 4444 when transparent), `.webm`. The format list greys out formats that cannot carry transparency when pad is transparent, with the reason on hover.
- **Also save PNG frames** checkbox (`--frames`).
- **Out**: a path field defaulting to `<folder>/<folder>.avif`. The sidecar is written beside it; the UI says so in one line.

`--vf` is not in the UI. See "Left out".

## Platform

**Recommendation: a route inside enframe (`/pin`), served by `enframe-serve`, with `anchor.py` as a subprocess.** Not a Tauri app of its own, not a browser port, not the static web build.

Why, tied to the WASM spike (`git show spike-wasm:docs/wasm.md`):

- Tracking in the browser is 12x slower than native (0.8 s per photo in Pyodide, ~45 s for the sample) and costs a 26 MB download. The marking loop above assumes a re-track under a second. Only native OpenCV delivers that.
- The browser cannot write the default output. WebCodecs has no alpha in Chromium, so `--pad transparent` would need ffmpeg.wasm (32 MB, GPL, needs cross-origin isolation that enframe's static site turns off).
- `enframe-serve` already exists, already runs on the Mac mini, already confines file access to read and write roots, and is already the sidecar inside the Tauri desktop app. One `POST /pinframe` route that spawns `uv run anchor.py` covers both the desktop app and the headless-serve mode with no new runtime.
- enframe already has the pieces the UI needs: normalized 0–1 coordinates, a box tool with handles (`ResizableFrame`), point placement (`DraggableFocalPoint`), a floating pill toolbar, AVIF decode, sidecar export. The pinframe route reuses them and adds a filmstrip and the marking gestures.

What the route does per interaction: `POST /pinframe` with `{photos, box, marks, pad, fps}` returns the sidecar JSON (JSON-only mode, no encode). `POST /pinframe/export` with the same body plus `out` and `frames` runs the full render and returns the path. Photos are served from the read root as they are today in enframe. Keypoint caching lives in `anchor.py` and is keyed by photo path and size, so repeated calls are cheap.

What this costs: the static web build (`enframe` on Cloudflare) does not get pinframe. That matches its "no backend" contract and the spike's conclusion.

Alternatives rejected:

- **Separate Tauri app with a Python sidecar.** Bundling opencv-python is tens of MB and a second toolchain; it reaches desktop only. enframe's Tauri shell already ships `enframe-serve`, so the route gives desktop for free.
- **Rust port in WASM.** The only OpenCV-compatible SIFT crate is a 0.1.0 with 8 stars. A week of work plus validation, for a slower result.
- **Standalone web app with its own server.** Duplicates file-root confinement, AVIF decode, and the box tool that enframe already has.

## Deliberately left out

- **`--vf` filters.** Deflicker, blend, and hold-last-frame are ffmpeg knobs for after the clip exists. The CLI keeps them. If one earns a place, it is "hold last frame for N s" as a single field, nothing else.
- **Multiple anchors.** One feature per clip. Two anchors is a different product (a stabiliser).
- **Reordering, excluding or rotating photos.** Order is filename order, as in the CLI. Excluding a photo is a folder operation.
- **Smoothing, soft lock, same-size versus full-frame.** README next steps, not UI. When they exist they are one switch each in the Preview island.
- **A library, projects, history.** The session file beside the folder is the project. Reopening a folder reopens the session.
- **Confidence tuning** (`--min-matches`, `--max-zoom`, `--max-turn`). Not CLI flags today either.
- **Pinch and hover on phones.** Pinch is left to the browser for inspecting the photo, not used to set the zoom. Feature counts and filenames that desktop shows on hover are not shown on a phone.
- **Frame-style settings depth.** Frame's polish comes from its density of correct detail in one panel. Here the panel has five controls, and the polish goes into the marking loop.

## Open questions

1. **Box semantics.** `--box` and the `--at` box both give a top-left corner. The prior-art survey proposes centre-based boxes so `--at P=1000,200` and `--at P=1000,200,300,300` agree on the anchor. The UI draws boxes around a centre regardless; should `anchor.py` change to match, or should the UI convert?
2. **Zoom from the drag.** When the user drags a search box on a lens-switch photo, should its size relative to the first box set the zoom automatically, so the scroll gesture is rarely needed? It would save a step but conflates search area with feature size. Proposed: no; keep the ghost.
3. **Session file.** Is `<folder>.pin.json` the right place, and should it be the CLI's input too (`anchor.py --session file.pin.json`), so the UI and the CLI are two front ends to one file?
4. **Lost-photo policy.** Lost photos stay in the clip at an estimated position today. Should the UI offer "drop lost photos" as an export option (`--drop-lost` in the survey), or is that a later CLI flag?
5. **Preview fidelity.** The browser draws the preview with bilinear resampling and ffmpeg with area or cubic. Is a visible difference acceptable in preview, or should preview fetch `--frames` PNGs from the server at reduced size?
6. **EXIF zoom.** When photos carry focal length, should the UI pre-set the ghost zoom from it, or hide the zoom gesture entirely on those sets?
7. **Where in enframe.** A route at `/pin` next to the framer at `/`, or a mode of the framer reached from a multi-select in the library?
8. **Sidecar `anchor` field.** Is `[cx, cy, w, h]` per row, normalized to the photo, the right shape? Adding it to `anchor.py` removes the derivation and its soft-lock caveat.
9. **Trail across scene changes.** Should the trail stop at a mark that starts a new scene, since positions from another scene are not comparable? The proposal is no: a long segment ending in a mark's ring already shows where the scene changes, and stopping would hide lens switches that happen at a mark, such as IMG_4698 at 2.5x. On IMG_4725 that segment crosses most of the photo; if that reads as noise in use, draw segments into a mark dotted instead of stopping.
10. **Auto-advance delay.** Is 450 ms after a mark the right pause? A longer one shows the mark more clearly; a shorter one makes 24 marks faster. The undo toast covers mistakes either way.
