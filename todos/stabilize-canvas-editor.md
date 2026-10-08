# Stabilize: a canvas editor on the marking screen

From the 2026-10-08 conversation on over-zoomed frames. The same-size rule (every photo scaled so the feature keeps its size) stays; what is missing is seeing and shaping the canvas while matching, not after.

## What exists (branch `stabilize/canvas-size-control`)

- `--zoom PHOTO=scale` and `--canvas WxH`, both saved in the document's marks per feature, set from the preview island. A canvas change re-tracks (SIFT is cached, under a second) and the old frames stay up until the new ones are drawn.

## Wanted

1. **A canvas that grows when it obviously should.** Today a padded clip takes the median margin on each side, so a photo at scale 0.4 overflows and is cut. Cheapest rule: a "fit all" choice that takes the largest margin per side, so every photo lands whole inside the clip (at the cost of more padding). One line in `crop_frame` (`pick` = max) behind a third pad mode or a `--canvas fit` value. A smarter rule (grow only when the overflow is above some fraction of the photo) can come after measuring on train clouds.
2. **Crop controls on the marking screen.** While fixing a photo, show the canvas as a frame over it, like enframe's crop frames: drag its edges to re-crop, drag the photo to scale it (the zoom), with the anchor pinned. The previous and next photos drawn as ghosts on the same canvas to line things up. What overflows the canvas sits under a smoke screen rather than being hidden, so the cut is visible.
3. **Smooth transitions.** Keep the swap-when-drawn behaviour; consider drawing the old bitmaps transformed to the new canvas geometry at once, then replacing them with the sharp redraw, so a canvas change is visible instantly.

## Not wanted

- Per-browser canvas state. The document is the only source; two sessions on one folder already post the whole document and the newest wins.
