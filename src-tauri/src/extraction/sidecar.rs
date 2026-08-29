use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

use crate::error::AppError;
use crate::models::Track;
use crate::process::NoWindow;

pub struct Extractor {
    /// App-managed yt-dlp location. Resolution happens per spawn so a
    /// runtime update (see `ytdlp::update`) takes effect without restart.
    managed: PathBuf,
}

impl Extractor {
    pub fn new(data_dir: &std::path::Path) -> Self {
        Self {
            managed: super::ytdlp::managed_bin_path(data_dir),
        }
    }

    fn bin(&self) -> String {
        super::ytdlp::resolve_bin(Some(&self.managed))
    }

    /// Search YouTube Music specifically for tracks.
    ///
    /// NOTE: yt-dlp has no `ytmusicsearch` special URL; that prefix was
    /// silently unsupported and returned nothing. The supported form is the
    /// music.youtube.com search page, which yt-dlp extracts as a playlist.
    /// Used by the Explore page seeds only: the user-facing search command
    /// deliberately does NOT call this, because flat entries from this page
    /// carry no artist metadata (doctrine D5, issue #42).
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<Track>, AppError> {
        let output = Command::new(self.bin()).no_window()
            .args([
                &format!(
                    "https://music.youtube.com/search?q={}",
                    encode_query(query)
                ),
                "--dump-json",
                "--flat-playlist",
                "--playlist-end",
                &limit.to_string(),
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
        Ok(parse_flat_tracks(&stdout))
    }

    /// Search generic YouTube (useful for remixes, covers, and obscure tracks).
    pub async fn search_youtube(&self, query: &str, limit: usize) -> Result<Vec<Track>, AppError> {
        let output = Command::new(self.bin()).no_window()
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
        Ok(parse_flat_tracks(&stdout))
    }

    /// Fetch metadata for a single video/track.
    pub async fn metadata(&self, video_id: &str) -> Result<Track, AppError> {
        let output = Command::new(self.bin()).no_window()
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
        let output = Command::new(self.bin()).no_window()
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
        use std::sync::LazyLock;
        static RE_TAGS: LazyLock<regex_lite::Regex> =
            LazyLock::new(|| regex_lite::Regex::new(r"<[^>]+>").unwrap());
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
            .map(|l| RE_TAGS.replace_all(l, "").to_string())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(lyrics)
    }

    pub async fn extract_playlist(
        &self,
        url: &str,
    ) -> Result<(String, Option<String>, Vec<Track>), AppError> {
        let output = Command::new(self.bin()).no_window()
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

/// Parse `--dump-json --flat-playlist` output into playable tracks.
///
/// Flat-playlist sources (the music.youtube.com search page in particular,
/// which feeds Explore) interleave non-video cards (albums `MPREb_…`,
/// channels `UC…`, playlists `VL…`) with real song results, and put the
/// cards first. Their flat JSON has no title and their ids are not video
/// ids, so Sunder used to render them as unplayable "Unknown" entries at
/// the top of search results (issue #42). They are dropped here.
fn parse_flat_tracks(stdout: &str) -> Vec<Track> {
    stdout
        .lines()
        .filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            if !is_playable_video(&v) {
                return None;
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
        .collect()
}

/// A flat-playlist entry is a playable video only when it points at a watch
/// page and carries a title. Browse cards point at `/browse/` pages, so
/// playback (`watch?v=<id>`) could never work for them.
fn is_playable_video(v: &serde_json::Value) -> bool {
    let url_ok = v["url"].as_str().is_some_and(|u| u.contains("/watch"));
    let title_ok = v["title"].as_str().is_some_and(|t| !t.trim().is_empty());
    url_ok && title_ok
}

fn best_thumbnail(v: &serde_json::Value) -> String {
    if let Some(thumbs) = v["thumbnails"].as_array() {
        // Pick a medium-res thumbnail (~320x180) instead of the largest one.
        // Using a huge thumbnail wastes VRAM when displayed at 48-200px in the UI.
        let target = thumbs.iter().find(|t| {
            t["width"].as_u64().is_some_and(|w| (280..=400).contains(&w))
        });
        let chosen = target
            .or_else(|| thumbs.get(thumbs.len().min(2).saturating_sub(0).min(thumbs.len() - 1)))
            .or_else(|| thumbs.first());
        if let Some(t) = chosen {
            if let Some(url) = t["url"].as_str() {
                return url.to_string();
            }
        }
    }
    // Fallback
    let base = v["thumbnail"].as_str().unwrap_or_default();
    if !base.is_empty() && base.contains("i.ytimg.com") && !base.contains("?sqp=") {
        return base.replace("maxresdefault", "mqdefault")
                   .replace("hqdefault", "mqdefault");
    }
    base.to_string()
}

/// Percent-encode a query value per RFC 3986 (unreserved set only).
/// "Daft Punk & Justice" → "Daft%20Punk%20%26%20Justice". YouTube accepts
/// %20 for spaces in the q parameter.
fn encode_query(q: &str) -> String {
    let mut out = String::with_capacity(q.len());
    for b in q.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{encode_query, parse_flat_tracks};

    #[test]
    fn encodes_reserved_chars_and_leaves_unreserved() {
        assert_eq!(encode_query("daft punk"), "daft%20punk");
        assert_eq!(encode_query("a&b=c?d/e"), "a%26b%3Dc%3Fd%2Fe");
        assert_eq!(encode_query("AC/DC-Top_Tracks.2024~"), "AC%2FDC-Top_Tracks.2024~");
        // Non-ASCII must not be passed through raw.
        assert_eq!(encode_query("té"), "t%C3%A9");
    }

    // Fixtures captured 2026-08-29 from `yt-dlp --dump-json --flat-playlist`
    // on `https://music.youtube.com/search?q=daft%20punk` (2026.07.04) and
    // `ytsearch10:` (same version). Shapes, not full lines.

    /// Issue #42: the search page opens with album/artist/playlist browse
    /// cards that have no title and cannot be played. All must be dropped.
    #[test]
    fn drops_non_video_browse_cards() {
        let stdout = [
            r#"{"id":"MPREb_K8qWMWVqXGi","url":"https://music.youtube.com/browse/MPREb_K8qWMWVqXGi","_type":"url","ie_key":"Youtube"}"#,
            r#"{"id":"UCZ2J4f4052BX-NcUh55AFgg","url":"https://music.youtube.com/browse/UCZ2J4f4052BX-NcUh55AFgg","_type":"url","ie_key":"Youtube"}"#,
            r#"{"id":"VLRDCLAK5uy_n20FRYQXNt1p1wS","url":"https://music.youtube.com/browse/VLRDCLAK5uy_n20FRYQXNt1p1wS","_type":"url","ie_key":"Youtube"}"#,
            // A browse card that DOES have a title is still not playable.
            r#"{"id":"MPREb_CtOaObihCXA","title":"Discovery","url":"https://music.youtube.com/browse/MPREb_CtOaObihCXA","_type":"url","ie_key":"Youtube"}"#,
        ]
        .join("\n");
        assert!(parse_flat_tracks(&stdout).is_empty());
    }

    #[test]
    fn keeps_watch_entries_with_metadata() {
        let stdout = [
            r#"{"id":"5NV6Rdv1a3I","title":"Daft Punk - One More Time (Official Video)","channel":"Daft Punk","uploader":"Daft Punk","duration":322,"url":"https://www.youtube.com/watch?v=5NV6Rdv1a3I","_type":"url","ie_key":"Youtube"}"#,
        ]
        .join("\n");
        let tracks = parse_flat_tracks(&stdout);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "5NV6Rdv1a3I");
        assert_eq!(tracks[0].title, "Daft Punk - One More Time (Official Video)");
        assert_eq!(tracks[0].artist, "Daft Punk");
        assert!((tracks[0].duration_secs - 322.0).abs() < f64::EPSILON);
        assert!(tracks[0].stream_url.is_none());
    }

    /// Music-page songs carry no channel/uploader/duration in flat mode:
    /// they must still be kept (playable), with artist "Unknown".
    #[test]
    fn keeps_music_page_songs_without_artist() {
        let stdout = [
            r#"{"id":"qPRNIHxLhmc","title":"I Feel It Coming (feat. Daft Punk)","url":"https://music.youtube.com/watch?v=qPRNIHxLhmc","_type":"url","ie_key":"Youtube"}"#,
        ]
        .join("\n");
        let tracks = parse_flat_tracks(&stdout);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "qPRNIHxLhmc");
        assert_eq!(tracks[0].title, "I Feel It Coming (feat. Daft Punk)");
        assert_eq!(tracks[0].artist, "Unknown");
        assert_eq!(tracks[0].duration_secs, 0.0);
    }

    /// Realistic page order: junk cards first, songs after (exactly the
    /// ordering that pushed "Unknown" to the top of issue #42's results).
    #[test]
    fn filters_real_search_page_ordering() {
        let stdout = [
            r#"{"id":"MPREb_K8qWMWVqXGi","url":"https://music.youtube.com/browse/MPREb_K8qWMWVqXGi","_type":"url","ie_key":"Youtube"}"#,
            r#"{"id":"UCZ2J4f4052BX-NcUh55AFgg","url":"https://music.youtube.com/browse/UCZ2J4f4052BX-NcUh55AFgg","_type":"url","ie_key":"Youtube"}"#,
            r#"{"id":"qPRNIHxLhmc","title":"I Feel It Coming (feat. Daft Punk)","url":"https://music.youtube.com/watch?v=qPRNIHxLhmc","_type":"url","ie_key":"Youtube"}"#,
            r#"{"id":"Jb6gcoR266U","title":"Around the World","url":"https://music.youtube.com/watch?v=Jb6gcoR266U","_type":"url","ie_key":"Youtube"}"#,
            // Malformed lines and blanks must be skipped, not fatal.
            "not json",
            "",
        ]
        .join("\n");
        let tracks = parse_flat_tracks(&stdout);
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].id, "qPRNIHxLhmc");
        assert_eq!(tracks[1].id, "Jb6gcoR266U");
    }

    /// A watch entry with a blank title is not a usable result either.
    #[test]
    fn drops_watch_entries_with_empty_title() {
        let stdout = [
            r#"{"id":"abc123","title":"","url":"https://www.youtube.com/watch?v=abc123","_type":"url","ie_key":"Youtube"}"#,
        ]
        .join("\n");
        assert!(parse_flat_tracks(&stdout).is_empty());
    }
}
