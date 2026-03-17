use std::process::Stdio;
use tokio::process::Command;

use crate::error::AppError;
use crate::models::Track;

pub struct Extractor {
    bin: String,
}

impl Extractor {
    pub fn new() -> Self {
        Self {
            bin: std::env::var("SUNDER_YTDLP_PATH").unwrap_or_else(|_| "yt-dlp".into()),
        }
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<Track>, AppError> {
        let output = Command::new(&self.bin)
            .args([
                &format!("ytsearch{limit}:{query}"),
                "--dump-json",
                "--flat-playlist",
                "--no-warnings",
                "--ignore-errors",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| AppError::Extraction(format!("failed to run yt-dlp: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("yt-dlp failed with status {}. stderr: {}", output.status, stderr);
            return Err(AppError::Extraction(format!("yt-dlp search failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        eprintln!("yt-dlp stdout (first 200 chars): {}", &stdout.chars().take(200).collect::<String>());
        let tracks: Vec<Track> = stdout
            .lines()
            .filter_map(|line| {
                let v: serde_json::Value = serde_json::from_str(line).ok()?;
                Some(Track {
                    id: v["id"].as_str()?.to_string(),
                    title: v["title"].as_str().unwrap_or("Unknown").to_string(),
                    artist: v["channel"].as_str()
                        .or_else(|| v["uploader"].as_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                    thumbnail: best_thumbnail(&v),
                    duration_secs: v["duration"].as_f64().unwrap_or(0.0),
                    stream_url: None,
                })
            })
            .collect();

        Ok(tracks)
    }

    pub async fn metadata(&self, video_id: &str) -> Result<Track, AppError> {
        let output = Command::new(&self.bin)
            .args([
                &format!("https://www.youtube.com/watch?v={video_id}"),
                "-j",
                "--no-playlist",
                "--no-warnings",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await
            .map_err(|e| AppError::Extraction(format!("yt-dlp metadata failed: {e}")))?;

        let v: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| AppError::Extraction(e.to_string()))?;

        Ok(Track {
            id: v["id"].as_str().unwrap_or(video_id).to_string(),
            title: v["title"].as_str().unwrap_or("Unknown").to_string(),
            artist: v["channel"].as_str()
                .or_else(|| v["uploader"].as_str())
                .unwrap_or("Unknown")
                .to_string(),
            thumbnail: best_thumbnail(&v),
            duration_secs: v["duration"].as_f64().unwrap_or(0.0),
            stream_url: None,
        })
    }

    pub async fn extract_playlist(
        &self,
        url: &str,
    ) -> Result<(String, Option<String>, Vec<Track>), AppError> {
        let output = Command::new(&self.bin)
            .args([
                url,
                "--dump-json",
                "--flat-playlist",
                "--no-warnings",
                "--ignore-errors",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| AppError::Extraction(format!("failed to run yt-dlp: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut tracks = Vec::new();
        let mut playlist_title = "Imported Playlist".to_string();
        let mut playlist_thumbnail = None;

        for line in stdout.lines() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if playlist_title == "Imported Playlist" {
                    if let Some(t) = v["playlist_title"].as_str() {
                        playlist_title = t.to_string();
                    } else if let Some(t) = v["playlist"].as_str() {
                        playlist_title = t.to_string();
                    }
                }
                let thumb = best_thumbnail(&v);
                if playlist_thumbnail.is_none() && !thumb.is_empty() {
                    playlist_thumbnail = Some(thumb.clone());
                }
                if let Some(track) = v["id"].as_str().map(|id| Track {
                    id: id.to_string(),
                    title: v["title"].as_str().unwrap_or("Unknown").to_string(),
                    artist: v["channel"].as_str()
                        .or_else(|| v["uploader"].as_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                    thumbnail: thumb,
                    duration_secs: v["duration"].as_f64().unwrap_or(0.0),
                    stream_url: None,
                }) {
                    tracks.push(track);
                }
            }
        }

        Ok((playlist_title, playlist_thumbnail, tracks))
    }
}

fn best_thumbnail(v: &serde_json::Value) -> String {
    if let Some(thumbs) = v["thumbnails"].as_array() {
        if let Some(last) = thumbs.last() {
            if let Some(url) = last["url"].as_str() {
                return url.to_string();
            }
        }
    }
    v["thumbnail"].as_str().unwrap_or_default().to_string()
}
