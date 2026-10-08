#!/usr/bin/env bash
# All 57 train clouds photos in one clip, with the anchor re-marked wherever tracking can't link two photos.
set -euo pipefail
cd "$(dirname "$0")/../.."

OUT=${1:-out/all.avif}
# PAD= (empty) runs without padding, the tool's default crop. BIN= overrides the binary (e.g. to add
# flags or point at a different build); it defaults to the release build.
PAD=${PAD-transparent}
BIN=${BIN:-src-tauri/target/release/stabilize}

fixes=(
	--at IMG_4680.avif=485,1100/655,1100/485,1180/655,1180
	--at IMG_4684.avif=855,695/1005,695/855,845/1005,845@3
	--at IMG_4695.avif=1240,675/1360,675/1240,745/1360,745@1
	--at IMG_4697.avif=1230,655/1350,655/1230,725/1350,725@1
	--at IMG_4698.avif=940,637.5/1140,637.5/940,812.5/1140,812.5
	--at IMG_4703.avif=846,641.5/1046,641.5/846,816.5/1046,816.5
	--at IMG_4706.avif=960,705/1040,705/960,755/1040,755@1
	--at IMG_4708.avif=870,705/1050,705/870,795/1050,795@1
	--at IMG_4713.avif=1160,645/1320,645/1160,795/1320,795
	--at IMG_4716.avif=750,707.5/950,707.5/750,782.5/950,782.5
	--at IMG_4718.avif=535,985/805,985/535,1075/805,1075@1
	--at IMG_4766.avif=900,165/1200,165/900,335/1200,335@1
	--at IMG_4767.avif=1000,200
	--at IMG_4768.avif=1000,230
	--at IMG_4769.avif=1000,400
	--at IMG_4770.avif=1000,330
	--at IMG_4772.avif=1000,380
	--at IMG_4773.avif=650,600
	--at IMG_4774.avif=700,400
	--at IMG_4780.avif=1000,300
	--at IMG_4791.avif=675,345/985,345/675,595/985,595@1
	--at IMG_4796.avif=1620,60
	--at IMG_4798.avif=1000,400
)

$BIN "samples/train clouds"/*.avif --points 1267.5,510/1782.5,510/1267.5,710/1782.5,710 "${fixes[@]}" ${PAD:+--pad "$PAD"} --out "$OUT" "${@:2}"
