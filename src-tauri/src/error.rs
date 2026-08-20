use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("audio: {0}")]
    Audio(String),
    #[error("database: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("extraction: {0}")]
    Extraction(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Classify a playback failure string into a remedy selector the UI acts on.
/// The string is yt-dlp's stderr as captured by the download path.
///
/// Kinds:
/// - `"ytdlp_blocked"`: YouTube's enforcement rejected the fetch itself
///   (media-CDN 403 / bot check). This is a *tool* problem, not a track
///   problem: every track fails the same way, and the remedy is updating
///   yt-dlp (newer releases rotate player clients that YouTube still allows).
/// - `"load_failed"`: anything else; track-level remedy (find alternative).
pub fn classify_playback_error(msg: &str) -> &'static str {
    if msg.contains("HTTP Error 403")
        || msg.contains("unable to download video data")
        || msg.contains("Sign in to confirm")
    {
        "ytdlp_blocked"
    } else {
        "load_failed"
    }
}

#[cfg(test)]
mod tests {
    use super::classify_playback_error;

    #[test]
    fn classifies_403_media_fetch_as_blocked() {
        // The exact failure observed with yt-dlp 2026.07.04 on 2026-08-20:
        // resolution succeeds, media download 403s.
        assert_eq!(
            classify_playback_error(
                "yt-dlp failed (exit code: 1): ERROR: unable to download video data: HTTP Error 403: Forbidden"
            ),
            "ytdlp_blocked"
        );
        assert_eq!(
            classify_playback_error("ERROR: Sign in to confirm you're not a bot"),
            "ytdlp_blocked"
        );
        // A bare 403 (any fetch stage) maps to blocked too: the remedy is
        // identical and harmless to try.
        assert_eq!(
            classify_playback_error("HTTP Error 403: Forbidden"),
            "ytdlp_blocked"
        );
    }

    #[test]
    fn unrelated_failures_are_load_failed() {
        assert_eq!(classify_playback_error("no such video"), "load_failed");
        assert_eq!(classify_playback_error("decoder init failed"), "load_failed");
        assert_eq!(classify_playback_error("yt-dlp wait: interrupted"), "load_failed");
    }
}
