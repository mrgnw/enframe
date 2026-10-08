# E6 · Several features per sequence

Depends on: E3 · Model: Opus · Size: L
Touches: enframe repo (the framer's frames and selection, the sequence document), `rust/` only if the CLI gains a multiplexer

## Goal

Mark more than one feature in the same folder, as enframe keeps several crop frames on one photo, and export one clip per feature. Researched on 2026-10-08; placed after the enframe integration (E2 to E4) on purpose, because enframe's frame model and selection UI are the natural home for it.

- A pinframe result is one enframe frame per photo, with the tracked points as that frame's focal points. A feature is then a named set of those frames across the sequence, with a label and a destination, as enframe's frames have.
- The tracker needs no change: `track()` and `plan()` take one marks set and return one path, so N features are N calls. The feature cache is keyed by photo, so SIFT is shared and a second feature costs only matching (about 0.2 s per re-track on the M4 Pro).
- The CLI runs once per feature. A `--feature` multiplexer in one process would save decode and encode time on export; optional.
- The marking screen gets a feature switcher (enframe's frame selection: panel list, keyboard cycling), a colour per feature for points and trail, the fix list, history and trail per feature, and the Points step for each new feature. Switching features re-tracks unless the sidecar is cached per feature in the client.
- Preview and export: a feature picker, a destination per feature, "export all".

## Not this card

Holding several features still in the same clip. The tracker fits one similarity transform per photo from one point cloud; two features that move differently cannot both be held still by a crop. That would need per-photo warping, a new renderer and a new plan: a research card of its own. The limited case works today: `--points` on two objects that move together, the anchor is their mean.

## Done when

- [ ] A folder holds two features with their own marks; each tracks, previews and exports on its own, and switching between them keeps both documents.
- [ ] The train clouds set with a second feature on the mountain ridge exports two clips whose sidecars equal two CLI runs.
- [ ] The cost of one mark (`docs/ui-plan.md`) is unchanged on a folder with one feature.
