# E2 · enframe-serve endpoint that runs pinframe

Depends on: enframe's `phase-2-remote-targets` branch merging (it rewrites `src-tauri/serve/src/main.rs`) · Blocks: E3 · Model: Opus · Size: M
Issue: https://github.com/mrgnw/enframe/issues/6
Touches: the enframe repo (`/Users/user/dev/enframe`, its server), not pinframe

## Goal

The server route the marking screen calls, as recommended in `docs/wasm.md` and `docs/ui-plan.md`. It takes a project's photos and marks (from `<folder>.pin.json`), runs pinframe in JSON-only mode with the feature cache, and returns the sidecar. Export runs the full clip.

## Requirements

- One run per project at a time. A new mark cancels the run in progress, so only the newest result counts.
- Uses the sidecar's run ID to drop stale results.
- Calls the pinframe binary from a configured path; no pinframe code is copied into enframe.

## Done when

- [ ] A re-run after a mark returns in about 0.15 s on train clouds with a warm cache, measured through the endpoint.
- [ ] Rapid repeated marks never return an older result.
- [ ] enframe's own checks pass.

## Note from B2

Capture-time ordering reads every photo's EXIF on each run, about 15 ms of a 0.19 s warm train clouds re-run. If that matters for the marking loop, the server can fix the order after the first run instead of re-reading it.
