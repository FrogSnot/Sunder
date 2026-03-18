use tauri_plugin_notification::NotificationExt;

pub fn trigger_notification(app: &tauri::AppHandle, title: &str, artist: &str, art_url: Option<String>) {
    let mut builder = app.notification()
        .builder()
        .title(title)
        .body(artist);

    if let Some(url) = art_url {
        // Pass URL directly. Note: tauri-plugin-notification handles URL/Path strings
        // depending on the platform's support.
        builder = builder.icon(url);
    }

    let _ = builder.show();
}
