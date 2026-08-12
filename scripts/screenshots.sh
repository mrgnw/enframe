#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
TAURI_DIR="$ROOT_DIR/src-tauri"
FIXTURE_DB="$ROOT_DIR/tests/fixtures/test.db"

# Seed fixture DB if missing
if [ ! -f "$FIXTURE_DB" ]; then
	echo "Seeding fixture DB..."
	python3 "$SCRIPT_DIR/seed_fixture.py"
fi

# Build screenshot binary if needed
BINARY="$TAURI_DIR/target/release/examples/screenshots"
if [ ! -f "$BINARY" ] || [ "$TAURI_DIR/examples/screenshots.rs" -nt "$BINARY" ]; then
	echo "Building screenshot binary..."
	cargo build --release --features screenshots --manifest-path "$TAURI_DIR/Cargo.toml" --example screenshots
fi

# Build the app if needed
APP_BINARY="$TAURI_DIR/target/release/enframe"
if [ ! -f "$APP_BINARY" ]; then
	echo "Building Enframe app..."
	cargo build --release --manifest-path "$TAURI_DIR/Cargo.toml"
fi

echo "Running screenshots..."
"$BINARY" "$@"
