#!/usr/bin/env bash
# Profile the Girona run N times and print median self-time per stage: [OUT=out/x.mov] analysis/profile.sh 5 [--frames]
set -euo pipefail
cd "$(dirname "$0")/.."

RUNS=${1:-5}
FRAMES=${2:-}
OUT=${OUT:-out/profile.avif}
mkdir -p out
log=out/profile.jsonl
walls=out/profile-wall.txt
rm -f "$log" "$walls"
TIMEFORMAT=%R
for _ in $(seq "$RUNS"); do
	rm -rf out/profile-frames
	if [[ "$FRAMES" == --frames ]]; then
		{ time PROFILE_LOG=$log RUN=analysis/profile_stages.py bash analysis/girona_all.sh "$OUT" --frames out/profile-frames > /dev/null; } 2>> "$walls"
	else
		{ time PROFILE_LOG=$log RUN=analysis/profile_stages.py bash analysis/girona_all.sh "$OUT" > /dev/null; } 2>> "$walls"
	fi
done
uv run analysis/profile_stages.py --summarize "$log" "$walls"
