use tauri::State;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;

use crate::config::{AppConfig, ConfigManager};
use crate::discord::{DiscordPresence, PresenceCommand};

#[tauri::command]
pub fn get_config(config: State<'_, ConfigManager>) -> AppConfig {
    config.get()
}

#[tauri::command]
pub fn set_config(config: AppConfig, manager: State<'_, ConfigManager>) {
    manager.update(config);
}

use crate::audio::AudioHandle;
use crate::audio::engine::AudioCommand;
use crate::audio::equalizer::BAND_COUNT;
use crate::db::{CachedLyrics, SearchCache};
use crate::downloads::DownloadManager;
use crate::extraction::Extractor;
use crate::models::{Playlist, SearchResult, SearchSource, Track};

#[tauri::command]
pub async fn search(
    query: String,
    limit: Option<usize>,
    db: State<'_, SearchCache>,
    extractor: State<'_, Extractor>,
) -> Result<SearchResult, String> {
    let limit = limit.unwrap_or(20).min(100);
    let local = db.search_local(&query).map_err(|e| e.to_string())?;
    let local_count = local.len();

    // Always search both YT Music and YouTube in parallel, then merge with local
    let (music, youtube) = tokio::join!(
        extractor.search(&query, limit),
        extractor.search_youtube(&query, limit)
    );

    let mut seen = HashSet::new();
    let mut tracks = Vec::new();

    let music_err = music.as_ref().err().map(|e| e.to_string());
    let youtube_err = youtube.as_ref().err().map(|e| e.to_string());

    // Local results first (priority)
    for t in local {
        if seen.insert(t.id.clone()) {
            tracks.push(t);
        }
    }

    // YT Music results next
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

    // If both sources failed, propagate the error instead of returning empty results
    if tracks.is_empty() {
        if let Some(e) = music_err {
            return Err(e);
        }
        if let Some(e) = youtube_err {
            return Err(e);
        }
    }

    let _ = db.upsert_tracks(&tracks);

    let remote_added = tracks.len().saturating_sub(local_count);
    let source = if local_count > 0 && remote_added > 0 {
        SearchSource::Mixed
    } else if local_count > 0 {
        SearchSource::Local
    } else {
        SearchSource::Remote
    };

    Ok(SearchResult { tracks, source })
}

#[tauri::command]
pub async fn play_track(
    track_id: String,
    audio: State<'_, AudioHandle>,
    db: State<'_, SearchCache>,
    extractor: State<'_, Extractor>,
    discord: State<'_, DiscordPresence>,
) -> Result<(), String> {
    // Look up duration from DB by primary key (instant).
    // Only fall back to yt-dlp metadata if the track was never seen before.
    let (duration_ms, title, artist, thumbnail) = match db.get_track_by_id(&track_id) {
        Ok(Some(t)) => ((t.duration_secs * 1000.0) as u64, t.title, t.artist, t.thumbnail),
        _ => {
            match extractor.metadata(&track_id).await {
                Ok(t) => {
                    let _ = db.upsert_tracks(std::slice::from_ref(&t));
                    ((t.duration_secs * 1000.0) as u64, t.title, t.artist, t.thumbnail)
                }
                Err(_) => (0u64, "Unknown".to_string(), "Unknown".to_string(), String::new()),
            }
        }
    };

    audio.send(AudioCommand::Play { video_id: track_id.clone(), duration_ms });
    audio.send(AudioCommand::UpdateMetadata { 
        title: title.clone(), 
        artist: artist.clone(), 
        thumbnail: thumbnail.clone(),
        track_id: track_id.clone(),
    });
    discord.send(PresenceCommand::SetActivity {
        title,
        artist,
        thumbnail,
    });
    let _ = db.record_listen(&track_id);
    Ok(())
}

#[tauri::command]
pub async fn pause(audio: State<'_, AudioHandle>, discord: State<'_, DiscordPresence>) -> Result<(), String> {
    audio.send(AudioCommand::Pause);
    discord.send(PresenceCommand::Pause);
    Ok(())
}

#[tauri::command]
pub async fn resume(audio: State<'_, AudioHandle>, discord: State<'_, DiscordPresence>) -> Result<(), String> {
    audio.send(AudioCommand::Resume);
    discord.send(PresenceCommand::Resume);
    Ok(())
}

#[tauri::command]
pub async fn stop(audio: State<'_, AudioHandle>, discord: State<'_, DiscordPresence>) -> Result<(), String> {
    audio.send(AudioCommand::Stop);
    discord.send(PresenceCommand::Clear);
    Ok(())
}

