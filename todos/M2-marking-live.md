# M2 · Marking screen, live

Depends on: M1 · Blocks: M3 · Model: Opus · Size: L (split if it grows)
Touches: `web/src/` , `mockups/01-marking.html` (read-only source)

## Goal

Turn the signed-off mockup (`mockups/01-marking.html`) into the real marking screen at `http://localhost:7341/`, as Svelte 5 components in the M1 app — written to port into enframe's SvelteKit app at E3 with minimal change:

- Pick a folder (text field or `?folder=` for the MVP).
- First load calls `/api/track`, renders the filmstrip with lost photos flagged, the reference points and the trail (`steps` default, `?trail=flow` kept).
- Click adds or removes a point, Enter accepts, Tab walks the fix list, marked photos stay until "Clear fixed", undo — the keyboard spec as mocked.
- Every accepted mark posts `/api/track` and redraws from the fresh sidecar; stale responses (old run ID) are ignored.

## Done when

- [ ] Marking the 4 lost train clouds photos works end to end against the real folder.
- [ ] Playwright screenshots match the mockup for the main states, desktop and phone widths.
- [ ] The mockup files are untouched; the live screen lives in `web/src/`.
