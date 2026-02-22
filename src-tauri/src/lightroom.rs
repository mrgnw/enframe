use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct LightroomPhoto {
	pub asset_id: String,
	pub file_path: Option<String>,
	pub preview_path: Option<String>,
	pub width: Option<i32>,
	pub height: Option<i32>,
	pub title: Option<String>,
	pub rating: Option<i32>,
}

#[tauri::command]
pub fn get_lightroom_library_path() -> Result<String, String> {
	let home = std::env::var("HOME").map_err(|e| e.to_string())?;
	let lr_path = PathBuf::from(home)
		.join("Pictures")
		.join("Lightroom Library.lrlibrary");

	if lr_path.exists() {
		Ok(lr_path.to_string_lossy().to_string())
	} else {
		Err("Lightroom library not found at ~/Pictures/Lightroom Library.lrlibrary".to_string())
	}
}

#[tauri::command]
pub async fn list_lightroom_photos() -> Result<Vec<LightroomPhoto>, String> {
	// For now, return empty list
	// We'll implement the actual SQLite reading via tauri-plugin-sql from the frontend
	// or use a custom implementation once we solve the rusqlite conflict
	Ok(vec![])
}
