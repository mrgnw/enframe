---
branch: claude/amazing-mendel-xmtd5j
---

# W2 · Spike: the export's done state hands over the clip

Depends on: M3 · Model: Sonnet · Size: S
Touches: `web/src/lib/preview/Done.svelte` (new), `web/src/routes/api/clip/`, `web/src/routes/api/reveal/`, `web/src/lib/server/pinframe.ts`

## Question

M3's done state showed only the clip's path, a dead end on a phone. Can the screen show the clip itself and hand it over?

## Result

Kept. `GET /api/clip?path=` streams a clip (`.avif`, `.mp4`, `.mov`, `.webm`) or the `.json` beside one, with range requests (206, `Accept-Ranges`, 416) and the photo route's cache headers; other extensions get 400. The done state (`Done.svelte`) shows an AVIF as an image and MP4 or WebM as a muted looping video with controls, a MOV as a download only, with Download, Sidecar and Export again links and the size, frame count, fps and duration; a clip the browser cannot play falls back to the download. At 390 px the clip fills the column. `POST /api/reveal` runs `open -R` on macOS only; the button shows only when the page is on localhost, since from a phone it would open Finder on the Mac mini. Time to first paint of a 14 MB transparent AVIF, measured in Chromium with throttling: 0.3 s at 1 Gbit, 1.4 s at 100 Mbit, 5.9 s at 20 Mbit; an AVIF paints only once fully loaded, where a video starts sooner, so on Wi-Fi or Tailscale MP4 or WebM feel faster. Browser support from memory, not tested: animated AVIF in Chrome, Firefox 93+ and Safari 16.4+; H.264 everywhere; VP9 WebM in Chrome and Firefox, Safari 14.1+ probably without alpha; ProRes MOV nowhere in a browser. The Chromium here lacks H.264, so the MP4 path was only seen through its fallback.
