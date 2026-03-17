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

    pub async fn get_subtitles(&self, video_id: &str, lang: &str) -> Result<String, AppError> {
        let tmp = std::env::temp_dir();
        let output = Command::new(&self.bin)
            .args([
                &format!("https://www.youtube.com/watch?v={video_id}"),
                "--skip-download",
                "--write-subs",
                "--sub-langs",
                lang,
                "--sub-format",
                "vtt",
                "-o",
                &tmp.join(video_id).to_string_lossy(),
                "--no-warnings",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| AppError::Extraction(format!("failed to run yt-dlp: {e}")))?;

        if !output.status.success() {
            return Err(AppError::Extraction("failed to fetch subtitles".into()));
        }

        let vtt_path = tmp.join(format!("{video_id}.{lang}.vtt"));
        if !vtt_path.exists() {
            return Err(AppError::Extraction(format!("no {lang} subtitles found")));
        }

        let content = std::fs::read_to_string(&vtt_path)
            .map_err(|e| AppError::Extraction(format!("failed to read subtitles: {e}")))?;
        let _ = std::fs::remove_file(&vtt_path);

        // Parse VTT: extract text lines, skip timestamps and metadata
        let re_tags = regex_lite::Regex::new(r"<[^>]+>").unwrap();
        let lyrics = content
            .lines()
            .filter(|l| {
                let l = l.trim();
                !l.is_empty()
                    && !l.starts_with("WEBVTT")
                    && !l.starts_with("Kind:")
                    && !l.starts_with("Language:")
                    && !l.starts_with("NOTE")
                    && !l.contains(" --> ")
                    && l.parse::<u32>().is_err()
            })
            .map(|l| re_tags.replace_all(l, "").to_string())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(lyrics)
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
