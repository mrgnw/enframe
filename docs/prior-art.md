# Prior art for pinframe

Survey date: 2026-10-05. This is a quick check of tools that keep one chosen feature fixed across a sequence, and how they name their options.

Parameter names and defaults below come from source code or official docs, linked per section. Anything taken from general knowledge, without a primary source, is marked **unverified**.

## Summary

No tool found does the full pinframe job: a user-chosen feature in **photos** (not video), followed across lens changes (scale), with manual fixes for lost frames, a hard lock, and crop output.

The closest matches:

- **Video trackers with a stabilize mode** (Blender, Natron, Nuke, After Effects, Resolve Fusion): same idea, but GUI-first and built for video.
- **Face-a-day aligners**: same output shape (photo sequence → clip with the anchor at a fixed spot and size), but the anchor is always a detected face.
- **Astro registration** (Siril, PlanetarySystemStacker): photo sequences with a reference frame and crop/border options, but they align on stars or the whole disc.

Global stabilizers (vid.stab, OpenCV videostab, Warp Stabilizer, Hugin) lock onto the dominant motion. They fail here because the landscape dominates the frame, not the cloud. This confirms the earlier survey.

## Tools

| Tool | Similar part | Interface | License | Link |
|-|-|-|-|-|
| Blender 2D Stabilization | Track markers, anchor frame, translation/rotation/scale lock | GUI + Python API | GPL (**unverified** here) | [rna_tracking.cc](https://github.com/blender/blender/blob/main/source/blender/makesrna/intern/rna_tracking.cc) |
| Natron Tracker | Pattern + search window, Stabilize vs Remove Jitter, reference frame | GUI + Python | GPL-2.0 | [TrackerContextPrivate.h](https://github.com/NatronGitHub/Natron/blob/master/Engine/TrackerContextPrivate.h) |
| ffmpeg `vidstabdetect`/`vidstabtransform` | Reference-frame "tripod" lock, crop/zoom border handling, transforms file | CLI (filter) | GPL (libvidstab) | [vf_vidstabtransform.c](https://github.com/FFmpeg/FFmpeg/blob/master/libavfilter/vf_vidstabtransform.c) |
| ffmpeg `deshake` | Search **rectangle** `x,y,w,h` | CLI (filter) | LGPL/GPL | [vf_deshake.c](https://github.com/FFmpeg/FFmpeg/blob/master/libavfilter/vf_deshake.c) |
| OpenCV videostab | Motion model choice, feature masks, save/load motions, trim ratio | C++ library + sample CLI | Apache-2.0 | [videostab.cpp](https://github.com/opencv/opencv_contrib/blob/master/modules/videostab/samples/videostab.cpp) |
| Siril `register` / `seqapplyreg` | Photo sequence, reference image, `-transf=similarity`, `-framing=` | CLI script commands + GUI | GPL-3.0 (**unverified** here) | [Commands](https://siril.readthedocs.io/en/stable/Commands.html), [Registration](https://siril.readthedocs.io/en/stable/preprocessing/registration.html) |
| PlanetarySystemStacker | Patch-based stabilization, reference from best frames | CLI + GUI | none on GitHub | [pss_console.py](https://github.com/Rolf-Hempel/PlanetarySystemStacker/blob/master/planetary_system_stacker/pss_console.py) |
| Hugin `align_image_stack` | Photo stack, align to first, `.pto` output, autocrop | CLI | GPL (**unverified** here) | [wiki](https://wiki.panotools.org/Align_image_stack) |
| imreg_dft (`ird`) | Similarity registration with prior constraints on angle/scale | CLI + Python | BSD | [cli.py](https://github.com/matejak/imreg_dft/blob/master/src/imreg_dft/cli.py) |
| MediaPipe AutoFlip | Keeps a salient subject in frame for a target aspect ratio | CLI (Bazel graph) | Apache-2.0 (**unverified** here); **deprecated** Mar 2023 | [autoflip.md](https://github.com/google-ai-edge/mediapipe/blob/master/docs/solutions/autoflip.md) |
| facelapse | Photos → clip with eyes at a fixed spot and size; pin/exclude files | CLI | none on GitHub | [boraedis/facelapse](https://github.com/boraedis/facelapse) |
| face-timelapse-generator | Eye landmarks → fixed point in a 1080×1920 frame, EXIF ordering | CLI | MIT | [reinhole/face-timelapse-generator](https://github.com/reinhole/face-timelapse-generator) |
| facemation | Eye-aligned selfie timelapse, config file, per-file overrides | CLI + `config.py` | MIT | [FWDekker/facemation](https://github.com/FWDekker/facemation) |
| AgeLapse | Landmarks → "goal" coordinates; saved manual edits per photo | GUI app | GPL-3.0 | [hugocornellier/agelapse](https://github.com/hugocornellier/agelapse) |
| kornia | Differentiable feature matching + RANSAC; building blocks only | Python library | Apache-2.0 | [kornia/kornia](https://github.com/kornia/kornia) |
| Gyroflow | Gyro-data stabilization; no feature anchor | GUI + CLI | GPL-3.0 | [gyroflow/gyroflow](https://github.com/gyroflow/gyroflow) |
| Adobe Auto Reframe, Nuke Tracker, Mocha, LRTimelapse, Microsoft Hyperlapse, Panolapse, PixInsight StarAlignment, DeepSkyStacker, Sequator, AutoStakkert, ImageMagick/vips | GUI-first, global-motion based, or only render primitives | GUI / various | commercial or mixed | not examined, **unverified** |

## Tools worth learning from

### ffmpeg vid.stab (`vidstabdetect` → `vidstabtransform`)

Source: [vf_vidstabdetect.c](https://github.com/FFmpeg/FFmpeg/blob/master/libavfilter/vf_vidstabdetect.c), [vf_vidstabtransform.c](https://github.com/FFmpeg/FFmpeg/blob/master/libavfilter/vf_vidstabtransform.c).

- Two passes joined by a file: detect writes `result` (default `transforms.trf`), transform reads `input`. `fileformat=ascii|binary` (binary default).
- Reference frame: detect `tripod` is an integer, "motion is compared to a reference frame (frame # is the value)", default 0 = off. Transform `tripod` is "virtual tripod mode (same as relative=0:smoothing=0)". This is the hard-lock mode, equivalent to what pinframe always does.
- Smoothing: `smoothing` = "number of frames*2 + 1 used for lowpass filtering", default **15** in source, range 0–1000. (The earlier note said 10; the source says 15.)
- Limits: `maxshift` (px, default -1 = none), `maxangle` (radians, default -1).
- Borders: `crop=keep|black` (default `keep`, the previous frame's content), `zoom` (percent, default 0), `optzoom=0|1|2` ("0: nothing, 1: optimal static zoom, 2: optimal dynamic zoom", default 1), `zoomspeed` (default 0.25).
- Detection tuning: `shakiness` 1–10 (5), `accuracy` 1–15 (15), `stepsize` (6), `mincontrast` 0–1 (0.25).
- No region of interest, so it still follows the dominant motion.

### ffmpeg `deshake` (correction to earlier survey)

Source: [vf_deshake.c](https://github.com/FFmpeg/FFmpeg/blob/master/libavfilter/vf_deshake.c).

- It **does** take a region: `x`, `y`, `w`, `h` = "rectangular search area", default -1 (whole frame). The format is top-left pixels, the same as pinframe's old `--box` (now `--points`, the box's four quarter centres; A5b).
- But `rx`/`ry` (allowed shift) default to 16 and cap at `MAX_R 64` px. Photos where the cloud moves hundreds of pixels are out of reach. It has no scale model, so lens switches break it.
- Borders: `edge=blank|original|clamp|mirror` (default `mirror`).

### OpenCV videostab sample CLI

Source: [videostab.cpp](https://github.com/opencv/opencv_contrib/blob/master/modules/videostab/samples/videostab.cpp), [stabilizer.hpp](https://github.com/opencv/opencv_contrib/blob/master/modules/videostab/include/opencv2/videostab/stabilizer.hpp).

- `-m=, --model=(transl|transl_and_scale|rigid|similarity|affine|homography)`, default affine. pinframe uses `similarity` but does not use the rotation in the render (see below).
- `--min-inlier-ratio` (default 0.1), `--nkps` (1000), `--thresh` (auto), `--outlier-ratio` (0.5): the confidence knobs pinframe hardcodes.
- `--feature-masks=(file_path|no)`: per-frame masks to limit where features are found. This is the closest thing to a region of interest in a global stabilizer.
- `-sm=, --save-motions=` / `-lm=, --load-motions=`: write motions to a file, then reuse them.
- `-r=, --radius=` (15) smoothing window; `-t=, --trim-ratio=` (0.1); `-et=, --est-trim=` (yes); `-ic=, --incl-constr=` (no).

### Blender 2D Stabilization

Source: [rna_tracking.cc](https://github.com/blender/blender/blob/main/source/blender/makesrna/intern/rna_tracking.cc), struct `MovieTrackingStabilization`. The docs page [bpy.types.MovieTrackingStabilization](https://docs.blender.org/api/current/bpy.types.MovieTrackingStabilization.html) returned 403 to the fetcher. Defaults were not read and are **unverified**.

- `anchor_frame`: "Reference point to anchor stabilization".
- `tracks` (translation) and `rotation_tracks` are separate collections.
- `use_stabilize_rotation`, `use_stabilize_scale`: separate toggles per degree of freedom.
- `target_position`, `target_rotation`, `target_scale` ("Expected Position/Rotation/Scale"): deliberate offsets applied after locking.
- `use_autoscale` ("scale footage to cover unfilled areas") with `scale_max` (0–10): Blender's version of pinframe's default no-overhang crop.
- `influence_location`, `influence_rotation`, `influence_scale` (0–1): a partial lock.

This confirms the earlier survey.

### Natron Tracker

Source: [TrackerContextPrivate.h](https://github.com/NatronGitHub/Natron/blob/master/Engine/TrackerContextPrivate.h), [TrackMarker.h](https://github.com/NatronGitHub/Natron/blob/master/Engine/TrackMarker.h).

- `motionType`: `None`, `Stabilize` ("so that the tracked points do not move"), `Match-Move`, `Remove Jitter`, `Add Jitter`.
- `referenceFrame`: "the frame number at which the transform will be an identity".
- `transformType`: `Transform` (translation, scale, rotation) or `CornerPin`.
- `robustModel`: drop points that don't fit the model, like RANSAC.
- `smooth`: "averaging this number of frames together" for t/r/s. `jitterPeriod`.
- Each marker has a **pattern** (`patternTopLeft` … quad, the thing to track) and a separate **search window** (`searchWndBtmLeft`/`searchWndTopRight`, "relative to the center point"). pinframe has the same two concepts: `--points` is the pattern, and an `--at` mark is a search window. Its flags don't name them that way.

### Siril `register` / `seqapplyreg`

Source: [Commands](https://siril.readthedocs.io/en/stable/Commands.html), [Registration](https://siril.readthedocs.io/en/stable/preprocessing/registration.html).

- `-transf=shift|similarity|affine|homography` (default homography). The docs give minimum pairs: 1 for shift, 2 for similarity, 3 for affine, 4 for homography. `-minpairs=` defaults to 3, or 4 for homography.
- `-framing=current|min|max|cog`:
  - `current` keeps the reference frame's framing.
  - `max` "adds a black border around each image as required so that no part of the image is cropped".
  - `min` "crops each image to the area it has in common with all images".
  - `cog` uses the centre of gravity of all images.
  This is the closest match to pinframe's crop-vs-`--pad` choice, named by framing rather than by fill.
- Reference image: set with `setref`. Otherwise it is the best-quality frame (lowest FWHM, the star-sharpness measure) or the first non-excluded image.
- `-2pass` computes transforms without writing images. `seqapplyreg` applies them later, a two-step design like vid.stab.
- Image Pattern Alignment and KOMBAT: translation only, with a GUI selection the object must stay inside. This confirms the earlier survey.

### Hugin `align_image_stack`

Source: [panotools wiki](https://wiki.panotools.org/Align_image_stack).

- `--align-to-first`; `--use-given-order` ("Use the image order as given from command line").
- `-p file` writes a `.pto` project "for debugging, or further refinement"; `-a prefix` writes `prefix_xxxx.tif`.
- `-C` "Auto crop the image to the area covered by all images".
- `-t num` removes control points with error above num px (default 3); `-c` points per grid cell (8); `-g` grid size (5).
- No region of interest; it aligns the whole frame.

### imreg_dft (`ird`)

Source: [cli.py](https://github.com/matejak/imreg_dft/blob/master/src/imreg_dft/cli.py).

- Priors instead of hard limits: `--angle MEAN[,STD]` (default 0), `--scale MEAN[,STD]` (default 1), `--tx`/`--ty`. A soft version of pinframe's `MAX_STEP_ZOOM`/`MAX_STEP_TURN`.
- `--extend PIXELS`, `--order` (interpolation), `--print-result` with a format string (default prints `scale: … +-…`, a confidence per parameter).

### PlanetarySystemStacker

Source: [pss_console.py](https://github.com/Rolf-Hempel/PlanetarySystemStacker/blob/master/planetary_system_stacker/pss_console.py).

- `-m, --stab_mode Surface|Planet` (default Surface), `--stab_size` (33 % of frame), `--stab_sw` (34 px search width), `--rf_percent` (5 % of best frames build the reference).
- Sizes are given as a percentage of the frame, not pixels.

### Face-a-day aligners

- **facelapse** ([README](https://github.com/boraedis/facelapse)):
  - Framing is `--width/--height/--eye-y/--eye-dist`, which sets the anchor's output position and size explicitly.
  - Manual choices live in `work/pin.txt` and `work/exclude.txt`, which "persist, so this year's choices carry over".
  - The `select` step writes a `review.html` that shows each frame cropped.
  - `--fps` defaults to 12.
- **face-timelapse-generator** ([README](https://github.com/reinhole/face-timelapse-generator)): orders frames by EXIF capture time and puts the eyes at a fixed point in a 1080×1920 frame.
- **facemation** ([config_default.py](https://github.com/FWDekker/facemation/blob/master/src/main/resources/config_default.py)): a config file, not flags. `face_selection_overrides` is a per-filename manual fix (a dict keyed by file name). `ffmpeg.fps` defaults to 24.
- **AgeLapse** ([README](https://github.com/hugocornellier/agelapse)):
  - Landmarks are mapped to fixed "goal" coordinates on a fixed-size canvas.
  - Manual edits (offset, scale, rotation) are saved per photo, take precedence over automatic alignment, and survive re-runs. Changing the output resolution "rescales saved manual edits proportionally".

### MediaPipe AutoFlip (deprecated)

Source: [autoflip.md](https://github.com/google-ai-edge/mediapipe/blob/master/docs/solutions/autoflip.md).

- `aspect_ratio=1:1`, format `width:height`.
- `motion_stabilization_threshold_percent: 0.5` chooses between a static and a moving virtual camera.
- Padding has `blur_cv_size` and `overlay_opacity` (blurred fill, not a solid colour).
- Support ended 2023-03-01. Useful only for naming.

## Pinframe's CLI today

```
anchor.py PHOTOS... [--box x,y,w,h] [--at PHOTO=x,y[,w,h[,zoom]]]... [--pad COLOUR] [--fps 4] [--out out.mp4]
```

Observations from reading `main()` and `track()`:

- `--box` is top-left `x,y,w,h` in first-photo pixels, the same as OpenCV `selectROI`, ffmpeg `deshake` and `crop`.
- In `--at`, two numbers are the anchor **centre**, but the first two of four are the box's **top-left**. The same leading numbers mean different things depending on how many follow.
- Zoom in `--at` is relative to the first photo, which is always the reference frame.
- The rotation is fitted but used only to reject fits over 15°. The render crops axis-aligned rectangles, so rotation is never corrected.
- The sidecar `<out>.json` has `file, x, y, w, h` (normalized crop). It has no scale and no tracked/marked/interpolated status.
- Photos are re-sorted by filename regardless of argument order.

## Recommendations for pinframe's CLI

Ranked by impact. 1–4 matter; 5–9 are cosmetic or optional.

1. **Marks file instead of repeated `--at`** (matters).
   - Add `--marks marks.json`: a list of `{"file", "x", "y", "w"?, "h"?, "zoom"?}`. Keep `--at` for one-offs; it overrides the file.
   - Today `analysis/girona_all.sh` carries 24 `--at` flags. Hand-made fixes should persist across re-runs, as in facelapse `pin.txt`/`exclude.txt`, AgeLapse saved manual edits and facemation `face_selection_overrides`.
   - This file is also the natural interface for enframe's click-to-mark UI: enframe writes marks, pinframe reads them.
   - Use **normalized 0–1** coordinates in the file. They match the existing `<out>.json` and enframe's model, and survive resizing, which AgeLapse handles explicitly. Keep pixels on the command line.
2. **Report status per photo in the sidecar** (matters).
   - Add `"scale"`, `"matches"` (agreeing keypoints) and `"source": "tracked" | "mark" | "interpolated"` to each row of `<out>.json`.
   - enframe needs this to show which photos are `LOST`. Today that information only goes to stdout.
   - Precedents: vid.stab `result=transforms.trf`, OpenCV `--save-motions`, Hugin `-p` `.pto`, imreg_dft `--print-result`.
   - Optionally accept the same file back as `--marks`, round-tripping like OpenCV `--load-motions`.
3. **Make the two `--at` forms unambiguous** (matters; a likely source of wrong marks).
   - Pick one rule: `x,y` is always the anchor centre, or always the top-left.
   - Natron gives a search window relative to the centre point. A centre-based box `cx,cy,w,h` would make `--at P=1000,200` and `--at P=1000,200,300,300` agree on where the anchor is.
   - If the box stays top-left (to match `--box`, `selectROI` and `deshake`), the help text must say the point form is a centre.
4. **Choosable reference frame** (matters once photos aren't shot in order).
   - Let `--box` take the same `PHOTO=x,y,w,h` form as `--at` (bare `x,y,w,h` still means the first photo), and make `--at` zoom relative to that photo.
   - Precedents: Blender `anchor_frame`, vidstabdetect `tripod=N`, Natron `referenceFrame`, Siril `setref`, Hugin `--align-to-first`.
5. **Split `--pad` into framing and fill** (cosmetic).
   - Siril names the decision `-framing=min|max|current|cog`; vid.stab uses `crop=keep|black`; deshake uses `edge=`.
   - pinframe's two modes are framing `min` (the current default) and a median framing. `--pad COLOUR` sets both the framing and the colour in one flag.
   - Possible shape: `--framing min|median` plus `--fill transparent|black|white|#rrggbb`. Only worth it if a third framing (`max`) is wanted.
6. **Output aspect and anchor placement** (optional, likely needed for enframe).
   - `--aspect W:H` in AutoFlip's `aspect_ratio=W:H` format.
   - `--anchor-at X,Y` (normalized output position), like facelapse `--eye-y`, AgeLapse "goal" coordinates and Blender `target_position`.
   - Today the anchor's output position comes out of the crop math and can't be set.
7. **Say what happens to rotation** (cosmetic, or a feature).
   - Blender exposes `use_stabilize_rotation` and OpenCV exposes `--model=similarity|transl_and_scale`.
   - pinframe fits a similarity model but renders `transl_and_scale`. Either document that, or add `--rotate` if handheld shots tilt.
8. **Expose the confidence limits only if tuning comes up** (optional).
   - Names to borrow: `--min-matches 8` (Siril `-minpairs=`, OpenCV `--min-inlier-ratio`), `--max-zoom 6` (Blender `scale_max`), `--max-turn 15` (vid.stab `maxangle`, in degrees not radians).
   - Not needed now; the constants have not needed tuning.
9. **Ordering** (cosmetic).
   - Hugin has `--use-given-order`; face-timelapse-generator sorts by EXIF time.
   - pinframe silently re-sorts by filename, so the shell's order is ignored. Mention it in `--help`. Sort by EXIF `DateTimeOriginal` when present, falling back to filename.

Things pinframe does differently, on purpose:

- It **hard-locks** by default with no smoothing. In other tools that is the special mode (vid.stab `tripod`, Natron `Stabilize` vs `Remove Jitter`). No smoothing option is needed.
- It **interpolates lost photos** instead of dropping them. Siril `-selected` and facelapse `exclude.txt` drop frames. An `--drop-lost` flag may be useful later.
- `--fps 4` is low next to facelapse (12) and facemation (24). That suits photos with large jumps between them; keep it.
