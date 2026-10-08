#!/usr/bin/env bash
# Build the stabilize CLI once per compile-time option, cumulatively, into out/stabilize-<name>, and time
# single-threaded SIFT (fastest of 7) in each: bash analysis/stabilize/rust_builds.sh
set -euo pipefail
cd "$(dirname "$0")/../../src-tauri/stabilize"

build() {
	local name=$1
	shift
	env "$@" cargo build --release --quiet
	cp ../target/release/stabilize "../../out/stabilize-$name"
	local sift
	sift=$(env "$@" cargo test --release --quiet -- --ignored --nocapture sift_bench 2>/dev/null | grep -o 'fastest [0-9.]*ms')
	echo "$name: SIFT $sift, binary $(stat -f %z "../../out/stabilize-$name") bytes"
}

build default
build lto CARGO_PROFILE_RELEASE_LTO=fat
build cgu1 CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
build abort CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_PANIC=abort
build native CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_PANIC=abort \
	RUSTFLAGS='-C target-cpu=native'
