# Inline Export & Sidebar Rework

Replace the export dialog with inline controls in the sidebar.

## Original Spec

- Default output location (folder icon). Default to same as source input.
- Export button for rendering images of selected frames -- all frames selected by default.
- Frame list items have export checkboxes, with select all/none at top.
- Blue for export-checked, white/grey for unchecked, orange for editing-selected.

## Tasks

- [x] Refactor export.ts: split pickDirectory from exportFrames
- [x] Add exportChecked Set, exportDir, inline export functions to ImageCanvas
- [x] Add export checkbox per frame in sidebar (before frame number)
- [x] Add select all/none checkbox in sidebar header
- [x] Color scheme: blue=export, grey=unchecked, orange=editing (sidebar + canvas overlays)
- [x] Sidebar footer: output directory picker + Export button with count
- [x] Default output directory to source photo directory
- [x] ResizableFrame: exportChecked prop, color-coded borders + handles + labels
- [x] Remove ExportDialog.svelte and all references
- [x] Remove Export button from editor toolbar
