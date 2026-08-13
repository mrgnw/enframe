use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "tiff", "tif", "heic", "heif", "webp", "avif", "jxl", "dng", "arw",
    "cr2", "cr3", "nef", "raf",
];

#[derive(Debug, Serialize, Deserialize)]
pub struct InboxFile {
    pub path: String,
    pub filename: String,
    pub size: u64,
    pub modified: u64,
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

#[tauri::command]
pub fn scan_inbox(directory: String) -> Result<Vec<InboxFile>, String> {
    let dir = Path::new(&directory);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {directory}"));
    }

    let entries = fs::read_dir(dir).map_err(|e| format!("read_dir: {e}"))?;
    let mut files = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| format!("entry: {e}"))?;
        let path = entry.path();
        if !path.is_file() || !is_image(&path) {
            continue;
        }
        let meta = entry.metadata().map_err(|e| format!("metadata: {e}"))?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        files.push(InboxFile {
            path: path.to_string_lossy().to_string(),
            filename: entry.file_name().to_string_lossy().to_string(),
            size: meta.len(),
            modified,
        });
    }

    files.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(files)
}
