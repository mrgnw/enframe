# M2 · Marking screen, live

Depends on: M1 · Blocks: M3 · Model: Opus · Size: L (split if it grows)
Touches: `serve/static/` (new files), `mockups/01-marking.html` (read-only source)

## Goal

Turn the signed-off mockup (`mockups/01-marking.html`) into the real marking screen at `http://localhost:7341/`, plain HTML/JS, no framework, no build step:

- Pick a folder (text field or `?folder=` for the MVP).
- First load calls `/track`, renders the filmstrip with lost photos flagged, the reference points and the trail (`steps` default, `?trail=flow` kept).
- Click adds or removes a point, Enter accepts, Tab walks the fix list, marked photos stay until "Clear fixed", undo — the keyboard spec as mocked.
- Every accepted mark posts `/track` and redraws from the fresh sidecar; stale responses (old run ID) are ignored.

## Done when

- [ ] Marking the 4 lost Girona photos works end to end against the real folder.
- [ ] Playwright screenshots match the mockup for the main states, desktop and phone widths.
- [ ] The mockup files are untouched; the live screen lives in `serve/static/`.
