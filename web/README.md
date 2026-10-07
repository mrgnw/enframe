# pinframe web

The MVP's SvelteKit app (Svelte 5, adapter-node): server routes that wrap the pinframe binary, and from M2 on, the marking and export screens. See `todos/M1-serve.md` and `docs/ui-plan.md`.

```sh
cd rust && cargo build --release && cd ..   # the binary the routes run
cd web && pnpm install
pnpm dev      # http://localhost:7341/, on every interface
pnpm build && pnpm start   # the production server on the same port
```

`PINFRAME_BIN` names another binary (default `rust/target/release/pinframe`), `PORT` another port. Paths in requests are absolute or relative to the repository root.

The marking screen (M2, from `mockups/01-marking.html`) is at `/?folder=PATH`: a folder with no document opens on its first photo for the reference points, then Track; a folder with a saved document tracks at once and opens on the first lost photo. `?trail=flow` or `fade` picks another trail style. The screen's code is `src/lib/marking/`: `session.svelte.ts` holds the document, the sidecar and the mockup's rules, `trail.ts` draws the overlay, `Stage`, `Filmstrip` and `Marking` are the components. Thumbnails are made in the browser from the photos themselves. The routes it uses:

- `GET /api/document?folder=`: the folder's photos by name and its saved document, or null.
- `GET /api/photo?folder=&file=`: the photo's bytes, for the stage and the thumbnails.

The preview and export screen (M3, from `mockups/02-preview.html`) is at `/preview?folder=PATH`, which the marking screen's Preview button opens. It tracks the saved document, draws each photo at its sidecar crop rect into a frame the size of the clip (what the CLI's `place()` does), and loops the frames in the browser, so pad colour and fps change the picture at once; only switching pad between none and a colour re-tracks, because the rects overhang. Its code is `src/lib/preview/`. `POST /api/track` takes an optional `pad` (`transparent`, `black`, `white` or `#rrggbb`; absent is the CLI's default crop), and the export panel posts `/api/export` and reads its event stream.

```sh
curl -s localhost:7341/api/track -H 'content-type: application/json' \
  -d '{"folder": "samples/train clouds", "marks": {"points": [[675,345],[985,345],[675,595],[985,595]]}}'
curl -sN localhost:7341/api/export -H 'content-type: application/json' \
  -d '{"folder": "samples/train clouds", "marks": {"points": [[675,345],[985,345]]}, "out": "out/web.avif"}'
```
