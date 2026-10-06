# M3 · Preview and export screen, live

Depends on: M2 · Model: Sonnet · Size: M
Touches: `web/src/`

## Goal

The preview/export screen from `mockups/02-preview.html`, in the same SvelteKit app:

- The aligned frames loop in the browser from the sidecar's crop rects — no encode for preview.
- Padding swatches change the fill live.
- Export posts `/api/export`: format, PNG frames, path, progress from the stream, done state linking the file.

## Done when

- [ ] An exported Girona clip matches the CLI's output for the same marks (identical sidecar rects).
- [ ] Playwright screenshots against the mockup.
- [ ] Full MVP walkthrough works: open folder → fix lost photos → preview → export, server started with one command.
