---
branch: e2-serve-endpoint (enframe)
---

# E2 · enframe-serve endpoint that runs pinframe

Depends on: enframe's `phase-2-remote-targets` (merged 2026-10-08) · Blocks: E3 · Model: Opus · Size: M
Issue: https://github.com/mrgnw/enframe/issues/6
Touches: the enframe repo (`/Users/user/dev/enframe`, its server), not pinframe

## Goal

The server route the marking screen calls, as recommended in `docs/wasm.md` and `docs/ui-plan.md`. It takes a project's photos and marks (from `<folder>.pin.json`), runs pinframe in JSON-only mode with the feature cache, and returns the sidecar. Export runs the full clip.

## Requirements

- One run per project at a time. A new mark cancels the run in progress, so only the newest result counts.
- Uses the sidecar's run ID to drop stale results.
- Calls the pinframe binary from a configured path; no pinframe code is copied into enframe.

## Done when

- [x] A re-run after a mark returns in about 0.15 s on train clouds with a warm cache, measured through the endpoint.
- [x] Rapid repeated marks never return an older result.
- [x] enframe's own checks pass.

## Note from B2

Capture-time ordering reads every photo's EXIF on each run, about 15 ms of a 0.19 s warm train clouds re-run. If that matters for the marking loop, the server can fix the order after the first run instead of re-reading it.

## Result

In enframe-serve (`src-tauri/serve/src/pinframe.rs`): `POST /api/pinframe/track` {folder, marks, pad?} saves `<folder>.pin.json`, runs the binary JSON-only with the feature cache, and returns the sidecar annotated with each photo's size and note; `POST /api/pinframe/export` {folder, marks, out?, pad?, fps?, frames?, vf?} renders the clip and returns {out, sidecar}. Folder in a read root; document, clip and frames in a write root. A newer request for the same folder kills the run in flight, which answers 409. Binary from `--pinframe` / `ENFRAME_PINFRAME`, no default; unset answers 503. Warm re-run through the endpoint: 155 ms on train clouds. Differs from the SvelteKit reference: export answers once at the end rather than streaming progress; the track sidecar goes to a temp file. enframe checks pass (27 tests).
