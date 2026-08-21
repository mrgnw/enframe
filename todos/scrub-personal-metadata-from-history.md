# Scrub personal metadata from public history

The public repo (`github.com/mrgnw/enframe`) has personal metadata committed on
`ux-improvements`:

- `analysis/lr_*.py` and `analysis/broken_tile_probe.py` hard-code the Lightroom
  catalog GUID `00000000000000000000000000000000` and personal filenames (`A7_*.ARW`)
- the `commands.rs` thumbnail test references `~/Pictures/samples/IMG_0001.ARW`

None of it is a credential — the GUID is useless without Adobe auth — but it is
personal metadata. Already pushed, so scrubbing means rewriting branch history
(or squash-merging the branch and deleting it).

`ux-improvements` was fast-forwarded into `main` on 2026-08-21, so a history scrub
now means rewriting `main`. The forward-looking fix is a content scrub: parameterize
the scripts via env var / first CLI arg and drop the hard-coded GUID and filenames
(they also appear in `todos/cloud-only-lightroom-thumbnails.md`).
