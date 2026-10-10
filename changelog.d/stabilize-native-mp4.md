### Changed

- Stabilize: `.mp4` clips no longer need ffmpeg. They are encoded as H.264 by a built-in encoder, with the same colour tags as before; files are about 14% larger at the same quality. ffmpeg is still needed for `.mov`, `.webm` and `--vf`.
