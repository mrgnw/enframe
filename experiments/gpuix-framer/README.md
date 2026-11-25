# gpuix-svelte framer prototype

This standalone experiment renders a synthetic JPEG in a native macOS window. Dragging a focal point moves a square crop preview. The preview clips and offsets a copy of the image through GPUI layout.

## Run on macOS

The prototype targets Apple Silicon and Node.js 24 or newer. The lockfile pins gpuix-svelte 0.6.0 and its native dependencies.

Open a terminal in `experiments/gpuix-framer`.

```sh
npm ci
```

```sh
npm start
```

Drag on the photo to move the focal point. Press `z` to maximize the window.

## Checks

```sh
npm run check:geometry
```

```sh
npm run check:headless
```

The native check simulates pointer input in a visible window. Keep the host pointer idle during the run so manual input does not interrupt the synthetic drag.

```sh
npm run check:native
```

The checks write local screenshots to `evidence/`. The native check uses window zoom to verify that the stage and photo resize, the marker stays anchored, and dragging still maps to the resized photo. It samples the focal point and crop every 10 drag steps against the pointer, covers clamping past the right, top-right and bottom-left edges, and releases outside the photo. Screenshot comparisons verify that the preview changes while dragging and stays identical when clamped, after release, and after resize.

Native layout keeps the photo and marker aligned while the window edge is dragged. JavaScript readouts catch up on mouseup. The automated checks cover zoom. Live edge dragging, physical pointer feel, and release outside the photo or window with a real device are manual checks.

## Synthetic image

Regenerate the bundled 1200×800 grid with uv and Pillow.

```sh
npm run photo
```

## License

[MIT](../../LICENSE)
