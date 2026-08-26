#!/usr/bin/env bash
# Build enframe-serve and stage it where tauri.conf.json's externalBin expects it.
# Tauri wants the target triple in the filename and strips it again when bundling.
set -euo pipefail

cd "$(dirname "$0")/.."

TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
BINARIES=src-tauri/binaries

STAGED="$BINARIES/enframe-serve-$TRIPLE"

# enframe-serve depends on the app crate, whose build script refuses to run while the
# externalBin is missing — so stand a placeholder up before the build that replaces it.
mkdir -p "$BINARIES"
[ -e "$STAGED" ] || touch "$STAGED"

cargo build --release --manifest-path src-tauri/Cargo.toml -p enframe-serve
install -m 0755 src-tauri/target/release/enframe-serve "$STAGED"
