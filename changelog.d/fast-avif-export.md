### Changed

- AVIF export is about 11x faster and about 30% smaller by default. The encoder now uses every core (it ran on one) at speed 9 instead of 6, and the default quality is 0.87 instead of 0.92: on the 37 photos in `images/`, 3.1 s and 437 KB per photo became 0.29 s and 313 KB. Quality and speed are both arguments of `crop_and_export`; the defaults live in `EXPORT_QUALITY` and `AVIF_SPEED`.
