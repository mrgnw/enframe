#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod lightroom;

use tauri::Manager;
use std::fs;

#[tauri::command]
fn save_image(path: String, data: String) -> Result<(), String> {
	let bytes = base64::decode(&data).map_err(|e| e.to_string())?;
	fs::write(&path, bytes).map_err(|e| e.to_string())?;
	Ok(())
}

fn main() {
	tauri::Builder::default()
		.plugin(tauri_plugin_sql::Builder::new().build())
		.plugin(tauri_plugin_fs::init())
		.plugin(tauri_plugin_dialog::init())
		.invoke_handler(tauri::generate_handler![
			lightroom::get_lightroom_library_path,
			lightroom::list_lightroom_photos,
			save_image
		])
		.setup(|app| {
			// Initialize the database with schema
			let app_handle = app.handle().clone();
			tauri::async_runtime::spawn(async move {
				if let Err(e) = init_database(&app_handle).await {
					eprintln!("Failed to initialize database: {}", e);
				}
			});
			Ok(())
		})
		.run(tauri::generate_context!())
		.expect("error while running tauri application")
}

async fn init_database(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
	use tauri_plugin_sql::SqlExt;
	
	let db = app.sql("sqlite:enframe.db")?;
	
	let schema = include_str!("../../database.sql");
	db.execute(schema).await?;
	
	Ok(())
}
