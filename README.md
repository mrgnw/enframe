# pinframe

Turn a series of photos into a short clip where one chosen feature, like a cloud seen from a train window, stays in the same place in frame while everything else changes.

> [!NOTE]
> This project was built with heavy LLM assistance and should be considered proof-of-concept. Contributions are welcome, but I can't guarantee the accuracy of the code or that I will continue to maintain it. If there are any mistakes in attribution or anything else, please let me know.

## Usage

```sh
uv run anchor.py "samples/girona clouds"/IMG_479{1,2,3,4,5}.avif --out out/heart.mp4
```

A window opens on the first photo. Drag a box around the feature to hold still, then press Enter. To skip the window, pass the box as `--box x,y,w,h` in first-photo pixels.

Writes the clip and a `.json` beside it with each photo's crop rect, normalized to 0–1 like enframe's coordinates.

Run with no arguments for the self-check:

```sh
uv run anchor.py
```

## How it works

1. Photos are taken in filename order.
2. The box is matched in each next photo with OpenCV template matching (`TM_CCOEFF_NORMED`) over the whole frame. Each good match becomes the template for the next photo, so the tracker follows a cloud as it slowly changes shape.
3. A match scoring under 0.5 counts as lost. That photo is dropped, and the next photo is matched against the last good template.
4. The crop is the largest window that keeps the anchor at one fixed spot in every photo. Holding a feature still is just sliding that window: nothing is warped, so there are no black borders. Portrait and landscape photos mix freely.
5. ffmpeg assembles the crops at `--fps` (default 4).

## Results on the Girona samples

| Run | Photos | Kept | Crop kept | Result |
|-|-|-|-|-|
| Heart cloud, IMG_4791–4795 | 5 | 5 | 65% | Holds across a portrait-to-landscape switch |
| Cumulus over turbines, IMG_4698–4704 | 7 | 6 | 81% | Holds; one photo dropped at a 0.49 score |
| Portrait cumulus, IMG_4720–4729 | 10 | 8 | 77% | Holds; the tree (4721) and building (4725) occlusions were dropped, and tracking recovered right after each |

Where it fails: IMG_4719 → 4720 switches to the 2x lens, so the cloud doubles in size and template matching can't follow it. The tool assumes constant zoom.

## Verdict

Plain template matching is enough for this. A learned point tracker was never needed on these samples. Clouds deform slowly enough between shots that following the last good match works, and the score gate catches occlusions.

The worthwhile next steps, in order:

- **Point picking in enframe.** The click-a-point UI, normalized coordinates and crop export already exist there. The `.json` output matches that model.
- **Scale tracking** to survive lens switches (match at a few scales, or fit a similarity transform from two anchor points).
- **Soft lock.** Today the anchor is pinned exactly. Letting it drift a little along a smoothed path may look more natural.

## No-code alternative

Blender's 2D Stabilization does the same job for free on an image sequence: put one tracking marker on the feature, enable location stabilization, and set an anchor frame. For soft features like clouds, set the marker's motion model to Affine, enable Normalize, and raise Correlation to 0.9. It needs manual marker fixes when the track drifts, and has no notion of dropping occluded frames.

The one-click stabilizers (ffmpeg `vidstab`, Warp Stabilizer, Resolve's automatic stabilizer, Hugin's `align_image_stack`) don't work here. They lock onto the dominant motion, which from a train is the landscape, not the cloud.
