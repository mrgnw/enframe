---
branch: claude/amazing-mendel-xmtd5j
---

# M3 · Preview and export screen, live

Depends on: M2 (done) · Model: Sonnet · Size: M
Touches: `web/src/`

## Goal

The preview/export screen from `mockups/02-preview.html`, in the same SvelteKit app:

- The aligned frames loop in the browser from the sidecar's crop rects — no encode for preview.
- Padding swatches change the fill live.
- Export posts `/api/export`: format, PNG frames, path, progress from the stream, done state linking the file.

## Done when

- [x] An exported train clouds clip matches the CLI's output for the same marks: identical sidecar rects for all 57 photos; the preview's frames differ from the CLI's PNG frames by a mean of 3 to 7 of 255 (browser resampling).
- [x] Playwright screenshots against the mockup at 1280x800 and 390x844, compared and corrected.
- [x] Full MVP walkthrough works: open folder, 4 points, track, fix 25 lost photos (15 clicks, 10 skips), preview, export 57 frames, back to Fix; server started with `pnpm build && pnpm start`.

## Result

`web/src/lib/preview/` is the screen at `/preview?folder=PATH`: `session.svelte.ts` loads the saved document, tracks it with the chosen pad and draws each photo at its sidecar crop rect into a frame the size of the clip (`clip.ts`, what the CLI's `place()` does), so the loop plays from the sidecar with no encode; frames are pre-rendered at clip size, at most 1000 px, rather than keeping 57 decoded photos. Pad swatches change the fill at once; switching between none and a colour re-tracks, since the rects overhang with padding, so `/api/track` took an optional `pad`. fps presets 2, 4, 8, 12 and a number field; Space plays and pauses; the filmstrip scrubs. The export panel has format (MP4 greyed out under transparent padding, with the reason on hover), PNG frames, the out path (default beside the folder) and progress from the event stream, then the path in a done state; it also says when photos are still lost. The phone layout stacks the clip, controls and panel. Beyond the card: the marking screen's filmstrip drag was broken in Chrome (the thumbnail links were dragged) and is fixed. Left out: no "reveal" link in the done state, only the path; desktop has no Back island, the step links do that; the export progress bar is estimated because the CLI prints in bursts. Done by a Sonnet agent on the Linux host; the Mac has not run it yet. The MVP (M1 to M3) is complete: open a folder, mark the first photo, fix lost photos, preview, export.
