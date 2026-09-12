# Serve exposure policy

## Why

`pitchfork.toml` runs serve with `--host 0.0.0.0` so a tailnet address reaches it without a
proxy. That also answers on the local network, and serve has no authentication. Today the
only thing bounding the damage is the filesystem root allowlist (`src-tauri/serve/src/roots.rs`)
and the SQL authorizer — a remote caller still gets the full API surface, including writes.

The interesting question is not "add a password". It is which capabilities should be
reachable over a network at all, versus only from the machine itself.

## Ideas to explore

**Split the API by trust, not by user.** *Shipped, then removed 2026-08-26.* A `local_only`
route list checked against `ConnectInfo<SocketAddr>` refused `/api/ls`, `save_image`, and
`crop_and_export` with a 403 off loopback, leaving the database, image reads, inbox
uploads, and folder listing remote-usable.

It came out in phase 2 of [one-writer-instance-connection](one-writer-instance-connection.md).
Once the app can connect to an instance on another machine, that app is an off-loopback
caller: it would have browsed the library and then been refused every export, and without
identity there is no telling it from a stranger. Roots are the only boundary now. The rest
of this section is kept as the record of what the address test was reasoning about — the
capability-reduction ideas below are still live, and are the ones worth pursuing, because
they narrow what *any* caller can do rather than guessing at who is calling.

The operations that were inherently local-only and lost nothing by being refused over a
non-loopback connection:

- `--read-root` / `--write-root` equivalents, if root management ever becomes runtime —
  still open, since roots are start-up only today
- `save_image` / `crop_and_export` writing to arbitrary allowlisted paths
- the server-side file browser (`/api/ls`) that backs the remote picker

The flag turned out cheaper than a per-route field: one `local_only(path)` in a
middleware, sharing its list of caller-chosen writes with the root confinement so the two
cannot drift. Framing held: the *reading and framing* loop is safe to expose; the
*reconfiguration* surface is not.

*Revised 2026-08-21:* `scan_inbox` moved back to remote-safe. It was classified as an
administrative act when the watch folder was the only caller; the bookmarked-folders tab
made it the read path behind ordinary browsing, and it has always been confined to a read
root, so a remote caller can only list a directory it already knows inside the allowlist.

**The remote picker — resolved by freeing it.** `/api/ls` is root-confined (`check_read`
on the requested dir, parent link restricted to read roots), so the "it browses the server"
framing always overstated it: it cannot escape the roots either. It was kept local-only on
the narrower ground that enumerating the tree is *discovery*, not framing, at the cost of a
remote browser having no way to add a folder bookmark at all. Freed with the rest of the
address test.

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
