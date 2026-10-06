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
| [E1](todos/E1-sidecar-points.md) | Reference points per photo in the sidecar | — | `rust/src/main.rs` (sidecar), docs | Sonnet | S |
| [A1](todos/A1-tracking-benchmark.md) | Tracking accuracy and stability benchmark | E1 | `analysis/` (new), `docs/tracking.md` (new) | Opus | M |
| [A2](todos/A2-tracker-sensitivity.md) | Tracker sensitivity to tiny pixel differences | A1 | `rust/src/track.rs`, `check.sh` expectations | Opus | M |
| [A3](todos/A3-track-edge.md) | Smaller tracking size for speed, if accuracy holds | A1 | `rust/src/track.rs` (constant), docs | Terra | S |
| [A4](todos/A4-search-window.md) | Look for the feature near where it should be | A1, F1 | `rust/src/track.rs`, docs | Opus | M |
| [A5](todos/A5-reference-points.md) ⚑ | Several reference points instead of a box | A4 | `track.rs`, `main.rs` (CLI, sidecar), README | Opus | L |
| [B2](todos/B2-capture-time-order.md) | Order photos by capture time | — | `rust/src/main.rs` (path order), `decode.rs` (EXIF read) | Terra | S |
| [B4](todos/B4-zoom-from-exif.md) | Seed lens-switch zoom from EXIF focal length | A1, B2 | `rust/src/track.rs`, `decode.rs` | Opus | M |
| [D1](todos/D1-linux-build.md) | Build and test on Linux | — | `rust/build.rs`, `rust/src/decode.rs`, `imageio.rs` gating | Opus | M |
| [B1](todos/B1-colour.md) | Keep Display P3 and label the clip | — | `decode.rs`, `imageio.rs`, `avif.rs`, `main.rs` (outputs) | Opus | M |
| [E5](todos/E5-marking-mockup.md) | Marking mockup: points and the fix list | E1 | `mockups/`, `analysis/mockup_data.py`, `docs/ui-plan.md` | Sonnet | S |
| [E2](todos/E2-serve-endpoint.md) | enframe-serve endpoint that runs pinframe | E1 | enframe repo | Opus | M |
| [E3](todos/E3-marking-screen.md) | Marking screen in enframe | E2, E5, A5 | enframe repo | Opus | L |
| [E4](todos/E4-preview-export.md) | Preview and export screen in enframe | E3 | enframe repo | Sonnet | M |
| [F1](todos/F1-python-reference.md) | Retire the Python reference | A1 | `anchor.py`, `check.sh`, README | Sonnet | S |

Done: [B3](todos/done/1006-B3-heic10-cost.md).

Order is in [todos/_priority.md](todos/_priority.md). Safe pairs, in order, after E1:  **A1 + B1**, then **F1 + E5**, then **B2 + A4**.

## Decisions

Made on 2026-10-06:

- **B1 colour:** keep Display P3 pixel values and label the clip with the photos' profile. This also covers the old C1 card.
- **UI (E0):** marks are reference points, not boxes; marked photos stay in the fix list until "Clear fixed"; the other proposals as written. Recorded in `docs/ui-plan.md`, "Decisions".
- **F1 Python:** retire `anchor.py` once A1's benchmark baseline can replace it in `check.sh`.

Waiting for the user:

- ⚑ **A5:** the CLI syntax for points and what happens to `--box`. The A5 thread proposes it in its card and stops.
- E5's updated mockup, for you to look at before E3.

## Out of scope for now

Interactive marking in the CLI (the UI does it), Windows, soft lock and full-frame mode (README "Next steps"), a WASM build (`docs/wasm.md`).
