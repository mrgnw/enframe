# pinframe UI plan

Written 2026-10-05 against `anchor.py` at commit e030dd8; updated 2026-10-06 to the decisions at the end of this file (card E5). Mockups: `mockups/00-start.html`, `mockups/01-marking.html`, `mockups/02-preview.html`.

## Who it is for, and the one job

One person, with a folder of photos shot from a moving viewpoint (a train window, a walk, a flight), who wants a short clip where one feature stays put. They already know which feature. They are not editing video; they are answering, photo by photo, the single question "where is the cloud in this one?"

The one job: **get from a folder to a pinned clip with as few clicks as there are scene changes.** On the 57-photo Girona set the tracker links 30 photos on its own and 24 need a human. Today each of those 24 is a hand-typed `--at`. The UI exists so each one is a click.

Everything in the design is judged by the cost of one mark. A mark should cost one click, one re-track, and no reading.

## Data model

The UI edits one document and shows one result.

The document is the CLI's input: the ordered photo list, the first photo's reference points, and the marks (`photo → points`, optionally with a zoom). The result is the sidecar JSON the CLI writes: per photo `file`, `x,y,w,h` (the crop, normalized, may overhang 0–1 with padding), `scale`, `source` (`tracked`, `mark`, `lost`) and `points` (`[[x, y], …]`, normalized 0–1 to that photo). The UI never computes a crop or a point; it draws what the sidecar says.

Both are small. The document is a few hundred bytes and is what gets saved beside the folder (`<folder>.pin.json`), so a session can be reopened and the exact CLI call reproduced.

## Screens and states

One window, one photo at a time, a filmstrip below it, controls floating over the photo. No sidebar, no settings page, no project list. Three stages, shown as a three-step strip in the top island: **Points → Fix → Export**. Stage changes are the only navigation.

| State | What the user sees | What advances it |
|-|-|-|
| Empty | A dashed drop zone filling the window, one sentence, a "Choose folder" button. The sentence says what the tool does, in the user's terms. | Drop a folder or files, or pick one. |
| Photos loaded | The first photo fills the stage. Filmstrip shows all photos in filename order, all neutral. Island: "Click points on the feature to hold still." The cursor is a crosshair. | Click one or more points. |
| Points placed | The points on the first photo. Island gains a primary "Track" button and shows the point count. | Press Track or Enter. Adding or removing points before tracking is free. |
| Tracking | Filmstrip fills left to right as results arrive: each thumbnail gets its dot. The stage keeps the first photo. Island shows a thin progress bar and "Tracking 23/57". With cached keypoints this lasts under a second, so there is no cancel button. | Tracking ends. |
| Lost photos to fix | Stage jumps to the first lost photo. Filmstrip shows tracked, marked and lost states (next section). Island: "3 to fix" with a next arrow for the fix list. Reference patch in the corner. | Click the feature. Each click re-tracks and advances. |
| All linked | Same screen; island reads "All 57 linked" with a primary "Preview" button. Lost count is zero but the user may still re-mark any photo. | Preview. |
| Preview | The clip plays in a loop at the chosen fps, drawn by the browser from the sidecar (no encode). Pad and fps controls. Scrubbing the filmstrip scrubs the clip. | Export. |
| Export | A panel with format, destination, optional per-photo PNGs. Progress is the encoder's; ~1 s on the sample. Done state shows the file and a "Reveal" link. | Done; or back to Fix. |
| Error | Inline where the error is, never a modal: a photo that fails to decode is greyed in the filmstrip with a tooltip; a mark outside the photo is refused by the cursor (the stage does not accept clicks outside the image); "ffmpeg not found" or "out path not writable" appear in the Export panel in place of the Export button. Tracker refusals (fewer than 2 photos, mixed folders) appear on the drop zone. | Fixing the cause. |

Tracking after a mark re-runs in JSON-only mode (no encode, cached keypoints, ~0.3–0.5 s). During it the filmstrip keeps the old dots but dims them; dots are repainted when the new sidecar arrives. The stage is already on the next lost photo, so the user can look at it while the re-track runs. If a re-track takes more than one second, the island shows a progress bar; under that, nothing, to avoid flicker.

