---
status: done
---

# Fix 4 bugs: fit/fill, fit-to-frame, save status, export

## Bugs fixed

### 1. "Fit" button doesn't fill window
- Removed `max-width: 100%; height: auto` from `.canvas-content img` in ImageCanvas.svelte
- Image now renders at natural pixel dimensions, sized entirely via CSS `transform: scale()`

### 2. "Frame" fit button puts image in top-left quarter
- Changed `.canvas-content { transform-origin: center center }` to `transform-origin: 0 0`
- The fitToView() and fitToFrame() translate math assumes top-left origin

### 3. Save button always says "Saved"
- Removed broken `$effect` that watched `[frames, focalPoints]` array references
- Added `onchange` callback prop to ResizableFrame, DraggableFocalPoint, and ImageCanvas
- Children call `onchange()` on pointer-up after persisting mutations
- `+page.svelte` passes `handleChange` callback that calls `captureSnapshot()`

### 4. Export does nothing
- Moved crop logic from browser canvas to Rust backend (`crop_and_export` command)
- Added `image` and `jxl-oxide` crates to Cargo.toml
- Rust command: reads file, decodes JXL via jxl-oxide, crops, encodes JPEG, writes to disk
- Frontend `export.ts` now calls `invoke('crop_and_export', ...)` instead of canvas API
- Eliminates cross-origin tainted canvas issues with Tauri asset protocol
