---
branch: e4-preview-export (enframe)
---

# E4 · Preview and export screen in enframe

Depends on: E3 · Model: Opus · Size: M
Issue: https://github.com/mrgnw/enframe/issues/8
Touches: the enframe repo

## Goal

The preview/export screen from `mockups/02-preview.html`:

- the aligned frames loop in the browser from the sidecar's crop rects (no encode)
- padding swatches change the fill live
- export runs the full clip through E2: format, PNG frames, path, progress, done

## Done when

- [x] An exported train clouds clip matches the CLI's output for the same marks (identical sidecar rects).
- [x] Playwright screenshots against the mockup.
- [x] enframe's own checks pass.

## Result

Serve's `POST /api/pinframe/export` streams server-sent events (`progress` per pinframe line, then `done` {out, sidecar} or `error` {error, superseded?}); 400/403/409 answer before a run starts and closing the connection stops the run. New `POST /api/pinframe/reveal` {path} (`open -R` inside a read root on macOS, 501 elsewhere). The MVP's preview screen is ported to `src/lib/pinframe/` and `/pin/export`: frames loop from the sidecar rects with no encode, live padding swatches, export with format, frames, path, progress and the done state (clip inline through `/img`, Download, Sidecar, Reveal when the instance is local). Differences from the MVP: padding defaults to none (the CLI default); the marking screen's last sidecar is reused so Preview opens without a re-track; the clip is served through `/img`. The e2e export walkthrough asserts the exported sidecar's rects equal a direct CLI run on the same document; screenshots in `captures/e4/`. Checks pass (serve: 31 tests; web: 140 unit, 2 e2e).
