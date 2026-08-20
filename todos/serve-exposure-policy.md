# Serve exposure policy

## Why

`services.toml` runs serve with `--host 0.0.0.0` so a tailnet address reaches it without a
proxy. That also answers on the local network, and serve has no authentication. Today the
only thing bounding the damage is the filesystem root allowlist (`src-tauri/serve/src/roots.rs`)
and the SQL authorizer — a remote caller still gets the full API surface, including writes.

The interesting question is not "add a password". It is which capabilities should be
reachable over a network at all, versus only from the machine itself.

## Ideas to explore

**Split the API by trust, not by user.** *Shipped:* a `local_only` route list checked
against `ConnectInfo<SocketAddr>` refuses `/api/ls`, `/api/scan_inbox`, `save_image`, and
`crop_and_export` with a 403 off loopback, leaving the database, image reads, and inbox
uploads remote-usable.

The operations that were inherently local-only and lost nothing by being refused over a
non-loopback connection:

- setting the watch folder (`WatchDir`) — picking a directory to ingest from is an
  administrative act; the remote UI can use one already configured
- `--read-root` / `--write-root` equivalents, if root management ever becomes runtime —
  still open, since roots are start-up only today
- `save_image` / `crop_and_export` writing to arbitrary allowlisted paths
- the server-side file browser (`/api/ls`) that backs the remote picker

The flag turned out cheaper than a per-route field: one `local_only(path)` in a
middleware, sharing its list of caller-chosen writes with the root confinement so the two
cannot drift. Framing held: the *reading and framing* loop is safe to expose; the
*reconfiguration* surface is not.

**A config file only writable locally.** Something like `~/Library/Application Support/
com.enframe.app/serve.toml`, read at startup, declaring which interfaces to bind, which
roots are allowed, and which capabilities are remote-reachable. Serve refuses to modify it
over HTTP, so the exposed surface can never widen its own permissions. Check ownership and
mode on load and refuse a world-writable file.

**Interface allowlist rather than `0.0.0.0`.** Bind explicitly to named interfaces
(loopback + tailnet) instead of everything. Binding the tailnet IP alone already works
(verified) but fails at boot if launchd starts before Tailscale, and drops `localhost` —
so this needs either retry-until-bindable or multi-listener support.

**Read-only mode.** A flag serving the framer and library for viewing with every mutating
route refused. Useful for showing work on a phone without risking the library.

## Notes

- Do not reach for auth first. Cloudflare Access or a bearer token in front is orthogonal
  and the user has said exposure is theirs to manage; the goal here is that being exposed
  is *less dangerous by construction*.
- Related hardening already shipped: root allowlist with canonicalisation (blocks `..` and
  symlink escapes), `SQLITE_LIMIT_ATTACHED = 0` plus an authorizer denying ATTACH/PRAGMA,
  and a 60s deadline on `/img`.
- The desktop app is unaffected by all of this and should stay unrestricted — the user is
  at the keyboard.
