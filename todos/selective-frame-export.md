# Selective Frame Export

## Overview
Allow users to select specific frames for export using Shift/Cmd+click multi-selection. When no frames are explicitly selected, all frames are exported (current behavior). Show an export button with the count of frames to export.

## Behavior
- **Click** a frame: single-select (current behavior, for editing)
- **Shift+click** or **Cmd+click** a frame: toggle that frame in/out of the export selection
- **No export selection**: all frames are exported (default)
- **With export selection**: only selected frames are exported
- Export button label: `Export N` (where N = number of frames that will be exported)

## State Changes
- New state: `exportSelection = $state<Set<number>>(new Set())` — set of frame IDs marked for export
- `selectedFrameId` stays as-is for the editing/focus selection (single frame)
- `exportSelection` is a separate concept from `selectedFrameId`
- Derived: `framesToExport = $derived(exportSelection.size > 0 ? frames.filter(f => exportSelection.has(f.id)) : frames)`

## Implementation

### Editor page (`src/routes/edit/[id]/+page.svelte`)
- Add `exportSelection` state and `framesToExport` derived
- Modify `handleExport()` to use `framesToExport` instead of `frames`
- Pass export selection state to components that need it

### ImageCanvas / ResizableFrame
- On Shift+click or Cmd+click a frame: toggle in `exportSelection` instead of changing `selectedFrameId`
- Visual indicator on frames in the export selection (e.g. subtle green border or checkmark badge)

### PreviewPanel
- Show export selection state on preview cards (checkmark or highlight)
- Shift/Cmd+click on preview cards also toggles export selection

### FloatingToolbar
- Replace or augment the existing export trigger with a button showing `Export N`
- Button disabled when exporting or no frames exist

### FramePointPanel
- Show export selection checkmarks per frame row

### CommandBar
- Add "Select All for Export" / "Clear Export Selection" commands
