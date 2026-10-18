Open GitHub issues (2026-10-11, after #43 closed):

- #44 lossless JPEG XL export as a smaller alternative to PNG (not scheduled)

Done from the storytold exploration: #30 native .mp4, #31 face focal points, #32 EXIF orientation,
#33 no libavif C build on macOS, #35 ImageIO AVIF with Display P3, and the AVIF speedup (#34).


1. one-writer-instance-connection — phases 1 and 2 done. Remaining: phase 3, export
   destination chosen per export, which is what a remotely-connected app needs to write
   back to the machine you are sitting at.
2. serve-exposure-policy — the loopback half is gone: roots are the only boundary now.
   Still open, and now the only kind of hardening left worth doing, because each narrows
   what *any* caller can do: config file, interface allowlist, read-only mode.
3. scrub-personal-metadata-from-history — parameterize the catalog GUID and filenames out
   of the analysis scripts
4. cloud-only-lightroom-thumbnails — PARKED: no viable route (previews cache underivable,
   Adobe API enterprise-only); LR local-storage setting is the answer
5. stabilize-canvas-editor — a "fit all" canvas rule first, then crop controls with prev/next ghosts on the marking screen
6. start-screen-story — taglines and flip-book visuals: one thing across many images, many stories from one image
7. carved-versions — the Carve view is on branch `explore-claire-duplicate-photo-crops` (no PR yet).
   Follow-ups in the todo: grid grouping, framer chips and the morph, a Rust render, Standalone.

Remaining idea from auto-dual-frame Phase 3: tab-cycle alternative auto placements — tracked in ROADMAP.md.
