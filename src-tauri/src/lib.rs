mod audio;
pub mod config;
mod db;
mod error;
mod extraction;
mod ipc;
pub mod models;

use tauri::{Emitter, Manager};

use audio::AudioHandle;
use config::ConfigManager;
use db::SearchCache;
use extraction::Extractor;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::current_dir().unwrap().join("sunder_data"));

            app.manage(SearchCache::new(&data_dir).expect("failed to init database"));
            app.manage(AudioHandle::new(app.handle().clone()));
            app.manage(Extractor::new());
            app.manage(ConfigManager::new(&data_dir));

            // Wayland Icon Fix & Window Decoration
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_icon(app.default_window_icon().unwrap().clone());
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::commands::search,
            ipc::commands::search_local,
            ipc::commands::play_track,
            ipc::commands::pause,
            ipc::commands::resume,
            ipc::commands::stop,
            ipc::commands::set_volume,
            ipc::commands::seek,
            ipc::commands::get_playback_state,
            ipc::commands::create_playlist,
            ipc::commands::list_playlists,
            ipc::commands::delete_playlist,
            ipc::commands::rename_playlist,
            ipc::commands::add_to_playlist,
            ipc::commands::remove_from_playlist,
            ipc::commands::playlists_containing_track,
            ipc::commands::get_playlist_tracks,
            ipc::commands::reorder_playlist_tracks,
            ipc::commands::get_recently_played,
            ipc::commands::get_explore,
            ipc::commands::prefetch_track,
            ipc::commands::set_eq_gains,
            ipc::commands::set_eq_enabled,
            ipc::commands::get_eq_settings,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::ScaleFactorChanged { .. } = event {
                // Force a layout recalculation. A tiny resize often helps "snap" the webview.
                let window_clone = window.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    if let Ok(size) = window_clone.outer_size() {
                        let _ = window_clone.set_size(size);
                    }
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run Sunder");
}
