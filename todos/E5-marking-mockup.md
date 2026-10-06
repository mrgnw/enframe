# E5 · Marking mockup: points and the fix list

Depends on: E1 · Blocks: E3 · Model: Sonnet · Size: S
Touches: `mockups/01-marking.html`, `mockups/pin.css`, `analysis/mockup_data.py`, `mockups/assets/rects.json`, `docs/ui-plan.md`

## Goal

Bring the marking mockup and `docs/ui-plan.md` in line with the decisions of 2026-10-06 (`docs/ui-plan.md`, "Decisions"), so the user can check them before E3 builds anything:

- **Points.** No box anywhere. Show the reference points in each photo, and the trail as the path of their centre. Click adds a point to the current photo; clicking a point removes it (proposal for the user to judge). The zoom ghost is the points' pattern at the current zoom.
- **Fix list.** Auto-advance after a mark stays (450 ms). A marked photo stays in the fix list: `Tab` and the next arrow still visit it. A "Clear fixed" button removes the marked photos from the list.
- `analysis/mockup_data.py` reads the sidecar's `points` (E1) instead of deriving the anchor.

## Done when

- [x] https://mox.xcc.es/pinframe/01-marking.html shows the points, the trail and the fix list, at desktop and phone widths (Playwright screenshots).
- [x] `docs/ui-plan.md` describes no box gestures, apart from history notes.
- [ ] ⚑ The user has looked at it and said yes, or the changes they asked for are in.

## Status (2026-10-06)

Merged from branch `e5-marking-mockup`; waiting for the user's review. Open points for the user:

- On a lost photo, one click places the whole pattern of points (keeps a mark at one click). Clicking adds or removes single points only on a photo already marked.
- The three extra points are simulated in a fixed pattern around the real anchor.
- `mockups/00-start.html` still says "Box" and draws a box.

## Round 2 (2026-10-06)

The user's feedback: "maybe the trail could have some feature that ensures they're at least somewhat visible regardless of the photo it's on?"

- [x] The trail (lines, dots, points) stays visible on any photo: bright sky, white cloud, dark foliage, busy texture. Compare a few treatments on the hardest Girona photos with screenshots and keep the simplest that works.
- [x] `mockups/00-start.html` uses points instead of a box (`--box` is being removed).

Round 2 merged from branch `e5-trail-visibility`: a stronger halo that fades with its line, a gentler fade (1 to 0.55), dark discs under trail dots, outlined points; the start page uses points. Waiting for the user's review.

## Round 3 (2026-10-06)

Merged from branch `e5-trail-colour`. The trail shows distance from the current photo as stepped colour (OKLCH): violet to pink before, teal to green after, full opacity, round-2 halo kept. `?trail=steps` (default), `?trail=fade` (round 2), `?trail=pulse` (a white light steps along the trail every 420 ms, off when the system asks for less motion). Waiting for the user to compare them.

## Round 4 (2026-10-06)

The user: "Maybe pulse was the wrong word… I meant for it to look like the colors were flowing from one point to the next." Replace `?trail=pulse` with `?trail=flow`: the stepped colours move along the trail in photo order, each colour flowing from one point to the next. `steps` stays the default until the user picks.

Round 4 merged from branch `e5-trail-flow`: `?trail=flow` replaces `pulse` (full-colour dashes travel along a darker blended line in photo order, still under reduced motion); Skip (`S` or "Skip, hidden here") on lost photos, grey dot in the filmstrip, no trail dot for skipped photos. Waiting for the user to pick the trail style.

## Round 5 (2026-10-06)

The user picked: `steps` stays the default, `flow` stays as the option — but the flow rendering must be much smoother and cooler than travelling dashes.

- [x] Rebuild `?trail=flow` without dashes: a continuous, smooth motion of colour along the trail (e.g. an animated gradient sweeping point to point). Still OKLCH distance colours, still the round-2 halo, still off under reduced motion.
- [x] Screenshots at desktop and phone widths on the hardest Girona photos.

Round 5 merged from branch `e5-trail-flow-smooth`: the dashes are replaced by a wide, blurred, brighter glow sliding continuously along the gradient-blended line (1.6 s cycle). The agent dropped the reduced-motion rule and reported it present; the coordinator restored it. Waiting for the user's sign-off on the mockup.
