# One writer: instances that connect to each other

## Why

The desktop app and `serve` both open `~/Library/Application Support/com.enframe.app/enframe.db`
directly — the app through `tauri-plugin-sql`, serve through rusqlite. Same file, two
writers. WAL allows concurrent readers but only one writer, and serve sets no
`busy_timeout`, so a save from one while the other holds the write lock fails immediately
with "database is locked".

The goal is one writer, and beyond that the paseo model: any instance can attach to any
other, so the laptop app can drive the library that lives on another machine.

## The shape

**Every instance is a server; the frontend is always a client.** Whoever owns the database
serves it. The native app, a phone browser, and another Mac are all clients of that one
writer. The app stops reaching into SQLite.

This collapses complexity rather than adding it, because `src/lib/bridge.ts` already
contains a complete HTTP implementation of every capability — and the browser client has
already solved every problem a remotely-connected app has: no local filesystem, uploads
instead of path-carrying drag-drop, a server-side picker, `/img/<path>` instead of
`convertFileSrc`. Connecting the app to another instance means running the browser data
path inside the native shell.

### Split the overloaded boolean

`isTauri()` currently answers two unrelated questions with one value:

- **`hasNativeShell()`** — window chrome, drag region, native dialogs. A runtime capability.
- **`connection`** — base URL of the instance that owns the data. Everything else routes
  through it.

Welding these together is exactly why the app cannot be a native shell over a remote
writer. Splitting them is the core of the change; the ~15 `isTauri()` call sites in
`bridge.ts` each pick one meaning or the other.

## One writer, guaranteed

Two mechanisms, because probing alone is not enough:

1. **Probe, then spawn.** At launch the app tries its target (default
   `http://127.0.0.1:5911`). Reachable → connect. Not reachable → spawn the bundled
   `enframe-serve` as a Tauri sidecar, wait for the port, connect.
2. **Database ownership lock.** The probe is racy and only covers the port it knows, so a
   second server could still open the same file on another port. At startup the server
   takes an exclusive advisory lock (`flock`) on a lock file beside the database and holds
   it for the process lifetime; a second server fails the lock and exits with a message
   naming the holder's port, so the app can connect to it instead of spawning. An advisory
   lock is used rather than SQLite's own locking because SQLite only contends per
   transaction — it would not stop a second server from starting and interleaving writes.
   This is what makes one-writer structural rather than conventional.

`PRAGMA busy_timeout` goes into `serve/src/db.rs` regardless, as a safety net during the
transition and for any case the lock does not cover.

### Sidecar, not in-process

`enframe-serve` depends on `app_lib` (`serve/Cargo.toml`), so the dependency runs
server → app. The app cannot host the axum router in-process without a dependency cycle or
extracting a Tauri-free core crate — a real refactor, since `commands.rs`, `lightroom.rs`
and `inbox.rs` carry `#[tauri::command]` annotations. The sidecar gets the same behaviour
with no crate surgery: bundle `enframe-serve` as an `externalBin` and clean up the child on
quit. Revisit only if in-process buys something the sidecar cannot.

## Paths are the server's vocabulary

The app passes absolute filesystem paths everywhere (`photos.file_path`, export targets,
Lightroom resolution). That works only while both sides share a filesystem. Once connected
to another machine, a path means whatever it means *on the server*, and the client must
never interpret it locally:

- image rendering → `${base}/img/${path}`, never `convertFileSrc`
- picking files → the server-side picker (`/api/ls`), not the native dialog, because you
  are choosing files that live on the server
- dropping local files → upload, as the browser already does
- native dialogs survive only for genuinely local acts (choosing a local export folder)

## Trust model: roots become the only boundary

Full remote collides with the loopback policy. A connected app is, to serve, an
off-loopback caller: it would 403 on `save_image`, `crop_and_export` and `ls`, so it could
browse but never export. Without identity there is no way to distinguish "my other
instance" from a stranger, and this project has ruled out auth.

**Decision: drop the loopback test entirely.** Root confinement plus the SQL authorizer
become the whole defence, and network exposure stays the operator's concern. This
supersedes the shipped policy in `serve-exposure-policy.md`.

What is actually lost: the guarantee that a network caller cannot make the server write
anywhere, even inside a write root. What remains: canonicalised read/write roots (no `..`,
no symlink escape), `SQLITE_LIMIT_ATTACHED = 0`, an authorizer denying ATTACH/PRAGMA, and
the `/img` deadline. One clearly-stated defence beats two that disagree with each other.

## Exports: destination chosen per export

`crop_and_export` always encodes server-side into a write root. If the user picked a local
destination, the client then fetches each output over `/img/<path>` and writes it with the
native dialog. One encode path, two endings, no new transport.

## Gaps to close

- **`raw_thumbnail` is missing from serve's `dispatch()`** — registered in `lib.rs` for the
  desktop app only, which is why RAW tiles fall back to placeholders in the browser. A
  remotely-connected app would have the same hole.
- **`busy_timeout`** unset in `serve/src/db.rs`.
- **`tauri-plugin-sql`** can leave the app once the frontend always talks HTTP; migrations
  already single-source from `app_lib::migrations::MIGRATIONS`, so only the executor goes.

## Phases

Ordered so each is independently verifiable, not to reduce scope.

1. **One writer, same machine.** App always talks HTTP; sidecar spawned when no instance
   answers; database ownership lock; `busy_timeout`. Kills the two-writer problem on its
   own.
2. **Remote targets.** Connection UI (default `127.0.0.1:5911`, remembered instances, URL
   field on failure), path vocabulary, `raw_thumbnail` in serve, policy change.
3. **Export destination choice.**

## Risks

- A connected app that loses its server mid-session needs a coherent reconnect story rather
  than a wall of failed fetches.
- Spawning a sidecar while pitchfork also runs one is the exact race the ownership lock
  exists to catch; the lock must be tested, not assumed.
- Latency: every DB read becomes an HTTP round trip. Negligible on loopback, real over a
  tailnet — list views that issue many small queries may need batching.

## Testing

- Ownership lock: second server against the same database refuses to start.
- Probe-then-spawn: with an instance already running, the app connects and spawns nothing.
- Remote path handling: a client with no access to the server's filesystem renders
  thumbnails, imports, and exports.
- Policy change: extend `serve/src/main.rs` tests so the removal of the loopback test is
  asserted deliberately rather than by deleting the tests that covered it.
