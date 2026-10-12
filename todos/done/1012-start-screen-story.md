---
issue: 29
branch: start-screen-story
pull-request: 49
---
# Start screens that tell each app's story

Stabilize tells the story of one thing across multiple images; enframe tells multiple stories from one image. The taglines and the visuals above them should say that, as a pair. "Hold one thing still" is not the message.

The visual is a flip book: something mostly consistent across many frames that changes slightly each frame, in position or in how it is drawn. The variants in `mockups/stabilize/03-start-visual.html` missed that.

Mockup first (both taglines, both visuals, a few variants), then build the chosen pair into `Standalone.svelte`'s empty screen and the Stabilize folder picker. Details in the issue.
