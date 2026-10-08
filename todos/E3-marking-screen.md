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

- [ ] Marking the 4 lost photos of the mockup scenario works end to end on the real train clouds folder.
- [ ] Playwright screenshots against the mockup for the main states.
- [ ] enframe's own checks pass.
