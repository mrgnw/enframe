#!/bin/sh
# Records only the prototype window while the native check drives it.
set -eu
cd "$(dirname "$0")/.."
OUT=evidence/native-drag.mov
PACE_MS=60 npm run -s check:native &
check=$!
for _ in $(seq 1 50); do
	window=$(uv run -q scripts/window_id.py 'gpuix framer spike' | cut -d' ' -f1)
	[ -n "$window" ] && break
	sleep 0.2
done
screencapture -x -v -V 8 -l "$window" "$OUT" &
wait "$check"
wait
ls -la "$OUT"
