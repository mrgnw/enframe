# Running Enframe

[Back to the README](../README.md)

| | What you get | Backend |
|-|-|-|
| Desktop app | Library, Lightroom import, watch folder, framer | SQLite via local sidecar |
| Headless serve | The same UI in a browser, behind a tunnel | SQLite over HTTP |
| Static web build | The standalone framer only, one image at a time | none |

All three come out of this one repo. `/` is the framer itself everywhere, with the images
you opened recently under it. The library is just one way to pick an image: it lives at
`/library`, linked from the home page, and the static build has no backend to link to.

## Desktop app

```sh
bun run dev
```

Release build:

```sh
bun run build
```

## Headless serve mode

After the [source setup](../README.md#quick-start-macos), the same UI runs in a browser,
backed by an HTTP server instead of Tauri. It serves your own local images to you, with no authentication, and is meant for localhost or your
own tailnet, not the public internet.

```sh
bun run sveltekit:build
```

```sh
bun run serve
```

That runs in the foreground and stops when the terminal closes. To keep it running after
the terminal closes, `pitchfork.toml` registers it with
[pitchfork](https://pitchfork.jdx.dev):

```sh
pitchfork start serve
```

```sh
pitchfork status serve
```

It does not start at login; add `boot_start = true` to `pitchfork.toml` if you want that.

It runs the release binary directly, so build and stage it once before the first start:

```sh
bun run sidecar
```

A supervised process inherits none of your terminal's privacy grants, so photos in
iCloud Drive, Photos, Desktop or Documents are unreadable until you grant the binary
**Full Disk Access** (System Settings → Privacy & Security → Full Disk Access → add
`src-tauri/target/release/enframe-serve`). Without it those reads hang rather than fail,
so `/img` gives up after 60s with a 504. Photos under `~/Pictures` are unaffected, and
running the serve command from a terminal sidesteps it entirely.

`pitchfork.toml` passes `--host 0.0.0.0`, so the port answers on every interface — a
tailnet address reaches it directly with nothing to proxy, and so does anything else on
the same network. To keep it off the local network, bind the tailnet address instead
(the `--host` flag with your tailnet IP address); the cost is that `localhost` stops working and the service
fails to start if it comes up before Tailscale does. Default `127.0.0.1` when run by hand.

The server owns the same database the desktop app connects to
(`~/Library/Application Support/com.enframe.app/enframe.db`) and creates it with all
migrations applied if it does not exist yet. It takes an exclusive lock on that database
at startup, so a second server against the same file refuses to start and names the port
of the one already serving it. It has **no authentication**: who can reach the port is
yours to decide, with a tunnel, a firewall, or `--host`.

Overrides, as flags or environment variables:

| Flag | Env | Default |
|-|-|-|
| `--host` | `ENFRAME_HOST` | `127.0.0.1` |
| `--port` | `ENFRAME_PORT` | `5911` |
| `--db` | `ENFRAME_DB` | app-support `enframe.db` |
| `--web` | `ENFRAME_WEB` | `build` |
| `--inbox` | `ENFRAME_INBOX` | app-support `inbox` |
| `--read-root` | `ENFRAME_READ_ROOTS` | see below |
| `--write-root` | `ENFRAME_WRITE_ROOTS` | see below |

Browsers give no filesystem paths, so serve mode substitutes a server-side file picker
for the native dialogs, and drag-dropped files are uploaded into the inbox directory and
imported from there.

### What serve mode can touch

Unlike the desktop app, the server confines every path it accepts to a set of roots.
Paths are canonicalised first, so `..` and symlinks cannot escape. By default the roots
are `~/Pictures`, the directories of photos already in the library (so an iCloud Drive or
external-volume library keeps working), the database directory, and the inbox.

Both flags are repeatable; the env vars take a colon-separated list. Add a root when a
photo lives somewhere the library has not seen yet — for example a volume that was not
mounted at startup:

```sh
PHOTOS="$HOME/Pictures"
```

```sh
bun run serve --read-root "$PHOTOS" --write-root "$PHOTOS"
```

The effective roots are logged at boot.

The roots are the whole boundary. Every caller gets the same API surface no matter which
address it connects from, and what bounds a request is where the path lands, not who sent
it: reads must resolve inside a read root, writes inside a write root. An earlier version
also refused a handful of routes off loopback, which was dropped when the app gained the
ability to connect to an instance on another machine — that app is an off-loopback caller
too, so the address test would have let it browse a library and then refused it every
export. Widen the roots and you widen what any caller can reach.

### Stabilize

Stabilize holds one feature still across a photo series: mark it on the first photo,
Track, fix the photos it lost, then export a clip. It is the `stabilize` crate in
`src-tauri/stabilize/`, a library serve calls in-process and a CLI for scripts; see the
[Stabilize guide](stabilize/README.md). It was developed as
[pinframe](https://github.com/mrgnw/pinframe), now archived. Open it from the command bar
("Stabilize a Series…") or at `/stabilize?folder=PATH`.

Its gate builds, lints and tests the crate, runs every command its README documents and
compares the train clouds job with the tracking baseline. It needs the samples linked at
`samples/`:

```sh
just check-stabilize
```

- `POST /api/stabilize/track` `{folder, features, feature, pad?}` saves the document,
  `features` (`[{name, marks}]`, one tracker run each), beside the folder as
  `<folder>.stabilize.json` (a `<folder>.pin.json` is still read when that is absent),
  tracks the feature named `feature` without rendering, using the feature cache, and
  answers with the sidecar: each photo's crop, plus its size and the tracker's note. One run per folder: a newer request stops the one in flight.
- `POST /api/stabilize/export` `{folder, features, feature, out?, pad?, fps?, frames?, vf?}`
  renders the feature's clip, `<folder>-<feature>.mp4` by default, and answers with server-sent events: `progress`
  `{line}` for each line the tracker reports, then `done` `{out, sidecar}` or `error`
  `{error, superseded?}`. A bad request still answers 400 or 403 before anything runs.
  Closing the connection stops the run.
- `POST /api/stabilize/reveal` `{path}` shows a file inside a read root in Finder
  using the platform reveal action. Off macOS it answers 501.

The finished clip and its `.json` sidecar are read through `/img/{path}`, like photos.

`marks` is `{points, at?, near?, skip?, order?}`, the CLI's `--points`, `--at`, `--near`,
`--skip` and `--order`. The folder must be inside a read root, and the document, the clip
and `frames` inside a write root. One run per folder: a newer request cancels the one in
flight, which answers 409 (an export already streaming ends with `error`
`{superseded: true}` instead), so the newest mark is the only one that gets a result.

### On a Linux host

`pitchfork.toml` and the Full Disk Access note above are macOS concerns. On a headless
Linux server, `deploy/enframe-serve.service` runs the same binary under systemd:

```sh
sudo cp deploy/enframe-serve.service /etc/systemd/system/
```

```sh
sudo systemctl enable --now enframe-serve
```

Build the binary with the sidecar script rather than cargo directly. The app crate's
build script refuses to run until both the staged sidecar binary and a `build/` directory
exist, and the error it gives for the second one names `frontendDist`, which is a long way
from the cause.

That unit leaves the server on `127.0.0.1` and puts TLS and the address gate in front of
it instead — `deploy/Caddyfile.example` is that half, terminating HTTPS on a DNS-01
certificate and answering 403 to anything outside the tailnet range, the host's own public
interface included. Serve still has no authentication of its own; this only decides who
reaches the port.

## Static web build

No server, no database, nothing to tunnel. `/` opens one image, you frame it in memory,
and it exports through the canvas and writes a sidecar JSON beside the output. `/library`
and `/edit/:id` do not exist here and redirect home.

Once it has loaded, it keeps working without the network. A service worker precaches the
shell and the wasm codecs, so a reload offline still opens the app and a RAW opened for
the first time offline still develops. Only the AVIF encoder variant that can actually run
on the origin is stored, which is the difference between 8.8 and 5.4 MiB. A manifest makes
it installable, and a pill in the corner reports the install while it happens.

None of that reaches the desktop build, which serves from `tauri://` where a worker cannot
register and whose assets are already on disk.

```sh
bun run build:web
```

The site lands in `build-web/`, separate from `build/` so it never shadows what
the serve command hands out. Check the config without publishing:

```sh
wrangler deploy --dry-run
```

Deploying needs a Cloudflare account you have already logged into (sign in first):

```sh
bun run deploy:web
```

Wrangler prints the workers.dev address after publishing. For a custom hostname, open
Workers & Pages → enframe → Settings → Domains & Routes in the Cloudflare dashboard.
Cloudflare creates the DNS record when the zone is already on your account.
