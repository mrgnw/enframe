# Enframe Roadmap

Enframe is an image framing tool: set focal points and crop frames once, then export
the same shot to many aspect ratios while keeping what matters in view. Built on
Tauri v2 + SvelteKit 5, with Lightroom catalog integration.

This roadmap captures what's shipped, what's planned, and a separate section of new
suggestions. Status reflects the codebase as of the latest commit on `main`.

**Legend:** ✅ shipped · 🚧 in progress · 📋 planned · 💡 suggestion

---

## ✅ Shipped

### Foundation
- Tauri v2 + SvelteKit 5 (runes) app on the native WebView
- SQLite via `tauri-plugin-sql` with migrations (`photos`, `frames`, `focal_points`)
- DB abstraction layer (`src/lib/db.ts`) with normalized 0–1 coordinates
- Buildable from a fresh clone (lockfiles committed, deps pruned, asset protocol fixed)

### Library & import
- Photo browser with frame overlays and filename on hover
- Lightroom catalog reader in Rust — albums, search, dynamic catalog discovery,
  asset-path resolution, LR proxy preview
- Drag-drop / file import with image dimensions captured on insert
- Inbox folder watcher that auto-imports new photos

### Editor
- Full-viewport photo fitting; fit/fill toggle (`0`); fit-to-frame; cursor-centered zoom/pan
- Draggable focal points; resizable crop frames with on-canvas handles (zoom-scaled)
- Frame manipulation: Option+drag symmetric resize, Shift+drag aspect lock,
  ratio presets, per-frame aspect lock
- Orientation-aware ratio buttons with flip animation and smooth frame transitions
- Frame rotation (swaps pixel dimensions)
- Frame selection: sidebar list, keyboard cycling (`,` / `.`), `Delete` to remove
- Floating toolbar + expandable frame/point panel
- Command palette (Cmd+K)
- Overlay title bar with animated back button

### Save & history
- Metadata save with visual feedback (no confirmation dialog)
- Undo/redo (Cmd+Z / Cmd+Shift+Z); Option+scroll to scrub history
- Save progress bar + toast notifications

### Export
- Inline sidebar export controls (replaced the old dialog)
- Per-frame export checkboxes with select all/none; auto-naming
  `{original}.{p|l}.{frame-no}.{ratio}.{ext}`
- Multi-format encode: AVIF (default), JPG, PNG
- JXL decode via `jxl-oxide`; `sips` fallback for DNG/RAW sources
- Cmd+E export shortcut with radial progress indicator and toast
- Default output directory matches the source photo's directory

---

## 🚧 / 📋 Features we've thought of so far

### Focal-point-aware cropping (core value, not yet built)
Focal points are currently stored and draggable but do not drive cropping — frames are
placed manually. The original premise is that focal points marked `required` must stay
visible in every frame.
- 📋 Auto-suggest a crop for a target ratio that keeps all required focal points in view
- 📋 Warn / highlight when a frame clips a required focal point
- 📋 One photo → batch-generate every common ratio at once, focal-point-constrained

### Export
- 📋 JXL **encode** export (deferred — no permissive-license encoder available yet)
- 📋 Export metadata JSON alongside images (frames + focal points)
- 📋 Per-format quality controls in the export UI

### Lightroom integration
- 📋 Write crops/metadata back to Lightroom (currently read-only)
- 📋 Smarter proxy vs. original selection for large RAW files

### Editor polish
- 📋 Fit should also re-center the image
- 📋 "Fit Frame" button to zoom/center the selected frame
- 📋 Cmd+scroll to cycle frames under the cursor
- 📋 Keys 1–9 to select frames by index
- 📋 Click-without-drag to cycle overlapping frames at a point
- 📋 `runed` StateHistory with debounced snapshots

### Tooling / housekeeping
- 📋 Decide: adopt Tailwind/shadcn throughout, or remove the unused setup
- 📋 Keep README in sync with actual state (several "Planned" items already shipped)

---

## 💡 Suggestions

New ideas not yet discussed — separated per the issue request.

### Smarter framing
- 💡 Saliency / face detection to seed focal points automatically on import
- 💡 Rule-of-thirds and headroom guides as optional overlays
- 💡 Named ratio "presets" mapped to destinations (e.g. "Instagram 4:5", "OG 1.91:1")

### Throughput
- 💡 Batch apply a set of frames/ratios across many selected photos
- 💡 Watch-folder → auto-export pipeline (extends the inbox watcher into full output)
- 💡 Export queue with progress, cancel, and per-photo results

### Reliability & DX
- 💡 Snapshot tests for the crop math (normalized coords → pixel rects) and filename generation
- 💡 Deterministic export goldens for AVIF/JPG/PNG to catch encoder regressions
- 💡 Structured error surface (replace remaining `alert()`-style paths with toasts everywhere)

### Distribution
- 💡 Signed/notarized macOS build + auto-update via Tauri updater
- 💡 CLI / headless mode to run framing + export without the UI for scripting

---

## Suggested near-term order

1. README + roadmap truth-up (this PR)
2. Focal-point-constrained auto-crop for a target ratio — the core differentiator
3. Metadata JSON export + batch ratio generation
4. Editor cycle/select polish (Cmd+scroll, 1–9, click-cycle)
5. Test coverage for crop math and export goldens
