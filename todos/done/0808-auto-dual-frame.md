---
branch: ux-improvements
---

# Auto Dual-Frame (Quick Frame)

## Summary

Option+click on the canvas to place global focal points and auto-generate a landscape (16:9) + portrait (9:16) frame pair. Frames recompute on every focal point change. Compositional guide overlays available per frame.

## Interaction

| Input | Context | Action |
|---|---|---|
| Click | Blank canvas | Add global focal point |
| Click | Inside selected frame | Add frame-scoped focal point |
| Option+Click | Canvas | Add global focal point + ensure auto frame pair exists + recompute |
| G | Frame selected | Cycle guide overlay: off → thirds → phi → spiral → triangle → diagonal → off |

Option+click when auto frames already exist: adds focal point, recomputes the same two frames. Never creates a second pair.

Click on blank canvas (no modifier) always adds a global focal point. If auto frames exist, they recompute.

## Auto Frame Pair

- Exactly one pair: **"Auto L"** (16:9) and **"Auto P"** (9:16)
- Identified by name — no special DB flag needed
- Input: all global focal points
- Always recomputed when global focal points change (add, move, delete)
- Manual resize/move is allowed, but next focal point change recomputes and overrides
- User can delete them like any frame; Option+click recreates the pair

## Scoring Algorithm

For each orientation independently:

### 1. Constraint solve

Find the largest frame at the target ratio that:
- Contains all global focal points with 5% inset padding (points aren't at the very edge)
- Fits within the image bounds

### 2. Position search

Slide the frame across valid positions (grid search at ~1% increments on normalized image coords). For each candidate position, score the placement.

### 3. Score each position

Transform each focal point into frame-relative coords (0-1). Measure proximity to power points from multiple compositional systems.

**Compositional systems and power points (frame-relative):**

| System | Power Points |
|---|---|
| Rule of Thirds | (1/3, 1/3), (2/3, 1/3), (1/3, 2/3), (2/3, 2/3) |
| Phi Grid | (0.382, 0.382), (0.618, 0.382), (0.382, 0.618), (0.618, 0.618) |
| Golden Spiral | Eye at ~(0.382, 0.382) in 4 rotations (one per corner) |
| Diagonal Method | 45-degree lines from corners of inscribed square |
| Center | (0.5, 0.5) |

**Scoring:**
```
for each candidate (x, y):
    score = 0
    for each focal_point:
        fp_rel = transform to frame-relative coords
        point_score = 0
        systems_matched = 0
        for each system:
            min_dist = min distance from fp_rel to system's power points
            proximity = max(0, 1 - min_dist * k)
            if proximity > threshold:
                systems_matched++
            point_score += proximity * system_weight
        if systems_matched >= 2:
            point_score *= consensus_bonus  // e.g. 1.3
        score += point_score
    pick (x, y) with highest score
```

**Orientation-specific weights:**

| System | Landscape (16:9) | Portrait (9:16) |
|---|---|---|
| Phi Grid | 1.0 | 0.8 |
| Rule of Thirds | 0.9 | 0.6 |
| Golden Spiral | 0.7 | 1.0 |
| Center | 0.4 | 0.7 |
| Diagonal Method | 0.6 | 0.5 |

### 4. Per-point behavior

- **1 point**: largest possible frame, point placed at best consensus power point
- **2 points**: both must be inside frame, system finds position where both align well with power points
- **3+ points**: all must be inside frame, system maximizes total score across all points

## Compositional Guide Overlays

Toggle per selected frame with `G` key. Renders guide lines/curves on the canvas over the frame.

| Guide | Visual |
|---|---|
| Rule of Thirds | 2 horizontal + 2 vertical lines, frame divided into 9 equal parts |
| Phi Grid | Same layout but lines at 0.382/0.618 |
| Golden Spiral | Logarithmic spiral within the frame (4 rotation variants) |
| Golden Triangle | Diagonal + perpendiculars from opposite corners |
| Diagonal Method | 45-degree lines from corners of inscribed square |

## Frame Labels

Auto frames labeled **"Auto L"** and **"Auto P"** in the FramePointPanel.

Each frame (auto or manual) shows which compositional system(s) its focal points best align with — e.g. "Phi + Thirds". Derived at display time from the scoring function, not persisted.

## Phases

### Phase 1 — Core
- Option+click adds global focal points and creates/recomputes auto dual-frame pair
- Click on blank canvas adds global focal point (recomputes auto frames if they exist)
- Scoring algorithm with all 5 compositional systems
- Auto frames labeled "Auto L" / "Auto P"
- Compositional match labels on frames

### Phase 2 — Guides
- `G` key cycles guide overlays on selected frame
- Guide rendered as semi-transparent lines/curves on canvas
- Guide type shown in toolbar or panel

### Phase 3 — Refinement
- Additional aspect ratio pairs (4:3, 3:2)
- "Suggest alternative" — show 2nd/3rd best placements, tab to cycle
- Tunable weights (power user setting)
