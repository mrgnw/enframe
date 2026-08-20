# Desktop Lightroom thumbnails for RAW-only assets

## Symptom

In the desktop app the Lightroom tab shows broken-image tiles for any asset whose only
local file is RAW (`.ARW`, `.DNG`, …). Serve mode is unaffected because `/img` transcodes
RAW to JPEG server-side; the desktop path hands the RAW bytes to WKWebView through the
asset protocol (`convertFileSrc`), and WebKit cannot decode them.

Thumbnails already prefer the Lightroom 2560px proxy JPEG when one exists locally
(`thumbnailPath` in `src/lib/lightroom.ts`), which covers cloud-synced albums. Recent
shoots typically have no local proxy, so the grid a photographer most wants — the newest
photos — is exactly the one that breaks.

## Fix direction

Desktop `thumbnailSrc` should detect a RAW extension and resolve through
`invoke('raw_as_jpeg', { path })` (command already exists in `src-tauri/src/commands.rs`,
returns embedded-preview JPEG bytes) into a blob URL, instead of `convertFileSrc`.

Notes:
- Make it async per tile with a bounded concurrency queue; 200 eager invokes would decode
  200 raws at once.
- `raw_as_jpeg` returns the embedded preview when it is close to full size; that is the
  cheap path (no full demosaic) and is exactly right for thumbnails.
- Revoke blob URLs on destroy or the grid leaks the decoded JPEGs.
- The pano that panics rawler (`IMG_0001-Pano.dng`, >50000px guard) will hit the same
  panic here — treat a failed invoke as "no preview", never crash the tile loop.
