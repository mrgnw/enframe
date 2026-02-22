# Canvas / Viewport

Editor viewport UX — zoom, pan, and fit behavior.

## Tasks

- [ ] Fit-to-view on image open (calculate scale from image dimensions vs container)
- [ ] Press `0` to toggle between fit and fill modes
- [ ] Fix view reset on interaction — `onUpdate` callback re-fetches from DB causing re-render and zoom/pan state loss
- [ ] Lightroom-style zoom/pinch behavior (smooth, centered on cursor)
