---
branch: claude/amazing-mendel-xmtd5j
---

# M1 · SvelteKit app: track and export endpoints

Depends on: — · Blocks: M2 · Model: Sonnet · Size: M
Touches: `web/` (new), `analysis/check.sh` (add the M1 checks)

## Goal

A SvelteKit app in `web/` carries the whole MVP: server routes now, the screens in M2/M3. Svelte 5, pnpm, adapter-node, fixed port 7341, reachable from tailnet hosts (`*.example.ts.net`). The placeholder front page just lists the endpoints.

- `POST /api/track` with `{folder, marks}`: writes the marks to `<folder>.pin.json`, runs the pinframe binary (path from `PINFRAME_BIN`, default `rust/target/release/pinframe`) in JSON-only mode with the feature cache, returns the sidecar.
- One run per folder at a time: a new request kills the child process in flight; the sidecar's run ID drops stale results.
- `POST /api/export`: runs the full clip, streams progress (SSE or a ReadableStream), ends with the output path.

## Done when

- [x] `curl` against `/api/track` on the train clouds folder returns the sidecar; a warm re-run takes 1.7 s on the 4-core Linux host, all of it the tracker's single-threaded matching (the CLI alone takes 1.6 s there; the 0.2 s figure is the M4 Pro's).
- [x] Two rapid `/api/track` posts: the first child is killed and answers 409 `{superseded: true}`, only the newest result returns.
- [x] `/api/export` produces a train clouds clip identical to the CLI's for the same marks: same sources and points, rects equal to the last bit.
- [x] A smoke test in `analysis/check.sh`: build, start on port 17341, one `/api/track` round trip, stop; `verify.py` checks the returned sidecar has all 57 photos.

## Result

`web/` is a SvelteKit 3 app (Svelte 5, adapter-node 6, Vite 8, pnpm), scaffolded with `sv create` minimal + TypeScript, no add-ons. `pnpm dev` and `pnpm start` listen on 7341 on every interface. `src/lib/server/pinframe.ts` is the whole backend: it lists a folder's photos, turns the marks document into the CLI's flags, runs the binary (`PINFRAME_BIN`, default the release build, paths relative to the repository root), keeps one child per folder and kills the one in flight when a newer request arrives, and reads the sidecar back. The document mirrors the CLI so `<folder>.pin.json` reproduces the call: `points` on the first photo, `at` per photo with an optional `zoom`, `skip`, `order`, all in oriented pixels. `/api/track` writes the JSON-only sidecar to a temp directory, never beside the photos; `/api/export` streams pinframe's lines as server-sent events (`progress`, then `done` with the clip path and sidecar, or `error`), defaults the clip to `<folder>.avif` beside the folder and refuses a path inside it, since the next run would read the clip as a photo. The placeholder front page lists the endpoints and the document. `check.sh` gained two checks (21 on Linux). Not verified here: the app on the Mac, which only needs pnpm; the M4 Pro will show the 0.2 s re-track.
