# Plan

pinframe's Rust CLI is the main line and runs: `bash analysis/check.sh` passes 19 of 19 on `main` (build, lint, tests, every README command, AVIF/HEIC/JXL/JPEG/PNG input, valid outputs, Girona within 2.95e-4 of the Python reference). Everything left is below, as small task cards in `todos/`.

## How work runs

1. **One card, one branch, one thread.** Each card in `todos/` is sized for one agent session. Its branch is named after it (for example `e1-sidecar-points`).
2. **A card lists what it may touch.** Stay inside those files. Anything else becomes a new card.
3. **Gate:** `bash analysis/check.sh` passes, plus the card's own "Done when". A card that changes tracking on purpose says so and updates the expected values in `check.sh` in the same branch.
4. **Merge:** rebase onto `main`, fast-forward, one merge at a time. Delete the branch and worktree after.
5. **Parallel threads:** at most two at once, and only cards with no dependency between them and no shared files.
6. **Decisions** marked ⚑ wait for the user. A thread never makes them.

## Cards

| Card | What | Depends on | Touches | Model | Size |
|-|-|-|-|-|-|
| [B5](todos/B5-exif-followups.md) | Finish B4: EXIF in benchmark variants, JXL focal length | — | `tracking_benchmark.py`, `imageio.rs`, `decode.rs` | Sonnet | S |
| [D1](todos/D1-linux-build.md) | Build and test on Linux | — | `rust/build.rs`, `rust/src/decode.rs`, `imageio.rs` gating | Opus | M |
| [E5](todos/E5-marking-mockup.md) ⚑ | Marking mockup: points and the fix list (merged, waiting for review) | — | `mockups/`, `analysis/mockup_data.py`, `docs/ui-plan.md` | Sonnet | S |
| [E2](todos/E2-serve-endpoint.md) | enframe-serve endpoint that runs pinframe | — | enframe repo | Opus | M |
| [E3](todos/E3-marking-screen.md) | Marking screen in enframe | E2, E5 | enframe repo | Opus | L |
| [E4](todos/E4-preview-export.md) | Preview and export screen in enframe | E3 | enframe repo | Sonnet | M |

Done: [B3](todos/done/1006-B3-heic10-cost.md), [E1](todos/done/1006-E1-sidecar-points.md), [A1](todos/done/1006-A1-tracking-benchmark.md), [B1](todos/done/1006-B1-colour.md), [F1](todos/done/1006-F1-python-reference.md), [B2](todos/done/1006-B2-capture-time-order.md), [A4](todos/done/1006-A4-search-window.md), [A5a](todos/done/1006-A5a-points-tracker.md), [D2](todos/done/1006-D2-build-downloads.md), [A5b](todos/done/1006-A5b-points-syntax.md) (design: [A5](todos/done/1006-A5-reference-points.md)), [A2](todos/done/1006-A2-tracker-sensitivity.md), [A3](todos/done/1006-A3-track-edge.md) (kept 1024), [B4](todos/done/1006-B4-zoom-from-exif.md), [A6](todos/done/1006-A6-skip-photos.md).

Order is in [todos/_priority.md](todos/_priority.md). Running: E2, B5.

## Decisions

Made on 2026-10-06:

- **B1 colour:** keep Display P3 pixel values and label the clip with the photos' profile. This also covers the old C1 card.
- **UI (E0):** marks are reference points, not boxes; marked photos stay in the fix list until "Clear fixed"; the other proposals as written. Recorded in `docs/ui-plan.md`, "Decisions".
- **F1 Python:** retire `anchor.py` once A1's benchmark baseline can replace it in `check.sh`.

Waiting for the user:

- E5's updated mockup, for you to look at before E3.

## Out of scope for now

Interactive marking in the CLI (the UI does it), Windows, soft lock and full-frame mode (README "Next steps"), a WASM build (`docs/wasm.md`).
