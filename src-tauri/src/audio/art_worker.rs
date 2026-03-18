use std::path::PathBuf;
use tauri_plugin_notification::NotificationExt;

pub fn download_art(video_id: String, url: String) -> Option<PathBuf> {
    let cache_dir = std::env::temp_dir().join("sunder").join("art");
    if !cache_dir.exists() {
        let _ = std::fs::create_dir_all(&cache_dir);
    }

    let path = cache_dir.join(format!("{}.jpg", video_id));
    if path.exists() {
        return Some(path);
    }

    match reqwest::blocking::get(url) {
        Ok(res) => {
            if let Ok(bytes) = res.bytes() {
                if std::fs::write(&path, bytes).is_ok() {
                    return Some(path);
                }
            }
        }
        Err(e) => eprintln!("[sunder] art download failed: {e}"),
    }
    None
}

pub fn trigger_notification(app: &tauri::AppHandle, title: &str, artist: &str, art_path: Option<PathBuf>) {
    let mut builder = app.notification()
        .builder()
        .title(title)
        .body(artist);

    if let Some(path) = art_path {
        // Some platforms/notification servers might not support local file URIs directly
        // but tauri-plugin-notification handles it if possible.
        if let Some(path_str) = path.to_str() {
             builder = builder.icon(path_str);
        }
    }

    let _ = builder.show();
}
