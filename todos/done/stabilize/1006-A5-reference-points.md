# A5 · Several reference points instead of a box

Design card. Built as [A5a](A5a-points-tracker.md) (tracker) then [A5b](A5b-points-syntax.md) (syntax). Blocks: E3
Touches: `rust/src/track.rs`, `rust/src/main.rs` (CLI, sidecar), README, `docs/rust.md`, `analysis/check.sh`, `analysis/train_clouds_all.sh`

## Decision (2026-10-06)

The user marks the feature with one or more points instead of a box, and sees all of them in every photo. The tracker passes where the points were to the next photo as the hint for where to look (A4). Still one feature per clip.

## Goal

- The first photo and each mark take one or more points. The anchor that stays still in the clip is their centre.
- The tracker follows the keypoints near the points. The area around each point comes from the points' spacing and the keypoint density, not a fixed number; one point must still work.
- The fitted transform moves every point, and the sidecar's `points` (E1) lists them all.

## ⚑ Design first

Before building, the thread writes into this card and stops for the user's OK:

- the CLI syntax for points on the first photo and in `--at` marks, including zoom
- what happens to `--box` (kept as a shorthand, or removed)
- the rule for the area around each point

## Decisions (2026-10-06)

1. `--box` is removed.
2. Syntax as proposed: `x,y` per point, points joined with `/`, optional `@zoom` last (`--points x,y/x,y`, `--at PHOTO=x,y/x,y@2.5`).
3. A mark is exactly the points it lists; it replaces the set. The UI sends the moved pattern. Photos where the feature is hidden get a skip mark instead ([A6](A6-skip-photos.md)).
4. The anchor is the mean of the points.
5. Area rule as proposed: 96 keypoints, at least half the smallest point spacing, at most 512 px.
6. Split into A5a (tracker) and A5b (syntax).

## Design (proposed)

Written 2026-10-06 from a scratch prototype in `track.rs` (not committed) run through `analysis/tracking_benchmark.py`. Numbers below are measured, not estimated.

### CLI syntax

- A point is `x,y` in photo pixels. Points are joined with `/`. An optional zoom goes last as `@zoom`, relative to the first photo, as today.
- First photo: `--points x,y[/x,y]...`. It replaces `--box` and is required. It takes no zoom: the first photo defines scale 1.
- Later photos: `--at PHOTO=x,y[/x,y]...[@zoom]`.
- `/` and `@` need no shell quoting, so the server can pass marks as plain arguments.
- The old `x,y,w,h` form fails loudly ("a point is x,y, not 4 numbers") instead of being read as two points. A flat `x,y,x,y` list would silently turn every old box into two points.
- Limits: 1 to 32 points per mark, each inside its photo; zoom positive. Out-of-range input stops with a message.

The train clouds job (`analysis/train_clouds_all.sh`), rewritten. Each box becomes the four centres of its quarters, so the anchor (their mean) is the box centre as before; point marks are unchanged:

```sh
fixes=(
	--at IMG_4680.avif=485,1100/655,1100/485,1180/655,1180
	--at IMG_4684.avif=855,695/1005,695/855,845/1005,845@3
	--at IMG_4695.avif=1240,675/1360,675/1240,745/1360,745@1
	--at IMG_4697.avif=1230,655/1350,655/1230,725/1350,725@1
	--at IMG_4698.avif=940,637.5/1140,637.5/940,812.5/1140,812.5@2.5
	--at IMG_4703.avif=846,641.5/1046,641.5/846,816.5/1046,816.5
	--at IMG_4706.avif=960,705/1040,705/960,755/1040,755@1
	--at IMG_4708.avif=870,705/1050,705/870,795/1050,795@1
	--at IMG_4713.avif=1160,645/1320,645/1160,795/1320,795@2.5
	--at IMG_4716.avif=750,707.5/950,707.5/750,782.5/950,782.5@1
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
```

### `--box`: removed

Recommend removing it. The UI never makes a box, and the four-point form above covers every box mark mechanically. Keeping it as a shorthand would keep a second input form, its checks and its tests, for hand-typed commands only. `--box` then fails with "use --points; a box x,y,w,h becomes the four quarter centres" and the conversion rule.

### The area around each point

At the first photo and at every mark, on the downscaled photo SIFT ran on:

1. For each keypoint, the distance to the nearest point.
2. `r` = the 96th smallest of those distances (12 × `MIN_INLIERS`): the radius at which the discs hold 96 keypoints. Dense texture gives small discs, plain sky large ones.
3. `r` is at least half the smallest spacing between two points, so the discs of a spread pattern touch and cover the feature between the points. With one point this term is absent.
4. `r` is at most half of `TRACK_EDGE` (512 px). On train clouds it reached 313 px at most, so the cap never binds there.
5. The reference features are those within `r` of any point. One `r` for all points.

On a tracked photo the fit moves every point and multiplies `r` by the fitted zoom, as the box width is today; `r` is not re-derived per photo, so it cannot drift. The RANSAC tolerance uses the area's width (the points' extent plus `2r`) where it uses the box width today. The A4 search window is unchanged: it already works from the reference features' bounds.

Measured (first column is `main`; patterns are made from each box as its centre or its four quarter centres, and point marks replace the set):

