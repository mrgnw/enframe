# M4 · Marks stay inside the photo

Depends on: — · Model: Sonnet · Size: S
Issue: https://github.com/mrgnw/pinframe/issues/1
Touches: `web/src/lib/marking/` (`session.svelte.ts`, `Stage.svelte`, `trail.ts`), `web/e2e/walkthrough.spec.ts`

## Problem

Found by the W1 spike. On a lost photo a click places the ghost's whole point pattern, centred at the click. Near an edge, or right after auto-advance when the reference photo's scale is still the stale local value rather than the sidecar's, some of those points land outside the photo. The CLI refuses them (`IMG_4792.avif: point -346,200 is outside the 1536x2048 photo`), `/api/track` answers with that error, and because the document was saved before the run, the next open of the folder fails its first track and shows "Can't open" with no way out but editing the JSON.

## Goal

- A mark never leaves the photo: refuse the click when any ghost point would fall outside (the cursor shows it, as `docs/ui-plan.md` says: "a mark outside the photo is refused by the cursor"), or shrink the pattern to fit; pick one and say why.
- The ghost's spread uses the reference photo's scale as the sidecar last reported it, not a value the screen set locally and the sidecar has not confirmed.
- A saved document whose first track fails names the photo and the error and offers to clear that mark and track again, instead of a dead end.
- The walkthrough test no longer needs its workaround of landing on each photo afresh.

## Done when

- [ ] Clicking at the edge of a lost photo at zoom 3 does not produce a mark the CLI refuses.
- [ ] `pnpm test:e2e` passes with the workaround removed.
- [ ] A document with a bad mark, planted by hand, opens to a message naming the photo and a button that clears it.
