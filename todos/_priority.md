Active, tracked as GitHub issues (order and parallel lanes from 2026-10-10):

- Lane A: #32 EXIF orientation at export, then #35 ImageIO AVIF (both change `crop_and_export`)
- Lane B: #30 `.mp4` clips without ffmpeg, then #33 drop the libavif build
- Lane C: #31 face focal points (its decode uses #32's orientation fix)

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

Remaining idea from auto-dual-frame Phase 3: tab-cycle alternative auto placements — tracked in ROADMAP.md.
