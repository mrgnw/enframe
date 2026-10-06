# M1 · SvelteKit app: track and export endpoints

Depends on: — · Blocks: M2 · Model: Sonnet · Size: M
Touches: `web/` (new), `analysis/check.sh` (add the M1 checks)

## Goal

A SvelteKit app in `web/` carries the whole MVP: server routes now, the screens in M2/M3. Svelte 5, pnpm, adapter-node, fixed port 7341, reachable from tailnet hosts (`*.example.ts.net`). The placeholder front page just lists the endpoints.

- `POST /api/track` with `{folder, marks}`: writes the marks to `<folder>.pin.json`, runs the pinframe binary (path from `PINFRAME_BIN`, default `rust/target/release/pinframe`) in JSON-only mode with the feature cache, returns the sidecar.
- One run per folder at a time: a new request kills the child process in flight; the sidecar's run ID drops stale results.
- `POST /api/export`: runs the full clip, streams progress (SSE or a ReadableStream), ends with the output path.

## Done when

- [ ] `curl` against `/api/track` on train clouds returns the sidecar; a warm re-run takes about 0.2 s.
- [ ] Two rapid `/api/track` posts: the first child is killed, only the newest result returns.
- [ ] `/api/export` produces a train clouds clip identical to the CLI's for the same marks.
- [ ] A smoke test in `analysis/check.sh`: build, start, one `/api/track` round trip, stop.
