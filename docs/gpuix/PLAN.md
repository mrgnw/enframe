# Enframe on gpuix-svelte: staged investigation

Status: plan, 2026-11-24. No app code changes. Each stage is a small spike that answers one question. A failed stop criterion ends the investigation unless stated otherwise.

gpuix-svelte is a Svelte custom renderer that draws into a native window through GPUI, the GPU user-interface framework behind the Zed editor. The app's JavaScript runs in Node or Bun, not in a browser.

## What Enframe is, and what would change

Enframe marks focal points on a photo, fits crop frames for five destinations (9:16, 16:9, 4:5, 1.91:1, 1:1), and exports them as AVIF, JPG or PNG. It also browses folders and a local Lightroom catalog, and stabilizes a photo series into an `.mp4` clip. One Svelte 5 / SvelteKit interface (about 10,600 lines of `.svelte`) runs in three modes: a Tauri 2 desktop app, a browser served by `enframe-serve`, and a static web build of the framer alone.

| Part | Today | With a GPUI desktop app |
|-|-|-|
| Window, WebView, dialogs, file drop | Tauri 2 (`src-tauri/src`) | Replaced by gpuix-svelte |
| Markup, CSS, `bits-ui`, `<canvas>`, `<video>`, `ResizeObserver`, web workers, `localStorage` | `src/lib/**/*.svelte` | Rewritten or dual-targeted; none of these exist in GPUI |
| Pure logic: fit engine, history, sessions, geometry | `src/lib/*.ts`, `*.svelte.ts` | Kept, if free of DOM calls (unverified per file) |
| Database, image decode (`/img`), RAW, JPEG XL, face detection, Stabilize tracking, export, `.mp4` | Rust: `enframe-serve` (axum, port 5911), `stabilize`, `faces` | Kept unchanged; reached over loopback HTTP |
| Browser serve mode and static web build | Same Svelte interface | Kept. The interface then exists twice unless components render on both targets |

The last row is the main cost. A GPUI app replaces one of three modes.

## Facts so far

Verified in `experiments/gpuix-framer` on this branch:

- A gpuix-svelte 0.6.0 prototype renders a JPEG in a native macOS window. Dragging a focal point moves a square crop preview through GPUI layout. Scripted checks pass: geometry, headless, and native with synthetic pointer input.
- During a macOS window-edge drag, GPUI's `tick()` blocks inside AppKit's resize tracking loop. Node callbacks (`onlayout`, window-size polls) run only after mouseup. Layout expressed natively (aspect-ratio frame, percentage anchoring) follows live. Live JavaScript logic during resize needs an upstream change.
- The photo kept its ratio only because GPUI's layout engine (Taffy) shrinks width when `max-height` clamps. The same CSS behaves differently in a browser. Existing Svelte/CSS cannot be assumed to carry over.
- Still pending: the hand test of focal drag, release outside the photo, and focal position after resize.
- Unexplained: the native check's `move→laid-out ms` metric rose from about 15 ms to a median of about 1013 ms in one review run.

From upstream docs and the GitHub API on 2026-11-24, not tested by us:

