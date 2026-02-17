use std::sync::atomic::Ordering;

use tauri::State;

use crate::audio::AudioHandle;
use crate::audio::engine::AudioCommand;
use crate::db::SearchCache;
use crate::extraction::Extractor;
use crate::models::{Playlist, SearchResult, SearchSource, Track};

#[tauri::command]
pub async fn search(
    query: String,
    db: State<'_, SearchCache>,
    extractor: State<'_, Extractor>,
) -> Result<SearchResult, String> {
    let local = db.search_local(&query).map_err(|e| e.to_string())?;
    if !local.is_empty() {
        return Ok(SearchResult { tracks: local, source: SearchSource::Local });
    }

    let tracks = extractor.search(&query, 10).await.map_err(|e| e.to_string())?;
    let _ = db.upsert_tracks(&tracks);

    Ok(SearchResult { tracks, source: SearchSource::Remote })
}

#[tauri::command]
pub async fn play_track(
    track_id: String,
    audio: State<'_, AudioHandle>,
    db: State<'_, SearchCache>,
    extractor: State<'_, Extractor>,
) -> Result<(), String> {
    let duration_ms = {
        let local = db.search_local(&track_id).unwrap_or_default();
        if let Some(t) = local.first() {
            (t.duration_secs * 1000.0) as u64
        } else {
            match extractor.metadata(&track_id).await {
                Ok(t) => {
                    let _ = db.upsert_tracks(&[t.clone()]);
                    (t.duration_secs * 1000.0) as u64
                }
                Err(_) => 0,
            }
        }
    };

    let _ = db.record_listen(&track_id);
    audio.send(AudioCommand::Play { video_id: track_id, duration_ms });
    Ok(())
}

#[tauri::command]
pub async fn pause(audio: State<'_, AudioHandle>) -> Result<(), String> {
    audio.send(AudioCommand::Pause);
    Ok(())
}

#[tauri::command]
pub async fn resume(audio: State<'_, AudioHandle>) -> Result<(), String> {
    audio.send(AudioCommand::Resume);
    Ok(())
}

#[tauri::command]
pub async fn stop(audio: State<'_, AudioHandle>) -> Result<(), String> {
    audio.send(AudioCommand::Stop);
    Ok(())
}

#[tauri::command]
pub async fn set_volume(volume: f32, audio: State<'_, AudioHandle>) -> Result<(), String> {
    audio.send(AudioCommand::SetVolume(volume.clamp(0.0, 1.0)));
    Ok(())
}

#[tauri::command]
pub async fn seek(position_secs: f64, audio: State<'_, AudioHandle>) -> Result<(), String> {
    audio.send(AudioCommand::Seek(position_secs));
    Ok(())
}

#[tauri::command]
pub async fn get_playback_state(audio: State<'_, AudioHandle>) -> Result<serde_json::Value, String> {
    let state = audio.state.read().unwrap().clone();
    let pos = audio.position_ms.load(Ordering::Relaxed);
    let dur = audio.duration_ms.load(Ordering::Relaxed);
    let vol = *audio.volume.read().unwrap();

    Ok(serde_json::json!({
        "state": state.to_string(),
        "position_ms": pos,
        "duration_ms": dur,
        "volume": vol,
    }))
}

#[tauri::command]
pub async fn search_local(query: String, db: State<'_, SearchCache>) -> Result<Vec<Track>, String> {
    db.search_local(&query).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_playlist(name: String, db: State<'_, SearchCache>) -> Result<Playlist, String> {
    db.create_playlist(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_playlists(db: State<'_, SearchCache>) -> Result<Vec<Playlist>, String> {
    db.list_playlists().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_playlist(playlist_id: i64, db: State<'_, SearchCache>) -> Result<(), String> {
    db.delete_playlist(playlist_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_playlist(playlist_id: i64, name: String, db: State<'_, SearchCache>) -> Result<(), String> {
    db.rename_playlist(playlist_id, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_to_playlist(playlist_id: i64, track_id: String, db: State<'_, SearchCache>) -> Result<(), String> {
    db.add_to_playlist(playlist_id, &track_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_from_playlist(playlist_id: i64, track_id: String, db: State<'_, SearchCache>) -> Result<(), String> {
    db.remove_from_playlist(playlist_id, &track_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_playlist_tracks(playlist_id: i64, db: State<'_, SearchCache>) -> Result<Vec<Track>, String> {
    db.get_playlist_tracks(playlist_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_recently_played(db: State<'_, SearchCache>) -> Result<Vec<Track>, String> {
    db.recently_played(20).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_explore(
    db: State<'_, SearchCache>,
    extractor: State<'_, Extractor>,
) -> Result<serde_json::Value, String> {
    let top_artists = db.top_artists(5).unwrap_or_default();
    let recent = db.recently_played(10).unwrap_or_default();

    let mut sections: Vec<serde_json::Value> = Vec::new();

    if !recent.is_empty() {
        sections.push(serde_json::json!({
            "title": "Recently Played",
            "tracks": recent,
        }));
    }

    for artist in top_artists.iter().take(3) {
        let query = format!("{artist} music");
        if let Ok(tracks) = extractor.search(&query, 6).await {
            if !tracks.is_empty() {
                let _ = db.upsert_tracks(&tracks);
                sections.push(serde_json::json!({
                    "title": format!("More from {artist}"),
                    "tracks": tracks,
                }));
            }
        }
    }

    let seeds = ["trending music 2026", "chill lofi beats", "indie rock new", "electronic ambient"];
    for seed in &seeds {
        if let Ok(tracks) = extractor.search(seed, 6).await {
            if !tracks.is_empty() {
                let _ = db.upsert_tracks(&tracks);
                let title = seed
                    .split_whitespace()
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            Some(f) => f.to_uppercase().to_string() + c.as_str(),
                            None => String::new(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                sections.push(serde_json::json!({
                    "title": title,
                    "tracks": tracks,
                }));
            }
        }
    }

    Ok(serde_json::json!({ "sections": sections }))
}
