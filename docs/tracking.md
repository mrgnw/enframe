# Tracking benchmark

```sh
uv run analysis/tracking_benchmark.py
```

It builds the release binary, runs the Rust CLI in JSON-only mode with a temporary `--cache`, prints one summary line and writes `out/tracking_benchmark.json`. It takes about 26 s on the M4 Pro. Runs are deterministic: two runs give identical reports apart from `seconds`.

`analysis/tracking_baseline.json` is the report for the current tracker; a branch that changes tracking regenerates it and explains every change. A tracking change compares its report against it. `stability.original` holds every Girona photo's point, scale, source and crop rect, the same values the `girona_all.sh` sidecar has, so `check.sh` can compare against it once the Python reference retires (F1).

## Accuracy

Five synthetic sequences, 31 frames in total. Each one warps a single Girona photo with a known zoom, rotation and offset, so the true anchor in every frame is exact. Frames are 1024 px on the long edge, the size SIFT runs at. Zooms below 0.75x sample a box-filtered half-size photo, so zooming out does not alias. Frame 0 is the mark, the box's four quarter centres; the other 26 are scored against the mean of each frame's points.

| sequence | photo | what changes |
|-|-|-|
| shift | IMG_4698 | anchor 0–230 px from the frame centre |
| crop | IMG_4713 | frame size 560×720 to 1024×768, no resampling |
| zoom | IMG_4791 | 1x, 2x, 2x, 1x, 3x, 1x, 1.5x, 3x: lens-switch jumps of 2x and 3x |
| rotate | IMG_4716 | −5° to +8°, with shifts |
| mixed | IMG_4698 | zoom, rotation and shift together, up to 3.3x |

Baseline on `main`:

| sequence | max anchor error | max scale error | LOST |
|-|-:|-:|-:|
| shift | 0.14 px | 0.05% | 0 |
| crop | 0.03 px | 0.03% | 0 |
| zoom | 0.83 px | 0.21% | 0 |
| rotate | 0.15 px | 0.13% | 0 |
| mixed | 1.09 px | 0.56% | 0 |
| all 26 frames | median 0.10 px | | 0 |

The tracker is accurate to about a pixel on clean geometric changes, including 3x jumps. Errors are in frame pixels; LOST frames would be counted, not scored.

## Stability

The real Girona job (`analysis/girona_all.sh`: its box, 23 marks and `--pad transparent`) on the 57 AVIF originals, then on copies with slightly different pixels. Movement is the distance between each photo's point in the copy and in the original, in original pixels (2048 on the long edge). Rect is the largest change in the normalized crop rect, the measure `check.sh` and `docs/rust.md` use.

| variant | how | max move | median | max scale change | max rect change | LOST set |
|-|-|-:|-:|-:|-:|-|
| imageio_avif | same AVIF bytes, ftyp brand relabelled so pinframe decodes them through ImageIO | 2.2 px (IMG_4777) | 0.15 px | 0.7% | 1.49e-2 (IMG_4704) | same |
| imageio_png | `sips -s format png`: 16-bit PNG, which pinframe truncates to 8 bits | 0.9 px (IMG_4705) | 0.10 px | 0.7% | 9.59e-3 | same |
| heic | `sips -s format heic` | 2.8 px (IMG_4777) | 0.12 px | 0.9% | 7.78e-3 | same |
| jpeg_q90 | `sips -s format jpeg`, quality 90 | 2.1 px (IMG_4726) | 0.09 px | 0.7% | 1.01e-2 | same |
| pillow_png | Pillow's libavif decode, as PNG | 0 | 0 | 0 | 0 | same |
| noise_2 | Pillow's decode plus uniform noise in [−2, 2] levels, fixed seed | 1.9 px (IMG_4777) | 0.14 px | 0.8% | 1.29e-2 | same |

The baseline LOST set is IMG_4675, IMG_4721 and IMG_4725 in every variant.

- **No photo stands out.** The worst move in any variant is 2.8 px and the largest rect change 1.49e-2. IMG_4795, which used to jump 30–45 px between two fits, moves at most 1.8 px; see "RANSAC (A2)". IMG_4689/4690 move under 1 px since points (A5a).
- `pillow_png` moving nothing confirms that Pillow and pinframe decode AVIF to identical pixels, so `noise_2` adds noise to exactly the pixels pinframe sees.
- The median move stays near 0.1 px everywhere: the sensitivity is a few photos, not the whole run.

## Search window (A4)

