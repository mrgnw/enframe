# Floating Toolbar + Expandable Panel UI

## Overview
Replace the current minimal editor UI with a floating toolbar and expandable frames/points panel. Add multi-format export (avif default), frame rotation, keyboard shortcuts for frame cycling/deletion.

## Implementation Steps

### Batch 1 (parallel, no dependencies)

#### Step 1: DB Migration — Add `rotation` to frames
- `database.sql`: Add `rotation REAL DEFAULT 0` column
- `src-tauri/src/lib.rs`: Add migration v2 with ALTER TABLE
- `src/lib/db.ts`: Add `rotation: number` to Frame, update `addFrame()` and `updateFrame()`

#### Step 2: Rust export — Multi-format support
- `src-tauri/Cargo.toml`: Add `avif` feature to `image` crate
- `src-tauri/src/lib.rs`: Add `format: String` param to `crop_and_export`, switch encoder
- `src/lib/export.ts`: Add `ExportFormat = 'avif' | 'jpg' | 'png'`, pass format, change extension. Default: avif

#### Step 3: Lift selection state to editor page
- `src/routes/edit/[id]/+page.svelte`: Own `selectedFrameId` and `selectedPointId`
- `src/lib/components/ImageCanvas.svelte`: Accept selection as props instead of local state

### Batch 2 (parallel, depends on Batch 1)

#### Step 4: Create `FloatingToolbar.svelte`
Pill-shaped bar, top-right:
- Undo (Lucide Undo2) / Redo (Lucide Redo2)
- Add Frame (+ Lucide Crop)
- Add Point (+ inline SVG center_focus_strong)
- Save (Lucide Check)
- Expand/Collapse toggle

#### Step 5: Create `FramePointPanel.svelte`
Expandable, auto-hides after 300ms of inactivity:
- Per-frame row: number, W/H (px), lock aspect, center x/y, rotation, rotate frame (swap W/H), ratio presets (16:9, 4:3, 3:2, 1:1), selection highlight
- Per-point row: •N label, x/y (px), selection highlight

#### Step 7: Update `ResizableFrame.svelte`
- CSS `transform: rotate(Xdeg)` from `frame.rotation`
- Label = frame index number only
- Selection color synced with panel

#### Step 8: Update `DraggableFocalPoint.svelte`
- Selection support (selectedPointId prop, click to select)
- Label = `•N` format

### Batch 3 (sequential, depends on Batch 2)

#### Steps 6 + 9 + 10: Wire everything + shortcuts + CommandBar
- Wire FloatingToolbar and FramePointPanel into editor page
- `exportFormat` state (default 'avif')
- Keyboard shortcuts: `,` prev frame, `.` next frame, `Delete`/`Backspace` delete frame
- CommandBar: add Next/Prev/Delete Frame commands, Set Export Format (avif/jpg/png)

## Decisions Made
- Default export format: **avif**
- JXL export: **deferred** (no permissive-license encoder available)
- Icon for add point: **inline SVG** (material-symbols center_focus_strong)
- Frame labels: numbers only (1, 2, 3)
- Point labels: •1, •2, •3
- Keep Cmd+K CommandBar alongside toolbar
- Frame cycling shortcuts: `,` and `.`
