# Priority

Work top to bottom. Run at most two at once, and only a pair the plan marks safe. See [PLAN.md](../PLAN.md) for the rules.

1. [E5](E5-marking-mockup.md) ⚑: merged; waiting for the user to review the mockup.
2. [A5a](A5a-points-tracker.md) ⚑: track a set of points (waits on decision 3), then [A5b](A5b-points-syntax.md): the points syntax.
3. [A2](A2-tracker-sensitivity.md): tracker sensitivity to tiny pixel differences, measured on the tracker A4 and A5 leave.
4. [A3](A3-track-edge.md): smaller tracking size, if accuracy holds.
5. [B4](B4-zoom-from-exif.md): seed lens-switch zoom from EXIF focal length. Needs real iPhone originals.
6. [D1](D1-linux-build.md): build and test on Linux.
7. UI in enframe: [E2](E2-serve-endpoint.md), [E3](E3-marking-screen.md), [E4](E4-preview-export.md), in that order.
