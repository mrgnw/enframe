1. one-writer-instance-connection — every instance a server, app connects to one instead of
   opening the database itself; supersedes the loopback half of serve-exposure-policy
2. serve-exposure-policy — mostly absorbed by the above: the decision is now to drop the
   loopback test and let roots be the only boundary. Still open: config file, interface
   allowlist, read-only mode.
3. scrub-personal-metadata-from-history — parameterize the catalog GUID and filenames out
   of the analysis scripts
4. cloud-only-lightroom-thumbnails — PARKED: no viable route (previews cache underivable,
   Adobe API enterprise-only); LR local-storage setting is the answer

Remaining idea from auto-dual-frame Phase 3: tab-cycle alternative auto placements — tracked in ROADMAP.md.
