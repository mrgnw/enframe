#!/usr/bin/env bash
# All 57 Girona photos in one clip, with the anchor re-marked wherever tracking can't link two photos.
set -euo pipefail
cd "$(dirname "$0")/.."

OUT=${1:-out/all.avif}
# PAD= (empty) runs without padding, the tool's default crop.
PAD=${PAD-transparent}

fixes=(
	--at IMG_4680.avif=400,1060,340,160
	--at IMG_4684.avif=780,620,300,300,3.0
	--at IMG_4695.avif=1180,640,240,140,1.0
	--at IMG_4697.avif=1170,620,240,140,1.0
	--at IMG_4698.avif=840,550,400,350,2.5
	--at IMG_4703.avif=746,554,400,350
	--at IMG_4706.avif=920,680,160,100,1.0
	--at IMG_4708.avif=780,660,360,180,1.0
	--at IMG_4713.avif=1080,570,320,300,2.5
	--at IMG_4716.avif=650,670,400,150,1.0
	--at IMG_4718.avif=400,940,540,180,1.0
	--at IMG_4766.avif=750,80,600,340,1.0
	--at IMG_4767.avif=1000,200
	--at IMG_4768.avif=1000,230
	--at IMG_4769.avif=1000,400
	--at IMG_4770.avif=1000,330
	--at IMG_4772.avif=1000,380
	--at IMG_4773.avif=650,600
	--at IMG_4774.avif=700,400
	--at IMG_4780.avif=1000,300
	--at IMG_4791.avif=520,220,620,500,1.0
	--at IMG_4796.avif=1620,60
	--at IMG_4798.avif=1000,400
)

uv run "${RUN:-anchor.py}" "samples/girona clouds"/*.avif --box 1010,410,1030,400 "${fixes[@]}" ${PAD:+--pad "$PAD"} --out "$OUT" "${@:2}"