- Elements: `div`, `text`, `img`, `svg`, `input`, `textarea`, `virtual-list`, `button` and a few more. Layout is flexbox plus a limited grid.
- Not supported: `transform`, `transition`, `z-index`, `box-shadow`, gradients, descendant selectors, `bind:`. Enframe uses the first five in 29 files (216 matches).
- Images: `<img src>` takes a file path, `data:` URL or http(s) URL. `set_image_pixels` takes RGBA or BGRA buffers. Listed formats are PNG, JPEG, WebP, GIF, SVG. No AVIF, JPEG XL, HEIC or RAW. No canvas or video element.
- Desktop: open-file picker without type filter, file drop, window title/fullscreen/zoom. Save dialog, app menus, clipboard API and multiple windows are not documented.
- Shipping: `.app` of about 110 MB bundling Bun; signing and notarization through environment variables; macOS Apple Silicon only on Mac.
- Accessibility: `role` and some `aria-*` reach VoiceOver. `aria-live`, `aria-modal`, `aria-checked`, `aria-disabled`, `aria-hidden` do not.
- Project: MIT, created 2026-08-30, 485 stars, one human author (176 of 187 commits), four releases in the last 24 days. Its README says "Experimental. Expect breaking changes between minor versions." It bundles a Svelte build from an unmerged custom-renderer pull request. The pull request the README links (sveltejs/svelte #18511) shows as closed and not merged; whether a successor exists is unverified.

## Stages

Order: cheapest and most decisive first. Effort is focused working time. "Baseline" means the same action measured in the current Tauri app on the same Mac and file.

Agent types: Claude Opus 5.5 for Svelte spikes needing judgment, Claude Sonnet 5.5 for settled mechanical work, GPT profile for research, Rust and written verdicts. Each diff is reviewed by the other provider.

### 0. Close out the prototype

- Question: does the prototype behave under a real pointer, and is the 1013 ms reading real?
- Spike: hand test the three pending items with mouse and trackpad. Run the native check 10 times with the host pointer idle and record the metric per run. If it reproduces, time the automation call and the screenshot call separately.
- Pass: crop follows the hand without visible stall; release outside the photo ends the drag; marker stays on the same photo spot after edge resize; median metric at or below 33 ms, or the delay is traced to the test harness.
- Stop: visible stall under a real pointer, or more than 100 ms median traced to the runtime.
- Effort: 1 to 2 hours.
- Who: Morgan for the hand test. Claude Sonnet 5.5 for the metric.

### 1. Can we survive upstream stopping?

- Question: is a one-author, three-month-old runtime on an unmerged Svelte API an acceptable base?
- Spike: no app code. Build `@gpuix/native` and gpuix-svelte from source and run the prototype's three checks against that build. Find the current state of Svelte's custom-renderer API. Record which GPUI revision the native addon pins.
- Pass: source build passes the checks within half a day; the Svelte API has a live pull request or release path; all licenses are permissive.
- Stop: cannot build from source, or the Svelte custom-renderer work is abandoned.
- Effort: half a day.
- Who: GPT profile.

### 2. Real photos at speed

- Question: can the framer's main surface show and move a real photo as well as the WebView does?
- Spike: extend the prototype to open one 61-megapixel JPEG and one AVIF or RAW from `~/dev/enframe/samples`. Load through `<img src>` from a file path, and through an `enframe-serve` `/img/` URL. Add wheel zoom at the cursor and drag pan using `left`, `top`, `width`, `height` in pixels, since `transform` is unavailable. Log time to first paint, pointer-to-layout time, and resident memory.
- Pass: time to first paint within 1.5 times baseline; zoom and pan p95 at or below 33 ms and smooth by hand; AVIF or RAW displays through the `/img/` route; photo stays sharp at 100 percent zoom.
- Stop: pan or zoom stutters with no workaround, or the image is downsampled with no control over it.
- Effort: 1 day.
- Who: Claude Opus 5.5.

### 3. Editor overlays

- Question: can crop frames and focal points be built without `transform` and `z-index`, and do they survive live window resize?
- Spike: on the stage from the previous step, add one resizable crop frame with eight handles, Shift aspect lock, two focal points, and a clip warning border. Position everything with native layout so it follows an edge drag with no JavaScript.
- Pass: handles hit-test correctly at all zoom levels; overlays stay registered to the photo during edge resize; keyboard shortcuts (`,` `.` `1-9` `Delete` `0`) work through `on_window_key`.
- Stop: overlays drift from the photo during resize, or paint order cannot be controlled.
- Effort: 1 day.
- Who: Claude Opus 5.5.

### 4. Backend over HTTP

- Question: can a gpuix-svelte app drive the unchanged Rust backend?
- Spike: spawn `enframe-serve` from `app.ts`, or connect to a running one. Call `/sql/select`, `/api/ls`, `/img/`, `crop_and_export` and face detection with Node `fetch`. Export one frame as AVIF. Note what hot reload preserves and that `.ts` edits need a restart.
- Pass: all five calls work with no backend change; the exported file is byte-identical to the Tauri app's export for the same crop; the interface stays responsive during export.
- Stop: export or decode blocks the frame loop with no way to move it off the main thread.
- Effort: half a day.
- Who: GPT profile.

### 5. Shell surfaces

- Question: do the non-photo surfaces work natively?
- Spike: one window with a virtual grid of 2,000 thumbnails from a real folder, a text field filtering it, a command palette on ⌘K, a dialog with focus trap, file drop of an image and of a folder, the open-file picker, and a save destination chosen without a save dialog.
- Pass: grid scrolls at p95 33 ms or better; typing, selection, paste and an input-method composition (for example Japanese) work; a drop yields absolute paths; export destination can be chosen.
- Stop: text input unusable, or no path from a drop. A missing save dialog or menu bar is a recorded gap, not a stop.
- Effort: 1 day.
- Who: Claude Sonnet 5.5.

### 6. One interface or two

- Question: can one component render in the GPUI window and in the browser modes, or must the interface be maintained twice?
- Spike: take `DraggableFocalPoint.svelte` and one panel (`PreviewPanel.svelte`). Make each render on both targets using gpuix-svelte's DOM build. Count lines shared, forked and rewritten. Count pure-logic modules that import cleanly in Node.
- Pass: at least 60 percent of lines shared for both components, and the session and fit-engine modules import unchanged.
- Stop: under 30 percent shared. Then the choice is GPUI-only desktop with a frozen web interface, or no GPUI.
- Effort: 1 day.
- Who: Claude Fable 5.2 for the verdict, Claude Sonnet 5.5 for the port.

### 7. Stabilize scrubbing

- Question: can a 100-photo series be scrubbed frame by frame with no canvas element?
- Spike: preload 100 frames at 1920 pixels wide, swap them with `set_image_pixels` or preloaded `<img>` nodes on scrub, and draw the feature trail as `svg`.
- Pass: 30 frames per second or better while scrubbing, memory stable across 10 passes.
- Stop: under 15 frames per second. Stabilize then stays in the browser mode; this does not end the investigation.
- Effort: half a day.
- Who: Claude Opus 5.5.

### 8. Memory

- Question: does memory stay bounded with many large images?
- Spike: script opening 50 different 61-megapixel photos in sequence with the 2,000-thumbnail grid mounted. Log resident memory after each.
- Pass: peak under 2 GB; memory after the 50th within 20 percent of after the 5th.
- Stop: unbounded growth with no release call available.
- Effort: 2 to 3 hours.
- Who: Claude Sonnet 5.5.

### 9. Packaging

- Question: can we ship a signed app with the Rust sidecar inside it?
- Spike: build the `.app` with `enframe-serve` bundled, sign and notarize it, install on a second Apple Silicon Mac, open a photo from `~/Documents`, export.
- Pass: opens without Gatekeeper warning; privacy prompts appear once; cold start within 1.5 times baseline; size recorded.
- Stop: the sidecar cannot be signed inside the bundle. Intel Macs are out of scope; record that as a product decision.
- Effort: half a day.
- Who: GPT profile.

### 10. Accessibility

- Question: are the framer and library usable with VoiceOver and keyboard only?
- Spike: on the stages 3 and 5 builds, complete open, mark, generate, export with keyboard only and with VoiceOver.
- Pass: every control reachable and named; export progress announced by some means despite the missing `aria-live`.
- Stop: none alone. Gaps are listed against the current app's own gaps.
- Effort: 2 to 3 hours.
- Who: Morgan with a Claude Opus 5.5 write-up.

Dropped: multiple windows. Enframe has one window and upstream does not document more.

## Go / no-go

| After stage | Result | Decision |
|-|-|-|
| 0 | Fail | Stop. Record the outcome on the hand-test issue (#16). |
| 1 | Fail | Stop. Revisit when Svelte ships the custom-renderer API. |
| 2 or 3 | Fail | Stop. The framer is the product. |
| 4 | Fail | Stop, unless a worker or separate process fixes blocking. |
| 5 | Fail | Stop. |
| 6 | Under 30 percent shared | Go only if a second interface is accepted; otherwise stop. |
| 7 | Fail | Continue without Stabilize in the GPUI app. |
| 8 | Fail | Stop. |
| 9 | Fail | Stop. |
| 10 | Gaps | Continue with a listed gap. |
| 0 to 6 and 8 to 9 pass | | Go: plan a framer-only GPUI build beside the Tauri app, not a replacement. |

Total if everything runs: about 7 working days. Stages 0 to 2 cost under 2 days and carry most of the risk.

Start now: stage 0. Stage 1 can run in parallel because it touches no app code.

When the investigation ends either way, record the outcome and feed it to the evaluation of gpuix-svelte for a second desktop app (#15).

## What we do not know

- What a GPUI build must beat. No measured complaint about the Tauri app is on record, so pass criteria use Tauri as the baseline.
- Why the pointer-to-layout metric read about 1013 ms once.
- Whether Svelte will ship a custom-renderer API, and when. Unverified.
- Which GPUI revision `@gpuix/native` pins, and how GPUI's own breaking changes reach us. Unverified.
- At what resolution GPUI holds a decoded image, and whether textures are freed on unmount. Unverified.
- Whether the generic `/api/{command}` route of `enframe-serve` covers every Tauri command (Lightroom, inbox, export). Unverified.
- Whether colour management matches: the current export keeps Display P3; how GPUI displays a P3 photo is unverified.
- Whether input-method composition, a save dialog, an app menu bar and a clipboard API exist. Upstream docs are silent.
- Whether upstream will accept a change so JavaScript runs during a window-edge drag.
- Every item under "From upstream docs" above is read, not tested.
