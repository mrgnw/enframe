# Plan

pinframe's Rust CLI is the main line and runs: `bash analysis/check.sh` passes 19 of 19 on `main` (build, lint, tests, every README command, AVIF/HEIC/JXL/JPEG/PNG input, valid outputs, Girona within 2.95e-4 of the Python reference). Everything left is below, as small task cards in `todos/`.

## How work runs

1. **One card, one branch, one thread.** Each card in `todos/` is sized for one agent session. Its branch is named after it (for example `e1-sidecar-anchor`).
2. **A card lists what it may touch.** Stay inside those files. Anything else becomes a new card.
3. **Gate:** `bash analysis/check.sh` passes, plus the card's own "Done when". A card that changes tracking on purpose says so and updates the Python comparison in `check.sh` in the same branch.
4. **Merge:** rebase onto `main`, fast-forward, one merge at a time. Delete the branch and worktree after.
5. **Parallel threads:** at most two at once, and only cards with no dependency between them and no shared files.
6. **Decisions** marked ⚑ wait for the user. A thread never makes them.

## Cards

| Card | What | Depends on | Touches | Model | Size |
|-|-|-|-|-|-|
| [E1](todos/E1-sidecar-anchor.md) | Anchor box per photo in the sidecar | — | `rust/src/main.rs` (sidecar), `analysis/mockup_data.py`, docs | Terra or Sonnet | S |
| [A1](todos/A1-tracking-benchmark.md) | Tracking accuracy and stability benchmark | E1 | `analysis/` (new), docs | Opus | M |
| [A2](todos/A2-tracker-sensitivity.md) | Tracker sensitivity to tiny pixel differences | A1 | `rust/src/track.rs`, `check.sh` expectations | Opus | M |
| [A3](todos/A3-track-edge.md) | Smaller tracking size for speed, if accuracy holds | A1 | `rust/src/track.rs` (constant), docs | Terra | S |
| [B2](todos/B2-capture-time-order.md) | Order photos by capture time | — | `rust/src/main.rs` (path order), `decode.rs` (EXIF read) | Terra | S |
| [B3](todos/B3-heic10-cost.md) | Cut 10-bit HEIC decode CPU | — | `rust/src/imageio.rs` | Terra | S |
| [B4](todos/B4-zoom-from-exif.md) | Seed lens-switch zoom from EXIF focal length | A1, B2 | `rust/src/track.rs`, `decode.rs` | Opus | M |
| [D1](todos/D1-linux-build.md) | Build and test on Linux | — | `rust/build.rs`, `rust/src/decode.rs`, `imageio.rs` gating | Opus | M |
| [B1](todos/B1-colour.md) ⚑ | Colour policy for Display P3 input | decision | `decode.rs`, `imageio.rs`, `avif.rs` | Opus | M |
| [C1](todos/C1-output-colour.md) | Colour profile in the output clip | B1 | `rust/src/avif.rs`, render | Terra | S |
| [E0](todos/E0-ui-decisions.md) ⚑ | Answer the UI plan's open questions | decision | `docs/ui-plan.md` | — | — |
| [E2](todos/E2-serve-endpoint.md) | enframe-serve endpoint that runs pinframe | E1, E0 | enframe repo | Opus | M |
| [E3](todos/E3-marking-screen.md) | Marking screen in enframe | E2 | enframe repo | Opus | L |
| [E4](todos/E4-preview-export.md) | Preview and export screen in enframe | E3 | enframe repo | Sonnet | M |
| [F1](todos/F1-python-reference.md) ⚑ | Keep or retire the Python reference | decision | `anchor.py`, `check.sh` | — | S |

Order is in [todos/_priority.md](todos/_priority.md). Safe pairs to run together today: **E1 + B3**, then **A1 + B2**.

## Decisions waiting for the user

- ⚑ **B1:** what to do with Display P3 photos (ignore, convert to sRGB, or keep and tag the output).
- ⚑ **E0:** the seven open questions in `docs/ui-plan.md`, starting with whether a box mark means its centre and whether dragging a box sets zoom.
- ⚑ **F1:** keep `anchor.py` as the reference that `check.sh` compares against, or retire it once the Rust tests carry their own fixtures.

## Out of scope for now

Interactive box drawing in the CLI (the UI does it), Windows, soft lock and full-frame mode (README "Next steps"), a WASM build (`docs/wasm.md`).
