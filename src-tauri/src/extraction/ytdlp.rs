//! yt-dlp binary resolution and the opt-in self-update.
//!
//! Resolution order (single source of truth for every spawn site):
//!   1. `SUNDER_YTDLP_PATH` env: explicit user override, always wins
//!   2. `<app_data>/bin/yt-dlp`: the app-managed copy (see [`update`])
//!   3. `"yt-dlp"`: system PATH lookup
//!
//! The managed copy exists only because the user clicked "Update yt-dlp" in
//! the UI. Sunder never downloads anything without that explicit action.
//! The download always comes from the official yt-dlp GitHub release over
//! HTTPS and is smoke-tested (`--version` must run) before being adopted
//! via atomic rename, so a broken or partial download is never installed.
//!
//! Platform gate: the release asset at [`RELEASE_URL`] is the *nix Python
//! zipapp. The one-click update is therefore Linux-only, and additionally
//! disabled while `SUNDER_YTDLP_PATH` is set, because the override outranks
//! the managed copy and an "update" would silently change nothing.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The one and only download source: official repo, official release asset
/// (the *nix Python zipapp, which is the reason the updater is Linux-gated).
pub const RELEASE_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp";

/// Location of the app-managed binary: `<data_dir>/bin/yt-dlp`.
pub fn managed_bin_path(data_dir: &Path) -> PathBuf {
    data_dir.join("bin").join("yt-dlp")
}

