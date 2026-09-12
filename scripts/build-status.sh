#!/usr/bin/env bash
# Show enframe-serve build progress and server status.
# One-shot by default; `./scripts/build-status.sh -w` refreshes every 5s.

set -u
TAURI_DIR="$(cd "$(dirname "$0")/../src-tauri" && pwd)"
BIN="$TAURI_DIR/target/release/enframe-serve"
PORT="${ENFRAME_PORT:-5911}"

status() {
  local total done pct
  total=$(grep -c '^name = ' "$TAURI_DIR/Cargo.lock")
  # Each dependency leaves an .rlib or .so in deps/ — a decent proxy for progress.
  done=$(find "$TAURI_DIR/target/release/deps" \( -name '*.rlib' -o -name '*.so' \) 2>/dev/null | wc -l)
  ((done > total)) && done=$total
  pct=$((done * 100 / total))

  echo "== enframe build status — $(date '+%H:%M:%S') =="
  echo "crates compiled : ~$done / $total ($pct%)"

  if pgrep -x rustc >/dev/null || pgrep -fx '.*cargo build.*' >/dev/null; then
    echo "cargo           : running ($(pgrep -cx rustc) rustc jobs)"
  else
    echo "cargo           : not running"
  fi

  if [[ -x "$BIN" ]]; then
    echo "binary          : ready ($(du -h "$BIN" | cut -f1), built $(date -r "$BIN" '+%H:%M:%S'))"
  else
    echo "binary          : not built yet"
  fi

  if curl -sf -o /dev/null --max-time 2 "http://127.0.0.1:$PORT/"; then
    echo "server          : responding on http://127.0.0.1:$PORT"
  else
    echo "server          : not responding on port $PORT"
  fi
}

if [[ "${1:-}" == "-w" ]]; then
  while true; do
    clear
    status
    sleep 5
  done
else
  status
fi