The tracker now matches near the reference first and falls back to the whole photo (`docs/rust.md`, "Search window"). The baseline was regenerated in that branch. Before → after:

| measure | whole photo | window |
|-|-|-|
| accuracy, all 26 frames | median 0.213 px, max 0.999 px, scale max 0.526%, 0 LOST | identical, frame by frame |
| imageio_avif max / median move | 7.8 px (IMG_4690) / 0.11 px | 32.7 px (IMG_4795) / 0.14 px |
| imageio_png | 29.3 px (IMG_4795) / 0.14 px | 35.7 px (IMG_4795) / 0.13 px |
| heic | 44.7 px (IMG_4795) / 0.12 px | 32.9 px (IMG_4795) / 0.12 px |
| jpeg_q90 | 33.6 px (IMG_4795) / 0.09 px | 3.6 px (IMG_4795) / 0.12 px |
| noise_2 | 3.9 px (IMG_4689) / 0.09 px | 30.4 px (IMG_4795) / 0.11 px |
| LOST set, every variant | IMG_4675, IMG_4721, IMG_4725 | same |

- **The original run's IMG_4795 switched fit.** It moved 32.6 px (scale 1.076 → 1.089), so the variants that used to land on the other fit now agree with it and the ones that agreed now differ. A visual check of both points against IMG_4794 does not settle which is right: the cloud deforms between the two photos.
- **Every other photo moved at most 1.5 px** (IMG_4777) in the original run, and the variants' worst move outside IMG_4795 is unchanged (7.8 px at IMG_4690). The medians rose by up to 0.03 px.

## Choices

- `NOISE_LEVELS = 2`: the size of the ImageIO-vs-libavif difference in `docs/rust.md`.
- `FRAME = 1024×768`: the tracker's `TRACK_EDGE`, so synthetic frames are tracked at full size.
- Sequence offsets and zooms are picked by hand to cover the card's cases; frames that would leave the photo are shifted back inside and the truth follows the frame actually rendered.

## Points (A5a)

The tracker follows a set of points instead of a box (`todos/done/1006-A5-reference-points.md`, "Design"). It keeps the features within one radius of any point: the radius that holds 96 keypoints, at least half the smallest point spacing, at most 512 px, set at the first photo and each mark and scaled by each fit's zoom. The fit moves every point; the anchor is their mean. The command line is unchanged: `--box` and box marks become the box's four quarter centres (their mean is the box centre), and a point mark becomes a one-point set that replaces the current set. The baseline was regenerated in that branch. Before → after:

| measure | box | points |
|-|-|-|
| accuracy, all 26 frames | median 0.213 px, max 0.999 px, scale max 0.526%, 0 LOST | median 0.096 px, max 1.092 px, scale max 0.56%, 0 LOST |
| imageio_avif max / median move, max rect | 32.7 px (IMG_4795) / 0.14 px, 1.60e-1 | 44.4 px (IMG_4795) / 0.16 px, 1.89e-2 |
| imageio_png | 35.7 px (IMG_4795) / 0.13 px, 2.68e-2 | 6.5 px (IMG_4777) / 0.10 px, 1.18e-2 |
| heic | 32.9 px (IMG_4795) / 0.12 px, 1.70e-2 | 37.3 px (IMG_4795) / 0.13 px, 2.15e-2 |
| jpeg_q90 | 3.6 px (IMG_4795) / 0.12 px, 1.39e-2 | 41.7 px (IMG_4795) / 0.09 px, 1.67e-2 |
| noise_2 | 30.4 px (IMG_4795) / 0.11 px, 1.00e-1 | 41.5 px (IMG_4795) / 0.14 px, 1.85e-2 |
| IMG_4689 move, imageio_avif / noise_2 | 7.8 px / 3.9 px | 0.9 px / 0.2 px |
| LOST set, every variant | IMG_4675, IMG_4721, IMG_4725 | same |

These match the design prototype's measurement (median/max 0.10/1.09 px, IMG_4689 0.9 px, IMG_4795 worst 44.4 px).

