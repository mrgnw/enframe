# pinframe

Turn a series of photos into a short clip where one chosen feature, like a cloud seen from a train window, stays in the same place in frame while everything else changes.

> [!NOTE]
> This project was built with heavy LLM assistance and should be considered proof-of-concept. Contributions are welcome, but I can't guarantee the accuracy of the code or that I will continue to maintain it. If there are any mistakes in attribution or anything else, please let me know.

## Usage

```sh
uv run anchor.py "samples/girona clouds"/IMG_479{1,2,3,4,5}.avif --out out/heart.mp4
```

A window opens on the first photo. Drag a box around the feature to hold still, then press Enter. To skip the window, pass the box as `--box x,y,w,h` in first-photo pixels.

Any photo the tracker reports as `LOST` can be fixed by boxing the feature there too. Tracking carries on from that box:

```sh
uv run anchor.py "samples/girona clouds"/IMG_{4698,4699,4700,4701,4702,4703,4704,4705}.avif --box 840,550,400,350 --at IMG_4703.avif=746,554,400,350 --out out/turbines.mp4
```

Writes the clip and a `.json` beside it with each photo's crop rect, normalized to 0–1 like enframe's coordinates.

Run with no arguments for the self-check:

```sh
uv run anchor.py
```

## How it works

1. Photos are taken in filename order.
2. SIFT keypoints inside the box are matched into the next photo, and a scale + rotation + shift is fitted to them with RANSAC. That follows the anchor across lens switches and resolution changes, and the number of agreeing keypoints is an honest confidence: an occluded cloud gets 0–2, not a wrong answer.
3. Each photo is matched against the last photo where the anchor was found, so the tracker follows a cloud as it slowly changes shape.
4. A photo with fewer than 8 agreeing keypoints is `LOST`. It stays in the clip, with the anchor's position and scale interpolated from the photos either side. A box from `--at` replaces the estimate, and its size sets the scale.
5. Each photo is scaled so the anchor is the same size in all of them. The crop is the largest window that then keeps the anchor at one fixed spot in every photo. Portrait, landscape, 1x and 2x photos mix freely.
6. The clip is sized at the median photo's scale, so a single zoomed-out photo doesn't shrink the rest. ffmpeg assembles the crops at `--fps` (default 4).

## Results on the Girona samples

| Run | Photos | Boxes | Clip | Result |
|-|-|-|-|-|
| Heart cloud, IMG_4791–4795 | 5 | 1 | 1420x1452 | Holds across a portrait-to-landscape switch |
| Cumulus over turbines, IMG_4698–4705 | 8 | 2 | 1674x1462 | Holds; IMG_4703 shows the cloud from a new angle and needed a box |
| Portrait cumulus, IMG_4718–4729 | 12 | 1 | 1200x1940 | Holds across a 1x → 2x lens switch (measured 2.55x); the tree (4721) and building (4725) frames are kept at estimated positions |
| Whole folder, all 57 | 57 | 1 | 1046x1366 | Keypoints carry the cloud to IMG_4673, then lose it for the remaining 53 photos |

## What didn't work

- **Template matching across scales.** Normalized correlation favours small templates. At the lens switch it picked a 17 px patch at 0.2x and then tracked that patch with 0.97 confidence.
- **Template matching as a fallback when keypoints fail.** It recovered IMG_4703, but across the whole folder it chained onto blank sky, which matches blank sky at 0.87–0.98. Matching on detail only (blur subtracted) removed those, and the real matches with them.

## Next steps, in order

- **Point picking in enframe.** Draw a box on the first photo, then on any photo flagged `LOST`. The box UI, normalized coordinates and crop export already exist there, and the `.json` output matches that model.
- **Crops that can leave the frame.** One photo with the anchor near its edge shrinks the crop for every photo. Padding that photo, or blurring its border, would keep the clip large.
- **Soft lock.** Today the anchor is pinned exactly. Letting it drift a little along a smoothed path may look more natural.

## No-code alternative

Blender's 2D Stabilization does the same job for free on an image sequence: put one tracking marker on the feature, enable location stabilization, and set an anchor frame. For soft features like clouds, set the marker's motion model to Affine, enable Normalize, and raise Correlation to 0.9. It needs manual marker fixes when the track drifts, and a sudden lens switch means re-placing the marker by hand.

The one-click stabilizers (ffmpeg `vidstab`, Warp Stabilizer, Resolve's automatic stabilizer, Hugin's `align_image_stack`) don't work here. They lock onto the dominant motion, which from a train is the landscape, not the cloud.
