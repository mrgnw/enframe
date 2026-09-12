1. one-writer-instance-connection — phases 1 and 2 done. Remaining: phase 3, export
   destination chosen per export, which is what a remotely-connected app needs to write
   back to the machine you are sitting at.
2. serve-exposure-policy — the loopback half is gone: roots are the only boundary now.
   Still open, and now the only kind of hardening left worth doing, because each narrows
   what *any* caller can do: config file, interface allowlist, read-only mode.
3. scrub-personal-metadata-from-history — parameterize the catalog GUID and filenames out
   of the analysis scripts
4. cloud-only-lightroom-thumbnails — PARKED: no viable route (previews cache underivable,
   Adobe API enterprise-only); LR local-storage setting is the answer

Remaining idea from auto-dual-frame Phase 3: tab-cycle alternative auto placements — tracked in ROADMAP.md.
