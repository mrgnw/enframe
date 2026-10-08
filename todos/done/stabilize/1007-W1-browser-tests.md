---
branch: claude/amazing-mendel-xmtd5j
---

# W1 · Spike: browser tests for the web app, in the repository

Depends on: M3 · Model: Sonnet · Size: S
Touches: `web/e2e/` (new), `web/playwright.config.ts`, `web/package.json`, `analysis/check.sh` (opt-in flag)

## Question

M2 and M3 were verified with Playwright scripts that stayed outside the repository, so the Mac could not reproduce the browser checks. What does it take to bring them in, and should `check.sh` gate on them?

## Result

Kept, as an opt-in. `@playwright/test` 1.56.1 is a devDependency of `web/`; `web/e2e/walkthrough.spec.ts` runs the whole MVP on a sandbox: 57 hard links of the sample photos under `out/` (5 to 8 ms, `listPhotos` accepts them; the document and the exports land there, and the test fails if the user's own `samples/train clouds.pin.json` changes). It opens the folder, places 4 points, tracks (53 lost), fixes 25 photos with 15 clicks and 10 skips to "All 57 linked", previews, exports, and compares the exported sidecar to a CLI run on the saved marks: identical. A second test seeds the marks from `analysis/train_clouds_all.sh` and matches that script's run. Eight screenshots go to `out/e2e/shots/` as artifacts, not pixel-diffed, since the mockup is a scenario rather than the tracker's output. `pnpm test:e2e` runs it; `bash analysis/check.sh --browser` adds it as a check after the round trip. Without the flag `check.sh` is unchanged.

Cost on the 4-core Linux host: 93 s warm, 137 s cold; `check.sh --browser` takes about 90 s more than `check.sh`. Recommendation, taken: opt-in, run for cards that touch `web/`, not a default gate, because it adds half again to every card, needs a one-time Chromium download on the Mac (`cd web && pnpm install && pnpm exec playwright install chromium`, about 150 MB under `~/Library/Caches/ms-playwright`), and depends on UI selectors. Headless needs no display or flags on macOS, by Playwright's docs; not run on a Mac yet.

Found on the way: clicking a lost photo right after auto-advance can place the ghost's points outside the photo, which the CLI refuses (`point -346,200 is outside the 1536x2048 photo`); the bad mark is already saved, so the folder fails to open next time. Card [M4](../M4-marks-in-bounds.md). The test works around it by landing on each photo afresh.
