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
/// - `"ytdlp_botwall"`: YouTube's interactive bot wall ("Sign in to confirm
///   you're not a bot"). Unlike the generic blocked kind, this one is about
///   *who* is asking, not *what* tool is asking: the remedy is proving
///   personhood - setting a session cookie source (`set_cookie_source`) so
///   yt-dlp presents the user's own logged-in jar. Matches both the curly
///   apostrophe (U+2019) and the plain `'` variant yt-dlp emits depending on
///   locale/encoding. Checked BEFORE `ytdlp_blocked` so the specific kind
///   wins over the generic 403 wording that often accompanies it.
/// - `"ytdlp_blocked"`: YouTube's enforcement rejected the fetch itself
///   (media-CDN 403). This is a *tool* problem: every track fails the same
///   way, and the remedy is updating yt-dlp (newer releases rotate player
///   clients that YouTube still allows).
/// - `"load_failed"`: anything else; track-level remedy (find alternative).
pub fn classify_playback_error(msg: &str) -> &'static str {
    // Apostrophe-agnostic and case-agnostic: match the sentence shape, not
    // the punctuation. yt-dlp emits U+2019 on UTF-8 locales and ' otherwise,
    // and neither matters when both halves of the sentence are present.
    let lower = msg.to_lowercase();
    if lower.contains("sign in to confirm") && lower.contains("not a bot") {
        "ytdlp_botwall"
    } else if msg.contains("HTTP Error 403")
        || msg.contains("unable to download video data")
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
        // A bare 403 (any fetch stage) maps to blocked too: the remedy is
        // identical and harmless to try.
        assert_eq!(
            classify_playback_error("HTTP Error 403: Forbidden"),
            "ytdlp_blocked"
        );
    }

    #[test]
    fn classifies_bot_wall_both_apostrophe_variants() {
        // Curly apostrophe (U+2019) - what yt-dlp prints on UTF-8 locales.
        assert_eq!(
            classify_playback_error(
                "ERROR: Sign in to confirm you\u{2019}re not a bot. This helps protect our community."
            ),
            "ytdlp_botwall"
        );
        // Plain apostrophe - ASCII-escaped stderr.
        assert_eq!(
            classify_playback_error("ERROR: Sign in to confirm you're not a bot"),
            "ytdlp_botwall"
        );
        // Lowercase/transcript-style occurrence still matches the signature.
        assert_eq!(
            classify_playback_error("sign in to confirm you're not a bot"),
            "ytdlp_botwall"
        );
    }

    #[test]
    fn generic_403s_are_not_bot_wall() {
        assert_eq!(
            classify_playback_error("HTTP Error 403: Forbidden"),
            "ytdlp_blocked"
        );
        assert_eq!(
            classify_playback_error(
                "unable to download video data: HTTP Error 403"
            ),
            "ytdlp_blocked"
        );
    }

    #[test]
    fn unrelated_failures_are_load_failed() {
        assert_eq!(classify_playback_error("no such video"), "load_failed");
        assert_eq!(classify_playback_error("decoder init failed"), "load_failed");
        assert_eq!(classify_playback_error("yt-dlp wait: interrupted"), "load_failed");
        // Mentions of sign-in that are NOT the bot wall stay generic.
        assert_eq!(
            classify_playback_error("login required: please sign in to continue"),
            "load_failed"
        );
    }
}
