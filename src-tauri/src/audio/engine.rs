use std::io;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use rodio::{Decoder, OutputStream, Sink, Source};
use tauri::Emitter;

use super::equalizer::{EqSettings, EqSource};
use super::state::PlaybackState;

/// Commands sent to the audio thread to control playback.
pub enum AudioCommand {
    /// Start a new playback session for a track.
    Play { video_id: String, duration_ms: u64 },
    /// Internal: track download/prep is finished, swap the sink.
    Prepared { session_id: u64, sink: Sink, duration_ms: u64 },
    /// Internal: track preparation failed (e.g. network error).
    LoadFailed { session_id: u64, error: String },
    /// Pause current playback.
    Pause,
    /// Resume paused playback.
    Resume,
    /// Stop playback and clear the sink.
    Stop,
    /// Set volume immediately.
    SetVolume(f32),
    /// Seek to a time in seconds.
    Seek(f64),
}

pub struct AudioHandle {
    tx: std::sync::mpsc::Sender<AudioCommand>,
    pub state: Arc<RwLock<PlaybackState>>,
    pub position_ms: Arc<AtomicU64>,
    pub duration_ms: Arc<AtomicU64>,
    pub volume: Arc<RwLock<f32>>,
    pub eq_settings: Arc<RwLock<EqSettings>>,
    pub current_session: Arc<AtomicU64>,
}

impl AudioHandle {
    pub fn new(app: tauri::AppHandle) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let state = Arc::new(RwLock::new(PlaybackState::Idle));
        let position_ms = Arc::new(AtomicU64::new(0));
        let duration_ms = Arc::new(AtomicU64::new(0));
        let volume = Arc::new(RwLock::new(0.8_f32));
        let eq_settings = Arc::new(RwLock::new(EqSettings::default()));
        let current_session = Arc::new(AtomicU64::new(0));

        let handle = Self {
            tx: tx.clone(),
            state: state.clone(),
            position_ms: position_ms.clone(),
            duration_ms: duration_ms.clone(),
            volume: volume.clone(),
            eq_settings: eq_settings.clone(),
            current_session: current_session.clone(),
        };

        std::thread::Builder::new()
            .name("sunder-audio".into())
            .spawn(move || {
                audio_thread(tx, rx, state, position_ms, duration_ms, volume, eq_settings, current_session, app);
            })
            .expect("failed to spawn audio thread");

        handle
    }

    pub fn send(&self, cmd: AudioCommand) {
        if matches!(cmd, AudioCommand::Play { .. }) {
            self.current_session.fetch_add(1, Ordering::SeqCst);
        }
        let _ = self.tx.send(cmd);
    }
}

fn ytdlp_bin() -> String {
    std::env::var("SUNDER_YTDLP_PATH").unwrap_or_else(|_| "yt-dlp".into())
}

