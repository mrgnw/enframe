# Priority

Work top to bottom. Run at most two at once, and only a pair the plan marks safe. See [PLAN.md](../PLAN.md) for the rules.

1. [E1](E1-sidecar-anchor.md): anchor box per photo in the sidecar. Small, and it unblocks A1 and the UI. Safe alongside B3.
2. [B3](B3-heic10-cost.md): cut 10-bit HEIC decode CPU. Small and independent.
3. [A1](A1-tracking-benchmark.md): tracking accuracy and stability benchmark. Unblocks A2, A3 and B4. Safe alongside B2.
4. [B2](B2-capture-time-order.md): order photos by capture time. Small and independent.
5. [A2](A2-tracker-sensitivity.md): tracker sensitivity to tiny pixel differences.
6. [D1](D1-linux-build.md): build and test on Linux.
7. [A3](A3-track-edge.md): smaller tracking size, if accuracy holds.
8. [B4](B4-zoom-from-exif.md): seed lens-switch zoom from EXIF focal length. Needs real iPhone originals.
9. UI: [E0](E0-ui-decisions.md) ⚑ decisions, then [E2](E2-serve-endpoint.md), [E3](E3-marking-screen.md), [E4](E4-preview-export.md), in that order.
10. Colour: [B1](B1-colour.md) ⚑ decision, then [C1](C1-output-colour.md).
11. [F1](F1-python-reference.md) ⚑: keep or retire the Python reference. Decide before A2 merges a tracking change.
