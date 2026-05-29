use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Semaphore;

use crate::db::SearchCache;
use crate::models::Track;

/// Maximum number of concurrent yt-dlp download processes. Keeps bulk playlist
/// downloads from spawning hundreds of processes at once.
const MAX_CONCURRENT: usize = 3;

/// Manages persistent offline downloads: a stable directory, a concurrency
/// limiter, and a set of in-flight track ids to de-duplicate requests.
pub struct DownloadManager {
    dir: PathBuf,
    sem: Arc<Semaphore>,
    active: Arc<Mutex<HashSet<String>>>,
}

impl DownloadManager {
    pub fn new(data_dir: &Path) -> Self {
        let dir = data_dir.join("downloads");
        let _ = std::fs::create_dir_all(&dir);
        Self {
            dir,
            sem: Arc::new(Semaphore::new(MAX_CONCURRENT)),
            active: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Persistent path the offline MP3 lives at for a given track id.
    pub fn path_for(&self, track_id: &str) -> PathBuf {
        self.dir.join(format!("{track_id}.mp3"))
    }

    /// Resolve the offline downloads directory for a Tauri app handle. Shared
    /// with the audio engine so downloaded tracks play back without network.
    pub fn dir_for(app: &AppHandle) -> PathBuf {
        use tauri::Manager;
        let base = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap().join("sunder_data"));
        base.join("downloads")
    }

    fn try_begin(&self, track_id: &str) -> bool {
        self.active.lock().unwrap().insert(track_id.to_string())
    }

    fn finish(&self, track_id: &str) {
        self.active.lock().unwrap().remove(track_id);
    }

    /// Download a single track to the persistent directory and record it in the
    /// database. Emits `track-download` events describing progress. Idempotent:
    /// already-downloaded or in-flight tracks resolve immediately.
    pub async fn download(&self, app: &AppHandle, db: &SearchCache, track: &Track) -> Result<(), String> {
        let track_id = track.id.clone();
        let path = self.path_for(&track_id);

        // Already downloaded on disk: ensure DB knows and report done.
        if path.exists() {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let _ = db.upsert_tracks(std::slice::from_ref(track));
            let _ = db.mark_downloaded(&track_id, &path.to_string_lossy(), size);
            emit(app, &track_id, "done", 100.0);
            return Ok(());
        }

        if !self.try_begin(&track_id) {
            // Another request is already downloading this track.
            return Ok(());
        }

        // Make sure metadata is persisted so the offline library can display it.
        let _ = db.upsert_tracks(std::slice::from_ref(track));
        emit(app, &track_id, "queued", 0.0);

        let permit = self.sem.clone().acquire_owned().await;
        let result = run_ytdlp(app, &self.dir, &track_id).await;
        drop(permit);

        self.finish(&track_id);

        match result {
            Ok(final_path) => {
                let size = std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
                db.mark_downloaded(&track_id, &final_path.to_string_lossy(), size)
                    .map_err(|e| e.to_string())?;
                emit(app, &track_id, "done", 100.0);
                Ok(())
            }
            Err(e) => {
                cleanup_partials(&self.dir, &track_id);
                emit(app, &track_id, "error", 0.0);
                Err(e)
            }
        }
    }

    /// Delete a downloaded track from disk and the database.
    pub fn delete(&self, db: &SearchCache, track_id: &str) -> Result<(), String> {
        let path = self.path_for(track_id);
        let _ = std::fs::remove_file(&path);
        db.remove_download(track_id).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[derive(serde::Serialize, Clone)]
struct DownloadEvent {
    track_id: String,
    status: String,
    percent: f64,
}

fn emit(app: &AppHandle, track_id: &str, status: &str, percent: f64) {
    let _ = app.emit(
        "track-download",
        DownloadEvent {
            track_id: track_id.to_string(),
            status: status.to_string(),
            percent,
        },
    );
}

fn ytdlp_bin() -> String {
    std::env::var("SUNDER_YTDLP_PATH").unwrap_or_else(|_| "yt-dlp".into())
}

/// Runs yt-dlp, streaming download progress as `track-download` events.
/// Returns the path to the finished MP3 on success.
async fn run_ytdlp(app: &AppHandle, dir: &Path, track_id: &str) -> Result<PathBuf, String> {
    let bin = ytdlp_bin();
    let url = format!("https://www.youtube.com/watch?v={track_id}");
    let out_template = dir.join(format!("{track_id}.%(ext)s"));
    let expected_path = dir.join(format!("{track_id}.mp3"));

    let mut child = Command::new(&bin)
        .args([
            url.as_str(),
            "--extract-audio",
            "--audio-format",
            "mp3",
            "--audio-quality",
            "2",
            "-o",
            out_template.to_str().unwrap_or_default(),
            "--no-playlist",
            "--newline",
            "--concurrent-fragments",
            "4",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn yt-dlp: {e}"))?;

    if let Some(stdout) = child.stdout.take() {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(pct) = parse_download_pct(&line) {
                emit(app, track_id, "downloading", pct);
            } else if line.contains("[ExtractAudio]") {
                emit(app, track_id, "converting", 100.0);
            }
        }
    }

    let status = child.wait().await.map_err(|e| format!("yt-dlp wait: {e}"))?;

    if status.success() && expected_path.exists() {
        return Ok(expected_path);
    }

    let stderr = child
        .stderr
        .take()
        .map(|mut s| async move {
            use tokio::io::AsyncReadExt;
            let mut buf = String::new();
            let _ = s.read_to_string(&mut buf).await;
            buf
        });
    let stderr = match stderr {
        Some(fut) => fut.await,
        None => String::new(),
    };
    let trimmed = stderr.trim();
    Err(format!(
        "download failed ({status}): {}",
        trimmed.lines().last().unwrap_or("unknown error")
    ))
}

fn cleanup_partials(dir: &Path, track_id: &str) {
    for ext in ["mp3", "webm", "m4a", "opus", "part", "webm.part", "m4a.part"] {
        let _ = std::fs::remove_file(dir.join(format!("{track_id}.{ext}")));
    }
}

fn parse_download_pct(line: &str) -> Option<f64> {
    let content = line.trim().strip_prefix("[download]")?;
    let pct_end = content.find('%')?;
    content[..pct_end].trim().parse::<f64>().ok()
}
