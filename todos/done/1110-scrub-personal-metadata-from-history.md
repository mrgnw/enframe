# Scrub personal metadata from public history

Done for the working tree, 2026-11. History is intentionally kept.

The Lightroom catalog GUID, personal ARW filenames, the sample photo folder and
absolute home paths are gone from every tracked script, test and todo. Scripts read
`ENFRAME_LR_CATALOG`, `ENFRAME_SAMPLE_DIR` and `ENFRAME_MAIN_CHECKOUT` instead, and
`lightroom.rs` finds the catalog directory itself.

None of the old values is a credential (the GUID is useless without Adobe auth), so
rewriting `main` was judged not worth it.
