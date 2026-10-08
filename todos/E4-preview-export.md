# E4 · Preview and export screen in enframe

Depends on: E3 · Model: Sonnet · Size: M
Issue: https://github.com/mrgnw/enframe/issues/8
Touches: the enframe repo

## Goal

The preview/export screen from `mockups/02-preview.html`:

- the aligned frames loop in the browser from the sidecar's crop rects (no encode)
- padding swatches change the fill live
- export runs the full clip through E2: format, PNG frames, path, progress, done

## Done when

- [ ] An exported train clouds clip matches the CLI's output for the same marks (identical sidecar rects).
- [ ] Playwright screenshots against the mockup.
- [ ] enframe's own checks pass.
