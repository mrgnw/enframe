# Change notes

Each pull request that changes what the app does adds one note here, named after its branch: `changelog.d/<branch>.md`. Notes never conflict, because no two pull requests write the same file.

A note is bullets under one or more of `### Added`, `### Changed`, `### Fixed`, `### Removed`, written for someone using enframe:

```md
### Added

- Stabilize: the clip's canvas size is set on the preview screen.
```

At release time, `just changelog` merges every note into one dated entry in `CHANGELOG.md`, opens it in `$EDITOR` to change or rewrite, writes it, and deletes the notes. `just changelog --print` shows the draft and changes nothing.
