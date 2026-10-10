### Changed

- Stabilize: `.mp4` clips no longer need ffmpeg. They are encoded as H.264 by a built-in encoder, with the same colour tags as before. Files are about 16% larger at similar quality, and a long clip encodes somewhat slower than through ffmpeg. ffmpeg is still needed for `.mov`, `.webm` and `--vf`.
