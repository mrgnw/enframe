# Changelog

Notable changes to enframe, newest first. Every pull request that changes what the app does adds a line under Unreleased in the same pull request. Dates are merge dates; there are no version tags yet.

## Unreleased

## 2026-10-08

### Added

- Stabilize: hold one feature still across a photo series and export a clip. Mark the feature on the first photo, fix the photos the tracker loses, preview the aligned frames, export. Reached from the command bar ("Stabilize a Series…") at `/stabilize` and `/stabilize/export`.
- Several named features per folder, each with its own marks, fix list and clip; a feature picker on the preview screen and "Export all".
- Soft marks: place a lost photo without making it a tracking reference (shift-click or `N`).
- A saved document whose first track fails offers Clear mark, Skip photo and Soft mark instead of a dead end.
- Marks are refused when any point would fall outside the photo.
- Export streams progress lines; the done state shows the clip inline with Download and Sidecar links, and Reveal in Finder when the instance is local.
- Serve routes `POST /api/stabilize/track`, `/api/stabilize/export` (server-sent events) and `/api/stabilize/reveal`. One run per folder; a newer request cancels the one in flight, which answers 409.
- Playwright end-to-end tests, opt-in through `bun run test:e2e`: they run a real serve on a copy of the train clouds samples and assert exported sidecars equal a direct run of the tracker.
- `just check-stabilize`: the tracker's own gate (build, lint, tests, every documented command, codec inputs, tracking baseline).

### Changed

- The pinframe repository is merged in with its history as the `stabilize` crate (`src-tauri/stabilize/`); serve calls it in-process. The standalone pinframe web app and GitHub repository are retired.
- The `serve` daemon in `pitchfork.toml` no longer needs a tracker binary path.

## 2026-09-12

### Added

- Connect to an instance beyond loopback: the app reaches a serve on another machine.
- The web target works offline once loaded (service worker, installable manifest).
- Linux deployment: a systemd unit and a Caddy example for a headless server, and a build status script.

## 2026-08-26

### Changed

- One-writer instance connection, phase 1: the app connects to a serve instance instead of opening SQLite itself; the server is the sole owner of its database.

## 2026-08-25

### Added

- Bookmarked folders replace the watch folder; a remote caller can list one.
- Serve runs under pitchfork.

### Changed

- The roots are the only boundary serve enforces; the loopback-only policy is gone.

## 2026-08-20

### Added

- RAW-only Lightroom thumbnails render in the desktop app, from the Lightroom proxy JPEG when there is one.

### Changed

- Reconfiguring routes are refused off loopback.

## 2026-08-14 and 2026-08-15

### Added

- The framer is the home page; the picker has its own route; recently opened images are remembered.
- Decode camera RAW in the browser; open a RAW in the standalone framer.
- Frame selection drives editing, export and delete; point multi-select and `[` `]` navigation; a hero crop preview follows the selection.
- Tests run through vitest.

## Before 2026-08-14

Image framing with focal points and crop frames, export to many aspect ratios, Lightroom catalog integration, the Tauri desktop app, the serve mode and the static web build. See `git log` and ROADMAP.md.
