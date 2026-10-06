# E3 · Marking screen in enframe

Depends on: E2 · Blocks: E4 · Model: Opus · Size: L (split it if it grows)
Touches: the enframe repo

## Goal

The marking screen from `mockups/01-marking.html` (https://mox.xcc.es/pinframe/01-marking.html), built in enframe's SvelteKit app:

- the filmstrip with lost photos flagged
- scrubbing and the keyboard spec
- the ghost trail of neighbouring anchor boxes (from E1's `anchor` field)
- click to mark a point, drag to mark a search box, Enter to accept, Tab to the next lost photo
- undo

Each mark calls E2 and updates the view.

## Done when

- [ ] Marking the 4 lost photos of the mockup scenario works end to end on the real Girona folder.
- [ ] Playwright screenshots against the mockup for the main states.
- [ ] enframe's own checks pass.