## Marking a lost photo

The whole product is this loop, so it is specified to the keystroke.

**What the user needs to know before marking:** what the feature looked like in the last photo where it was found. The island shows that as a **reference patch**: the previous linked photo cropped around its points' centre, at thumbnail size, with the points drawn on it. On IMG_4703 that is the cloud from IMG_4702. Without this, the user has to flip back and forth; with it, they just look at the stage and find the same shape.

**What the tracker needs from the user:** where the feature is now, and optionally its zoom. The feature is a set of reference points, not a box. The points tell the tracker roughly what to look for and where; it uses where they were in the last photo as a hint for the next one.

| Gesture | On | Does |
|-|-|-|
| Click | Lost photo | Places the ghost's points with their centre at the click. This is the mark. The usual case. |
| Scroll, or `[` `]`, before clicking | Lost photo | Sets the zoom. The ghost's points spread out or close in. |
| Click on empty photo | Marked photo | Adds one point. The stage stays. |
| Click on a point | Marked photo | Removes it. The last point cannot be removed; "Clear mark" does that. |

The **ghost** makes the zoom visible: the last linked photo's points, as dashed rings around the cursor, spread to the current zoom. Scrolling scales the pattern in steps (1x, 1.5x, 2x, 2.5x, 3x, 5x; `[` and `]` step through the same list; `1` resets). When the rings sit on the same parts of the cloud, the zoom is right. This replaces typing `2.5` and avoids the main cause of wrong marks: a zoomed-in photo marked at 1x.

The tracker's own estimate for a lost photo (interpolated from neighbours) is drawn as faint dashed points. Often it is close, and `Enter` accepts it with no searching.

**Keyboard, complete list:**

| Key | Does |
|-|-|
| `→` `←` | Step one photo. Stops at the ends. |
| `Shift+→` `Shift+←` | Step ten photos. Shift is the 10-frame step in Premiere and Resolve. |
| `Home` `End` | First, last photo |
| `Tab`, `L` | Next photo in the fix list, marked ones included (wraps) |
| `Shift+Tab` | Previous photo in the fix list |
| `T` | Trail on or off; `−` `+` in the island set how many neighbours |
| `Enter` | Accept the tracker's estimate for this photo as a mark (zero mouse) |
| `Backspace`, `Delete` | Clear the mark on this photo, the same as the "Clear mark" button. The photo becomes lost again and joins the fix list. |
| `⌘Z`, `⌘⇧Z` | Undo or redo the last change (mark, point added or removed, mark cleared, "Clear fixed") and go to that photo. History holds 100 steps. |
| `[` `]` `1` | Zoom down, up, reset |
| `Space` | Play or pause the clip in Preview; in Fix, flip to the previous linked photo while held |

**Mouse, complete list:** the gestures above, plus: tracked photos ignore clicks, so scrubbing past them can't create a mark by accident. Press and drag on the filmstrip = scrub; a vertical wheel over the filmstrip scrolls it sideways; click on the next arrow = `Tab`.

**After a mark** the new points pop in place for 450 ms, about the time the re-track takes, and then the stage moves to the next lost photo. The pause is long enough to see where the mark landed, and short enough that 24 marks stay fast. A mark that fixes a scene change usually links several following photos too, so the next lost photo may be far away; the filmstrip scrolls to it.

**The fix list** is every photo that has been lost in this session. Marking a photo does not take it off the list: `Tab` and the next arrow still visit it, so the user can move fast and still jump back to a fixed photo to use it as a reference. Auto-advance skips fixed photos and goes to the next lost one. A "Clear fixed" button in the status island, shown when the list has fixed photos, takes them off the list. Clearing a mark puts the photo back on the list.

**Fixing a mistake.** Every change can be undone, in three ways:

- **Undo toast.** An island reads "Marked IMG_4721 · Undo" for 6 s after each change. Undo puts the photo back as it was, goes back to it, and re-tracks. It is the only undo on a phone, where there is no ⌘Z.
- **⌘Z** does the same at any time, back through the last 100 changes.
- **Go back and redo it.** `←`, Shift+Tab or the filmstrip reach any photo. On a marked photo the island reads "Your mark · 4 points" with a "Clear mark" button. Clicks add and remove points without advancing, so the fix can be checked in place.

