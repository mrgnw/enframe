# Enframe Roadmap

Enframe is an image framing tool: set focal points and crop frames once, then export
the same shot to many aspect ratios while keeping what matters in view. Built on
Tauri v2 + SvelteKit 5, with Lightroom catalog integration.

**Legend:** ✅ shipped · 📋 planned · 💡 suggestion

---

## ✅ Shipped

### Foundation
- Tauri v2 + SvelteKit 5 (runes) app on the native WebView
- SQLite via `tauri-plugin-sql` with migrations (`photos`, `frames`, `focal_points`, `history`)
- DB abstraction layer (`src/lib/db.ts`) with normalized 0–1 coordinates
- Buildable from a fresh clone; `bun test` suite for composition and history math

### Library & import
- Photo browser with frame overlays, filename on hover, destination coverage badges
- Lightroom catalog reader in Rust — albums, search, dynamic catalog discovery,
  asset-path resolution, LR proxy preview
- Drag-drop / file import with image dimensions captured on insert
- Inbox folder watcher that auto-imports new photos
- Metadata JSON re-import recreates photo + frames + focal points

### Editor
- Full-viewport fitting; cursor-centered zoom (wheel/pinch), drag pan, fit/fill toggle (`0`)
- Draggable focal points (added on click-without-drag); resizable crop frames with handles
- Frame manipulation: Option+drag symmetric resize, Shift+drag aspect lock,
  ratio presets, rotation (swaps pixel dimensions)
- Named destinations (mobile 9:16, desktop 16:9, instagram 4:5, og 1.91:1, square)
  assignable per frame; inline-editable frame labels
- Frame selection: panel list, keyboard cycling (`,` / `.`), `1–9` by index, `Delete`
- Floating toolbar + pinnable frame/point panel; command palette (Cmd+K) with cheatsheet
- Toasts for all failure paths (import, load, Lightroom proxy fallback)

### Focal-point-aware cropping
- Adaptive fit engine: auto-crop for any target ratio scored on composition
  power points + crop amount + zoom, with `required` focal points as a hard constraint
- "Generate versions" (`A`, toolbar, Cmd+K): batch-create one auto frame per destination;
  Option+click seeds points and generates the mobile+desktop pair via the same path
- Auto frames recompute when focal points change; manually adjusting one claims it
- Clip warnings (canvas border + panel glyph) when any frame excludes a required point

### Save & history
- Metadata save with visual feedback; save progress + toasts
- Persistent undo/redo (Cmd+Z / Cmd+Shift+Z): SQLite history journal per photo,
  capped at 100 snapshots, survives app restart

### Export
- Per-frame export checkboxes with select all/none
- Stable naming: destination slug > frame label > orientation+index fallback
- Multi-format encode: AVIF (default), JPG, PNG; JXL decode via `jxl-oxide`;
  `sips` fallback for DNG/RAW sources
- Sidecar metadata JSON with `fit` section: per-destination crop rect +
  CSS `object-position` for render-time fitting without new image files
- Cmd+E with radial progress; default output dir matches the source photo

---

## 📋 Planned

### Framing
- Tab-cycle 2nd/3rd-best auto placements as alternatives (creative exploration)
- Tunable engine weights (composition vs crop vs zoom)

### Export
- JXL **encode** export (deferred — no permissive-license encoder available yet)
- Per-format quality controls in the export UI
- Honor frame `rotation` in `crop_and_export` (currently ignored at encode time)

### Lightroom integration
- Write crops/metadata back to Lightroom (currently read-only)
- Smarter proxy vs. original selection for large RAW files

### Editor polish
- "Fit Frame" button to zoom/center the selected frame
- Cmd+scroll to cycle frames under the cursor
- Click-without-drag to cycle overlapping frames at a point

### Tooling / housekeeping
- Decide: adopt Tailwind/shadcn throughout, or remove the unused setup
- Real transactional snapshot restore (needs a Rust-side command with its own
  connection; `tauri-plugin-sql` pools statements across connections)

---

## 💡 Suggestions

### Smarter framing
- Saliency / face detection to seed focal points automatically on import

### Throughput
- Batch apply a set of frames/ratios across many selected photos
- Watch-folder → auto-export pipeline (extends the inbox watcher into full output)
- Export queue with progress, cancel, and per-photo results

### Reliability & DX
- Deterministic export goldens for AVIF/JPG/PNG to catch encoder regressions
- Filename-generation tests (slug/de-dupe cases)

### Distribution
- Signed/notarized macOS build + auto-update via Tauri updater
- CLI / headless mode to run framing + export without the UI for scripting
