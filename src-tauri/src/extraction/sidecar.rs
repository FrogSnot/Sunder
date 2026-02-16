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
            .stderr(Stdio::null())
            .output()
            .await
            .map_err(|e| AppError::Extraction(format!("failed to run yt-dlp: {e}")))?;

        if !output.status.success() {
            return Err(AppError::Extraction("yt-dlp search failed".into()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
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
