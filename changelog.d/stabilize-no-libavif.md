### Changed

- Stabilize exports `.mp4` by default. A transparent background now needs MOV or WebM, and the export panel switches to WebM when you pick one.
- Stabilize on macOS reads AVIF photos through the system's image decoder. Tracking can differ from before by up to about 2 px on a few photos.

### Removed

- Stabilize no longer writes animated AVIF clips.
