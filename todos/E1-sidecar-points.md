# E1 · Reference points per photo in the sidecar

Depends on: — · Blocks: A1, E2, E5 · Model: Sonnet · Size: S
Touches: `rust/src/main.rs` (sidecar writer), README, `docs/rust.md`

## Goal

Each photo in the `.json` sidecar gets `points`: where the reference points sit in that photo, as `[x, y]` pairs normalized 0–1 like the crop rect. Today there is one point, the anchor (the first box's centre, moved by tracking and by marks). A5 adds more.

```json
{"file": "IMG_4703.avif", "x": 0.21, "y": 0.18, "w": 0.55, "h": 0.55, "scale": 1.0, "source": "mark", "points": [[0.39, 0.31]]}
```

## Why

The UI decision of 2026-10-06 is points, not boxes (`docs/ui-plan.md`, "Decisions"). The marking screen draws each photo's points and their trail, and A1 scores tracking by where the anchor lands. No box size goes in the sidecar; `scale` already carries the size change.

## Done when

- [ ] Rust writes `points` for every photo: tracked, marked and lost (estimated) alike.
- [ ] The point matches the anchor `anchor.py` prints, within 1.5 px, on all 57 Girona photos.
- [ ] README and `docs/rust.md` describe the field.
- [ ] `bash analysis/check.sh` passes.
