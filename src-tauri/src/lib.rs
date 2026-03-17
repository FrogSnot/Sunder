mod audio;
pub mod config;
mod db;
mod error;
mod extraction;
mod ipc;
pub mod models;

use tauri::{Emitter, Manager};
use crate::config::ConfigManager;
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

            let config_manager = ConfigManager::new(&data_dir);
            let initial_config = config_manager.get();

            app.manage(SearchCache::new(&data_dir).expect("failed to init database"));
            app.manage(AudioHandle::new(
                app.handle().clone(),
                initial_config.volume as f32,
                crate::audio::equalizer::EqSettings {
                    enabled: initial_config.eq_enabled,
                    gains: {
                        let mut g = [0.0_f32; 10];
                        for (i, &v) in initial_config.eq_gains.iter().enumerate().take(10) {
                            g[i] = v as f32;
                        }
                        g
                    },
                },
            ));
            app.manage(Extractor::new());
            app.manage(config_manager);

            // System Tray Setup
            use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
            use tauri::tray::{TrayIconBuilder, TrayIconEvent, MouseButton};

            let play_pause = MenuItem::with_id(app, "play_pause", "Play / Pause", true, None::<&str>)?;
            let next = MenuItem::with_id(app, "next", "Next Track", true, None::<&str>)?;
            let prev = MenuItem::with_id(app, "prev", "Previous Track", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
            let restart = MenuItem::with_id(app, "restart", "Restart App", true, None::<&str>)?;
            let exit = MenuItem::with_id(app, "exit", "Exit Sunder", true, None::<&str>)?;

            let tray_menu = Menu::with_items(
                app,
                &[
                    &play_pause,
                    &next,
                    &prev,
                    &PredefinedMenuItem::separator(app)?,
                    &show,
                    &restart,
                    &exit,
                ],
            )?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .on_menu_event(|app, event| {
                    let id = event.id().0.as_str();
                    eprintln!("[sunder] tray menu event: {}", id);
                    
                    let window = app.get_webview_window("main");
                    
                    match id {
                        "play_pause" => {
                            if let Some(w) = window {
                                let _ = w.emit("media-toggle", ());
                            }
                        }
                        "next" => {
                            if let Some(w) = window {
                                let _ = w.emit("media-next", ());
                            }
                        }
                        "prev" => {
                            if let Some(w) = window {
                                let _ = w.emit("media-previous", ());
                            }
                        }
                        "show" => {
                            if let Some(w) = window {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "restart" => {
                            app.restart();
                        }
                        "exit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, .. } = event {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .build(app)?;

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
            ipc::commands::get_subtitles,
            ipc::commands::import_yt_playlist,
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
            ipc::commands::get_config,
            ipc::commands::set_config,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Sunder");
}
