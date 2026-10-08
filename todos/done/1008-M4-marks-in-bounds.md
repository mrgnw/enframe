---
branch: m4-marks-in-bounds
---

# M4 · Marks stay inside the photo

Depends on: — · Model: Sonnet · Size: S
Issue: https://github.com/mrgnw/pinframe/issues/1
Touches: `web/src/lib/marking/` (`session.svelte.ts`, `Stage.svelte`, `trail.ts`), `web/e2e/walkthrough.spec.ts`

## Problem

Found by the W1 spike. On a lost photo a click places the ghost's whole point pattern, centred at the click. Near an edge, or right after auto-advance when the reference photo's scale is still the stale local value rather than the sidecar's, some of those points land outside the photo. The CLI refuses them (`IMG_4792.avif: point -346,200 is outside the 1536x2048 photo`), `/api/track` answers with that error, and because the document was saved before the run, the next open of the folder fails its first track and shows "Can't open" with no way out but editing the JSON.

## Goal

- A mark never leaves the photo: refuse the click when any ghost point would fall outside (the cursor shows it, as `docs/ui-plan.md` says: "a mark outside the photo is refused by the cursor"), or shrink the pattern to fit; pick one and say why.
- The ghost's spread uses the reference photo's scale as the sidecar last reported it, not a value the screen set locally and the sidecar has not confirmed.
- A saved document whose first track fails names the photo and the error and offers to clear that mark or skip the photo and track again, instead of a dead end. A soft mark is a third option, added by [M5](M5-soft-marks.md).
- The walkthrough test no longer needs its workaround of landing on each photo afresh.

## Done when

- [x] Clicking at the edge of a lost photo at zoom 3 does not produce a mark the CLI refuses.
- [x] `pnpm test:e2e` passes with the workaround removed.
- [x] A document with a bad mark, planted by hand, opens to a message naming the photo and buttons that clear the mark or skip the photo.

## Result

A click whose ghost would put any point outside the photo is refused: the cursor shows not-allowed, the ghost turns red, and a toast explains. The check matches the CLI (`0 <= p < size`) and also covers Enter on an estimate and edits of a marked photo. Refused rather than shrunk, because shrinking would silently change the zoom the ghost shows. The ghost, the reference patch and the "Looking for" label use the scale the sidecar last reported (`refScale`). A document whose first track fails shows the CLI error with Clear mark and Skip photo (Clear only on the first photo, which goes back to the Points step). The walkthrough follows auto-advance with no workaround and gained tests for the edge click and both recovery buttons. `check.sh --browser`: 22 passed, 0 failed.
