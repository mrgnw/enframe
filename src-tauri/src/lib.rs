mod lightroom;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
	tauri::Builder::default()
		.plugin(tauri_plugin_sql::Builder::new().build())
		.plugin(tauri_plugin_fs::init())
		.plugin(tauri_plugin_dialog::init())
		.invoke_handler(tauri::generate_handler![
			lightroom::get_lightroom_library_path,
			lightroom::list_lightroom_photos
		])
		.run(tauri::generate_context!())
		.expect("error while running tauri application")
}