**Reaching the 24 marks on the sample:** 24 clicks, 24 re-tracks of under a second each, plus 12 scrolls where the lens changed. Around a minute. The same set costs about 40 minutes with the CLI.

## Showing tracked, marked and lost photos

The filmstrip is the only list. Every photo is a thumbnail, 56 px tall, in one scrollable row. State is one dot under each thumbnail and the thumbnail's opacity:

| Source | Dot | Thumbnail | Meaning |
|-|-|-|-|
| `tracked` | none | full | Linked by the tracker. Nothing to do. |
| `mark`, in the fix list | accent (amber), filled | full | Fixed by the user, still visited by `Tab`. |
| `mark` | accent (amber) ring | full | Placed by the user. Click to revisit. |
| `lost` | red, filled | dimmed to 55% | Needs a mark. The count in the island is the number of these. |
| decode failed | grey X | 30% | Not in the clip. |

No labels, no text per thumbnail. Hovering shows the filename and, for tracked photos, the feature count ("38 features"); for lost ones, "lost, 7 features, estimated".

**Moving through the photos.** The user can go anywhere, not only to the next lost photo, to see where the feature has been. Press and drag along the filmstrip to scrub: the stage switches to each photo the pointer crosses. Near either end of the strip it scrolls on its own, so one drag reaches all 57. A vertical wheel or a trackpad scrolls the strip without changing the photo. The keys are in the table above. Steps and scrubbing keep the current thumbnail in view. Jumps (`Tab`, `Home`, `End`) centre it. A re-track running in the background never blocks scrubbing.

**Which photo is current** is shown three ways: a 2 px amber outline, the thumbnail lifted 4 px above its row, and the filename with its position ("IMG_4721 · 30 / 57") in the top-left island.

A thin line runs above the dots showing which photos are **linked** to each other: a continuous bar across a tracked run, a break at every mark or lost photo. This is the derivative the user actually cares about: how many separate chains the clip has. One chain is the goal.

On the stage, state is the points, with a small tag above them:

- `mark` and `tracked`: solid amber points, outlined dark. Tracked points are smaller and slightly transparent. The tag reads "your mark · 2.55×" or "38 features · 1.00×".
- `lost`: dashed hollow points at the estimated position, the ghost following the cursor, the reference patch in the island.

Confidence (the inlier count) is shown on hover only. It is the tracker's honesty, not a decision input; the decision is binary (lost or not) and the dots show it.

## Neighbour trail

Over the current photo, the trail draws the feature's **path** through up to N photos before it and N after it. The path joins the centres of each photo's points (their mean) in photo order and passes through the current centre, with a dot at each neighbour. On a lost photo, the path shows where the feature was and where it goes next, so the click usually lands where the violet line meets the teal one. On a tracked photo, a drift the tracker missed shows up as a kink.

**Why a path of centres.** The first version drew corner ticks of the followed box for every neighbour. At 2.5x the box covered most of the photo, so the corners landed near the photo edges and read as scattered marks, not a sequence. Centres stay close together whatever the zoom, and a line between them shows order and connection directly. Only the current photo shows its points; neighbours show only their centre.

| Property | Choice | Why |
|-|-|-|
| Colour | Before: violet `#b49cff` at the nearest neighbour, stepping to pink `oklch(0.72 0.17 352)` at the Nth. After: teal `#3fd8c4`, stepping to green `oklch(0.78 0.17 145)`. One colour per photo, interpolated in OKLCH. | Distinct from amber, which is the current points and marks, and from red, which is lost. All steps are light, so they stand out on dark foliage by themselves; the halo covers bright sky and cloud. |
| Line | 2.5 px on a 6.5 px halo of 70% black. Every dot sits on a dark disc 2.5 px wider than it. A segment takes the colour of its farther end. | The line is the strongest element of the trail, because the connection is what the user reads. |
| Default N | 6 per side, adjustable from 5 to 10 with `−` `+`. `T` turns the trail off. | The Girona set's 57 photos form 27 linked chains, so most chains are one to three photos long. At 6, the path crosses a scene change or two on each side, which is where lost photos are. |
| Cost | One SVG over the photo: at most 20 line segments, 12 dots and the current photo's points. It is redrawn on photo change and on resize. | No images and no blur, so scrubbing stays instant. |

