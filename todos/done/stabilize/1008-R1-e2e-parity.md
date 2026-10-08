---
branch: r1-e2e-parity (enframe)
---

# R1 · enframe e2e covers the pinframe web walkthrough

Depends on: E6 · Blocks: R2 · Model: Opus · Size: M
Issue: https://github.com/mrgnw/enframe/issues/15
Touches: enframe repo (`e2e/`, `pitchfork.toml`, README)

## Problem

enframe now has the whole pinframe UI (`/pin`, `/pin/export`, several features). pinframe's standalone web MVP (`web/`, port 7341) is a duplicate, but its Playwright walkthrough (`web/e2e/walkthrough.spec.ts`) is still the only browser test of several behaviours, and `analysis/check.sh --browser` is the only browser gate. The MVP cannot go until enframe's e2e covers the same ground.

## Goal

enframe's `bun run test:e2e` covers everything pinframe's walkthrough covers:

- a click at the edge of a lost photo at zoom 3 is refused and no mark is saved (M4)
- a hand-planted bad mark opens to the error naming the photo with Clear mark, Skip photo and Soft mark, and each works (M4, M5)
- a soft mark by shift-click and by `N` reports `soft`, clears with Backspace, and survives a reload (M5)
- the export done state shows the clip inline with Download and Sidecar links, and Reveal only when the instance is local (W2)
- whatever else `web/e2e/walkthrough.spec.ts` asserts that `e2e/*.spec.ts` does not yet

Also: `pitchfork.toml` passes `--pinframe` (default `$HOME/dev/pinframe/rust/target/release/pinframe`, overridable by `ENFRAME_PINFRAME`), so the local daemon runs the integrated app.

## Done when

- [x] A table in the PR body maps each assertion of `web/e2e/walkthrough.spec.ts` to the enframe spec and test that covers it.
- [x] `bun run test:e2e` passes; screenshots of the new states in `captures/r1/`.
- [x] `pitchfork start serve` on the Mac comes up with the pinframe binary logged at boot.

## Result

enframe's `bun run test:e2e` (6 tests) covers every assertion of `web/e2e/walkthrough.spec.ts`; the coverage table is in the enframe PR. New tests: the refused edge click, a planted bad mark with Clear / Soft / Skip recovery (Clear only on the first photo, which returns to the Points step), soft marks by shift-click and `N` surviving a reload, autoplay, files on disk, back to Fix, phone layouts, and the export done state from a second browser reaching serve as `enframe.test` to prove Reveal shows only on a local instance. `pitchfork.toml` passes `--pinframe "${ENFRAME_PINFRAME:-$HOME/dev/pinframe/rust/target/release/pinframe}"` (pitchfork runs `run` through `sh -c`). No bugs found in the UI.
