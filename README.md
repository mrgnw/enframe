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

## Development

```sh
just dev
# or
bun run dev
```

## Headless serve mode

The same UI runs in a plain browser, backed by an HTTP server instead of Tauri.

```sh
bun run sveltekit:build
bun run serve
```

Then point a tunnel (Tailscale, cloudflared, ssh -R) at `127.0.0.1:5911`.

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

Browsers give no filesystem paths, so serve mode substitutes a server-side file picker
for the native dialogs, and drag-dropped files are uploaded into the inbox directory and
imported from there.

## Database Schema

- `photos`: imported images (local or from Lightroom)
- `frames`: crop rectangles on photos (x, y, width, height in 0-1)
- `focal_points`: important points that must stay visible (x, y in 0-1, with `required` flag)

Each frame can have its own focal points. Focal points with `frame_id = NULL` apply to the default (full-image) frame.


