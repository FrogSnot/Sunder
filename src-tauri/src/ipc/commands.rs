use std::sync::atomic::Ordering;

use tauri::State;

use crate::audio::AudioHandle;
use crate::audio::engine::AudioCommand;
use crate::db::SearchCache;
use crate::extraction::Extractor;
use crate::models::{SearchResult, SearchSource, Track};

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
