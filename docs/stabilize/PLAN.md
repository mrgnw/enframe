# Plan

pinframe's Rust CLI is the only implementation: `bash analysis/check.sh` passes 19 of 19 on `main` (build, lint, tests, every README command, AVIF/HEIC/JXL/JPEG/PNG input, colour labels, valid outputs, train clouds exactly equal to `analysis/tracking_baseline.json`). Everything left is below, as small task cards; the card files were removed from the tree and stay in git history.

## Where things live

- **The board** is the GitHub Project [nframe](https://github.com/users/mrgnw/projects/10): every open card of both repos, in plan order, with the tracking issue [pinframe#2](https://github.com/mrgnw/pinframe/issues/2) for the enframe integration. Status lives there and on the issues; the Done list below is the written history.
- **A card's status** is its GitHub issue (the Issue column below). Open the issue when the card starts, close it when the card is merged, and name the branch after the card as before. A card whose code lands in enframe has its issue in [mrgnw/enframe](https://github.com/mrgnw/enframe/issues) and is a sub-issue of [pinframe#2](https://github.com/mrgnw/pinframe/issues/2), the cross-repo view of the integration.
- The Done list below is the index of finished cards.

## How work runs

1. **One card, one branch, one thread.** Each card is sized for one agent session. Its branch is named after it (for example `e1-sidecar-points`).
2. **A card lists what it may touch.** Stay inside those files. Anything else becomes a new card.
3. **Gate:** `bash analysis/check.sh` passes, plus the card's own "Done when". The browser walkthrough runs in enframe, via `bun run test:e2e`. A card that changes tracking on purpose says so and updates the expected values in `check.sh` in the same branch.
4. **Merge:** rebase onto `main`, fast-forward, one merge at a time. Delete the branch and worktree after.
5. **Parallel threads:** at most two at once, and only cards with no dependency between them and no shared files.
6. **Decisions** marked ⚑ wait for the user. A thread never makes them.

## Coordinator

One coordinating thread runs the plan; agents do the card work. The coordinator writes briefings, verifies, merges and keeps `PLAN.md` current. Agents never edit it.

**Launch a card**

1. Paseo worktree workspace from `refs/heads/main`, branch named after the card.
2. Agent: Sonnet 5 for well-specified cards, the Opus 5.5 (medium) profile for investigation or judgment.
3. Briefing, self-contained (the agent starts with no context): task and card path; context with current numbers; relevant files; acceptance criteria; setup (`ln -s $PINFRAME_DIR/samples samples`; the parallel agent's files to avoid); constraints (files it may touch, no `PLAN.md`, tabs/fmt/clippy, no new crates, no remote so never push, commit each unit with a Co-Authored-By trailer naming its model, do not merge, never read `~/Library/Mobile Documents`, use `git grep` or Read on `.paseo` paths). End with: "Follow the `concise-output` skill. Final report under 2000 characters, outcomes not process."
4. Cards that change tracking: stop and report instead of regenerating the baseline if accuracy or the LOST set gets worse.

**Close a card**

1. Review `git diff main` in the worktree; send fixes back to the same agent (`send_agent_prompt`) rather than making them, unless trivial.
2. `git rebase main`, then `bash analysis/check.sh` (full output to a file outside the repo; `tail -1`).
3. If tracking could change: `uv run analysis/tracking_benchmark.py`, then `uv run analysis/compare_baseline.py out/tracking_benchmark.json analysis/tracking_baseline.json`.
4. Bookkeeping commit on the branch: update the card table, "Done" and "Running".
5. Fast-forward `main`, then archive the paseo workspace, then `git branch -d`. Archive only after the merge: it deletes the worktree.
6. Tell the user the outcome, any decision with a recommendation, and what runs next.

**Gotchas**

- A shell hook blocks `grep`/`rg`/`ls` on `.paseo` paths and some heredocs (it reads them as home-rooted walks). Write Python to a file in `/tmp` and `uv run` it; read logs with the Read tool.
- The Mac build has no C libraries (AVIF decodes through ImageIO, #33) and builds clean in about 5 s. Linux builds libavif and dav1d from sources cached in `~/.cache/pinframe-sources`, about 3 min on a 4-core host; a host whose network blocks a source host (github.com for libavif) sets `PINFRAME_SOURCES=DIR` to unpacked sources; see `docs/rust.md`, "Build and install".
- Linux hosts: HEIC and JPEG XL inputs are refused there (ImageIO is macOS-only), so cards that need them run on the Mac. Everything else, `check.sh` included, runs on both.
- The public mockups (https://mox.xcc.es/pinframe/) serve the main checkout, so they update only after a merge.

## Cards

| Card | Issue | What | Depends on | Touches | Model | Size |
|-|-|-|-|-|-|-|

Done: B3, E1, A1, B1, F1, B2, A4, A5a, D2, A5b (design: A5), A2, A3 (kept 1024), B4, A6, B5, E5, D1, M1, M2, M3, spikes W1 (browser tests, opt-in) and W2 (the clip in the done state), M4, M5, E2, E3, E4, E6, R1 (in enframe), R2.

Running: S1 (enframe branch `s1-stabilize`, Opus 5.5). After S1 this repository is archived and the plan continues in enframe. enframe's phase 2 landed on its `main` on 2026-10-08, so the E-cards are unblocked.

## Decisions

Made on 2026-10-06:

- **B1 colour:** keep Display P3 pixel values and label the clip with the photos' profile. This also covers the old C1 card.
- **UI (E0):** marks are reference points, not boxes; marked photos stay in the fix list until "Clear fixed"; the other proposals as written. Recorded in `docs/ui-plan.md`, "Decisions".
- **F1 Python:** retire `anchor.py` once A1's benchmark baseline can replace it in `check.sh`.
- **A5 points:** `--box` removed; `--points x,y/x,y` and `--at PHOTO=x,y/x,y@zoom`; a mark is exactly the points it lists; the anchor is their mean; area rule 96 keypoints, at least half the point spacing, at most 512 px.
- **A6 skip:** `--skip PHOTO` for a hidden feature; the photo stays in the clip at its estimated position.
- **B4 lens:** a mark without `@zoom` follows the EXIF lens ratio; a typed `@zoom` wins.
- **E5 trail style:** `steps` is the default, `flow` stays as an option, rebuilt in round 5 as a continuous glow. Mockup signed off 2026-10-06.
- **E2 timing:** wait for enframe's `phase-2-remote-targets` to merge before starting E2. Merged 2026-10-08; E2 started the same day.
- **E4 export progress:** E2's export answers once at the end; E4 streams pinframe's progress lines from the serve route as the MVP did, so the export screen can show them. Model raised to Opus because of the Rust half.
- **One repo (S1):** made 2026-10-08. pinframe moves into enframe as the `stabilize` crate with its history; serve uses it as a library; the feature is named Stabilize. The pinframe repository is archived with a pointer.
- **One app (R1, R2):** made 2026-10-08. enframe is the UI, pinframe the CLI and library; the standalone `web/` MVP is deleted once enframe's e2e covers its walkthrough, and the browser gate moves to enframe's `bun run test:e2e`.
- **E6 scope:** made 2026-10-08. The document holds named features (`{version: 2, features: [{name, marks}]}`); the CLI and tracker are unchanged, N features are N runs; the serve lock stays per folder; export writes `<folder>-<feature>.avif` (`.mp4` since #33). Writing results into enframe's SQLite frames and focal points is a possible later card, not done.
- **E2 binary, not crate:** enframe-serve calls the pinframe binary from a configured path rather than depending on the library crate, so enframe's build stays free of pinframe's C codec sources.
- **Several features per sequence (E6):** researched 2026-10-08 and placed after the enframe integration, because enframe's frame model (a frame per photo with its focal points) and its frame selection are the natural home; the tracker needs no change. Not in scope: several features held still in one clip, which needs per-photo warping.
- **Browser tests (W1):** in the repository under `web/e2e/`, opt-in through `check.sh --browser`, run for cards that touch `web/`; not a default gate, since it adds half again to every card and needs Chromium on the Mac. Screenshots are artifacts, not diffed against the mockups.
- **Clip in the done state (W2):** the export's done state shows the clip inline and offers Download and Sidecar links; "Reveal in Finder" only when the page is on localhost.
- **D1 Linux:** un-deferred 2026-10-06 and done on a Linux host, so the MVP can be developed there too; the macOS build is unchanged.
- **MVP (M1–M3):** ship the MVP as a SvelteKit app in this repo (`web/`, Svelte 5, adapter-node, port 7341), built from the signed-off mockups; server routes wrap the pinframe binary. Chosen over a Python or axum server because the marking UI is the expensive artifact and Svelte components port into enframe at E3; the thin backend gets rewritten as an axum route at E2 (enframe-serve is axum 0.8). E2–E4 stay as the later integration, still gated on phase-2.

- **Bad marks (M4, M5):** made 2026-10-08. No mark may leave the photo; the click is refused. A document whose first track fails offers Clear mark and Skip photo; M5 adds a soft mark, which places the photo without making it a tracking reference.

Waiting for the user: nothing.

## Out of scope for now

Interactive marking in the CLI (the UI does it), Windows, soft lock and full-frame mode (README "Next steps"), a WASM build (`docs/wasm.md`).
