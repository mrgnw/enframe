---
branch: claude/amazing-mendel-xmtd5j
---

# M2 · Marking screen, live

Depends on: M1 (done) · Blocks: M3 · Model: Opus · Size: L (split if it grows)
Touches: `web/src/` , `mockups/01-marking.html` (read-only source)

## Goal

Turn the signed-off mockup (`mockups/01-marking.html`) into the real marking screen at `http://localhost:7341/`, as Svelte 5 components in the M1 app — written to port into enframe's SvelteKit app at E3 with minimal change:

- Pick a folder (text field or `?folder=` for the MVP).
- First load calls `/api/track`, renders the filmstrip with lost photos flagged, the reference points and the trail (`steps` default, `?trail=flow` kept).
- Click adds or removes a point, Enter accepts, Tab walks the fix list, marked photos stay until "Clear fixed", undo — the keyboard spec as mocked.
- Every accepted mark posts `/api/track` and redraws from the fresh sidecar; stale responses (old run ID) are ignored.

## Done when

- [x] Marking the lost train clouds photos works end to end against the real folder. In the mockup's scenario (every mark but IMG_4703) the real tracker loses six, not four: un-marking IMG_4703 also loses the two after it. Skip on IMG_4675, one click on IMG_4703 (which relinks IMG_4704 and 4705), skip on IMG_4721 and 4725: "All 57 linked", with the document on disk following each step; then undo, Enter, Tab, Clear fixed and Backspace as mocked.
- [x] Playwright screenshots match the mockup for the main states, desktop and phone widths: lost, tracked and marked photos, the Points step, and `?trail=flow`.
- [x] The mockup files are untouched; the live screen lives in `web/src/`.

## Result

`web/src/lib/marking/` is the screen: `session.svelte.ts` (a Svelte 5 class) holds the document, the sidecar and the mockup's rules for marks, skips, point edits, the fix list, undo (100 snapshots of the document) and auto-advance; `trail.ts` is the mockup's overlay drawing as pure functions (points, ghost, the three trail modes, the reference patch), so E3 can take it as it is; `Stage`, `Filmstrip` and `Marking` are the components, with the mockup's CSS copied beside them. `/?folder=PATH` opens a folder: without a document, the Points step from `00-start.html` on the first photo, then Track; with one, it tracks and opens on the first lost photo. Every change edits the screen at once, posts the whole document, and redraws from the sidecar; a sequence number drops answers to older posts. Two routes were added to M1's server: `GET /api/document` (the folder's photos and saved document) and `GET /api/photo` (the bytes, cached by size and mtime); `/api/track` now adds each photo's size and the tracker's note from the run's log, which the sidecar lacks. Thumbnails are made in the browser with `createImageBitmap`, since the server has no decoder and the browser decodes each photo for the stage anyway. One rule beyond the mockup: when auto-advance lands on a photo that the re-track then links (a slow re-track, 1.7 s here against the mockup's 450 ms), the screen moves on to the next lost photo once the sidecar arrives. Known limits: the Points step shows the first photo by file name, while the CLI orders by capture time, so on a folder where the two differ the points land on the CLI's first photo; HEIC and JPEG XL photos cannot be shown by the browser; the folder picker is a text field. The Playwright scripts that took the screenshots and ran the marking loop are not in the repository (the card's files are `web/src/` only); a card for a screenshot test can bring them in.
