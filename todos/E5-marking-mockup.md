# E5 · Marking mockup: points and the fix list

Depends on: E1 · Blocks: E3 · Model: Sonnet · Size: S
Touches: `mockups/01-marking.html`, `mockups/pin.css`, `analysis/mockup_data.py`, `mockups/assets/rects.json`, `docs/ui-plan.md`

## Goal

Bring the marking mockup and `docs/ui-plan.md` in line with the decisions of 2026-10-06 (`docs/ui-plan.md`, "Decisions"), so the user can check them before E3 builds anything:

- **Points.** No box anywhere. Show the reference points in each photo, and the trail as the path of their centre. Click adds a point to the current photo; clicking a point removes it (proposal for the user to judge). The zoom ghost is the points' pattern at the current zoom.
- **Fix list.** Auto-advance after a mark stays (450 ms). A marked photo stays in the fix list: `Tab` and the next arrow still visit it. A "Clear fixed" button removes the marked photos from the list.
- `analysis/mockup_data.py` reads the sidecar's `points` (E1) instead of deriving the anchor.

## Done when

- [ ] https://mox.xcc.es/pinframe/01-marking.html shows the points, the trail and the fix list, at desktop and phone widths (Playwright screenshots).
- [ ] `docs/ui-plan.md` describes no box gestures, apart from history notes.
- [ ] The user has looked at it and said yes, or the changes they asked for are in.