/// The background audio thread that handles rodio Sink management.
/// Uses a session ID to ensure late-arriving downloads don't overwrite current playback.
fn audio_thread(
    tx_for_thread: std::sync::mpsc::Sender<AudioCommand>,
    rx: std::sync::mpsc::Receiver<AudioCommand>,
    state: Arc<RwLock<PlaybackState>>,
    position_ms: Arc<AtomicU64>,
    duration_ms: Arc<AtomicU64>,
    volume: Arc<RwLock<f32>>,
    eq_settings: Arc<RwLock<EqSettings>>,
    current_session: Arc<AtomicU64>,
    app: tauri::AppHandle,
) {
    let (_stream, stream_handle) = match OutputStream::try_default() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[sunder] FATAL: no audio output device: {e}");
            return;
        }
    };
    eprintln!("[sunder] audio thread started, output device ready");

    let mut sink: Option<Sink> = None;

    loop {
        let first = rx.recv_timeout(Duration::from_millis(50));

        let mut cmds: Vec<AudioCommand> = Vec::new();
        match first {
            Ok(cmd) => {
                cmds.push(cmd);
                while let Ok(more) = rx.try_recv() {
                    cmds.push(more);
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }

        for cmd in cmds {
            match cmd {
                AudioCommand::Play { video_id, duration_ms: dur } => {
                    duration_ms.store(dur, Ordering::Release);
                    position_ms.store(0, Ordering::Release);
                    *state.write().unwrap() = PlaybackState::Loading;
                    emit_state(&app, &state, &position_ms, &duration_ms);

                    let session_id = current_session.load(Ordering::SeqCst);
                    let app_clone = app.clone();
                    let current_session_clone = current_session.clone();
                    let stream_handle_clone = stream_handle.clone();
                    let vol = *volume.read().unwrap();
                    let eq_clone = eq_settings.clone();
                    let tx_clone = tx_for_thread.clone(); 

                    // Spawn non-blocking background preparation using Tauri's async runtime
                    tauri::async_runtime::spawn(async move {
                        match prepare_track_async(&video_id, session_id, &current_session_clone, &app_clone, vol, eq_clone, &stream_handle_clone).await {
                            Ok(new_sink) => {
                                let _ = tx_clone.send(AudioCommand::Prepared { 
                                    session_id, 
                                    sink: new_sink, 
                                    duration_ms: dur 
                                });
                            }
                            Err(e) => {
                                if !e.to_string().contains("Session cancelled") {
                                    let _ = tx_clone.send(AudioCommand::LoadFailed { 
                                        session_id, 
                                        error: e.to_string() 
                                    });
                                }
                            }
                        }
                    });
                }
                AudioCommand::Prepared { session_id, sink: new_sink, duration_ms: dur } => {
                    if session_id == current_session.load(Ordering::SeqCst) {
                        if let Some(s) = sink.take() {
                            s.stop();
                        }
                        duration_ms.store(dur, Ordering::Release);
                        position_ms.store(0, Ordering::Release);
                        sink = Some(new_sink);
                        *state.write().unwrap() = PlaybackState::Playing;
                        emit_state(&app, &state, &position_ms, &duration_ms);
                    }
                }
                AudioCommand::LoadFailed { session_id, error } => {
                    if session_id == current_session.load(Ordering::SeqCst) {
                        eprintln!("[sunder] load failed: {error}");
                        let _ = app.emit("playback-error", serde_json::json!({ "error": error }));
                        *state.write().unwrap() = PlaybackState::Idle;
                        emit_state(&app, &state, &position_ms, &duration_ms);
                    }
                }
                AudioCommand::Pause => {
                    if let Some(ref s) = sink {
                        s.pause();
                        *state.write().unwrap() = PlaybackState::Paused;
                    }
                }
                AudioCommand::Resume => {
                    if let Some(ref s) = sink {
                        s.play();
                        *state.write().unwrap() = PlaybackState::Playing;
                    }
                }
                AudioCommand::Stop => {
                    if let Some(s) = sink.take() {
                        s.stop();
                    }
                    *state.write().unwrap() = PlaybackState::Stopped;
                    position_ms.store(0, Ordering::Release);
                }
                AudioCommand::SetVolume(v) => {
                    *volume.write().unwrap() = v;
                    if let Some(ref s) = sink {
                        s.set_volume(v);
                    }
                }
                AudioCommand::Seek(secs) => {
                    if let Some(ref s) = sink {
                        let d = Duration::from_secs_f64(secs.max(0.0));
                        let vol = *volume.read().unwrap();
                        s.set_volume(0.0);
                        if let Err(e) = s.try_seek(d) {
                            eprintln!("[sunder] seek failed: {e}");
                            s.set_volume(vol);
                        } else {
                            position_ms.store((secs * 1000.0) as u64, Ordering::Release);
                            std::thread::sleep(Duration::from_millis(50));
                            s.set_volume(vol);
                        }
                    }
                }
            }
        }

        let mut track_ended = false;

        if let Some(ref s) = sink {
            if !s.empty() {
                let pos = s.get_pos();
                let pos_ms = pos.as_millis() as u64;
                let dur = duration_ms.load(Ordering::Relaxed);
                position_ms.store(pos_ms, Ordering::Release);

                // Force completion if position far exceeds reported duration
                // (guards against decoders that don't EOF cleanly)
                if dur > 0 && pos_ms > dur + 2000 && *state.read().unwrap() == PlaybackState::Playing {
                    eprintln!(
                        "[sunder] position ({pos_ms}ms) exceeded duration ({dur}ms), forcing track end"
                    );
                    track_ended = true;
                }
            } else if *state.read().unwrap() == PlaybackState::Playing {
                eprintln!("[sunder] track finished");
                track_ended = true;
            }
        }

        if track_ended {
            if let Some(s) = sink.take() {
                s.stop();
            }
            *state.write().unwrap() = PlaybackState::Idle;
            position_ms.store(0, Ordering::Release);
            let _ = app.emit("track-finished", ());
        }

        emit_state(&app, &state, &position_ms, &duration_ms);
    }
}

/// Download audio via yt-dlp to a temp MP3 file in a non-blocking background task.
async fn prepare_track_async(
    video_id: &str,
    session_id: u64,
    current_session: &Arc<AtomicU64>,
    app: &tauri::AppHandle,
    volume: f32,
    eq_settings: Arc<RwLock<EqSettings>>,
    stream_handle: &rodio::OutputStreamHandle,
) -> Result<Sink, crate::error::AppError> {
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let bin = ytdlp_bin();

    let cache_dir = std::env::temp_dir().join("sunder");
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| crate::error::AppError::Io(e))?;

    let expected_path = cache_dir.join(format!("{video_id}.mp3"));

    if !expected_path.exists() {
        // Quick session check before starting slow process
        if current_session.load(Ordering::SeqCst) != session_id {
            return Err(crate::error::AppError::Extraction("Session cancelled".into()));
        }

        let _ = app.emit("download-progress", serde_json::json!({
            "percent": 0.0, "stage": "preparing"
        }));

        let out_template = cache_dir.join(format!("{video_id}.%(ext)s"));
        let out_path_str = out_template.to_str().unwrap_or_default();
        
        let args: Vec<String> = vec![
            url,
            "--extract-audio".into(),
            "--audio-format".into(), "mp3".into(),
            "--audio-quality".into(), "2".into(),
            "-o".into(), out_path_str.into(),
            "--no-playlist".into(),
            "--newline".into(),
            "--concurrent-fragments".into(), "4".into(),
        ];

        let mut child = Command::new(&bin)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| crate::error::AppError::Extraction(format!("failed to spawn yt-dlp: {e}")))?;

        if let Some(stdout) = child.stdout.take() {
            let mut reader = tokio::io::BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                // Periodically check if session is still valid
                if current_session.load(Ordering::SeqCst) != session_id {
                    let _ = child.kill().await;
                    return Err(crate::error::AppError::Extraction("Session cancelled".into()));
                }

                if let Some(pct) = parse_download_pct(&line) {
                    let _ = app.emit("download-progress", serde_json::json!({
                        "percent": pct, "stage": "downloading"
                    }));
                }
            }
        }

        let _ = child.wait().await;
    }

    if !expected_path.exists() {
        return Err(crate::error::AppError::Extraction("yt-dlp produced no output".into()));
    }

    // Final session check before creating the sink
    if current_session.load(Ordering::SeqCst) != session_id {
        return Err(crate::error::AppError::Extraction("Session cancelled".into()));
    }

    let file = std::fs::File::open(&expected_path)
        .map_err(|e| crate::error::AppError::Io(e))?;
    let decoder = Decoder::new(io::BufReader::with_capacity(512 * 1024, file))
        .map_err(|e| crate::error::AppError::Audio(format!("decoder init failed: {e}")))?;

    let sink = Sink::try_new(stream_handle)
        .map_err(|e| crate::error::AppError::Audio(e.to_string()))?;
    sink.set_volume(volume);
    sink.append(EqSource::new(decoder.convert_samples(), eq_settings));

    Ok(sink)
}

#[derive(serde::Serialize, Clone)]
struct ProgressPayload {
    position_ms: u64,
    duration_ms: u64,
    state: String,
}

fn emit_state(
    app: &tauri::AppHandle,
    state: &Arc<RwLock<PlaybackState>>,
    position_ms: &Arc<AtomicU64>,
    duration_ms: &Arc<AtomicU64>,
) {
    let _ = app.emit(
        "playback-progress",
        ProgressPayload {
            position_ms: position_ms.load(Ordering::Relaxed),
            duration_ms: duration_ms.load(Ordering::Relaxed),
            state: state.read().unwrap().to_string(),
        },
    );
}

fn parse_download_pct(line: &str) -> Option<f64> {
    let content = line.trim().strip_prefix("[download]")?;
    let pct_end = content.find('%')?;
    content[..pct_end].trim().parse::<f64>().ok()
}
