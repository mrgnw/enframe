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

The checks write local screenshots to `evidence/`. The native check uses window zoom to exercise resize and reads crop coordinates from the app. It asserts that the stage and photo size change after the resize, samples the focal point and crop every 10 drag steps against the pointer, covers clamping past the right, top-right and bottom-left edges, releases outside the photo, and compares the preview pixels in screenshots (they change while dragging and stay identical when clamped, after release and after resize).

Not automated: physical mouse or trackpad drag feel, resizing with the window edge, and releasing with a real device outside the photo or window. Synthetic input bypasses the host pointer, so those need a hand run.

## Synthetic image

Regenerate the bundled 1200×800 grid with uv and Pillow.

```sh
npm run photo
```

## License

[MIT](../../LICENSE)
