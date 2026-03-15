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

    /// Search YouTube Music specifically for tracks.
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<Track>, AppError> {
        let output = Command::new(&self.bin)
            .args([
                &format!("ytmusicsearch{limit}:{query}"),
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

    /// Search generic YouTube (useful for remixes, covers, and obscure tracks).
    pub async fn search_youtube(&self, query: &str, limit: usize) -> Result<Vec<Track>, AppError> {
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

    /// Fetch metadata for a single video/track.
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

    pub async fn extract_playlist(&self, url: &str) -> Result<(String, Vec<Track>), AppError> {
        let output = Command::new(&self.bin)
            .args([
                url,
                "--flat-playlist",
                "--dump-json",
                "--no-warnings",
                "--ignore-errors",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await
            .map_err(|e| AppError::Extraction(format!("yt-dlp playlist failed: {e}")))?;

        if !output.status.success() {
            return Err(AppError::Extraction("yt-dlp playlist extraction failed".into()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut playlist_title = String::new();
        let tracks: Vec<Track> = stdout
            .lines()
            .filter_map(|line| {
                let v: serde_json::Value = serde_json::from_str(line).ok()?;
                // Grab playlist title from first entry
                if playlist_title.is_empty() {
                    if let Some(t) = v["playlist_title"].as_str() {
                        playlist_title = t.to_string();
                    } else if let Some(t) = v["playlist"].as_str() {
                        playlist_title = t.to_string();
                    }
                }
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

        if playlist_title.is_empty() {
            playlist_title = "Imported Playlist".to_string();
        }

        Ok((playlist_title, tracks))
    }

    pub async fn get_subtitles(&self, video_id: &str, lang: &str) -> Result<String, AppError> {
        let url = format!("https://www.youtube.com/watch?v={video_id}");
        let tmp = std::env::temp_dir().join("sunder_subs");
        let _ = std::fs::create_dir_all(&tmp);
        let out_tpl = tmp.join(video_id);

        let status = Command::new(&self.bin)
            .args([
                &url,
                "--write-subs",
                "--write-auto-subs",
                "--sub-langs", lang,
                "--sub-format", "vtt",
                "--skip-download",
                "-o", out_tpl.to_str().unwrap_or_default(),
                "--no-warnings",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|e| AppError::Extraction(format!("yt-dlp subtitles failed: {e}")))?;

        if !status.success() {
            return Err(AppError::Extraction(format!("subtitle extraction ({lang}) failed", lang = lang)));
        }

        // Look for the subtitle file
        let vtt_path = tmp.join(format!("{video_id}.{lang}.vtt", lang = lang));
        if !vtt_path.exists() {
            return Err(AppError::Extraction(format!("no {lang} subtitles found", lang = lang)));
        }

        let content = std::fs::read_to_string(&vtt_path)
            .map_err(|e| AppError::Extraction(format!("failed to read subtitles: {e}")))?;
        let _ = std::fs::remove_file(&vtt_path);

        // Parse VTT: extract text lines, skip timestamps and metadata
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
            .map(|l| {
                // Strip VTT tags like <c>, </c>, <00:01:02.345>
                let re_tags = regex_lite::Regex::new(r"<[^>]+>").unwrap();
                re_tags.replace_all(l.trim(), "").to_string()
            })
            .collect::<Vec<_>>();

        // Deduplicate consecutive identical lines (VTT often repeats)
        let mut deduped: Vec<String> = Vec::new();
        for line in lyrics {
            if deduped.last().map_or(true, |prev| prev != &line) {
                deduped.push(line);
            }
        }

        if deduped.is_empty() {
            return Err(AppError::Extraction("subtitles contained no text".into()));
        }

        Ok(deduped.join("\n"))
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
