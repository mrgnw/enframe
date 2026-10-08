---
branch: r2-retire-web-mvp
---

# R2 · Retire the standalone web MVP

Depends on: R1 · Model: Sonnet · Size: S
Issue: https://github.com/mrgnw/pinframe/issues/16
Touches: `web/` (deleted), `analysis/check.sh`, `README.md`, `docs/ui-plan.md`, `docs/wasm.md`, `PLAN.md` pointers

## Goal

One app. enframe is the UI; pinframe is the CLI and library. Delete `web/`, remove the `web:` steps and the `--browser` flag from `analysis/check.sh`, and point README ("The marking UI") and the docs at enframe's `/pin` and `bun run test:e2e`. The mockups under `mockups/` stay as the design record.

## Done when

- [x] `web/` is gone and `bash analysis/check.sh` passes with no web steps.
- [x] README and docs name enframe as the UI and say where the browser tests live.
- [x] Nothing listens on 7341.

## Result

`web/` deleted (50 files, 5318 lines). `analysis/check.sh` lost the `--browser` flag, the web build and `/api/track` round-trip steps and the matching `verify.py` assertion; it passes 19 of 19. README names enframe `/pin` as the UI and its `bun run test:e2e` as the browser walkthrough; PLAN "How work runs" step 3 says the same. Nothing listens on 7341. pinframe is the CLI and library; enframe is the app.
