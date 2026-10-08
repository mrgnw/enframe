# S1 · Move pinframe into enframe as the stabilize crate

Depends on: R2 · Model: Opus · Size: L
Issue: https://github.com/mrgnw/enframe/issues/17
Touches: enframe repo (everything the move lands in); pinframe only for the final README pointer, done by the coordinator

## Why

enframe is the app and pinframe is its tracker. Keeping them in two repos cost a cross-repo card for every step of the integration (E2 to R2), a configured binary path (`--pinframe`), a 503 when it is unset, a temp sidecar file per run, and two issue trackers. Nothing needs the split: the native build (libaom, dav1d from cached sources) moves with the crate and costs what it costs today; samples are not tracked; HEIC/JXL stay macOS-only wherever the crate lives.

## Decisions

- The feature is **Stabilize**. "Track" stays on the button that runs the tracker.
- History is kept: `git subtree add` of the pinframe repo, then moves into place.
- serve calls the tracker as a library. The CLI stays as a workspace binary for scripts and the benchmark.
- Environment and cache names that are not user-facing (`PINFRAME_SOURCES`, `~/Library/Caches/pinframe-sources`) stay as they are.

## Layout after the move

| From (pinframe) | To (enframe) |
|-|-|
| `rust/` | `src-tauri/stabilize/` (crate `stabilize`, workspace member) |
| `analysis/` (check.sh, benchmark, baseline, verify.py) | `analysis/stabilize/` |
| `docs/` | `docs/stabilize/` |
| `mockups/` | `mockups/stabilize/` |
| `todos/done/`, `PLAN.md` decisions | `todos/done/stabilize/`, a "Stabilize" section in enframe's plan |
| `README.md` | `docs/stabilize/README.md`; enframe's README gets a Stabilize section |
| `src/lib/pinframe/`, `/pin`, `/api/pinframe/*`, `.pinframe` CSS scope | `src/lib/stabilize/`, `/stabilize`, `/api/stabilize/*`, `.stabilize` |
| `<folder>.pin.json` | `<folder>.stabilize.json`, and `.pin.json` is still read |

## Done when

- [ ] `git log --follow src-tauri/stabilize/src/track.rs` shows pinframe's history.
- [ ] serve has no binary path: `--pinframe` / `ENFRAME_PINFRAME` are gone from the flags, README and `pitchfork.toml`; track and export call the library; a superseded run still answers 409 and its result is dropped; export still streams progress lines.
- [ ] `bash analysis/stabilize/check.sh` passes with the tracking baseline unchanged, and a `just` recipe runs it.
- [ ] enframe's checks pass: `bun run check`, `bun run lint`, `bun run test`, `bun run test:e2e` (specs renamed to the new route and document name), `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` across the workspace.
- [ ] Nothing in enframe says pinframe except history notes and the README pointer to the archived repo.
