# filmcraft (vendored)

`bitstream`, `h264enc` and `isobmff` from [storytold/filmcraft](https://github.com/storytold/filmcraft) at
`7bd76212629a067a9d1c5426d751de9faf4664b7` (2026-10-09), dual-licensed MIT OR Apache-2.0 (`LICENSE-MIT`,
`LICENSE-APACHE`). Stabilize uses them to write `.mp4` clips without ffmpeg (issue #30).

Changes from upstream:

- `Cargo.toml`: workspace inheritance replaced by literal values; dev-dependencies dropped.
- `bitstream` and `isobmff`: the `#[cfg(test)]` modules and `isobmff/tests` removed (they need upstream's
  `proptest` and `testkit`).

Fixes made here are ours to carry; note them in this list.
