# Plan

pinframe's Rust CLI is the only implementation: `bash analysis/check.sh` passes 19 of 19 on `main` (build, lint, tests, every README command, AVIF/HEIC/JXL/JPEG/PNG input, colour labels, valid outputs, train clouds exactly equal to `analysis/tracking_baseline.json`). Everything left is below, as small task cards in `todos/`.

## How work runs

1. **One card, one branch, one thread.** Each card in `todos/` is sized for one agent session. Its branch is named after it (for example `e1-sidecar-points`).
2. **A card lists what it may touch.** Stay inside those files. Anything else becomes a new card.
3. **Gate:** `bash analysis/check.sh` passes, plus the card's own "Done when". A card that touches `web/` also runs `bash analysis/check.sh --browser` (the Playwright walkthrough, about 90 s more; one-time Mac setup `cd web && pnpm install && pnpm exec playwright install chromium`). A card that changes tracking on purpose says so and updates the expected values in `check.sh` in the same branch.
4. **Merge:** rebase onto `main`, fast-forward, one merge at a time. Delete the branch and worktree after.
5. **Parallel threads:** at most two at once, and only cards with no dependency between them and no shared files.
6. **Decisions** marked ⚑ wait for the user. A thread never makes them.

## Coordinator

One coordinating thread runs the plan; agents do the card work. The coordinator writes briefings, verifies, merges and keeps `PLAN.md` and `todos/` current. Agents never edit those two.

**Launch a card**

