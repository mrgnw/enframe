#!/usr/bin/env bash
# Build the Rust prototype once per compile-time option, cumulatively, into out/pinframe-<name>, and time
# single-threaded SIFT (fastest of 7) in each: bash analysis/rust_builds.sh
set -euo pipefail
cd "$(dirname "$0")/../rust"

build() {
	local name=$1
	shift
	env "$@" cargo build --release --quiet
	cp target/release/pinframe "../out/pinframe-$name"
	local sift
	sift=$(env "$@" cargo test --release --quiet -- --ignored --nocapture sift_bench 2>/dev/null | grep -o 'fastest [0-9.]*ms')
	echo "$name: SIFT $sift, binary $(stat -f %z "../out/pinframe-$name") bytes"
}

build default
build lto CARGO_PROFILE_RELEASE_LTO=fat
build cgu1 CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
build abort CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_PANIC=abort
build native CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_PANIC=abort \
	RUSTFLAGS='-C target-cpu=native'
