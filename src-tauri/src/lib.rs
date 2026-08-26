pub mod commands;
pub mod inbox;
pub mod lightroom;
pub mod migrations;
mod server;

use std::process::Child;
use std::sync::Mutex;
use tauri::Manager as _;

// Only the server we spawned ourselves; one we merely connected to outlives the app.
struct Spawned(Mutex<Option<Child>>);

#[tauri::command]
fn connection_url() -> String {
    server::base_url()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // A server that cannot be reached is not fatal: the library UI already knows
            // how to say it is unreachable, which beats refusing to launch.
            let spawned = server::ensure().unwrap_or_else(|error| {
                eprintln!("enframe: {error}");
                None
            });
            app.manage(Spawned(Mutex::new(spawned)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            connection_url,
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
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                let Some(spawned) = app.try_state::<Spawned>() else {
                    return;
                };
                if let Ok(mut child) = spawned.0.lock()
                    && let Some(child) = child.as_mut()
                {
                    let _ = child.kill();
                }
            }
        })
}
