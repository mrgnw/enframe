# Build Health

Critical fixes to make the project buildable and stable from a fresh clone.

## Tasks

- [ ] Commit `Cargo.toml` with `protocol-asset` feature flag (currently only in working tree)
- [ ] Sync `lib.rs` with `main.rs` (missing migrations, save_image command)
- [ ] Commit lockfiles — stop gitignoring `Cargo.lock` and `bun.lock`
- [ ] Remove unused Rust deps (`rmp-serde`, `kamadak-exif`)
- [ ] Fix `svelte.config.js` adapter placement (may be at wrong level)
- [ ] Fix template repo URL in `package.json` — point to enframe
- [ ] Remove dummy `advancedChunks` group in `vite.config.ts`