1. Paseo worktree workspace from `refs/heads/main`, branch named after the card.
2. Agent: Sonnet 5 for well-specified cards, the Opus 5.5 (medium) profile for investigation or judgment.
3. Briefing, self-contained (the agent starts with no context): task and card path; context with current numbers; relevant files; acceptance criteria; setup (`ln -s /Users/user/dev/pinframe/samples samples`; the parallel agent's files to avoid); constraints (files it may touch, no `PLAN.md`/`todos/`, tabs/fmt/clippy, no new crates, no remote so never push, commit each unit with a Co-Authored-By trailer naming its model, do not merge, never read `~/Library/Mobile Documents`, use `git grep` or Read on `.paseo` paths). End with: "Follow the `concise-output` skill. Final report under 2000 characters, outcomes not process."
4. Cards that change tracking: stop and report instead of regenerating the baseline if accuracy or the LOST set gets worse.

**Close a card**

1. Review `git diff main` in the worktree; send fixes back to the same agent (`send_agent_prompt`) rather than making them, unless trivial.
2. `git rebase main`, then `bash analysis/check.sh` (full output to a file outside the repo; `tail -1`).
3. If tracking could change: `uv run analysis/tracking_benchmark.py`, then `uv run analysis/compare_baseline.py out/tracking_benchmark.json analysis/tracking_baseline.json`.
4. Bookkeeping commit on the branch: move the card to `todos/done/MMDD-name.md` with `branch:` frontmatter, ticked boxes and a "Result" paragraph; update the card table, "Done", "Running" and `todos/_priority.md`.
5. Fast-forward `main`, then archive the paseo workspace, then `git branch -d`. Archive only after the merge: it deletes the worktree.
6. Tell the user the outcome, any decision with a recommendation, and what runs next.

**Gotchas**

- A shell hook blocks `grep`/`rg`/`ls` on `.paseo` paths and some heredocs (it reads them as home-rooted walks). Write Python to a file in `/tmp` and `uv run` it; read logs with the Read tool.
- C library sources are cached in `~/Library/Caches/pinframe-sources` (Linux: `~/.cache/pinframe-sources`); a fresh worktree builds in about 30 s on the Mac, 3 min on a 4-core Linux host. A host whose network blocks a source host (github.com for libavif) sets `PINFRAME_SOURCES=DIR` to unpacked sources; see `docs/rust.md`, "Build and install".
- Linux hosts: HEIC and JPEG XL inputs are refused there (ImageIO is macOS-only), so cards that need them run on the Mac. Everything else, `check.sh` included, runs on both.
- The public mockups (https://mox.xcc.es/pinframe/) serve the main checkout, so they update only after a merge.

## Cards

| Card | What | Depends on | Touches | Model | Size |
|-|-|-|-|-|-|
| [M4](todos/M4-marks-in-bounds.md) | Marks stay inside the photo | — | `web/src/lib/marking/`, `web/e2e/` | Sonnet | S |
| [E2](todos/E2-serve-endpoint.md) | enframe-serve endpoint that runs pinframe | enframe phase-2 | enframe repo | Opus | M |
| [E3](todos/E3-marking-screen.md) | Marking screen in enframe | E2 | enframe repo | Opus | L |
| [E4](todos/E4-preview-export.md) | Preview and export screen in enframe | E3 | enframe repo | Sonnet | M |
| [E6](todos/E6-multiple-features.md) | Several features per sequence, one clip each | E3 | enframe repo | Opus | L |

Done: [B3](todos/done/1006-B3-heic10-cost.md), [E1](todos/done/1006-E1-sidecar-points.md), [A1](todos/done/1006-A1-tracking-benchmark.md), [B1](todos/done/1006-B1-colour.md), [F1](todos/done/1006-F1-python-reference.md), [B2](todos/done/1006-B2-capture-time-order.md), [A4](todos/done/1006-A4-search-window.md), [A5a](todos/done/1006-A5a-points-tracker.md), [D2](todos/done/1006-D2-build-downloads.md), [A5b](todos/done/1006-A5b-points-syntax.md) (design: [A5](todos/done/1006-A5-reference-points.md)), [A2](todos/done/1006-A2-tracker-sensitivity.md), [A3](todos/done/1006-A3-track-edge.md) (kept 1024), [B4](todos/done/1006-B4-zoom-from-exif.md), [A6](todos/done/1006-A6-skip-photos.md), [B5](todos/done/1006-B5-exif-followups.md), [E5](todos/done/1006-E5-marking-mockup.md), [D1](todos/done/1006-D1-linux-build.md), [M1](todos/done/1007-M1-serve.md), [M2](todos/done/1007-M2-marking-live.md), [M3](todos/done/1007-M3-preview-export.md), spikes [W1](todos/done/1007-W1-browser-tests.md) (browser tests, opt-in) and [W2](todos/done/1007-W2-clip-done-state.md) (the clip in the done state).

Order is in [todos/_priority.md](todos/_priority.md). Running: none. E2 waits for enframe's `phase-2-remote-targets` branch, which rewrites `src-tauri/serve/src/main.rs`.

## Decisions

Made on 2026-10-06:

- **B1 colour:** keep Display P3 pixel values and label the clip with the photos' profile. This also covers the old C1 card.
- **UI (E0):** marks are reference points, not boxes; marked photos stay in the fix list until "Clear fixed"; the other proposals as written. Recorded in `docs/ui-plan.md`, "Decisions".
- **F1 Python:** retire `anchor.py` once A1's benchmark baseline can replace it in `check.sh`.
- **A5 points:** `--box` removed; `--points x,y/x,y` and `--at PHOTO=x,y/x,y@zoom`; a mark is exactly the points it lists; the anchor is their mean; area rule 96 keypoints, at least half the point spacing, at most 512 px.
- **A6 skip:** `--skip PHOTO` for a hidden feature; the photo stays in the clip at its estimated position.
- **B4 lens:** a mark without `@zoom` follows the EXIF lens ratio; a typed `@zoom` wins.
- **E5 trail style:** `steps` is the default, `flow` stays as an option, rebuilt in round 5 as a continuous glow. Mockup signed off 2026-10-06.
- **E2 timing:** wait for enframe's `phase-2-remote-targets` to merge before starting E2.
- **Several features per sequence (E6):** researched 2026-10-08 and placed after the enframe integration, because enframe's frame model (a frame per photo with its focal points) and its frame selection are the natural home; the tracker needs no change. Not in scope: several features held still in one clip, which needs per-photo warping.
- **Browser tests (W1):** in the repository under `web/e2e/`, opt-in through `check.sh --browser`, run for cards that touch `web/`; not a default gate, since it adds half again to every card and needs Chromium on the Mac. Screenshots are artifacts, not diffed against the mockups.
- **Clip in the done state (W2):** the export's done state shows the clip inline and offers Download and Sidecar links; "Reveal in Finder" only when the page is on localhost.
- **D1 Linux:** un-deferred 2026-10-06 and done on a Linux host, so the MVP can be developed there too; the macOS build is unchanged.
- **MVP (M1–M3):** ship the MVP as a SvelteKit app in this repo (`web/`, Svelte 5, adapter-node, port 7341), built from the signed-off mockups; server routes wrap the pinframe binary. Chosen over a Python or axum server because the marking UI is the expensive artifact and Svelte components port into enframe at E3; the thin backend gets rewritten as an axum route at E2 (enframe-serve is axum 0.8). E2–E4 stay as the later integration, still gated on phase-2.

Waiting for the user: nothing.

## Out of scope for now

Interactive marking in the CLI (the UI does it), Windows, soft lock and full-frame mode (README "Next steps"), a WASM build (`docs/wasm.md`).