#[tauri::command]
pub async fn set_volume(volume: f32, audio: State<'_, AudioHandle>) -> Result<(), String> {
    audio.send(AudioCommand::SetVolume(volume.clamp(0.0, 1.0)));
    Ok(())
}

#[tauri::command]
pub async fn set_speed(speed: f32, audio: State<'_, AudioHandle>) -> Result<(), String> {
    audio.send(AudioCommand::SetSpeed(speed.clamp(0.25, 3.0)));
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
    let spd = *audio.speed.read().unwrap();

    Ok(serde_json::json!({
        "state": state.to_string(),
        "position_ms": pos,
        "duration_ms": dur,
        "volume": vol,
        "speed": spd,
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
pub async fn set_repeat_mode(mode: String, audio: State<'_, AudioHandle>) -> Result<(), String> {
    if !["off", "queue", "track"].contains(&mode.as_str()) {
        return Err(format!("Invalid repeat mode: {}", mode));
    }
    audio.send(AudioCommand::SetRepeat(mode));
    Ok(())
}

#[tauri::command]
pub async fn search_local(query: String, db: State<'_, SearchCache>) -> Result<Vec<Track>, String> {
    db.search_local(&query).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_playlist(name: String, db: State<'_, SearchCache>) -> Result<Playlist, String> {
    db.create_playlist(&name, "").map_err(|e| e.to_string())
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
pub async fn prefetch_track(
    track_id: String,
) -> Result<(), String> {
    let cache_dir = std::env::temp_dir().join("sunder");
    let _ = std::fs::create_dir_all(&cache_dir);
    let expected_path = cache_dir.join(format!("{track_id}.mp3"));
    if expected_path.exists() {
        return Ok(());
    }
    let bin = std::env::var("SUNDER_YTDLP_PATH").unwrap_or_else(|_| "yt-dlp".into());
    let url = format!("https://www.youtube.com/watch?v={track_id}");
    let out_template = cache_dir.join(format!("{track_id}.%(ext)s"));
    tokio::spawn(async move {
        let _ = tokio::process::Command::new(&bin)
            .args([
                &url,
                "--extract-audio",
                "--audio-format", "mp3",
                "--audio-quality", "2",
                "-o", out_template.to_str().unwrap_or_default(),
                "--no-playlist",
                "--concurrent-fragments", "4",
                "-q",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;
        eprintln!("[sunder] prefetch done: {track_id}");
    });
    Ok(())
}

#[tauri::command]
pub async fn get_subtitles(
    video_id: String,
    lang: String,
    extractor: State<'_, Extractor>,
) -> Result<String, String> {
    extractor.get_subtitles(&video_id, &lang).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_lyrics_cache(track_id: String, db: State<'_, SearchCache>) -> Option<CachedLyrics> {
    db.get_lyrics(&track_id).ok().flatten()
}

#[tauri::command]
pub fn save_lyrics_cache(
    track_id: String,
    content: String,
    synced_lyrics: String,
    source: String,
    db: State<'_, SearchCache>,
) -> Result<(), String> {
    db.upsert_lyrics(&track_id, &content, &synced_lyrics, &source)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_lyric_offset(track_id: String, db: State<'_, SearchCache>) -> i64 {
    db.get_lyric_offset(&track_id).unwrap_or(0)
}

#[tauri::command]
pub fn set_lyric_offset(track_id: String, offset_ms: i64, db: State<'_, SearchCache>) -> Result<(), String> {
    db.upsert_lyric_offset(&track_id, offset_ms)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_yt_playlist(
    url: String,
    playlist_name: String,
    db: State<'_, SearchCache>,
    extractor: State<'_, Extractor>,
) -> Result<Playlist, String> {
    let (extracted_name, playlist_thumbnail, tracks) = extractor
        .extract_playlist(&url)
        .await
        .map_err(|e| e.to_string())?;

    if tracks.is_empty() {
        return Err("No tracks found in playlist".into());
    }

    let name = if playlist_name.trim().is_empty() {
        extracted_name
    } else {
        playlist_name
    };

    let thumbnail = playlist_thumbnail.unwrap_or_default();
    let playlist = db.create_playlist(&name, &thumbnail).map_err(|e| e.to_string())?;
    let _ = db.set_playlist_source(playlist.id, &url);
    let _ = db.upsert_tracks(&tracks);
    for track in tracks {
        let _ = db.add_to_playlist(playlist.id, &track.id);
    }

    Ok(playlist)
}

#[tauri::command]
pub async fn refresh_yt_playlist(
    playlist_id: i64,
    db: State<'_, SearchCache>,
    extractor: State<'_, Extractor>,
) -> Result<i64, String> {
    let url = db
        .get_playlist_source(playlist_id)
        .map_err(|e| e.to_string())?
        .ok_or("This playlist was not imported from YouTube/YTM")?;

    let (_, _, tracks) = extractor
        .extract_playlist(&url)
        .await
        .map_err(|e| e.to_string())?;
    if tracks.is_empty() {
        return Err("No tracks found at source URL".into());
    }

    db.upsert_tracks(&tracks).map_err(|e| e.to_string())?;
    let ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
    db.replace_playlist_tracks(playlist_id, &ids)
        .map_err(|e| e.to_string())?;

    Ok(tracks.len() as i64)
}

async fn resolve_track(
    track_id: &str,
    db: &SearchCache,
    extractor: &Extractor,
) -> Result<Track, String> {
    if let Ok(Some(track)) = db.get_track_by_id(track_id) {
        return Ok(track);
    }
    extractor.metadata(track_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_track(
    track_id: String,
    app: tauri::AppHandle,
    db: State<'_, SearchCache>,
    dm: State<'_, DownloadManager>,
    extractor: State<'_, Extractor>,
) -> Result<(), String> {
    let track = resolve_track(&track_id, &db, &extractor).await?;
    dm.download(&app, &db, &track).await
}

#[tauri::command]
pub async fn download_tracks(
    track_ids: Vec<String>,
    app: tauri::AppHandle,
    db: State<'_, SearchCache>,
    dm: State<'_, DownloadManager>,
    extractor: State<'_, Extractor>,
) -> Result<(), String> {
    let mut tracks = db.get_tracks_by_ids(&track_ids).map_err(|e| e.to_string())?;
    let resolved: HashSet<String> = tracks.iter().map(|t| t.id.clone()).collect();
    for id in &track_ids {
        if !resolved.contains(id) {
            if let Ok(track) = extractor.metadata(id).await {
                tracks.push(track);
            }
        }
    }

    let jobs = tracks.iter().map(|track| dm.download(&app, &db, track));
    futures::future::join_all(jobs).await;
    Ok(())
}

#[tauri::command]
pub async fn download_playlist(
    playlist_id: i64,
    app: tauri::AppHandle,
    db: State<'_, SearchCache>,
    dm: State<'_, DownloadManager>,
) -> Result<(), String> {
    let tracks = db
        .get_playlist_tracks(playlist_id)
        .map_err(|e| e.to_string())?;
    let jobs = tracks.iter().map(|track| dm.download(&app, &db, track));
    futures::future::join_all(jobs).await;
    Ok(())
}

#[tauri::command]
pub async fn delete_download(
    track_id: String,
    db: State<'_, SearchCache>,
    dm: State<'_, DownloadManager>,
) -> Result<(), String> {
    dm.delete(&db, &track_id)
}

#[tauri::command]
pub async fn is_track_downloaded(
    track_id: String,
    db: State<'_, SearchCache>,
) -> Result<bool, String> {
    db.is_downloaded(&track_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_downloaded_ids(db: State<'_, SearchCache>) -> Result<Vec<String>, String> {
    db.downloaded_ids().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_downloads(db: State<'_, SearchCache>) -> Result<Vec<Track>, String> {
    db.downloaded_tracks().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_downloads_size(db: State<'_, SearchCache>) -> Result<i64, String> {
    db.downloads_size().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_download_sizes(
    db: State<'_, SearchCache>,
) -> Result<Vec<(String, i64)>, String> {
    db.download_sizes().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_recently_played(db: State<'_, SearchCache>) -> Result<Vec<Track>, String> {
    db.recently_played(20).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_explore(
    db: State<'_, SearchCache>,
    extractor: State<'_, Extractor>,
) -> Result<ExploreResponse, String> {
    let listen_count = db.listen_count().unwrap_or(0);
    let mut sections: Vec<ExploreSection> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    let mut queries = if listen_count < 5 {
        starter_queries()
    } else {
        let recent = db.recently_played(RECENTLY_PLAYED_LIMIT).unwrap_or_default();
        let artist_signals = db.artist_affinities(12).unwrap_or_default();
        let keywords = db.title_keywords(20).unwrap_or_default();
        let recent_ids = db.recent_track_ids(14).unwrap_or_default();
        seen_ids.extend(recent_ids);

        if !recent.is_empty() {
            seen_ids.extend(recent.iter().map(|track| track.id.clone()));
            sections.push(ExploreSection {
                title: "Jump Back In".to_string(),
                tracks: recent.clone(),
            });
        }

        personalized_queries(&artist_signals, &keywords, &recent, listen_count)
    };

    dedupe_queries(&mut queries);
    queries.sort_by_key(|b| std::cmp::Reverse(b.score));
    queries.truncate(MAX_EXPLORE_QUERIES);

    let results: Vec<Vec<Track>> = futures::future::join_all(
        queries.iter().map(|seed| async {
            let mut tracks = Vec::new();
            let mut result_seen = HashSet::new();

            if let Ok(music_tracks) = extractor.search(seed.query.as_str(), seed.limit).await {
                append_unique(&mut tracks, &mut result_seen, music_tracks);
            }

            if tracks.len() < EXPLORE_TRACK_LIMIT {
                let fill_limit = seed.limit.saturating_sub(tracks.len()).max(10);
                if let Ok(youtube_tracks) = extractor.search_youtube(seed.query.as_str(), fill_limit).await {
                    append_unique(&mut tracks, &mut result_seen, youtube_tracks);
                }
            }

            tracks
        })
    ).await;

    for (seed, tracks) in queries.into_iter().zip(results) {
        let filtered = select_section_tracks(tracks, &mut seen_ids);
        if !filtered.is_empty() {
            let _ = db.upsert_tracks(&filtered);
            sections.push(ExploreSection {
                title: seed.title,
                tracks: filtered,
            });
        }
    }

    Ok(ExploreResponse { sections })
}

const RECENTLY_PLAYED_LIMIT: usize = 20;
const EXPLORE_TRACK_LIMIT: usize = 24;
const MAX_EXPLORE_QUERIES: usize = 8;
const MAX_TRACKS_PER_ARTIST_IN_SECTION: usize = 3;

#[derive(serde::Serialize)]
pub struct ExploreResponse {
    sections: Vec<ExploreSection>,
}

#[derive(serde::Serialize)]
struct ExploreSection {
    title: String,
    tracks: Vec<Track>,
}

#[derive(Clone)]
struct ExploreQuery {
    title: String,
    query: String,
    limit: usize,
    score: i64,
}

fn starter_queries() -> Vec<ExploreQuery> {
    let starters = [
        ("Popular Right Now", "popular music hits official audio", 92),
        ("Fresh Finds", "new music discoveries official audio", 88),
        ("Chill Vibes", "chill relaxing songs official audio", 84),
        ("Upbeat Energy", "upbeat energetic songs official audio", 82),
        ("Indie Corner", "indie music discoveries official audio", 80),
        ("Electronic Beats", "electronic dance songs official audio", 78),
        ("Acoustic Sessions", "acoustic singer songwriter songs", 76),
        ("R&B Soul", "rnb soul songs official audio", 74),
    ];
    let offset = chrono_bucket() % starters.len();
    let mut queries = Vec::new();
    for i in 0..5.min(starters.len()) {
        let (title, query, score) = starters[(offset + i) % starters.len()];
        queries.push(ExploreQuery {
            title: title.to_string(),
            query: query.to_string(),
            limit: 32,
            score: score - i as i64,
        });
    }
    queries
}

fn personalized_queries(
    artist_signals: &[(String, i64, i64)],
    keywords: &[(String, i64)],
    recent: &[Track],
    listen_count: i64,
) -> Vec<ExploreQuery> {
    let mut queries = Vec::new();

    for (rank, (artist, total, recent_count)) in artist_signals.iter().take(4).enumerate() {
        let strategies = [
            format!("{artist} similar artists songs"),
            format!("{artist} radio similar songs"),
            format!("music like {artist} official audio"),
        ];
        let pick = simple_hash(artist) % strategies.len();
        queries.push(ExploreQuery {
            title: format!("{artist} Radio"),
            query: strategies[pick].clone(),
            limit: 32,
            score: 130 - rank as i64 * 8 + (*total).min(20) * 2 + (*recent_count).min(10) * 3,
        });
    }

    for (rank, track) in recent.iter().take(2).enumerate() {
        let title = clean_track_title(&track.title);
        if title.is_empty() || track.artist.trim().is_empty() {
            continue;
        }
        queries.push(ExploreQuery {
            title: format!("More Like {}", compact_title(&title, 30)),
            query: format!("songs like {} {}", track.artist, title),
            limit: 28,
            score: 112 - rank as i64 * 7,
        });
    }

    if artist_signals.len() >= 2 {
        let first = &artist_signals[0].0;
        let second = &artist_signals[artist_signals.len() / 2].0;
        queries.push(ExploreQuery {
            title: "Discovery Mix".to_string(),
            query: format!("{first} {second} similar songs"),
            limit: 32,
            score: 104,
        });
    }

    let mood_keywords = mood_keywords(keywords, artist_signals);
    for (rank, chunk) in mood_keywords.chunks(2).take(2).enumerate() {
        if chunk.len() < 2 {
            continue;
        }
        queries.push(ExploreQuery {
            title: format!(
                "More {}",
                chunk.iter().map(|word| capitalize(word)).collect::<Vec<_>>().join(" & ")
            ),
            query: format!("{} songs official audio", chunk.join(" ")),
            limit: 28,
            score: 96 - rank as i64 * 6,
        });
    }

    if artist_signals.len() >= 5 {
        let deep = &artist_signals[artist_signals.len() - 1].0;
        queries.push(ExploreQuery {
            title: format!("Deep Cuts: {deep}"),
            query: format!("{deep} deep cuts underrated songs"),
            limit: 28,
            score: 86,
        });
    }

    if let Some((artist, _, _)) = artist_signals.first() {
        queries.push(ExploreQuery {
            title: "Fresh For You".to_string(),
            query: if listen_count >= 20 {
                format!("new songs similar to {artist}")
            } else {
                format!("popular songs similar to {artist}")
            },
            limit: 28,
            score: 82,
        });
    }

    if queries.is_empty() {
        starter_queries()
    } else {
        queries
    }
}

fn mood_keywords(
    keywords: &[(String, i64)],
    artist_signals: &[(String, i64, i64)],
) -> Vec<String> {
    let artist_blob = artist_signals
        .iter()
        .map(|(artist, _, _)| artist.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    keywords
        .iter()
        .filter(|(word, count)| *count >= 2 && !artist_blob.contains(word.as_str()))
        .take(6)
        .map(|(word, _)| word.clone())
        .collect()
}

fn dedupe_queries(queries: &mut Vec<ExploreQuery>) {
    let mut seen = HashSet::new();
    queries.retain(|seed| {
        let key = format!("{}|{}", seed.title.to_lowercase(), seed.query.to_lowercase());
        seen.insert(key)
    });
}

fn append_unique(tracks: &mut Vec<Track>, seen: &mut HashSet<String>, incoming: Vec<Track>) {
    for track in incoming {
        if seen.insert(track.id.clone()) {
            tracks.push(track);
        }
    }
}

fn select_section_tracks(tracks: Vec<Track>, seen_ids: &mut HashSet<String>) -> Vec<Track> {
    let mut candidates: Vec<(i64, usize, Track)> = tracks
        .into_iter()
        .enumerate()
        .filter_map(|(index, track)| {
            if seen_ids.contains(&track.id) || !looks_like_song(&track) {
                return None;
            }
            Some((track_quality(&track, index), index, track))
        })
        .collect();

    candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

    let mut selected = Vec::new();
    let mut artist_counts: HashMap<String, usize> = HashMap::new();
    let mut title_keys: HashSet<String> = HashSet::new();
    for (_, _, track) in candidates {
        let artist_key = normalized_key(&track.artist);
        let title_key = normalized_key(&clean_track_title(&track.title));
        let artist_count = artist_counts.get(&artist_key).copied().unwrap_or(0);
        if !artist_key.is_empty() && artist_count >= MAX_TRACKS_PER_ARTIST_IN_SECTION {
            continue;
        }

        if !title_key.is_empty() && title_keys.contains(&title_key) {
            continue;
        }

        if !seen_ids.insert(track.id.clone()) {
            continue;
        }

        if !title_key.is_empty() {
            title_keys.insert(title_key);
        }

        if !artist_key.is_empty() {
            artist_counts.insert(artist_key, artist_count + 1);
        }

        selected.push(track);
        if selected.len() >= EXPLORE_TRACK_LIMIT {
            break;
        }
    }
    selected
}

fn normalized_key(value: &str) -> String {
    value.trim().to_lowercase()
}

fn track_quality(track: &Track, index: usize) -> i64 {
    let mut score = 100 - index as i64;
    let duration = track.duration_secs;
    if (120.0..=420.0).contains(&duration) {
        score += 24;
    } else if (60.0..=600.0).contains(&duration) {
        score += 10;
    } else if duration > 900.0 {
        score -= 80;
    }

    if !track.thumbnail.is_empty() {
        score += 8;
    }

    let title = track.title.to_lowercase();
    if title.contains("official audio") || title.contains("official video") {
        score += 6;
    }
    if title.contains("full album")
        || title.contains("complete album")
        || title.contains("compilation")
        || title.contains("reaction")
        || title.contains("tutorial")
    {
        score -= 90;
    }

    score
}

fn looks_like_song(track: &Track) -> bool {
    let duration = track.duration_secs;
    if duration > 0.0 && !(30.0..=1200.0).contains(&duration) {
        return false;
    }

    let title = track.title.to_lowercase();
    let long_form = [
        "full album",
        "complete album",
        "1 hour",
        "hour mix",
        "playlist",
        "compilation",
        "reaction",
        "tutorial",
        "documentary",
        "interview",
    ];
    if duration > 480.0 && long_form.iter().any(|needle| title.contains(needle)) {
        return false;
    }

    true
}

fn clean_track_title(title: &str) -> String {
    let mut cleaned = title.to_string();
    for marker in ["(", "[", " - Official", " | Official"] {
        if let Some(idx) = cleaned.find(marker) {
            cleaned.truncate(idx);
        }
    }
    cleaned.trim().to_string()
}

fn compact_title(title: &str, limit: usize) -> String {
    if title.chars().count() <= limit {
        return title.to_string();
    }
    let mut compact = title.chars().take(limit.saturating_sub(1)).collect::<String>();
    compact.push_str("...");
    compact
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

fn chrono_bucket() -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    (secs / (60 * 60 * 6)) as usize
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ExportedPlaylist {
    name: String,
    tracks: Vec<Track>,
}

#[tauri::command]
pub async fn export_playlist_json(
    playlist_id: i64,
    path: String,
    db: State<'_, SearchCache>,
) -> Result<(), String> {
    let playlists = db.list_playlists().map_err(|e| e.to_string())?;
    let playlist = playlists
        .into_iter()
        .find(|p| p.id == playlist_id)
        .ok_or("Playlist not found")?;
    let tracks = db.get_playlist_tracks(playlist_id).map_err(|e| e.to_string())?;
    let exported = ExportedPlaylist {
        name: playlist.name,
        tracks,
    };
    let json = serde_json::to_string_pretty(&exported).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_playlist_json(
    path: String,
    db: State<'_, SearchCache>,
) -> Result<Playlist, String> {
    let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let imported: ExportedPlaylist = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    if imported.tracks.is_empty() {
        return Err("No tracks in imported playlist".into());
    }
    let playlist = db.create_playlist(&imported.name, "").map_err(|e| e.to_string())?;
    let _ = db.upsert_tracks(&imported.tracks);
    for track in &imported.tracks {
        let _ = db.add_to_playlist(playlist.id, &track.id);
    }
    Ok(Playlist {
        id: playlist.id,
        name: imported.name,
        thumbnail: String::new(),
        track_count: imported.tracks.len() as i64,
    })
}

#[tauri::command]
pub async fn set_discord_rpc(
    enabled: bool,
    title: Option<String>,
    artist: Option<String>,
    thumbnail: Option<String>,
    discord: State<'_, DiscordPresence>,
    config_mgr: State<'_, ConfigManager>,
) -> Result<(), String> {
    discord.set_enabled(enabled);
    let mut cfg = config_mgr.get();
    cfg.discord_rpc_enabled = enabled;
    config_mgr.update(cfg);
    if enabled {
        if let (Some(t), Some(a)) = (title, artist) {
            discord.send(PresenceCommand::SetActivity {
                title: t,
                artist: a,
                thumbnail: thumbnail.unwrap_or_default(),
            });
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_tracks_by_ids(
    track_ids: Vec<String>,
    db: State<'_, SearchCache>,
) -> Result<Vec<Track>, String> {
    db.get_tracks_by_ids(&track_ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Only HTTPS URLs are allowed".into());
    }
    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "linux") {
        ("xdg-open", vec![&url])
    } else if cfg!(target_os = "macos") {
        ("open", vec![&url])
    } else if cfg!(target_os = "windows") {
        ("explorer", vec![&url])
    } else {
        return Err("Unsupported platform".into());
    };
    tokio::process::Command::new(cmd)
        .args(args)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
