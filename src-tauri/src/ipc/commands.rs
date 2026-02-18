use std::collections::HashSet;
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
    // Stop old track instantly so there's zero overlap
    audio.send(AudioCommand::Stop);

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
pub async fn reorder_playlist_tracks(playlist_id: i64, track_ids: Vec<String>, db: State<'_, SearchCache>) -> Result<(), String> {
    db.reorder_playlist_tracks(playlist_id, &track_ids).map_err(|e| e.to_string())
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
