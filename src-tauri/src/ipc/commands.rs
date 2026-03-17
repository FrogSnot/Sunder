use std::collections::HashSet;
use std::sync::atomic::Ordering;

use tauri::State;
use tauri_plugin_notification::NotificationExt;

use crate::audio::AudioHandle;
use crate::audio::engine::AudioCommand;
use crate::audio::equalizer::BAND_COUNT;
use crate::config::{AppConfig, ConfigManager};
use crate::db::SearchCache;
use crate::extraction::Extractor;
use crate::models::{Playlist, SearchResult, SearchSource, Track};

/// Main search entry point. Merges results from local cache, YouTube Music, and YouTube.
#[tauri::command]
pub async fn search(
    query: String,
    config: State<'_, ConfigManager>,
    db: State<'_, SearchCache>,
    extractor: State<'_, Extractor>,
) -> Result<SearchResult, String> {
    let limit = config.get().search_limit;
    let local = db.search_local(&query).map_err(|e: crate::error::AppError| e.to_string())?;
    
    // Only return local results if we have enough to satisfy the limit
    if !local.is_empty() && local.len() >= limit {
        return Ok(SearchResult { tracks: local, source: SearchSource::Local });
    }

    // Search both YT Music and YouTube, merge results
    let (music, youtube) = tokio::join!(
        extractor.search(&query, limit),
        extractor.search_youtube(&query, limit)
    );

    let mut seen = HashSet::new();
    let mut tracks = Vec::new();

    // YT Music results first (priority)
    if let Ok(music_tracks) = music {
        for t in music_tracks {
            if seen.insert(t.id.clone()) {
                tracks.push(t);
            }
        }
    }

    // Then YouTube results (fill gaps)
    if let Ok(yt_tracks) = youtube {
        for t in yt_tracks {
            if seen.insert(t.id.clone()) {
                tracks.push(t);
            }
        }
    }

    let _ = db.upsert_tracks(&tracks);

    Ok(SearchResult { tracks, source: SearchSource::Remote })
}

