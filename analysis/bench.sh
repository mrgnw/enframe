#!/usr/bin/env bash
# Run the Girona benchmark N times with stage timing and print each run's total and peak memory.
set -euo pipefail
cd "$(dirname "$0")/.."
for _ in $(seq "${1:-3}"); do
	RUN=analysis/stage_times.py bash analysis/girona_all.sh out/bench.mov | grep -E '^(load|track|features_in|place|render|png save|total|peak)' | tr -s ' ' | paste -sd ' ' -
done
uv run analysis/compare_rects.py out/base.keep.json out/bench.json
