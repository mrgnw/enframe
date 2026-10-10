### Changed

- Stabilize: `.mp4` clips no longer need ffmpeg. They are encoded as H.264 by a built-in encoder, with the same colour tags as before. Files are 15 to 20% larger, and a long clip encodes slower: 57 photos took 8.3 s instead of 4.8 s. ffmpeg is still needed for `.mov`, `.webm` and `--vf`.