- **Accuracy:** the median halves. The max rises 0.09 px, on the mixed sequence's 3.3x frame; the zoom sequence's max falls 0.17 px and its scale error rises from 0.09% to 0.21%. All within the 1.5 px bound.
- **The original run moved 41 of 57 photos by at most 1 px; median 0.18 px.** Every photo now tracks from discs instead of a box, so the reference features differ a little everywhere.
- **IMG_4777 moved 7.3 px** (scale 0.993 → 0.973). It is tracked from the point mark on IMG_4774. With the box, a point mark kept the size of the last box (IMG_4766's 600×340); now the point is a one-point set whose disc holds 96 keypoints, a smaller area. Its neighbour IMG_4775 moved 1.1 px. In the variants it is the second most sensitive photo (up to 6.5 px).
- **IMG_4795 moved 7.3 px** (scale 1.089 → 1.062). It is the photo with two fits 30–45 px apart (A4); tracked from the IMG_4791 mark, now four discs, it lands 7.3 px from the box result, and the variants still split between fits by up to 44.4 px. Discs do not fix it; it stays A2's problem.
- **IMG_4722–4729 moved 1.5–2.2 px**, at scale 2.6 after the IMG_4718 box mark (540×180): under 1 px at first-photo scale. IMG_4725 is lost; it follows its interpolated neighbours. IMG_4670, 4673, 4690, 4793 and 4794 moved 1.7–2.0 px for the same reason.
- **Rects are steadier.** The largest rect change in any variant falls from 1.60e-1 to 2.15e-2: discs remove IMG_4689/4690's sensitivity, the photos at scale 3.0 where it mattered most.
- **Sidecar:** `points` now lists every point. Tracked photos: the points moved by the fits. Marks: as given (a box as its four quarter centres). Lost photos: the last found photo's points, scaled about their centre by the interpolated scale and moved to the interpolated anchor. `check.sh` and the benchmark take the anchor as the points' mean.
- `AREA_KEYPOINTS = 96` (12 × `MIN_INLIERS`): the smallest tested count that keeps the LOST set with one point; 48 and 64 lose IMG_4795, 24 loses two to three more. `MAX_RADIUS = 512 px`, half of `TRACK_EDGE`; Girona's largest radius is 313 px, so the cap does not bind there.

## RANSAC (A2)

The fit now draws all 2000 RANSAC samples, scores each by its inliers less a penalty on its turn, and refits on the inliers until they settle (constants in `rust/src/track.rs`). The baseline was regenerated in that branch. Before → after:

| measure | before | after |
|-|-|-|
| accuracy, all 26 frames | median 0.096 px, max 1.092 px, scale max 0.56%, 0 LOST | identical, frame by frame |
| imageio_avif max / median move, max rect | 44.4 px (IMG_4795) / 0.16 px, 1.89e-2 | 2.2 px (IMG_4777) / 0.15 px, 1.49e-2 |
| imageio_png | 6.5 px (IMG_4777) / 0.10 px, 1.18e-2 | 0.9 px (IMG_4705) / 0.10 px, 9.59e-3 |
| heic | 37.3 px (IMG_4795) / 0.13 px, 2.15e-2 | 2.8 px (IMG_4777) / 0.12 px, 7.78e-3 |
| jpeg_q90 | 41.7 px (IMG_4795) / 0.09 px, 1.67e-2 | 2.1 px (IMG_4726) / 0.09 px, 1.01e-2 |
| noise_2 | 41.5 px (IMG_4795) / 0.14 px, 1.85e-2 | 1.9 px (IMG_4777) / 0.14 px, 1.29e-2 |
| IMG_4795 move, worst variant | 44.4 px | 1.8 px |
| LOST set, every variant | IMG_4675, IMG_4721, IMG_4725 | same |

### Why IMG_4795 had two fits

Coordinates here are on the 1024 px tracked image (half the original).

- **Few matches, split evenly.** IMG_4794's four discs hold about 100 keypoints; about 30 pass the ratio test into IMG_4795. Searching every pair of matches finds two fits with the same support: fit A turns +0.3° (scale 1.06, anchor near (711, 126)) and fit B turns +8.9° (scale 1.05, anchor near (725, 113)), 14–18 inliers each. They share 8 inliers. A's own inliers are on the bottom of the cloud (points 3 and 4); B's are on its right side (points 2 and 4). The cloud changed shape: IMG_4795 was taken 9.2 s after IMG_4794, where the other steps in this run are 1.5–2.5 s apart.
- **RANSAC stopped too early to tell them apart.** OpenCV's rule stops once it is 99% sure of one all-inlier sample: about 20 draws at half inliers. The original run kept a 14-inlier fit near B (anchor (727, 112)) when 17-inlier fits exist. A tiny pixel change shifts the match list, so the same seeded draws pick different pairs and land on A, B or a third fit.
- **Drawing all 2000 samples alone is not enough.** The best count then flips between A and B with the pixels: 17 to 14 for A in the original run, 16 to 15 for B in `jpeg_q90`, 18 to 17 for B in `noise_2`. JPEG and noise variants still moved 38 px.

### Which fit is right

Fit A, the one that turns 0.3°:

- **The cloud's outline.** The cloud's silhouette (bright, unsaturated pixels connected under the points) in IMG_4794, mapped by each fit onto IMG_4795's: A overlaps by 0.71 (intersection over union), B by 0.65; with a stricter brightness threshold, 0.62 and 0.53. The best similarity found by searching turns +1° and puts the anchor at (716, 130), 5–7 px from A and 19 px from B.
- **The rest of the photo.** Matching IMG_4794 to IMG_4795 over the whole photo, the sky's features turn +0.8° and the horizon −0.1°. The camera did not roll and the cloud as a whole did not turn 9°; B follows only the right side's change of shape.
- **Every other step.** Girona's tracked steps turn at most 1.4°.
- **The IMG_4796 mark cannot decide it.** Tracked back into IMG_4795, the mark lands at (517, 302), on a different, smaller cloud: IMG_4796's mark follows a new feature, not the one tracked from IMG_4791.

### The fix

- **All 2000 draws** (`RANSAC_DRAWS`), so the best fit is found, not the first good one. On its own: accuracy median 0.096 → 0.119 px (two zoom frames), IMG_4795 still split in two variants.
- **Turn penalty** (`TURN_SPREAD = 3°`): a sample scores its inliers less (turn / 3°)² / 2, so B's 9° costs 4.5 inliers and A's 0.3° nothing. Spreads of 2°, 3° and 5° give identical results; 10° leaves `jpeg_q90` and `noise_2` on B. A fit with clear support keeps its turn: the synthetic rotate sequence, with steps up to 13°, is unchanged.
- **Refit until the inliers settle** (`REFIT_ROUNDS = 10`): after the least-squares fit, recount its inliers and refit, until the set stops changing. It settles in at most 2 rounds on Girona and the synthetic sequences. Without it, the turn penalty alone leaves the accuracy median at 0.119 px and heic's largest rect change at 2.64e-2; with it, accuracy is identical to before frame by frame.
- **Tried and dropped:** larger areas (192 and 384 keypoints instead of 96) moved IMG_4795 by 8–160 px across variants; searching every pair of matches instead of random draws left IMG_4795 split the same way; keeping OpenCV's early stop with the other two changes left IMG_4795 at 3.2 px in `imageio_png`.
- **Cost:** none measurable; the benchmark takes 20–22 s before and after.

### What changed in the original run

44 of 57 photos are identical. IMG_4795 moved 40.5 px to fit A (scale 1.062 → 1.080). IMG_4793 and IMG_4794 moved 2.3 px, IMG_4777 2.5 px, and IMG_4721–4729 0.8–1.6 px. The last run is tracked at scale 2.6 from the IMG_4718 mark, where each pixel of anchor shift is a large crop shift: it held the old largest rect change, 2.15e-2 at IMG_4724 in `heic`, where the run moved 2–3 px. It now moves at most 1.2 px and heic's largest rect change is 7.78e-3.

## Track size (A3)

`TRACK_EDGE` (1024) sets the longest edge SIFT runs at (`track_factor`); the cache key already includes
`k` (`cache::key`), so changing it needs no cache version bump. Two other constants are expressed in
tracking pixels but need no code change: `MAX_RADIUS` is defined as `TRACK_EDGE / 2.0`, so it scales on
its own, and `RANSAC_TOLERANCE` is a ratio multiplied by `area_width`, itself computed from points and
radii on the downscaled photo, so it scales with whatever `TRACK_EDGE` is in effect. `AREA_KEYPOINTS` is a
count, not a distance, and `MOTION_MARGIN` a multiplier on the measured `reach`; neither needs a size term.

896 and 768 were tried by changing only `TRACK_EDGE` and rebuilding, no other edits:

| measure | 1024 (current) | 896 | 768 |
|-|-|-|-|
| accuracy, all 26 frames | median 0.096 px, max 1.092 px, scale max 0.56%, 0 lost | median 0.127 px, max 1.504 px, scale max 0.363%, 0 lost | median 0.111 px, max 0.913 px, scale max 0.34%, 0 lost |
| LOST set, every variant | IMG_4675, IMG_4721, IMG_4725 | imageio_avif and noise_2 lose IMG_4681 instead | same as 1024 |
| worst stability move | 2.8 px (heic, IMG_4777) | 240.4 px (imageio_avif/noise_2, IMG_4681) | 6.0 px (noise_2, IMG_4795) |
| largest crop change | 1.49e-2 (imageio_avif) | 8.99e-1 (imageio_avif) | 3.21e-2 (heic) |
| cold Girona, median of 5 | 2.244 s | 2.082 s (7.2% faster) | 1.847 s (17.7% faster) |
| warm Girona, median of 5 | 1.134 s | 1.121 s | 1.133 s |

Cold runs point `--cache` at a fresh empty directory each time; warm runs reuse one cache directory across
the 5 reps. Sizes were interleaved within each repetition (cold 1024, 896, 768, then warm 1024, 896, 768,
repeated 5 times) to average out any drift.

- **896 fails outright.** Accuracy max (1.504 px) is already past the 1.5 px bound, IMG_4681 joins the
  LOST set in two variants, and the worst stability move (240 px) is two orders of magnitude over
  baseline: a different, wrong fit. 896 px must be cutting too close to some feature set's edge.
- **768 passes accuracy** (max 0.913 px, same LOST set) but fails stability: heic's largest crop change
  (3.21e-2) and noise_2's worst move (6.0 px) both exceed the baseline-noise bounds (2e-2, 5 px).
- **Neither saves enough to matter anyway.** 768 would have saved 17.7% of cold wall time, over the 10%
  bar, but it fails stability. 896 saves only 7.2%, under the bar regardless of its accuracy failure.
- **Warm-cache time is flat across sizes** (1.12-1.13 s): once features are cached, almost all remaining
  work (decode, crop, encode) is independent of `TRACK_EDGE`.

`TRACK_EDGE` stays at 1024; nothing in `rust/src` changed and `analysis/tracking_baseline.json` is
unchanged.

### 16-bit PNG

pinframe still drops the low byte of 16-bit PNG. Rounding instead was measured: `imageio_png` then gives exactly `imageio_avif`'s numbers (2.2 px, median 0.15 px, rect 1.49e-2), because sips writes ImageIO's own decode, so rounding reproduces ImageIO's 8-bit result. Truncation gives 0.9 px, median 0.10 px, rect 9.59e-3. Neither matters for stability, and rounding takes a conversion of its own rather than a one-line change, so it is left as is.

## Lens from EXIF (B4)

The Girona originals keep their EXIF. pinframe now reads `FocalLengthIn35mmFormat` with the capture time, in the same read (AVIF, HEIC, JPEG and PNG; JXL reads its capture time through ImageIO and gets no focal length). Between the last found photo and the next, the expected zoom is the ratio of focal length times image diagonal. It is used in two places (constants in `rust/src/track.rs`):

- **A mark without `@zoom`** takes the last found photo's scale times that ratio. An explicit `@zoom` wins. Before, it kept the scale unchanged.
- **A tracked step across a lens switch** searches the whole photo (the search window cannot follow a zoom about an unknown centre), scores RANSAC samples less (ln(zoom / ratio) / ln 1.1)² / 2 (`ZOOM_SPREAD`), and rejects a fit whose zoom is more than 1.25x off the ratio (`LENS_SLACK`), in place of the 6x limit.

Photos without a focal length on either side behave exactly as before; so do steps between photos with the same lens and size.

### What EXIF says at each Girona lens switch

| step | 35 mm focal | EXIF ratio | before B4 |
|-|-|-:|-|
| IMG_4673 → 4675 | 120 → 14 | 0.12 | LOST (2 matches) |
| 4673 → 4680 | 120 → 48 | 0.4 | mark, no `@zoom`: scale kept at 1.00 |
| 4681 → 4684 | 48 → 120 | 2.5 | mark `@3` |
| 4690 → 4695 | 120 → 48 | 0.4 | mark `@1` (EXIF gives 3.03 × 0.4 = 1.21) |
| 4697 → 4698 | 48 → 120 | 2.5 | mark `@2.5`, equal to EXIF |
| 4705 → 4706 | 120 → 24 | 0.2 | mark `@1` (EXIF gives 0.52) |
| 4706 → 4708 | 24 → 48 | 2 | mark `@1` (EXIF gives 2) |
| 4708 → 4713 | 48 → 120 | 2.5 | mark `@2.5`, equal to EXIF |
| 4713 → 4716 | 120 → 48 | 0.4 | mark `@1`, equal to EXIF |
| 4719 → 4720 | 48 → 120 | 2.5 | tracked: 2.55x |
| 4772 → 4773 | 120 → 48 | 0.4 | point mark: scale kept at 0.99 |
| 4773 → 4774 | 48 → 120 | 2.5 | point mark: scale kept at 0.99 |
| 4780 → 4791 | 120 → 24 | 0.2 | mark `@1` (EXIF gives 0.20) |
| 4795 → 4796 | 24 → 48 | 2 | point mark: scale kept at 1.08 |

- **The 35 mm focal length alone predicts the step.** The one switch the tracker links measures 2.55x against 2.5x. 24 and 48 mm are the same main camera (`FocalLength` 6.8 mm); the 2x crop shows only in the 35 mm value. `DigitalZoomRatio` is absent or 1.0003–1.0013.
- **Most `@zoom` marks are framing choices, not lens ratios.** They reset 48 mm photos to about 1 and 120 mm to 2.5–3, while the first photo is 120 mm at 1. EXIF never overrides them.

### Gain

- **`@zoom` marks: 11 → 8.** IMG_4698 `@2.5`, IMG_4713 `@2.5` and IMG_4716 `@1` equal the EXIF default. Without them the sidecar is identical (every rect, scale, source and point), so `girona_all.sh` drops them. The other 8 differ from EXIF by 4.8e-3 (IMG_4697 `@1`, scale 0.998 from IMG_4696) up to a 2x framing choice.
- **Whole marks: none.** Dropping any lens-switch mark loses that photo. Matching fails before the fit: 1–7 matches pass the ratio test, 8 are needed, so the zoom prior has nothing to choose from. IMG_4716 tracks without its mark, as it already did on `main`. A larger reference area at a switch (the disc radius divided by the ratio, up to 8.6x) linked none of them either, and was dropped.
- **Zoomless marks at a switch now keep the sky's magnification.** IMG_4680 and 4681 go from scale 1.00 to 0.40, IMG_4773 from 0.99 to 0.40, IMG_4796 and 4798 from 1.08 to 2.16. Before, the sky in those photos showed at 0.4x or 2x the size of the photo before. IMG_4675 (lost) is interpolated to 0.70. The clip is 1476x1172 instead of 1430x1184, and every crop rect moves with it. No tracked step changed.

### Before → after

The baseline was regenerated in this branch.

| measure | before | after |
|-|-|-|
| accuracy, all 26 frames | median 0.096 px, max 1.092 px, scale max 0.56%, 0 LOST | identical, frame by frame (no EXIF) |
| imageio_avif max / median move, max rect | 2.2 px (IMG_4777) / 0.15 px, 1.49e-2 | 2.2 px / 0.15 px, 2.05e-2 (IMG_4689) |
| imageio_png | 0.9 px / 0.10 px, 9.59e-3 | 0.9 px / 0.10 px, 9.80e-3 |
| heic | 2.8 px / 0.12 px, 7.78e-3 | 2.8 px / 0.12 px, 8.03e-3 |
| jpeg_q90 | 2.1 px / 0.09 px, 1.01e-2 | 2.1 px / 0.09 px, 2.02e-2 (IMG_4705) |
| pillow_png | 0 | 0 px, rect 1.11 (no EXIF: see below) |
| noise_2 | 1.9 px / 0.14 px, 1.29e-2 | 1.9 px / 0.14 px, rect 1.11 (1.72e-2 with EXIF kept) |
| LOST set, every variant | IMG_4675, IMG_4721, IMG_4725 | same |
| warm JSON re-run of `girona_all.sh`, median of 25 | 155 ms | 155 ms |

- **Every move is identical:** tracking did not change in any variant.
- **The largest rect change rises past 2e-2 in two variants.** With `--pad`, each side of the crop is the median margin over all photos in first-photo scale. The new 0.4 and 2.16 scales change which photo sets that median, and the new one moves more under pixel noise; IMG_4689 and IMG_4705, at scale 3.0 and 2.6, magnify it. Either change alone stays under: with IMG_4680 `@1` the largest is 1.53e-2, with IMG_4796 and 4798 at their old 1.08, 1.40e-2.
- **`pillow_png` and `noise_2` compare a run without EXIF to one with it.** The benchmark writes them with Pillow, which drops EXIF, so their marks keep the old scales. A local copy of the benchmark that saves the EXIF into the PNGs gives `pillow_png` 0 and `noise_2` 1.9 px, 1.72e-2.
- **Header cost:** focal length comes from the same EXIF read as the capture time, so the warm re-run is unchanged. `--order name` now reads EXIF too.
