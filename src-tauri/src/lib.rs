pub mod commands;
pub mod inbox;
pub mod lightroom;
pub mod migrations;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations(
                    "sqlite:enframe.db",
                    migrations::MIGRATIONS
                        .iter()
                        .map(|m| tauri_plugin_sql::Migration {
                            version: m.version,
                            description: m.description,
                            sql: m.sql,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        })
                        .collect(),
                )
                .build(),
        )
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            lightroom::get_lightroom_library_path,
            lightroom::list_lr_albums,
            lightroom::list_lr_album_assets,
            lightroom::search_lr_assets,
            lightroom::list_lr_recent,
            lightroom::get_lr_asset_paths,
            inbox::scan_inbox,
            commands::save_image,
            commands::crop_and_export,
            commands::raw_thumbnail,
            commands::get_screenshot_mode
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}

