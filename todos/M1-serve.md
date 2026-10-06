# M1 · Standalone serve: track endpoint + static UI

Depends on: — · Blocks: M2 · Model: Sonnet · Size: M
Touches: `serve/` (new), `analysis/check.sh` (add the M1 checks)

## Goal

A single-file local server in the pinframe repo, so the MVP needs nothing from enframe. Python stdlib (`http.server`), one file: `serve/pinframe_serve.py`. Fixed port 7341, bound so tailnet hosts (`*.example.ts.net`) can reach it.

- `GET /` and static files from `serve/static/` (empty until M2).
- `POST /track` with `{folder, marks}`: writes the marks to `<folder>.pin.json`, runs the pinframe binary (path from `PINFRAME_BIN`, default `rust/target/release/pinframe`) in JSON-only mode with the feature cache, returns the sidecar.
- One run per folder at a time: a new request kills the run in progress (subprocess + lock); the sidecar's run ID drops stale results.
- `POST /export` with the clip settings: runs the full clip, streams progress lines, returns the output path.

## Done when

- [ ] `curl` against `/track` on Girona returns the sidecar; a warm re-run takes about 0.2 s.
- [ ] Two rapid `/track` posts: the first is killed, only the newest result returns.
- [ ] `/export` produces a Girona clip identical to the CLI's for the same marks.
- [ ] A smoke test in `analysis/check.sh`: start the server, one `/track` round trip, stop it.
