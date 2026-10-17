//! Session-scoped cookie support for yt-dlp spawns.
//!
//! PRIVACY (maintainer directive): the cookie source is NEVER persisted.
//! No config.json field, no disk state - a process-lifetime in-memory holder
//! only, so every app restart resets it to [`CookieSource::None`]. The value
//! is a *source description* ("use my Firefox jar" / "use this Netscape
//! cookies.txt"), never the cookie contents themselves; yt-dlp reads the
//! actual jar straight from the browser/file at spawn time.
//!
//! Threading rule: every yt-dlp spawn site (sidecar search/metadata/subtitles/
//! playlist-extract, the playback download path, the downloads path, and
//! prefetch) appends [`current_args`] to its command line. When the source is
//! [`CookieSource::None`] the list is empty and the spawn is unchanged.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Where yt-dlp should take cookies from for this session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CookieSource {
    /// No cookies (default; every restart resets to this).
    None,
    /// `--cookies-from-browser <name>` - name validated against ALLOWED_BROWSERS.
    Browser(String),
    /// `--cookies <path>` - a Netscape-format cookies.txt that must exist
    /// at parse time.
    File(PathBuf),
}

/// Browser names yt-dlp accepts that Sunder exposes. Lowercase; the parser
/// matches case-insensitively and normalizes to these.
pub const ALLOWED_BROWSERS: [&str; 4] = ["firefox", "chrome", "chromium", "edge"];

static COOKIE_SOURCE: Mutex<Option<CookieSource>> = Mutex::new(None);

/// Replace the session cookie source. In-memory only.
pub fn set(source: CookieSource) {
    let mut guard = COOKIE_SOURCE.lock().expect("cookie source mutex poisoned");
    *guard = Some(source);
}

/// The current session cookie source. None until `set` is called this process.
pub fn get() -> CookieSource {
    COOKIE_SOURCE
        .lock()
        .expect("cookie source mutex poisoned")
        .clone()
        .unwrap_or(CookieSource::None)
}

/// Extra CLI arguments every yt-dlp spawn site must append.
/// Empty when no source is set - the spawn is byte-identical to before.
pub fn current_args() -> Vec<String> {
    match get() {
        CookieSource::None => Vec::new(),
        CookieSource::Browser(b) => vec!["--cookies-from-browser".into(), b],
        CookieSource::File(p) => vec!["--cookies".into(), p.to_string_lossy().into_owned()],
    }
}

/// Parse the wire format of the `set_cookie_source` command:
/// "none" | "firefox" | "chrome" | "chromium" | "edge" | a path ending in
/// ".txt" (which must exist on disk right now).
pub fn parse_cookie_source(raw: &str) -> Result<CookieSource, String> {
    let s = raw.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") {
        return Ok(CookieSource::None);
    }
    let lower = s.to_ascii_lowercase();
    if ALLOWED_BROWSERS.contains(&lower.as_str()) {
        return Ok(CookieSource::Browser(lower));
    }
    let is_txt = Path::new(s)
        .extension()
        .map(|e| e.eq_ignore_ascii_case("txt"))
        .unwrap_or(false);
    if is_txt {
        let p = PathBuf::from(s);
        if p.is_file() {
            return Ok(CookieSource::File(p));
        }
        return Err(format!("cookies file not found: {s}"));
    }
    Err(
        "invalid cookie source: use none, firefox, chrome, chromium, edge, or an existing .txt cookies file"
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_none_variants() {
        assert_eq!(parse_cookie_source("none"), Ok(CookieSource::None));
        assert_eq!(parse_cookie_source("NONE"), Ok(CookieSource::None));
        assert_eq!(parse_cookie_source("  "), Ok(CookieSource::None));
        assert_eq!(parse_cookie_source(""), Ok(CookieSource::None));
    }

    #[test]
    fn parse_browsers_case_insensitive_normalized() {
        assert_eq!(
            parse_cookie_source("Firefox"),
            Ok(CookieSource::Browser("firefox".into()))
        );
        assert_eq!(
            parse_cookie_source("CHROME"),
            Ok(CookieSource::Browser("chrome".into()))
        );
        assert_eq!(
            parse_cookie_source("chromium"),
            Ok(CookieSource::Browser("chromium".into()))
        );
        assert_eq!(
            parse_cookie_source(" Edge "),
            Ok(CookieSource::Browser("edge".into()))
        );
    }

    #[test]
    fn parse_rejects_unknown_browser() {
        assert!(parse_cookie_source("safari").is_err());
        assert!(parse_cookie_source("brave").is_err());
        // A browser-looking path that does not end in .txt is invalid.
        assert!(parse_cookie_source("/tmp/firefox").is_err());
    }

    #[test]
    fn parse_file_requires_txt_extension_and_existence() {
        let dir = std::env::temp_dir().join("sunder-test-cookies");
        let _ = std::fs::create_dir_all(&dir);
        let jar = dir.join("cookies.txt");
        std::fs::write(&jar, b"# Netscape HTTP Cookie File\n").unwrap();

        assert_eq!(
            parse_cookie_source(jar.to_str().unwrap()),
            Ok(CookieSource::File(jar.clone()))
        );
        // Wrong extension: invalid even when it exists.
        let other = dir.join("cookies.dat");
        std::fs::write(&other, b"x").unwrap();
        assert!(parse_cookie_source(other.to_str().unwrap()).is_err());
        // Right extension but missing on disk: rejected.
        assert!(parse_cookie_source(dir.join("nope.txt").to_str().unwrap()).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn current_args_shapes_per_source() {
        set(CookieSource::None);
        assert!(current_args().is_empty());

        set(CookieSource::Browser("firefox".into()));
        assert_eq!(
            current_args(),
            vec!["--cookies-from-browser", "firefox"]
        );

        let dir = std::env::temp_dir().join("sunder-test-cookies-args");
        let _ = std::fs::create_dir_all(&dir);
        let jar = dir.join("c.txt");
        std::fs::write(&jar, b"x").unwrap();
        set(CookieSource::File(jar.clone()));
        let args = current_args();
        assert_eq!(args[0], "--cookies");
        assert_eq!(args[1], jar.to_string_lossy());

        // Reset so other tests start clean.
        set(CookieSource::None);
        assert!(current_args().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
