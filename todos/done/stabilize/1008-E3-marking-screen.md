---
branch: e3-marking-screen (enframe)
pull-request: mrgnw/enframe#12
---

# E3 · Marking screen in enframe

Depends on: E2, E5, A5 · Blocks: E4 · Model: Opus · Size: L (split it if it grows)
Issue: https://github.com/mrgnw/enframe/issues/7
Touches: the enframe repo

## Goal

The marking screen from `mockups/01-marking.html` (https://mox.xcc.es/pinframe/01-marking.html, as updated by E5), built in enframe's SvelteKit app:

- the filmstrip with lost photos flagged
- scrubbing and the keyboard spec
- the reference points in each photo and their trail (from the sidecar's `points`)
- click to add or remove a point, Enter to accept, Tab to the next photo in the fix list
- marked photos stay in the fix list until "Clear fixed"
- undo

Each mark calls E2 and updates the view.

## Done when

- [x] Marking the 4 lost photos of the mockup scenario works end to end on the real train clouds folder.
- [x] Playwright screenshots against the mockup for the main states.
- [x] enframe's own checks pass.

## Result

enframe route `/pin?folder=PATH` (or Open folder via the picker), linked from the command bar as "Track a Series with pinframe…". The MVP's `web/src/lib/marking/` was ported to `src/lib/pinframe/` with only `api.ts` changed: photos through `/img`, the document from `<folder>.pin.json`, each mark posted to E2's `/api/pinframe/track`, newest answer wins (sequence number, 409 retry). A missing document is told from a refused path by listing the parent, since serve answers both with 403. CSS scoped under `.pinframe`. Playwright added to enframe as opt-in `bun run test:e2e` (builds, stages the sidecar, runs serve with the real binary on a temp copy of train clouds): 15 marks and 10 skips reach "All 57 linked", screenshots in `captures/e3/`. The scenario is the walkthrough's 25 lost photos, not the mockup's 4. 2740 lines added, half of it the formatter expanding the ported CSS. `/pin/export` is a placeholder for E4.
