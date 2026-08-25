1. serve-exposure-policy — what should be reachable over a network at all, vs local-only
   Open: whether `/api/ls` (root-confined, but enumerates) should answer remotely, since
   without it a remote browser cannot add a folder bookmark.
2. scrub-personal-metadata-from-history — parameterize the catalog GUID and filenames out
   of the analysis scripts
3. cloud-only-lightroom-thumbnails — PARKED: no viable route (previews cache underivable,
   Adobe API enterprise-only); LR local-storage setting is the answer

Remaining idea from auto-dual-frame Phase 3: tab-cycle alternative auto placements — tracked in ROADMAP.md.
