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
- Bookmarked folders: browse a folder's images and import only the ones selected
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
- AVIF encodes on every core at speed 9, default quality 0.87: about 11x faster and
  30% smaller than before (#34)
- EXIF orientation applied at export, so rotated phone photos crop the region the
  editor shows (#32)
- AVIF through ImageIO on macOS, keeping the Display P3 profile (#35)

### Stabilize
- Hold one feature still across a photo series: mark it, Track, fix lost photos,
  export a clip (`/stabilize`, the `stabilize` crate in `src-tauri/stabilize/`)
- Decisions made before it moved into enframe (S1, 2026-10-08): marks are reference
  points; `--skip` hides a photo's feature and a soft mark places a photo without
  tracking from it; colour keeps the photos' Display P3 or sRGB values; a document
  holds named features, one run each; serve calls the crate in-process, one run per
  folder. The full list and its history is in [docs/stabilize/PLAN.md](docs/stabilize/PLAN.md)
- `.mp4` clips without ffmpeg: filmcraft's pure-Rust H.264 encoder and MP4 muxer,
  vendored (#30)
- No C build on macOS: AVIF photos decode through ImageIO; Linux keeps libavif; no
  animated `.avif` clip (#33)

### Framing
- Suggest focal points from detected faces: lightcraft's pure-Rust YuNet, vendored,
  model bundled (#31)

---

## 📋 Planned

### Framing
- Tab-cycle 2nd/3rd-best auto placements as alternatives (creative exploration)
- Tunable engine weights (composition vs crop vs zoom)

### Export
- JXL **encode** export: lossless exists (`zune-jpegxl`, no colour profile; #44); no
  permissive lossy encoder yet
- Per-format quality and speed controls in the export UI
- Arbitrary-angle frame rotation (stored but never set; export ignores it)

### Lightroom integration
- Write crops/metadata back to Lightroom (currently read-only)
- Smarter proxy vs. original selection for large RAW files

### Editor polish
- "Fit Frame" button to zoom/center the selected frame
- Cmd+scroll to cycle frames under the cursor

### Tooling / housekeeping
- Decide: adopt Tailwind/shadcn throughout, or remove the unused setup
- Real transactional snapshot restore (needs a Rust-side command with its own
  connection; `tauri-plugin-sql` pools statements across connections)

---

## 💡 Suggestions

### Smarter framing
- Saliency detection to seed focal points where there are no faces (faces: #31)

### Photo library sources
- **Lightcraft library as a source** (not scheduled; looked at 2026-10-11).
  [Lightcraft](https://github.com/storytold/lightcraft) is an open-source Rust
  rewrite of Lightroom (MIT/Apache-2.0, v0.5.0, changing daily). Unlike Lightroom
  Classic, whose private SQLite catalog enframe reads read-only and whose Lua
  plugin SDK has been static for years, it is open to outside tools:
  - Its library is a plain folder (default `~/Pictures/LightCraft Library`):
    `catalog.snap` (JSON snapshot: photos, ratings, flags, labels, albums,
    keywords, develop settings incl. crop) plus `catalog.log` (JSON lines of
    changes since the snapshot, which must be replayed). Its `catalog` crate does
    the loading and could be vendored.
  - `lightcraft --control 7980` runs a localhost JSON-lines API (no auth, app must
    be running) where `engine.execute` runs any command: `catalog.query`,
    `library.*`, `crop.set`, `photo.rate`, `app.export`. An MCP server exposes the
    same commands, also headless.
  - `library.importLightroom` imports a Lightroom Classic catalog with crops.
  - Would let enframe read the library and write crops/ratings back. Only useful
    if photos live in lightcraft. First step if picked up: import a Lightroom
    catalog headless, read `catalog.snap` + `catalog.log` from a script, and check
    a `crop.set` round trip.

### Throughput
- Batch apply a set of frames/ratios across many selected photos
- Folder → auto-export pipeline: pick a bookmarked folder, frame and export the
  selection in one pass
- Export queue with progress, cancel, and per-photo results

### Reliability & DX
- Deterministic export goldens for AVIF/JPG/PNG to catch encoder regressions
- Filename-generation tests (slug/de-dupe cases)

### Distribution
- Signed/notarized macOS build + auto-update via Tauri updater
- CLI / headless mode to run framing + export without the UI for scripting
