#!/usr/bin/env bash
# Profile the train clouds run N times and print median self-time per stage: [OUT=out/x.mov] analysis/stabilize/profile.sh 5 [--frames]
set -euo pipefail
cd "$(dirname "$0")/../.."

RUNS=${1:-5}
FRAMES=${2:-}
OUT=${OUT:-out/profile.mov}
mkdir -p out
log=out/profile.jsonl
walls=out/profile-wall.txt
rm -f "$log" "$walls"
TIMEFORMAT=%R
for _ in $(seq "$RUNS"); do
	rm -rf out/profile-frames
	if [[ "$FRAMES" == --frames ]]; then
		{ time PROFILE_LOG=$log RUN=analysis/stabilize/profile_stages.py bash analysis/stabilize/train_clouds_all.sh "$OUT" --frames out/profile-frames > /dev/null; } 2>> "$walls"
	else
		{ time PROFILE_LOG=$log RUN=analysis/stabilize/profile_stages.py bash analysis/stabilize/train_clouds_all.sh "$OUT" > /dev/null; } 2>> "$walls"
	fi
done
uv run analysis/stabilize/profile_stages.py --summarize "$log" "$walls"
