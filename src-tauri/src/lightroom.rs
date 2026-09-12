use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const CATALOG_GUID: &str = "00000000000000000000000000000000";

fn library_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let path = PathBuf::from(home)
        .join("Pictures")
        .join("Lightroom Library.lrlibrary");
    if path.exists() {
        Ok(path)
    } else {
        Err("Lightroom library not found".to_string())
    }
}

fn catalog_path() -> Result<PathBuf, String> {
    Ok(library_path()?.join(CATALOG_GUID))
}

fn open_wfindex() -> Result<Connection, String> {
    let path = catalog_path()?.join("Managed Catalog.wfindex");
    Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("wfindex open: {e}"))
}

fn open_mcat() -> Result<Connection, String> {
    let path = catalog_path()?.join("Managed Catalog.mcat");
    let conn = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("mcat open: {e}"))?;
    Ok(conn)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LrAlbum {
    pub album_id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub subtype: Option<String>,
    pub asset_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LrAsset {
    pub asset_id: String,
    pub filename: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub orientation: Option<i64>,
    pub capture_date: Option<String>,
    pub rating: Option<i64>,
    pub flag: Option<i64>,
    pub has_local_original: bool,
    pub has_local_proxy: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LrAssetPaths {
    pub asset_id: String,
    pub original_path: Option<String>,
    pub proxy_path: Option<String>,
    pub xmp_path: Option<String>,
    pub preview_path: Option<String>,
}

#[tauri::command]
pub fn get_lightroom_library_path() -> Result<String, String> {
    library_path().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn list_lr_albums() -> Result<Vec<LrAlbum>, String> {
    let db = open_wfindex()?;
    let mut stmt = db
        .prepare(
            "SELECT a.albumId, a.name, a.parentId, a.subtype, COUNT(aa.assetId) as cnt
			 FROM albums a
			 LEFT JOIN album_asset_v2 aa ON aa.albumId = a.albumId
			 WHERE a.hidden = 0 AND a.name IS NOT NULL
			 GROUP BY a.albumId
			 ORDER BY a.name COLLATE NOCASE",
        )
        .map_err(|e| format!("query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(LrAlbum {
                album_id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                subtype: row.get(3)?,
                asset_count: row.get(4)?,
            })
        })
        .map_err(|e| format!("query_map: {e}"))?;

    let mut albums = Vec::new();
    for row in rows {
        albums.push(row.map_err(|e| format!("row: {e}"))?);
    }
    Ok(albums)
}

#[tauri::command]
pub fn list_lr_album_assets(
    album_id: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<LrAsset>, String> {
    let db = open_wfindex()?;
    let lim = limit.unwrap_or(200);
    let off = offset.unwrap_or(0);

    let mut stmt = db
        .prepare(
            "SELECT a.assetId, a.filename, a.width, a.height, a.orientation,
			        a.captureDate, a.rating, a.flag,
			        COALESCE(s.hasLocalOriginal, 0), COALESCE(s.hasLocalProxy, 0)
			 FROM album_asset_v2 aa
			 JOIN assets a ON a.assetId = aa.assetId
			 LEFT JOIN localStatusAndOzStatus s ON s.docId = a.docId
			 WHERE aa.albumId = ?1
			 ORDER BY aa.sortOrder, a.captureDate DESC
			 LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| format!("query: {e}"))?;

    let rows = stmt
        .query_map(rusqlite::params![album_id, lim, off], |row| {
            Ok(LrAsset {
                asset_id: row.get(0)?,
                filename: row.get(1)?,
                width: row.get(2)?,
                height: row.get(3)?,
                orientation: row.get(4)?,
                capture_date: row.get(5)?,
                rating: row.get(6)?,
                flag: row.get(7)?,
                has_local_original: row.get::<_, f64>(8).unwrap_or(0.0) != 0.0,
                has_local_proxy: row.get::<_, f64>(9).unwrap_or(0.0) != 0.0,
            })
        })
        .map_err(|e| format!("query_map: {e}"))?;

    let mut assets = Vec::new();
    for row in rows {
        assets.push(row.map_err(|e| format!("row: {e}"))?);
    }
    Ok(assets)
}

#[tauri::command]
pub fn search_lr_assets(query: String, limit: Option<i64>) -> Result<Vec<LrAsset>, String> {
    let db = open_wfindex()?;
    let lim = limit.unwrap_or(100);
    let pattern = format!("%{}%", query.to_lowercase());

    let mut stmt = db
        .prepare(
            "SELECT a.assetId, a.filename, a.width, a.height, a.orientation,
			        a.captureDate, a.rating, a.flag,
			        COALESCE(s.hasLocalOriginal, 0), COALESCE(s.hasLocalProxy, 0)
			 FROM assets a
			 LEFT JOIN localStatusAndOzStatus s ON s.docId = a.docId
			 WHERE a.filenameLC LIKE ?1
			 ORDER BY a.captureDate DESC
			 LIMIT ?2",
        )
        .map_err(|e| format!("query: {e}"))?;

    let rows = stmt
        .query_map(rusqlite::params![pattern, lim], |row| {
            Ok(LrAsset {
                asset_id: row.get(0)?,
                filename: row.get(1)?,
                width: row.get(2)?,
                height: row.get(3)?,
                orientation: row.get(4)?,
                capture_date: row.get(5)?,
                rating: row.get(6)?,
                flag: row.get(7)?,
                has_local_original: row.get::<_, f64>(8).unwrap_or(0.0) != 0.0,
                has_local_proxy: row.get::<_, f64>(9).unwrap_or(0.0) != 0.0,
            })
        })
        .map_err(|e| format!("query_map: {e}"))?;

    let mut assets = Vec::new();
    for row in rows {
        assets.push(row.map_err(|e| format!("row: {e}"))?);
    }
    Ok(assets)
}

#[tauri::command]
pub fn list_lr_recent(limit: Option<i64>) -> Result<Vec<LrAsset>, String> {
    let db = open_wfindex()?;
    let lim = limit.unwrap_or(100);

    let mut stmt = db
        .prepare(
            "SELECT a.assetId, a.filename, a.width, a.height, a.orientation,
			        a.captureDate, a.rating, a.flag,
			        COALESCE(s.hasLocalOriginal, 0), COALESCE(s.hasLocalProxy, 0)
			 FROM assets a
			 LEFT JOIN localStatusAndOzStatus s ON s.docId = a.docId
			 WHERE a.subtype = 'image'
			 ORDER BY a.captureDate DESC
			 LIMIT ?1",
        )
        .map_err(|e| format!("query: {e}"))?;

    let rows = stmt
        .query_map(rusqlite::params![lim], |row| {
            Ok(LrAsset {
                asset_id: row.get(0)?,
                filename: row.get(1)?,
                width: row.get(2)?,
                height: row.get(3)?,
                orientation: row.get(4)?,
                capture_date: row.get(5)?,
                rating: row.get(6)?,
                flag: row.get(7)?,
                has_local_original: row.get::<_, f64>(8).unwrap_or(0.0) != 0.0,
                has_local_proxy: row.get::<_, f64>(9).unwrap_or(0.0) != 0.0,
            })
        })
        .map_err(|e| format!("query_map: {e}"))?;

    let mut assets = Vec::new();
    for row in rows {
        assets.push(row.map_err(|e| format!("row: {e}"))?);
    }
    Ok(assets)
}

fn existing_file(path: PathBuf) -> Option<String> {
    path.is_file().then(|| path.to_string_lossy().into_owned())
}

// The annotation lags the filesystem in both directions while Lightroom syncs: it can
// reference an original the cloud already evicted, and it can omit files that are
// sitting on disk. Every returned path is therefore stat-checked, and when the
// annotation says nothing the deterministic cache layout is tried directly —
// originals/YYYY/YYYY-MM-DD/<filename> and proxies/2560/<id[..3]>/<id[3..]>.
fn resolve_original(
    catalog: &std::path::Path,
    annotated: Option<String>,
    filename: Option<&str>,
    capture_date: Option<&str>,
) -> Option<String> {
    if let Some(path) = annotated
        && let Some(found) = existing_file(PathBuf::from(path))
    {
        return Some(found);
    }
    let name = filename?;
    let date = capture_date?;
    let day = date.get(..10)?;
    let year = date.get(..4)?;
    existing_file(catalog.join("originals").join(year).join(day).join(name))
}

fn resolve_proxy(
    catalog: &std::path::Path,
    annotated: Option<String>,
    asset_id: &str,
) -> Option<String> {
    if let Some(path) = annotated
        && let Some(found) = existing_file(PathBuf::from(path))
    {
        return Some(found);
    }
    if asset_id.len() < 4 || !asset_id.is_ascii() {
        return None;
    }
    existing_file(
        catalog
            .join("proxies")
            .join("2560")
            .join(&asset_id[..3])
            .join(&asset_id[3..]),
    )
}

#[tauri::command]
pub fn get_lr_asset_paths(asset_ids: Vec<String>) -> Result<Vec<LrAssetPaths>, String> {
    let mcat = open_mcat()?;
    let wfindex = open_wfindex()?;
    let catalog = catalog_path()?;
    let previews_db_path = catalog.join("previews.db");

    // Open previews.db to resolve preview paths
    let previews = if previews_db_path.exists() {
        Connection::open_with_flags(
            &previews_db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .ok()
    } else {
        None
    };

    // Build a lookup for preview paths
    let mut preview_map: HashMap<String, String> = HashMap::new();
    if let Some(ref pdb) = previews {
        // Get best preview per asset for the requested ids
        for asset_id in &asset_ids {
            if let Ok(path) = pdb.query_row(
                "SELECT path FROM previews WHERE localAssetId = ?1 ORDER BY longSide DESC LIMIT 1",
                [asset_id],
                |row| row.get::<_, String>(0),
            ) {
                preview_map.insert(asset_id.clone(), path);
            }
        }
    }

    mcat.execute("PRAGMA query_only = ON", [])
        .map_err(|e| format!("pragma: {e}"))?;

    let mut results = Vec::with_capacity(asset_ids.len());

    for asset_id in &asset_ids {
        // Lightroom writes the msgpack annotation with TEXT storage class, so ask for the
        // raw bytes: `get::<Vec<u8>>` only accepts a Blob and would silently yield None.
        let annotation: Option<Vec<u8>> = mcat
			.query_row(
				"SELECT annotation FROM docs WHERE fullDocId = ?1 AND type = 'asset' AND deleted = 0",
				[asset_id],
				|row| Ok(row.get_ref(0)?.as_bytes().ok().map(<[u8]>::to_vec)),
			)
			.ok()
			.flatten();

        let mut original_path: Option<String> = None;
        let mut proxy_path: Option<String> = None;
        let mut xmp_path: Option<String> = None;

        if let Some(blob) = annotation
            && let Ok(data) = rmp_serde::from_slice::<rmpv::Value>(&blob)
            && let Some(local_only) = data.as_map().and_then(|m| {
                m.iter()
                    .find(|(k, _)| k.as_str() == Some("_localOnly"))
                    .map(|(_, v)| v)
            })
        {
            if let Some(files) = local_only.as_map().and_then(|m| {
                m.iter()
                    .find(|(k, _)| k.as_str() == Some("files"))
                    .map(|(_, v)| v)
            }) {
                // original.master.relativePath
                original_path = get_nested_str(files, &["original", "master", "relativePath"])
                    .map(|rp| format!("/{rp}"));

                // proxy.2560.relativePath
                proxy_path = get_nested_str(files, &["proxy", "2560", "relativePath"])
                    .map(|rp| format!("/{rp}"));
            }

            // xmp_develop.path (absolute already)
            xmp_path = get_nested_str(local_only, &["xmp_develop", "path"]).map(|s| s.to_string());
        }

        let (filename, capture_date) = wfindex
            .query_row(
                "SELECT filename, captureDate FROM assets WHERE assetId = ?1",
                [asset_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .unwrap_or((None, None));

        let original_path = resolve_original(
            &catalog,
            original_path,
            filename.as_deref(),
            capture_date.as_deref(),
        );
        let proxy_path = resolve_proxy(&catalog, proxy_path, asset_id);
        let preview_path = preview_map.get(asset_id).cloned();

        results.push(LrAssetPaths {
            asset_id: asset_id.clone(),
            original_path,
            proxy_path,
            xmp_path,
            preview_path,
        });
    }

    Ok(results)
}

fn get_nested_str<'a>(val: &'a rmpv::Value, keys: &[&str]) -> Option<&'a str> {
    let mut current = val;
    for key in keys {
        current = current.as_map().and_then(|m| {
            m.iter()
                .find(|(k, _)| k.as_str() == Some(key))
                .map(|(_, v)| v)
        })?;
    }
    current.as_str()
}
