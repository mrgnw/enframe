# D2 · Fetch the C library sources once, reliably

Depends on: — · Model: Sonnet · Size: S
Touches: `rust/build.rs`

## Why

Every new worktree downloads libaom, dav1d and libavif again in `build.rs`. On 2026-10-06 code.videolan.org's bot check served an HTML page instead of the dav1d tarball, so a fresh build failed its checksum; the agent had to copy sources from another build. Agents build in fresh worktrees all the time.

## Goal

- Keep verified tarballs in a shared folder outside `target/` (under the user cache, next to pinframe's feature cache), so each version downloads once per machine.
- If a download fails or its checksum does not match, try the project's official release mirror (dav1d: downloads.videolan.org) before failing, with the SHA-256 for that file.
- Fail with a message that names the URL and what came back (for example "got HTML, not a tarball").
- Report how long the C libraries take to build in a fresh worktree; if it is minutes, say whether sharing the built libraries is worth a follow-up card. Do not build that here.

## Done when

- [ ] A second fresh worktree builds without network access to the three source hosts (block them or work offline to prove it).
- [ ] `bash analysis/check.sh` passes.
