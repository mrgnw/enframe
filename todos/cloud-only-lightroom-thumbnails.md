# Thumbnails for cloud-only Lightroom assets

## Status: no viable programmatic route — parked

Two routes investigated and both ruled out:

**Local previews cache — ruled out** (scripts in `analysis/lr_preview_*.py`, commit
a4256da). The cache filename format is fully decoded — base32 stems, size-tier suffixes,
24-char render key + 26-char settings key — but the key is not derivable from anything
local: no DB references it, ~90M hash-combination candidates over 2003 confirmed pairs
found nothing, and both key fields change on re-render. Dimension-matching identifies
previews for only ~6.5% of assets.

**Adobe Lightroom API — ruled out for this project** (verified against live docs,
Aug 2026). The API is real and the local catalog even embeds its URLs — annotations
carry `/v2/catalogs/{guid}/assets/{id}/...` links using the same GUIDs as the local
directory layout, so id mapping would have been trivial. But access is not self-service:
Lightroom API access now runs through Adobe Firefly Services, which requires an
enterprise contract arranged through Adobe sales. A July 2025 Adobe-employee forum reply
confirms individual developers are directed to sales. The legacy partner API also gated
`fullsize` renditions behind app approval, and community reports describe originals
(`/master`) returning 403 even for entitled apps.

Not pursued on principle: reusing the Lightroom desktop app's own OAuth tokens from
local storage would work mechanically but is fragile and against Adobe's terms.

## The actual answer: let Lightroom put files on disk

Enframe's resolution chain (original → proxy → RAW-embedded preview, all stat-checked
against the deterministic cache layout, shipped 5fba46c) renders anything with a local
file. So the fix for cloud-only tiles is a Lightroom setting, not code:

- Lightroom → Preferences → Local Storage → "Store a copy of all originals locally", or
- per-album "Store album locally" — proxies (2560 JPEG) count and are much smaller.

Once LR materialises either file, the existing code picks it up with no changes.

## Keep

The "Cloud only" badge on tiles with no local file is correct UI — it tells the user
which photos Lightroom has not stored locally, which is exactly the actionable fact.
