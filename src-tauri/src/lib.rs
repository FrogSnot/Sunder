mod audio;
mod db;
mod error;
mod extraction;
mod ipc;
pub mod models;

use tauri::Manager;

use audio::AudioHandle;
use db::SearchCache;
use extraction::Extractor;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::current_dir().unwrap().join("sunder_data"));

            app.manage(SearchCache::new(&data_dir).expect("failed to init database"));
            app.manage(AudioHandle::new(app.handle().clone()));
            app.manage(Extractor::new());

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
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Sunder");
}
