# plan-ideas

## Summary

Enframe is a Tauri v2 + SvelteKit 5 desktop app for image framing -- setting focal points and crop frames on photos with Lightroom Classic integration. The app is very active (10 todos completed today alone) with a working photo browser, editor canvas, Lightroom catalog reader, multi-format export, and metadata import/export. Uncommitted work adds missing-image handling and metadata import to the browser. The remaining todo is a floating toolbar UI rework.

## Action Items

- [ ] Commit or stash the in-progress PhotoBrowser.svelte and db.ts changes (missing image linking, metadata import)
- [ ] Push 4 unpushed commits to origin/main
- [ ] Complete the floating-toolbar-ui todo (the only remaining item in todos/)
- [ ] Add exported images directory to .gitignore (20+ test images sitting untracked in images/)
- [ ] Clean up debug console.log statements scattered through db.ts
- [ ] Add rotation field to exportMetadata output (currently omitted from metadata JSON)
- [ ] Decide on re-import behavior when photo already exists (db.ts:283-295 has uncertain logic with long comment)
- [ ] Wire up the scripts/ directory or clean it up (lr_backup/, lr_test_duplicate.py sitting loose)
- [ ] Replace placeholder repository URL in package.json (currently "yourusername/enframe")

## Detailed Assessment

### Project Status

The core loop works: import photos (local files, drag-drop, Lightroom catalog), place focal points, draw crop frames, export cropped images in avif/jpg/png with metadata JSON. The editor has a canvas viewport with zoom/pan, resizable frames, draggable focal points, keyboard shortcuts, and a command bar. Lightroom integration reads the local catalog database directly via Rust, including albums, search, and asset path resolution with proxy/preview fallbacks.

10 todo items were completed today (all in todos/done/0222-*), covering build health, canvas viewport, code quality, frame selection/manipulation, export rework, inline export, save history, and bug fixes. One todo remains: floating-toolbar-ui.md.

### Git Status

- Branch: `main`, ahead of origin by 4 commits
- 2 modified files uncommitted:
	- `src/lib/components/PhotoBrowser.svelte` (+305 lines) -- adds missing image detection, re-link dialog, metadata JSON import, improved drop handling
	- `src/lib/db.ts` (+124 lines) -- adds `importFromMetadata()`, `updatePhoto()`, `getPhotoByPath()`
- 20+ untracked files in `images/` (exported test images, avif/jpg/png)
- Untracked `scripts/` directory with Lightroom backup/test utilities

### Last Activity

- Last commit: 2026-02-23 00:34 (today)
- Last file edit: 2026-02-23 00:54 (today)

### Existing Plans & TODOs

**Priority list** (todos/_priority.md): All 8 items completed today except the floating toolbar is partially done (the components exist but the todo file remains).

**Floating toolbar todo** (todos/floating-toolbar-ui.md): Replace minimal editor UI with floating toolbar + expandable frames/points panel. Three batches:
1. DB migration for rotation column, Rust multi-format export, lift selection state (all done based on code)
2. FloatingToolbar.svelte, FramePointPanel.svelte, updated ResizableFrame/DraggableFocalPoint (components exist in src/lib/components/)
3. Wire everything together, keyboard shortcuts, CommandBar integration

The components from batch 2 exist in the codebase already. Batch 3 wiring may be partially done. This todo may be nearly complete.

No TODO/FIXME comments found in source code.

### Remaining Work

**Immediate**
- Commit the PhotoBrowser/db.ts changes -- they add real functionality (missing image handling, metadata import)
- Push to remote (4 commits behind)
- Verify floating toolbar todo completion -- components exist, check if wiring is done
- Move floating-toolbar-ui.md to done/ if complete

**Code cleanup**
- Remove console.log debug statements from db.ts (lines 8, 12, 61, 64, 294)
- The `importFromMetadata` function (db.ts:270-349) has a long uncertain comment block about re-import behavior -- needs a clear decision
- `exportMetadata` in export.ts doesn't include rotation in the frame output, but the schema has it

**Repository hygiene**
- Add `images/*.avif`, `images/*.jpg`, `images/*.png` patterns to .gitignore
- Fix package.json repository URL ("yourusername/enframe")
- Decide what to do with scripts/ (commit, gitignore, or remove)

### Ideas & Considerations

- The Lightroom integration reads the catalog SQLite directly. If Adobe changes the schema, this breaks silently. Consider versioning the expected schema or at least detecting unexpected changes.
- JXL export is deferred per the todo (no permissive-license encoder), but JXL is the stated preferred format for viewing. Worth revisiting as the ecosystem matures.
- The app currently has no undo/redo implementation despite the toolbar having undo/redo buttons. The save-history todo was marked done, but actual undo stack may still be missing.
- No tests exist for the frontend. The Rust side has Cargo tests but the Svelte components are untested.
- The metadata export/import cycle could become a first-class feature -- exporting a "frameset" that can be shared or version-controlled alongside photos.
- Consider batch export across multiple photos (currently one photo at a time).
- The inbox folder watcher (watch.ts) for auto-importing new photos is a nice touch -- could extend to watch for .metadata.json files too.