/// Start playback for a track ID. Resolves duration and sends a Play command to the engine.
#[tauri::command]
pub async fn play_track(
    track_id: String,
    config: State<'_, ConfigManager>,
    audio: State<'_, AudioHandle>,
    db: State<'_, SearchCache>,
    extractor: State<'_, Extractor>,
) -> Result<(), String> {
    let track_info = db.get_track_by_id(&track_id).ok().flatten();
    
    let duration_ms = if let Some(ref t) = track_info {
        (t.duration_secs * 1000.0) as u64
    } else {
        match extractor.metadata(&track_id).await {
            Ok(t) => {
                let _ = db.upsert_tracks(&[t.clone()]);
                (t.duration_secs * 1000.0) as u64
            }
            Err(_) => 0,
        }
    };

    if let Some(t) = track_info {
    }

    let config = config.get();
    audio.send(AudioCommand::Play { 
        video_id: track_id.clone(), 
        duration_ms,
        audio_quality: config.audio_quality,
    });
    let _ = db.record_listen(&track_id);
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
pub async fn set_eq_gains(gains: Vec<f32>, audio: State<'_, AudioHandle>) -> Result<(), String> {
    if gains.len() != BAND_COUNT {
        return Err(format!("expected {BAND_COUNT} gain values"));
    }
    let mut arr = [0.0_f32; BAND_COUNT];
    for (i, &g) in gains.iter().enumerate() {
        arr[i] = g.clamp(-12.0, 12.0);
    }
    audio.eq_settings.write().unwrap().gains = arr;
    Ok(())
}

#[tauri::command]
pub async fn set_eq_enabled(enabled: bool, audio: State<'_, AudioHandle>) -> Result<(), String> {
    audio.eq_settings.write().unwrap().enabled = enabled;
    Ok(())
}

#[tauri::command]
pub async fn get_eq_settings(audio: State<'_, AudioHandle>) -> Result<serde_json::Value, String> {
    let s = audio.eq_settings.read().unwrap();
    Ok(serde_json::json!({
        "enabled": s.enabled,
        "gains": s.gains.to_vec(),
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
pub async fn playlists_containing_track(track_id: String, db: State<'_, SearchCache>) -> Result<Vec<i64>, String> {
    db.playlists_containing_track(&track_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_playlist_tracks(playlist_id: i64, db: State<'_, SearchCache>) -> Result<Vec<Track>, String> {
    db.get_playlist_tracks(playlist_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reorder_playlist_tracks(playlist_id: i64, track_ids: Vec<String>, db: State<'_, SearchCache>) -> Result<(), String> {
    db.reorder_playlist_tracks(playlist_id, &track_ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_yt_playlist(
    url: String,
    playlist_name: String,
    db: State<'_, SearchCache>,
    extractor: State<'_, Extractor>,
) -> Result<Playlist, String> {
    let (extracted_name, tracks) = extractor.extract_playlist(&url).await.map_err(|e| e.to_string())?;
    if tracks.is_empty() {
        return Err("No tracks found in playlist".into());
    }

    // Use extracted name if client didn't provide one
    let name = if playlist_name.trim().is_empty() {
        extracted_name
    } else {
        playlist_name
    };

    let _ = db.upsert_tracks(&tracks);
    let playlist = db.create_playlist(&name).map_err(|e| e.to_string())?;

    for track in &tracks {
        let _ = db.add_to_playlist(playlist.id, &track.id);
    }

    Ok(Playlist {
        id: playlist.id,
        name,
        track_count: tracks.len() as i64,
    })
}



#[tauri::command]
pub async fn prefetch_track(
    track_id: String,
    config: State<'_, ConfigManager>,
) -> Result<(), String> {
    let cache_dir = std::env::temp_dir().join("sunder");
    let _ = std::fs::create_dir_all(&cache_dir);
    let expected_path = cache_dir.join(format!("{track_id}.mp3"));
    if expected_path.exists() {
        return Ok(());
    }
    let conf = config.get();
    let bin = std::env::var("SUNDER_YTDLP_PATH").unwrap_or_else(|_| "yt-dlp".into());
    let url = format!("https://www.youtube.com/watch?v={track_id}");
    let out_template = cache_dir.join(format!("{track_id}.%(ext)s"));
    let quality = conf.audio_quality.to_string();
    
    tauri::async_runtime::spawn(async move {
        let _ = tokio::process::Command::new(&bin)
            .args([
                &url,
                "--extract-audio",
                "--audio-format", "mp3",
                "--audio-quality", &quality,
                "-o", out_template.to_str().unwrap_or_default(),
                "--no-playlist",
                "-q",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;
        eprintln!("[sunder] prefetch done: {track_id} (q={quality})");
    });
    Ok(())
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
    let listen_count = db.listen_count().unwrap_or(0);
    let mut sections: Vec<serde_json::Value> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    macro_rules! fetch_section {
        ($title:expr, $query:expr, $limit:expr) => {{
            if let Ok(tracks) = extractor.search($query, $limit).await {
                let _ = db.upsert_tracks(&tracks);
                let filtered: Vec<Track> = tracks
                    .into_iter()
                    .filter(|t| seen_ids.insert(t.id.clone()))
                    .collect();
                if !filtered.is_empty() {
                    sections.push(serde_json::json!({
                        "title": $title,
                        "tracks": filtered,
                    }));
                }
            }
        }};
    }

    if listen_count < 5 {
        let starters = [
            ("Popular Right Now", "popular music hits"),
            ("Chill Vibes", "chill relaxing music"),
            ("Upbeat Energy", "upbeat energetic songs"),
            ("Discover Indie", "indie music discover"),
            ("Hip-Hop Spotlight", "hip hop rap new music"),
            ("Electronic Beats", "electronic dance music"),
            ("Acoustic Sessions", "acoustic singer songwriter"),
            ("R&B Soul", "rnb soul music"),
        ];
        let offset = chrono_minute() % starters.len();
        let count = 5.min(starters.len());
        for i in 0..count {
            let (title, query) = starters[(offset + i) % starters.len()];
            fetch_section!(title, query, 8);
        }
        return Ok(serde_json::json!({ "sections": sections }));
    }

    let recent = db.recently_played(10).unwrap_or_default();
    let top_artists = db.top_artists(8).unwrap_or_default();
    let keywords = db.title_keywords(15).unwrap_or_default();
    let recent_ids = db.recent_track_ids(7).unwrap_or_default();
    seen_ids.extend(recent_ids);

    // 1) Recently Played
    if !recent.is_empty() {
        sections.push(serde_json::json!({ "title": "Recently Played", "tracks": recent }));
    }

    // 2) "Because you listen to {artist}"
    for artist in top_artists.iter().take(3) {
        let strategies = [
            format!("{artist} similar artists music"),
            format!("{artist} fans also like"),
            format!("{artist} type music"),
        ];
        let pick = simple_hash(artist) % strategies.len();
        let title = format!("Because you listen to {artist}");
        fetch_section!(&title, &strategies[pick], 8);
    }

    // 3) surface genre signals from listening patterns
    let mood_keywords: Vec<&str> = keywords
        .iter()
        .filter(|(w, count)| {
            *count >= 2
                && !top_artists
                    .iter()
                    .any(|a| a.to_lowercase().contains(w.as_str()))
        })
        .take(6)
        .map(|(w, _)| w.as_str())
        .collect();

    if mood_keywords.len() >= 2 {
        for chunk in mood_keywords.chunks(2).take(2) {
            let query = format!("{} music", chunk.join(" "));
            let title = format!(
                "More {}",
                chunk.iter().map(|w| capitalize(w)).collect::<Vec<_>>().join(" & ")
            );
            fetch_section!(&title, &query, 8);
        }
    }

    // 4) combine two different artists for discovery
    if top_artists.len() >= 4 {
        let a1 = &top_artists[0];
        let a2 = &top_artists[top_artists.len() / 2];
        let query = format!("{a1} {a2} mix playlist");
        fetch_section!("Discovery Mix", &query, 8);
    }

    // 5) lesser-played artist gets a spotlight
    if top_artists.len() >= 5 {
        let deep = &top_artists[top_artists.len() - 1];
        let query = format!("{deep} best songs");
        let title = format!("Dig Deeper: {deep}");
        fetch_section!(&title, &query, 6);
    }

    // 6) use a keyword the user gravitates toward
    if !keywords.is_empty() {
        let idx = simple_hash(top_artists.first().map(|s| s.as_str()).unwrap_or(""))
            % keywords.len();
        let word = &keywords[idx].0;
        if !mood_keywords.contains(&word.as_str()) {
            let query = format!("{word} songs playlist");
            let title = format!("You Might Like: {}", capitalize(word));
            fetch_section!(&title, &query, 8);
        }
    }

    Ok(serde_json::json!({ "sections": sections }))
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
        None => String::new(),
    }
}

fn simple_hash(s: &str) -> usize {
    s.bytes().fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize))
}

fn chrono_minute() -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    (secs / 60) as usize
}