| | box | 1 point, N=48 | 1 point, N=96 | 4 points, N=48 | 4 points, N=96 |
|-|-|-|-|-|-|
| accuracy median / max | 0.21 / 1.00 px | 0.20 / 1.29 px | 0.12 / 1.11 px | 0.16 / 1.30 px | 0.10 / 1.09 px |
| scale error max | 0.53% | 0.73% | 0.55% | 0.73% | 0.56% |
| train clouds LOST | 3 | 4 (+IMG_4795) | 3, same | 3, same | 3, same |
| train clouds median move vs box | | 0.4 px | 0.6 px | 0.4 px | 0.9 px |
| IMG_4689 move, imageio_avif | 7.8 px | | 0.9 px | | 0.9 px |
| IMG_4795 move, worst variant | 35.7 px | | 36.2 px | | 44.4 px |

N=24 lost two to three more train clouds photos and moved some variants 240–650 px; N=48 and 64 lose IMG_4795 with one point. 96 is the smallest tested count that holds with one point. Benchmark time is unchanged (20.5 s against 21.0 s).

How A5 checks it with the benchmark: the accuracy sequences pass `--points` (their box's four quarter centres) and a second pass with one point at the truth, so one point is scored on every run. The train clouds stability run reads the new `train_clouds_all.sh`. Pass when accuracy max stays within 1.5 px, LOST stays 0 synthetic and IMG_4675/4721/4725 on train clouds, and the regenerated baseline's moves are explained in `docs/tracking.md`.

### Marks on later photos and the anchor

- **A mark replaces the whole point set** on that photo. Tracking carries that set forward until the next mark. One meaning, and the UI sends exactly what it draws. Measured: one-point marks replacing the set keep the same LOST set as box marks (4 points, N=48 to 96).
- The CLI does not move or scale an earlier pattern. The UI's ghost does that: it takes the last linked photo's `points` from the sidecar, scales them about their centre by `zoom / scale`, moves them to the click, and sends the result as `--at PHOTO=…@zoom`. Adding or removing a point on a marked photo resends that photo's full list.
- **The anchor is the mean of the photo's points**, as `docs/ui-plan.md` already draws it. A fit moves the mean exactly as it moves the points, so tracked photos are unaffected by how many points there are. Adding a point at offset `d` from the centre of `n` points moves that photo's anchor by `d / (n + 1)`, and the photos tracked from it follow. That is a visible shift in the clip at that mark. It is the honest result of changing what is pinned, and the UI shows the centre, so the user sees it before tracking.
- Without `@zoom` the scale carries from the previous photo, as point and box marks do today.

### Sidecar

- `points`: every point of the photo, in the order given, normalized 0–1. Tracked photos: the given points moved by the fits. Marks: as given. Lost photos: the last found photo's points, scaled about their centre by the interpolated scale and moved to the interpolated centre. That is the faint estimate the UI draws and `Enter` accepts as a mark.
- Nothing else is needed. `scale` is the zoom, and the anchor is the mean of `points`. The keypoint radius stays internal.

### Risks

- **Accuracy:** measured equal or better than the box, apart from a max error 0.1 px higher. The prototype only made patterns from the existing boxes; points a person clicks will differ, so A5 re-measures with the real `train_clouds_all.sh`.
- **IMG_4689/4690:** discs remove their sensitivity: 7.8 to 0.9 px, and the largest rect change on `imageio_avif` drops from 1.60e-1 to 1.89e-2.
- **IMG_4795:** still has two fits 30–45 px apart: worst move 36–44 px with points against 36 px with the box. Discs do not fix it; it stays A2's problem.
- **Size:** about +80/−40 lines in `track.rs`, +40/−30 in `main.rs`, the benchmark's mark builder, `train_clouds_all.sh`, README, `docs/rust.md`, `docs/tracking.md`, regenerated baseline and `check.sh` expectations. Split it:
  - **A5a, tracker:** the point set, the area rule, `points` in the sidecar. The CLI stays as it is; `--box` and box marks become their four quarter centres internally, point marks become one-point sets. Regenerates the baseline. This is the change that moves numbers.
  - **A5b, syntax:** `--points`, `--at …/…@zoom`, `--box` removed, `train_clouds_all.sh` rewritten as above, benchmark and README. Because A5a uses the same points, the baseline must stay identical; any difference is a parsing bug.

### Decisions for you

1. **`--box`:** remove it (recommended), or keep it as a shorthand for the four quarter centres.
2. **Syntax:** `--points x,y/x,y` and `--at PHOTO=x,y/x,y@zoom` (recommended).
3. **A mark replaces the point set**, and the UI computes the moved pattern (recommended), or the CLI also gets a "move the last pattern here" form for hand-typed marks.
4. **Anchor = mean of the points** (recommended), accepting the shift when points are added at a mark.
5. **Area rule:** 96 keypoints, at least half the point spacing, at most 512 px (recommended).
6. **Split into A5a (tracker) and A5b (syntax)** (recommended), or one card.

## Done when

- [ ] The train clouds job runs with points only, and A1 shows accuracy and LOST count no worse than with boxes.
- [ ] README describes the new marks.
- [ ] `bash analysis/check.sh` passes.