**Readable on any photo** (card E5, round 2). The Girona photos run from white cloud to dark foliage and rooftops, and at 2.5x the trail crosses most of the photo. Three treatments were compared on those photos: a stronger halo, a two-tone casing (dark outside, white rim, colour core, as on route maps), and a drop shadow on the whole overlay. The stronger halo won. The two-tone casing was a little crisper on rooftops but heavier and needed a third stroke. The drop shadow was too faint on white cloud. The fade had done more damage than the thin halo: at 0.35 the far segments vanished, and a halo that did not fade with its line turned them into black lines. So the halo now fades with its line, only to 0.55. The current photo's points have a 2 px dark outline. Estimated points are drawn larger (6 px) on a darker fill, because they are what `Enter` accepts.

**Distance as colour** (card E5, round 3). The round-2 fade, opacity from 1 to 0.55, told near from far only weakly: where the path folds back on itself, as at 2.5x on IMG_4721, the near and far segments looked alike. Each segment now gets one colour on a stepped ramp, at full opacity, so the order of the photos and their distance from the current one read at a glance, and the far end is as visible as the near one. Before and after stay two families (violet to pink, teal to green) rather than one ramp through the current photo, so the side is still readable from colour alone, as the legend says. Hue changes more than lightness, because darker steps would sink into foliage; the far "after" end stops at green so it stays clear of amber. At N = 10 two neighbouring steps differ only slightly, but the ramp's direction still shows. The mockup compares three modes with `?trail=fade|steps|pulse`; `steps` is the default. `pulse` adds a white light that lightens one segment at a time, oldest first, every 420 ms, then rests for three steps; it is off under `prefers-reduced-motion`. It shows the direction of travel, but it draws the eye while marking, so it stays a proposal for the user to judge.

How the source changes the trail:

| Source | Dot | Line into it | Meaning |
|-|-|-|-|
| `tracked` | Filled | Solid | Seen by the tracker |
| `mark` | Large ring | Solid | Placed by the user. A long segment ending in a ring is a scene change. The line keeps going through it; if those segments read as noise, they become dotted. |
| `lost` | Small, hollow, dashed | Dashed | Interpolated from neighbours, never seen |

**Coordinates.** Each neighbour's centre is drawn at its own position in its own photo, normalized 0–1, so photos of different resolutions line up. The trail is then the feature's path through the frame. A neighbour of the other orientation, such as a landscape photo next to a portrait one, is placed by its normalized position, which is still right.

### Points data

The sidecar's `points` (card E1) is all the marking screen needs: per photo, the reference points normalized 0–1 to that photo. The centre is their mean; the zoom is `scale`. Today the tracker follows one point, so `points` holds one pair. Card A5 makes it several.

```json
{"file": "IMG_4720.avif", "x": -0.8613, "y": 0.0092, "w": 2.368, "h": 1.4717, "scale": 2.5543, "source": "tracked", "points": [[0.3107, 0.4512]]}
```

`analysis/mockup_data.py` turns the sidecar into the mockup's `photos.js` and checks each point against the anchor the CLI prints. Until A5, the mockup simulates three more points around the real one: a fixed pattern in pixels at scale 1, grown by each photo's `scale`, centred on the real point so the trail is unchanged. The code marks them as simulated.

## Phone layout

Below 700 px wide, the islands stop floating over the photo. They stack in a dock above it and a dock below it, so nothing covers the photo or another island. The first mockup overlapped the wordmark, the steps and the status on a phone, and the trail control sat on top of the reference patch.

