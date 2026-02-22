#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod lightroom;

use base64::Engine;
use std::fs;

#[tauri::command]
fn save_image(path: String, data: String) -> Result<(), String> {
	let bytes = base64::engine::general_purpose::STANDARD
		.decode(&data)
		.map_err(|e| e.to_string())?;
	fs::write(&path, bytes).map_err(|e| e.to_string())?;
	Ok(())
}

fn main() {
	tauri::Builder::default()
		.plugin(
			tauri_plugin_sql::Builder::new()
				.add_migrations(
					"sqlite:enframe.db",
					vec![tauri_plugin_sql::Migration {
						version: 1,
						description: "create tables",
						sql: include_str!("../../database.sql"),
						kind: tauri_plugin_sql::MigrationKind::Up,
					}],
				)
				.build(),
		)
		.plugin(tauri_plugin_fs::init())
		.plugin(tauri_plugin_dialog::init())
		.invoke_handler(tauri::generate_handler![
			lightroom::get_lightroom_library_path,
			lightroom::list_lightroom_photos,
			save_image
		])
		.run(tauri::generate_context!())
		.expect("error while running tauri application")
}
