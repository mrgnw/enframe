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
| imageio_avif | same AVIF bytes, ftyp brand relabelled so pinframe decodes them through ImageIO | 44.4 px (IMG_4795) | 0.16 px | 0.7% | 1.89e-2 (IMG_4795) | same |
| imageio_png | `sips -s format png`: 16-bit PNG, which pinframe truncates to 8 bits | 6.5 px (IMG_4777) | 0.10 px | 0.7% | 1.18e-2 | same |
| heic | `sips -s format heic` | 37.3 px (IMG_4795) | 0.13 px | 2.1% | 2.15e-2 (IMG_4724) | same |
| jpeg_q90 | `sips -s format jpeg`, quality 90 | 41.7 px (IMG_4795) | 0.09 px | 2.0% | 1.67e-2 | same |
| pillow_png | Pillow's libavif decode, as PNG | 0 | 0 | 0 | 0 | same |
| noise_2 | Pillow's decode plus uniform noise in [−2, 2] levels, fixed seed | 41.5 px (IMG_4795) | 0.14 px | 1.4% | 1.85e-2 (IMG_4795) | same |

The baseline LOST set is IMG_4675, IMG_4721 and IMG_4725 in every variant.

- **IMG_4689/4690 no longer stand out.** With the box they moved 7.8 px in `imageio_avif` (rect 1.60e-1, the trial in `docs/rust.md`); with points they move 0.9 px. They are at scale 3.0, so a small point move becomes a large crop move. See "Points (A5a)".
- **IMG_4795 is the sensitive photo.** It has two fits about 30–45 px apart, and tiny pixel changes pick one or the other. It is the last tracked photo before the mark on IMG_4796, so the error does not carry on.
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
