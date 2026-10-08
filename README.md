# Enframe

Image framing tool for setting focal points and crops with Lightroom integration.

## Features

### Implemented ✅

- Tauri v2 + SvelteKit 5 foundation (using native WebView, not Servo)
- SQLite database with schema (photos, frames, focal_points)
- Tauri plugins: sql, fs, dialog
- Database abstraction layer
- Photo browser UI
- Editor route structure
- Lightroom integration stubs

### Planned

1. **File import** — drag-drop to add local JXL/JPEG files
2. **Lightroom browser** — read local library and import photos
3. **Editor canvas** — place focal points (draggable), draw crop frames (resizable)
4. **Export** — crop photos using Canvas API, save to disk, export metadata JSON

## Architecture

- **Frontend**: SvelteKit 5 + Svelte 5 (runes)
- **Backend**: Tauri v2 (Rust)
- **Database**: SQLite via `tauri-plugin-sql`
- **Image format**: JXL (natively supported in macOS WebKit)

All coordinates stored as normalized 0-1 values for resolution independence.

## 📋 Prerequisites

Make sure you have the following installed:

- **[Bun](https://bun.sh/)** (latest version) - Package manager and runtime
- **[Rust](https://rustup.rs/)** (latest stable) - Required for Tauri
- **[Node.js](https://nodejs.org/)** (v22.12+) - Fallback runtime support

### Platform-Specific Requirements

#### Windows
- Microsoft C++ Build Tools or Visual Studio with C++ support

#### macOS
- Xcode Command Line Tools: `xcode-select --install`

#### Linux (Ubuntu/Debian)
```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

## Setup

```sh
bun install
```

## Three ways to run enframe

| | What you get | Backend |
| --- | --- | --- |
| Desktop app | Library, Lightroom import, watch folder, framer | SQLite via Tauri |
| Headless serve | The same UI in a browser, behind a tunnel | SQLite over HTTP |
| Static web build | The standalone framer only, one image at a time | none |

All three come out of this one repo. `/` is the framer itself everywhere, with the images
you opened recently under it. The library is just one way to pick an image: it lives at
`/library`, linked from the home page, and the static build has no backend to link to.

### Desktop app

```sh
bun run dev
```

Release build:

```sh
bun run build
```

### Headless serve mode

The same UI runs in a plain browser, backed by an HTTP server instead of Tauri.

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

It runs the release binary directly, so build it once before the first start:

```sh
cargo build --release --manifest-path src-tauri/Cargo.toml -p enframe-serve
```

A supervised process inherits none of your terminal's privacy grants, so photos in
iCloud Drive, Photos, Desktop or Documents are unreadable until you grant the binary
**Full Disk Access** (System Settings → Privacy & Security → Full Disk Access → add
`src-tauri/target/release/enframe-serve`). Without it those reads hang rather than fail,
so `/img` gives up after 60s with a 504. Photos under `~/Pictures` are unaffected, and
running `bun run serve` from a terminal sidesteps it entirely.

`pitchfork.toml` passes `--host 0.0.0.0`, so the port answers on every interface — a
tailnet address reaches it directly with nothing to proxy, and so does anything else on
the same network. To keep it off the local network, bind the tailnet address instead
(`--host <tailscale-ip>`); the cost is that `localhost` stops working and the service
fails to start if it comes up before Tailscale does. Default `127.0.0.1` when run by hand.

The server owns the same database the desktop app connects to
(`~/Library/Application Support/com.enframe.app/enframe.db`) and creates it with all
migrations applied if it does not exist yet. It takes an exclusive lock on that database
at startup, so a second server against the same file refuses to start and names the port
of the one already serving it. It has **no authentication**: who can reach the port is
yours to decide, with a tunnel, a firewall, or `--host`.

Overrides, as flags or environment variables:

| Flag | Env | Default |
| --- | --- | --- |
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

#### What serve mode can touch

Unlike the desktop app, the server confines every path it accepts to a set of roots.
Paths are canonicalised first, so `..` and symlinks cannot escape. By default the roots
are `~/Pictures`, the directories of photos already in the library (so an iCloud Drive or
external-volume library keeps working), the database directory, and the inbox.

Both flags are repeatable; the env vars take a colon-separated list. Add a root when a
photo lives somewhere the library has not seen yet — for example a volume that was not
mounted at startup:

```sh
PHOTOS=/Volumes/disk/photos
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

#### Stabilize

Stabilize holds one feature still across a photo series: mark it on the first photo,
Track, fix the photos it lost, then export a clip. It is the `stabilize` crate in
`src-tauri/stabilize/`, a library serve calls in-process and a CLI for scripts; see
[docs/stabilize/README.md](docs/stabilize/README.md). It was developed as
[pinframe](https://github.com/mrgnw/pinframe), now archived. Open it from the command bar
("Stabilize a Series…") or at `/stabilize?folder=PATH`.

- `POST /api/stabilize/track` `{folder, features, feature, pad?}` saves the document,
  `features` (`[{name, marks}]`, one tracker run each), beside the folder as
  `<folder>.stabilize.json` (a `<folder>.pin.json` is still read when that is absent),
  tracks the feature named `feature` without rendering, using the feature cache, and
  answers with the sidecar: each photo's crop, plus its size and the tracker's note. One run per folder: a newer request stops the one in flight.
- `POST /api/stabilize/export` `{folder, features, feature, out?, pad?, fps?, frames?, vf?}`
  renders the feature's clip, `<folder>-<feature>.avif` by default, and answers with server-sent events: `progress`
  `{line}` for each line the tracker reports, then `done` `{out, sidecar}` or `error`
  `{error, superseded?}`. A bad request still answers 400 or 403 before anything runs.
  Closing the connection stops the run.
- `POST /api/stabilize/reveal` `{path}` shows a file inside a read root in Finder
  (`open -R`). Off macOS it answers 501.

The finished clip and its `.json` sidecar are read through `/img/{path}`, like photos.

`marks` is `{points, at?, near?, skip?, order?}`, the CLI's `--points`, `--at`, `--near`,
`--skip` and `--order`. The folder must be inside a read root, and the document, the clip
and `frames` inside a write root. One run per folder: a newer request cancels the one in
flight, which answers 409 (an export already streaming ends with `error`
`{superseded: true}` instead), so the newest mark is the only one that gets a result.

#### On a Linux host

`pitchfork.toml` and the Full Disk Access note above are macOS concerns. On a headless
Linux server, `deploy/enframe-serve.service` runs the same binary under systemd:

```sh
sudo cp deploy/enframe-serve.service /etc/systemd/system/
```

```sh
sudo systemctl enable --now enframe-serve
```

Build the binary with `scripts/sidecar.sh` rather than cargo directly. The app crate's
build script refuses to run until both the staged sidecar binary and a `build/` directory
exist, and the error it gives for the second one names `frontendDist`, which is a long way
from the cause.

That unit leaves the server on `127.0.0.1` and puts TLS and the address gate in front of
it instead — `deploy/Caddyfile.example` is that half, terminating HTTPS on a DNS-01
certificate and answering 403 to anything outside the tailnet range, the host's own public
interface included. Serve still has no authentication of its own; this only decides who
reaches the port.

### Static web build

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
`bun run serve` hands out. Check the config without publishing:

```sh
wrangler deploy --dry-run
```

Deploying needs a Cloudflare account you have already logged into (`wrangler login`):

```sh
bun run deploy:web
```

Wrangler publishes it to `enframe.<your-subdomain>.workers.dev`. To put it on your own
hostname, add a custom domain to the `enframe` Worker in the Cloudflare dashboard under
Workers & Pages → enframe → Settings → Domains & Routes:

```sh
HOSTNAME=enframe.example.com
```

Cloudflare creates the DNS record for `$HOSTNAME` itself, as long as the zone is already
on your account.

## Database Schema

- `photos`: imported images (local or from Lightroom)
- `frames`: crop rectangles on photos (x, y, width, height in 0-1)
- `focal_points`: important points that must stay visible (x, y in 0-1, with `required` flag)

Each frame can have its own focal points. Focal points with `frame_id = NULL` apply to the default (full-image) frame.

## License

Enframe itself is MIT — see [LICENSE](LICENSE).

The **web build** is the one exception worth knowing about. Browsers cannot decode camera
RAW, so `bun run build:web` bundles [`libraw-wasm`](https://github.com/ybouane/LibRaw-Wasm)
(ISC) which embeds [LibRaw](https://www.libraw.org/) compiled to WebAssembly. LibRaw is
offered under either LGPL-2.1 or CDDL-1.0, and **enframe elects LGPL-2.1**. That election
carries a relinking obligation: anyone who receives the web build must be able to swap in
their own LibRaw. The build keeps that possible by emitting the decoder as standalone,
replaceable artifacts rather than inlining it — `_app/immutable/workers/worker-*.js` and
`_app/immutable/workers/assets/libraw-*.wasm` — which can be rebuilt from LibRaw source
and dropped in place. LibRaw's source, and the script that compiles it, live in the
upstream `libraw-wasm` repository.

The desktop and `serve` builds are unaffected: they decode RAW through the Rust `rawler`
crate and never link LibRaw.