- **Top dock:** the position ("30 / 57"), the lost count with the next-lost arrow, and Preview. The wordmark, the folder name and the Points/Fix/Export steps are dropped; Preview is the next step.
- **Bottom dock:** a compact reference patch with "Looking for IMG_4720 · 2.55×", prev and next buttons in place of the arrow keys, then the trail and zoom steppers. Zoom uses `−` `+` buttons because there is no scroll wheel.
- **Photo:** sized to the space left between the docks, so it is never cut off. A tap marks, or adds or removes a point on a marked photo. The undo toast sits at the top of the photo, clear of the thumb.
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
- enframe already has the pieces the UI needs: normalized 0–1 coordinates, point placement (`DraggableFocalPoint`), a floating pill toolbar, AVIF decode, sidecar export. The pinframe route reuses them and adds a filmstrip, the points and the marking gestures.

What the route does per interaction: `POST /pinframe` with `{photos, points, marks, pad, fps}` returns the sidecar JSON (JSON-only mode, no encode). `POST /pinframe/export` with the same body plus `out` and `frames` runs the full render and returns the path. Photos are served from the read root as they are today in enframe. Keypoint caching lives in `anchor.py` and is keyed by photo path and size, so repeated calls are cheap.

What this costs: the static web build (`enframe` on Cloudflare) does not get pinframe. That matches its "no backend" contract and the spike's conclusion.

Alternatives rejected:

- **Separate Tauri app with a Python sidecar.** Bundling opencv-python is tens of MB and a second toolchain; it reaches desktop only. enframe's Tauri shell already ships `enframe-serve`, so the route gives desktop for free.
- **Rust port in WASM.** The only OpenCV-compatible SIFT crate is a 0.1.0 with 8 stars. A week of work plus validation, for a slower result.
- **Standalone web app with its own server.** Duplicates file-root confinement, AVIF decode, and the point placement that enframe already has.

## Deliberately left out

- **`--vf` filters.** Deflicker, blend, and hold-last-frame are ffmpeg knobs for after the clip exists. The CLI keeps them. If one earns a place, it is "hold last frame for N s" as a single field, nothing else.
- **Multiple features.** One feature per clip, marked with as many points as help. Two features held still at once is a different product (a stabiliser).
- **Reordering, excluding or rotating photos.** Order is filename order, as in the CLI. Excluding a photo is a folder operation.
- **Smoothing, soft lock, same-size versus full-frame.** README next steps, not UI. When they exist they are one switch each in the Preview island.
- **A library, projects, history.** The session file beside the folder is the project. Reopening a folder reopens the session.
- **Confidence tuning** (`--min-matches`, `--max-zoom`, `--max-turn`). Not CLI flags today either.
- **Pinch and hover on phones.** Pinch is left to the browser for inspecting the photo, not used to set the zoom. Feature counts and filenames that desktop shows on hover are not shown on a phone.
- **Frame-style settings depth.** Frame's polish comes from its density of correct detail in one panel. Here the panel has five controls, and the polish goes into the marking loop.

## Decisions (2026-10-06)

1. **Points, not boxes** (questions 1 and 8). Done in the mockup (E5). The user marks a feature with one or more reference points; nothing on screen is a box. The UI shows every reference point in every photo. The tracker uses where the points were in the last photo as a hint for where to look in the next one. Still one feature per clip: several points on the same cloud, not several anchors. The sidecar gets `points` per photo, `[x, y]` pairs normalized 0–1 (card E1, one point today; card A5, several). Box semantics no longer matter for the UI; the CLI's `--box` is gone too, replaced by `--points`/`--at` (A5b).
2. **Zoom from the drag.** Moot: there is no box to drag. Zoom stays its own gesture.
3. **Session file.** Yes, `<folder>.pin.json` beside the photos. The server turns it into CLI flags; no `--session` flag.
4. **Lost-photo policy.** Not in the first UI. A later CLI flag if wanted.
5. **Preview fidelity.** A small difference between preview and export is accepted.
6. **EXIF zoom.** Pre-set the zoom from focal length and keep the gesture for corrections. Confirm once B4 measures it.
7. **Where in enframe.** A `/pin` route; the framer stays untouched.
8. See 1.
9. **Trail across scene changes.** Keep drawing through a mark that starts a new scene. Switch those segments to dotted if they read as noise.
10. **After a mark.** Done in the mockup (E5). Auto-advance stays (450 ms). Marking a photo does not remove it from the fix list, so `Tab` and the next arrow still visit it and it can be used as a reference. A "Clear fixed" button removes the marked photos from the list.
