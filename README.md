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

All three come out of this one repo. The library is just one way to pick an image — the
static build has no backend to pick from, so `/` is the framer itself.

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

That runs in the foreground and stops when the terminal closes. To keep it running across
logouts and reboots, `services.toml` registers it with [kagaya](https://ky.xcc.es):

```sh
ky add && ky start
```

```sh
ky status
```

It runs the release binary directly, so build it once before the first start:

```sh
cargo build --release --manifest-path src-tauri/Cargo.toml -p enframe-serve
```

#### Reaching it from elsewhere

The server binds to loopback, so something has to bridge it. Tailnet-only, no public DNS:

```sh
tailscale serve --bg 5911
```

Or a throwaway Cloudflare tunnel, which prints a random `trycloudflare.com` URL:

```sh
cloudflared tunnel --url http://127.0.0.1:5911
```

For a stable hostname, create a named tunnel once and route DNS to it:

```sh
HOSTNAME=enframe.xcc.es
```

```sh
cloudflared tunnel create enframe
```

```sh
cloudflared tunnel route dns enframe "$HOSTNAME"
```

```sh
cloudflared tunnel run --url http://127.0.0.1:5911 enframe
```

The server reads and writes the same database as the desktop app
(`~/Library/Application Support/com.enframe.app/enframe.db`) and creates it with all
migrations applied if it does not exist yet. It has **no authentication** — it binds to
loopback only, and exposing it is the tunnel's job.

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

### Static web build

No server, no database, nothing to tunnel. `/` opens one image, you frame it in memory,
and it exports through the canvas and writes a sidecar JSON beside the output. `/edit/:id`
does not exist here and redirects home.

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
