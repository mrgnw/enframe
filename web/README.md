# pinframe web

The MVP's SvelteKit app (Svelte 5, adapter-node): server routes that wrap the pinframe binary, and from M2 on, the marking and export screens. See `todos/M1-serve.md` and `docs/ui-plan.md`.

```sh
cd rust && cargo build --release && cd ..   # the binary the routes run
cd web && pnpm install
pnpm dev      # http://localhost:7341/, on every interface
pnpm build && pnpm start   # the production server on the same port
```

`PINFRAME_BIN` names another binary (default `rust/target/release/pinframe`), `PORT` another port. Paths in requests are absolute or relative to the repository root.

```sh
curl -s localhost:7341/api/track -H 'content-type: application/json' \
  -d '{"folder": "samples/train clouds", "marks": {"points": [[675,345],[985,345],[675,595],[985,595]]}}'
curl -sN localhost:7341/api/export -H 'content-type: application/json' \
  -d '{"folder": "samples/train clouds", "marks": {"points": [[675,345],[985,345]]}, "out": "out/web.avif"}'
```
