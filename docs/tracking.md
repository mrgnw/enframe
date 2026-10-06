# Tracking benchmark

```sh
uv run analysis/tracking_benchmark.py
```

It builds the release binary, runs the Rust CLI in JSON-only mode with a temporary `--cache`, prints one summary line and writes `out/tracking_benchmark.json`. It takes about 26 s on the M4 Pro. Runs are deterministic: two runs give identical reports apart from `seconds`.

`analysis/tracking_baseline.json` is the report for `main` at the commit that added it. A tracking change compares its report against it. `stability.original` holds every Girona photo's point, scale, source and crop rect, the same values the `girona_all.sh` sidecar has, so `check.sh` can compare against it once the Python reference retires (F1).

## Accuracy

Five synthetic sequences, 31 frames in total. Each one warps a single Girona photo with a known zoom, rotation and offset, so the true anchor in every frame is exact. Frames are 1024 px on the long edge, the size SIFT runs at. Zooms below 0.75x sample a box-filtered half-size photo, so zooming out does not alias. Frame 0 is the mark; the other 26 are scored.

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
| shift | 0.24 px | 0.12% | 0 |
| crop | 0.02 px | 0.02% | 0 |
| zoom | 1.00 px | 0.09% | 0 |
| rotate | 0.19 px | 0.12% | 0 |
| mixed | 0.90 px | 0.53% | 0 |
| all 26 frames | median 0.21 px | | 0 |

The tracker is accurate to about a pixel on clean geometric changes, including 3x jumps. Errors are in frame pixels; LOST frames would be counted, not scored.

## Stability

The real Girona job (`analysis/girona_all.sh`: its box, 23 marks and `--pad transparent`) on the 57 AVIF originals, then on copies with slightly different pixels. Movement is the distance between each photo's point in the copy and in the original, in original pixels (2048 on the long edge). Rect is the largest change in the normalized crop rect, the measure `check.sh` and `docs/rust.md` use.

| variant | how | max move | median | max scale change | max rect change | LOST set |
|-|-|-:|-:|-:|-:|-|
| imageio_avif | same AVIF bytes, ftyp brand relabelled so pinframe decodes them through ImageIO | 7.8 px (IMG_4690) | 0.11 px | 5.7% | 1.60e-1 (IMG_4689) | same |
| imageio_png | `sips -s format png`: 16-bit PNG, which pinframe truncates to 8 bits | 29.3 px (IMG_4795) | 0.14 px | 3.9% | 3.85e-2 | same |
| heic | `sips -s format heic` | 44.7 px (IMG_4795) | 0.12 px | 4.2% | 4.17e-2 | same |
| jpeg_q90 | `sips -s format jpeg`, quality 90 | 33.6 px (IMG_4795) | 0.09 px | 1.9% | 1.82e-2 | same |
| pillow_png | Pillow's libavif decode, as PNG | 0 | 0 | 0 | 0 | same |
| noise_2 | Pillow's decode plus uniform noise in [−2, 2] levels, fixed seed | 3.9 px (IMG_4689) | 0.09 px | 3.4% | 9.61e-2 (IMG_4689) | same |

The baseline LOST set is IMG_4675, IMG_4721 and IMG_4725 in every variant.

- **IMG_4689/4690 reproduce.** `imageio_avif` is the trial in `docs/rust.md`: rects move 1.60e-1 at IMG_4689 and 1.57e-1 at IMG_4690, with 7 of 57 photos past 2e-2, the same numbers. Both photos are at scale 3.0, so a small point move becomes a large crop move. Noise of ±2 levels alone moves the same two photos most.
- **IMG_4795 is a second sensitive photo.** Every re-encode moves its point 29–45 px. It is the last tracked photo before the mark on IMG_4796, so the error does not carry on.
- `pillow_png` moving nothing confirms that Pillow and pinframe decode AVIF to identical pixels, so `noise_2` adds noise to exactly the pixels pinframe sees.
- The median move stays near 0.1 px everywhere: the sensitivity is a few photos, not the whole run.

## Choices

- `NOISE_LEVELS = 2`: the size of the ImageIO-vs-libavif difference in `docs/rust.md`.
- `FRAME = 1024×768`: the tracker's `TRACK_EDGE`, so synthetic frames are tracked at full size.
- Sequence offsets and zooms are picked by hand to cover the card's cases; frames that would leave the photo are shifted back inside and the truth follows the frame actually rendered.
