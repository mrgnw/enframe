# E1 · Anchor box per photo in the sidecar

Depends on: — · Blocks: A1, E2 · Model: Terra or Sonnet · Size: S
Touches: `rust/src/main.rs` (sidecar writer), `analysis/mockup_data.py`, README, `docs/rust.md`

## Goal

Each photo in the `.json` sidecar also gets `anchor: {x, y, w, h}`: the anchor box's centre and size in that photo, normalized 0–1 like the crop rect.

## Why

The marking screen's ghost trail and the A1 benchmark both need where the anchor is in each photo. Today it is derived from the crop rect, the scale and the marks (`analysis/mockup_data.py`), because the box changes whenever a box mark replaces it (`docs/ui-plan.md`).

## Done when

- [ ] Rust writes `anchor` for every photo, tracked, marked and lost (estimated) alike.
- [ ] Its centre matches the anchor `anchor.py` prints, within 1.5 px on all 57 Girona photos.
- [ ] `analysis/mockup_data.py` reads `anchor` instead of deriving it; the mockup assets regenerate unchanged or with a one-line explanation.
- [ ] README and `docs/rust.md` describe the field. The Python reference does not need it (`check.sh` compares crop rects only).
- [ ] `bash analysis/check.sh` passes.
