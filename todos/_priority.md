# Priority

Work top to bottom. Run at most two at once, and only a pair the plan marks safe. See [PLAN.md](../PLAN.md) for the rules.

1. [E1](E1-sidecar-points.md): reference points per photo in the sidecar. Small, and it unblocks A1, E5 and the UI. Running.
2. [A1](A1-tracking-benchmark.md): tracking accuracy and stability benchmark. Unblocks F1 and every tracking change. Safe alongside B1.
3. [B1](B1-colour.md): keep Display P3 and label the clip. Every clip from the Girona photos is mislabelled today.
4. [F1](F1-python-reference.md): retire the Python reference. Before any card that changes tracking. Safe alongside E5.
5. [E5](E5-marking-mockup.md): marking mockup with points and the fix list, for the user to check.
6. [B2](B2-capture-time-order.md): order photos by capture time. Safe alongside A4.
7. [A4](A4-search-window.md): look for the feature near where it should be.
8. [A5](A5-reference-points.md) ⚑: several reference points instead of a box. Stops after its design for the user.
9. [A2](A2-tracker-sensitivity.md): tracker sensitivity to tiny pixel differences, measured on the tracker A4 and A5 leave.
10. [A3](A3-track-edge.md): smaller tracking size, if accuracy holds.
11. [B4](B4-zoom-from-exif.md): seed lens-switch zoom from EXIF focal length. Needs real iPhone originals.
12. [D1](D1-linux-build.md): build and test on Linux.
13. UI in enframe: [E2](E2-serve-endpoint.md), [E3](E3-marking-screen.md), [E4](E4-preview-export.md), in that order.
