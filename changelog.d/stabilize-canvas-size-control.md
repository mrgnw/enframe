### Added

- Stabilize: the clip's canvas size and each photo's zoom are set on the preview screen. A photo whose feature tracked small was magnified to keep the feature the same size and showed only a patch of itself; its zoom can now be set by hand, and the canvas grown to show more around the feature. Both are saved in the document's marks (`zoom`, `canvas`), so they hold across sessions. CLI: `--zoom PHOTO=scale`, `--canvas WxH`.
