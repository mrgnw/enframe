# Thumbnails for cloud-only Lightroom assets

## What was ruled out

The local `previews/` cache cannot serve these. A full investigation (scripts in
`analysis/lr_preview_*.py`, commit a4256da) decoded the cache filename format completely
— base32 stems, size-tier suffixes, 24-char render key + 26-char settings key — and
proved the key is not derivable from anything local: no DB references it, ~90 million
hash-combination candidates over 2003 confirmed pairs found nothing, and both key fields
change when an asset is re-rendered. Dimension-matching via the embedded
`com.adobe.carousel.reference_size` segment identifies previews for only ~6.5% of assets
(catalog-unique dimensions); the rest are ambiguous.

## The remaining route: Adobe Lightroom Services API

https://developer.adobe.com/lightroom/ — OAuth via Adobe IMS, app registered in the
Adobe Developer Console. Relevant pieces:

- `GET /v2/catalogs/{catalog_id}/assets/{asset_id}/renditions/{size}` — server-rendered
  thumbnails (`thumbnail2x`, `640`, `1280`, `2048`) for any asset, local or not.
- Asset ids in the cloud API are the same 32-hex ids the local catalog uses, so the
  existing browser needs no id translation.
- Requires user login + network; fits as an optional enhancement layered onto the
  existing local-first resolution (original → proxy → cloud rendition).

Open questions: token storage (desktop keychain vs serve config), rate limits for a
200-tile grid, and whether the standard OAuth scopes cover renditions without partner
approval.

## Scope guard

Only thumbnails. Importing a cloud-only original is a separate feature (rendition
download tops out at 2048px; full originals have their own endpoint and much bigger
payloads).