fn is_executable_file(p: &Path) -> bool {
    let Ok(md) = std::fs::metadata(p) else {
        return false;
    };
    if !md.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        md.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Resolve which yt-dlp binary to spawn, in the documented order.
pub fn resolve_bin(managed: Option<&Path>) -> String {
    if let Ok(p) = std::env::var("SUNDER_YTDLP_PATH") {
        if !p.trim().is_empty() {
            return p;
        }
    }
    if let Some(p) = managed {
        if is_executable_file(p) {
            return p.to_string_lossy().into_owned();
        }
    }
    "yt-dlp".into()
}

/// Same resolution for spawn sites that hold an `AppHandle`.
pub fn resolve_bin_for(app: &tauri::AppHandle) -> String {
    use tauri::Manager;
    let managed = app.path().app_data_dir().ok().map(|d| managed_bin_path(&d));
    resolve_bin(managed.as_deref())
}

/// Run `<bin> --version` and return the single-line version string.
/// Returns None when the binary is missing, fails to run, or prints
/// anything that is not a clean single line (i.e. not yt-dlp).
pub async fn version_of(bin: &str) -> Option<String> {
    let out = tokio::process::Command::new(bin)
        .arg("--version")
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if v.is_empty() || v.contains('\n') {
        return None;
    }
    Some(v)
}

#[derive(serde::Serialize)]
pub struct YtdlpStatus {
    /// "override" (env var) | "managed" (app-data copy) | "system" (PATH)
    pub source: String,
    pub path: String,
    pub version: Option<String>,
    /// Whether the one-click update is offered. True only on Linux AND when
    /// no env override is active. Gates the UI button; `update` refuses
    /// otherwise, so this is a hint, not the enforcement.
    pub can_update: bool,
}

/// Can this build meaningfully self-update yt-dlp? The official asset at
/// `RELEASE_URL` is the *nix Python zipapp: wrong artifact on Windows/macOS
/// (scoop/choco/brew manage those builds). Compile-time constant.
fn update_capable() -> bool {
    cfg!(target_os = "linux")
}

/// True when `SUNDER_YTDLP_PATH` is set to a non-empty value. While set, the
/// override wins over the managed copy, so installing one would be a no-op
/// that reports success. Refuse instead (fail loud over silent no-op).
fn override_active() -> bool {
    std::env::var("SUNDER_YTDLP_PATH")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

/// Report which yt-dlp Sunder is currently using, plus its version.
pub async fn status(data_dir: &Path) -> YtdlpStatus {
    if let Ok(p) = std::env::var("SUNDER_YTDLP_PATH") {
        if !p.trim().is_empty() {
            return YtdlpStatus {
                source: "override".into(),
                version: version_of(&p).await,
                path: p,
                can_update: false,
            };
        }
    }
    let managed = managed_bin_path(data_dir);
    if is_executable_file(&managed) {
        let p = managed.to_string_lossy().into_owned();
        return YtdlpStatus {
            source: "managed".into(),
            version: version_of(&p).await,
            path: p,
            can_update: update_capable(),
        };
    }
    YtdlpStatus {
        source: "system".into(),
        version: version_of("yt-dlp").await,
        path: "yt-dlp".into(),
        can_update: update_capable(),
    }
}

static UPDATE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Download the latest official yt-dlp release into `<data_dir>/bin/`,
/// smoke-test it, and adopt it atomically. Returns the new version.
///
/// Convergence: the tmp file is cleaned up on failure and on the next
/// attempt, so a crashed update leaves nothing behind that a retry (or
/// simply deleting the managed file) cannot recover from.
pub async fn update(data_dir: &Path) -> Result<String, String> {
    // Refuse before anything touches the network. `can_update` in status()
    // mirrors these guards for the UI; this is the enforcement.
    if !update_capable() {
        return Err(
            "one-click yt-dlp update is only available on Linux. Update yt-dlp with your package manager"
                .into(),
        );
    }
    if override_active() {
        return Err(
            "SUNDER_YTDLP_PATH is set and overrides everything Sunder installs. Update that yt-dlp manually"
                .into(),
        );
    }
    if UPDATE_IN_FLIGHT.swap(true, Ordering::SeqCst) {
        return Err("yt-dlp update already in progress".into());
    }
    let result = do_update(data_dir).await;
    UPDATE_IN_FLIGHT.store(false, Ordering::SeqCst);
    result
}

async fn do_update(data_dir: &Path) -> Result<String, String> {
    let bin_dir = data_dir.join("bin");
    std::fs::create_dir_all(&bin_dir).map_err(|e| format!("failed to create bin dir: {e}"))?;
    let final_path = bin_dir.join("yt-dlp");
    let tmp_path = bin_dir.join(format!("yt-dlp.download-{}", std::process::id()));
    // Clear any stale partial from a previously crashed attempt.
    let _ = std::fs::remove_file(&tmp_path);

    let fail = |msg: String| -> Result<String, String> {
        let _ = std::fs::remove_file(&tmp_path);
        Err(msg)
    };

    // 1. Fetch the official release over HTTPS (rustls).
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| format!("failed to build http client: {e}"))?;
    let resp = client
        .get(RELEASE_URL)
        .send()
        .await
        .map_err(|e| format!("download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download failed: HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("download failed: {e}"))?;
    if bytes.len() < 1_000_000 {
        return fail(format!(
            "download suspiciously small ({} bytes); refusing to install",
            bytes.len()
        ));
    }
    std::fs::write(&tmp_path, &bytes).map_err(|e| format!("failed to write file: {e}"))?;

    // 2. Executable bit.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("failed to set executable bit: {e}"))?;
    }

    // 3. Smoke test BEFORE adoption: the binary must run and report a version.
    // This rejects HTML error pages, truncated downloads, and non-yt-dlp files.
    let bin_str = tmp_path.to_string_lossy().into_owned();
    let version = match version_of(&bin_str).await {
        Some(v) => v,
        None => {
            return fail(
                "downloaded binary failed verification (it does not run). Possibly out of disk space or a bad download. Try again."
                    .into(),
            );
        }
    };

    // 4. Atomic adopt. On unix, rename replaces in place; Windows needs the
    // target gone first.
    #[cfg(windows)]
    let _ = std::fs::remove_file(&final_path);
    std::fs::rename(&tmp_path, &final_path).map_err(|e| format!("failed to install: {e}"))?;

    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_prefers_env_over_managed_over_system() {
        let dir = std::env::temp_dir().join("sunder-test-resolve");
        let _ = std::fs::create_dir_all(dir.join("bin"));
        let managed = managed_bin_path(&dir);

        // Managed copy absent → system fallback.
        let _ = std::fs::remove_file(&managed);
        std::env::remove_var("SUNDER_YTDLP_PATH");
        assert_eq!(resolve_bin(Some(&managed)), "yt-dlp");

        // Managed copy present (an executable file) → wins over system.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(&managed, b"#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&managed, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        #[cfg(not(unix))]
        std::fs::write(&managed, b"stub").unwrap();
        assert_eq!(resolve_bin(Some(&managed)), managed.to_string_lossy());

        // Explicit env override beats everything.
        std::env::set_var("SUNDER_YTDLP_PATH", "/opt/custom/yt-dlp");
        assert_eq!(resolve_bin(Some(&managed)), "/opt/custom/yt-dlp");

        std::env::remove_var("SUNDER_YTDLP_PATH");
        let _ = std::fs::remove_file(&managed);
    }

    /// Real network test: downloads the actual release and verifies it.
    /// Run manually with: cargo test -p sunder -- --ignored
    #[tokio::test]
    #[ignore]
    async fn update_downloads_and_verifies() {
        let dir = std::env::temp_dir().join("sunder-test-update");
        let _ = std::fs::create_dir_all(&dir);
        let version = update(&dir).await.expect("update should succeed");
        assert!(
            version.chars().all(|c| c.is_ascii_digit() || c == '.'),
            "version should be numeric calver, got: {version}"
        );
        assert!(managed_bin_path(&dir).exists());
        // Converges on re-run (idempotent re-download + atomic replace).
        let v2 = update(&dir).await.expect("second update should succeed");
        assert_eq!(version, v2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
