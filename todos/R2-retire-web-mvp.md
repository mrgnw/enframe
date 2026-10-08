# R2 · Retire the standalone web MVP

Depends on: R1 · Model: Sonnet · Size: S
Issue: https://github.com/mrgnw/pinframe/issues/16
Touches: `web/` (deleted), `analysis/check.sh`, `README.md`, `docs/ui-plan.md`, `docs/wasm.md`, `PLAN.md` pointers

## Goal

One app. enframe is the UI; pinframe is the CLI and library. Delete `web/`, remove the `web:` steps and the `--browser` flag from `analysis/check.sh`, and point README ("The marking UI") and the docs at enframe's `/pin` and `bun run test:e2e`. The mockups under `mockups/` stay as the design record.

## Done when

- [ ] `web/` is gone and `bash analysis/check.sh` passes with no web steps.
- [ ] README and docs name enframe as the UI and say where the browser tests live.
- [ ] Nothing listens on 7341.
