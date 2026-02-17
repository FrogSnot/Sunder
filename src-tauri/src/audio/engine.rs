use std::io::{self, BufRead};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use rodio::{Decoder, OutputStream, Sink};
use tauri::Emitter;

use super::state::PlaybackState;

pub enum AudioCommand {
    Play { video_id: String, duration_ms: u64 },
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
    Seek(f64),
}

pub struct AudioHandle {
    tx: std::sync::mpsc::Sender<AudioCommand>,
    pub state: Arc<RwLock<PlaybackState>>,
    pub position_ms: Arc<AtomicU64>,
    pub duration_ms: Arc<AtomicU64>,
    pub volume: Arc<RwLock<f32>>,
}

impl AudioHandle {
    pub fn new(app: tauri::AppHandle) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let state = Arc::new(RwLock::new(PlaybackState::Idle));
        let position_ms = Arc::new(AtomicU64::new(0));
        let duration_ms = Arc::new(AtomicU64::new(0));
        let volume = Arc::new(RwLock::new(0.8_f32));

        let handle = Self {
            tx,
            state: state.clone(),
            position_ms: position_ms.clone(),
            duration_ms: duration_ms.clone(),
            volume: volume.clone(),
        };

        std::thread::Builder::new()
            .name("sunder-audio".into())
            .spawn(move || {
                audio_thread(rx, state, position_ms, duration_ms, volume, app);
            })
            .expect("failed to spawn audio thread");

        handle
    }

    pub fn send(&self, cmd: AudioCommand) {
        let _ = self.tx.send(cmd);
    }
}

fn ytdlp_bin() -> String {
    std::env::var("SUNDER_YTDLP_PATH").unwrap_or_else(|_| "yt-dlp".into())
}

fn audio_thread(
    rx: std::sync::mpsc::Receiver<AudioCommand>,
    state: Arc<RwLock<PlaybackState>>,
    position_ms: Arc<AtomicU64>,
    duration_ms: Arc<AtomicU64>,
    volume: Arc<RwLock<f32>>,
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
        match rx.try_recv() {
            Ok(AudioCommand::Play { video_id, duration_ms: dur }) => {
                if let Some(s) = sink.take() {
                    s.stop();
                }

                *state.write().unwrap() = PlaybackState::Loading;
                duration_ms.store(dur, Ordering::Release);
                position_ms.store(0, Ordering::Release);
                emit_state(&app, &state, &position_ms, &duration_ms);

                eprintln!("[sunder] starting playback for: {video_id}");
                let vol = *volume.read().unwrap();

                match start_streaming(&video_id, &state, &stream_handle, vol, &app) {
                    Ok(new_sink) => {
                        eprintln!("[sunder] playback started");
                        sink = Some(new_sink);
                        *state.write().unwrap() = PlaybackState::Playing;
                    }
                    Err(e) => {
                        eprintln!("[sunder] playback error: {e}");
                        *state.write().unwrap() = PlaybackState::Error(e.to_string());
                    }
                }
            }
            Ok(AudioCommand::Pause) => {
                if let Some(ref s) = sink {
                    s.pause();
                    *state.write().unwrap() = PlaybackState::Paused;
                }
            }
            Ok(AudioCommand::Resume) => {
                if let Some(ref s) = sink {
                    s.play();
                    *state.write().unwrap() = PlaybackState::Playing;
                }
            }
            Ok(AudioCommand::Stop) => {
                if let Some(s) = sink.take() {
                    s.stop();
                }
                *state.write().unwrap() = PlaybackState::Stopped;
                position_ms.store(0, Ordering::Release);
            }
            Ok(AudioCommand::SetVolume(v)) => {
                *volume.write().unwrap() = v;
                if let Some(ref s) = sink {
                    s.set_volume(v);
                }
            }
            Ok(AudioCommand::Seek(secs)) => {
                if let Some(ref s) = sink {
                    let dur = Duration::from_secs_f64(secs.max(0.0));
                    if let Err(e) = s.try_seek(dur) {
                        eprintln!("[sunder] seek failed: {e}");
                    } else {
                        position_ms.store((secs * 1000.0) as u64, Ordering::Release);
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        }

        if let Some(ref s) = sink {
            if !s.empty() {
                let pos = s.get_pos();
                position_ms.store(pos.as_millis() as u64, Ordering::Release);
            } else if *state.read().unwrap() == PlaybackState::Playing {
                *state.write().unwrap() = PlaybackState::Idle;
                position_ms.store(0, Ordering::Release);
                eprintln!("[sunder] track finished");
            }
        }

        emit_state(&app, &state, &position_ms, &duration_ms);
        std::thread::sleep(Duration::from_millis(16));
    }
}

/// Download audio via yt-dlp to a temp MP3 file, then decode with rodio.
/// symphonia 0.5 cannot decode YouTube's M4A containers (SeekError on init),
/// so we let yt-dlp + ffmpeg convert to MP3 which symphonia handles perfectly.
fn start_streaming(
    video_id: &str,
    state: &Arc<RwLock<PlaybackState>>,
    stream_handle: &rodio::OutputStreamHandle,
    volume: f32,
    app: &tauri::AppHandle,
) -> Result<Sink, crate::error::AppError> {
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let bin = ytdlp_bin();

    let cache_dir = std::env::temp_dir().join("sunder");
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| crate::error::AppError::Io(e))?;

    // yt-dlp outputs to <video_id>.%(ext)s with --audio-format mp3 the final ext is mp3
    let out_template = cache_dir.join(format!("{video_id}.%(ext)s"));
    let expected_path = cache_dir.join(format!("{video_id}.mp3"));

    *state.write().unwrap() = PlaybackState::Buffering;

    if !expected_path.exists() {
        let _ = app.emit("download-progress", serde_json::json!({
            "percent": 0.0, "stage": "preparing"
        }));

        let mut child = Command::new(&bin)
            .args([
                &url,
                "--extract-audio",
                "--audio-format", "mp3",
                "--audio-quality", "2",
                "-o", out_template.to_str().unwrap_or_default(),
                "--no-playlist",
                "--newline",
                "--concurrent-fragments", "4",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| crate::error::AppError::Extraction(format!("failed to spawn yt-dlp: {e}")))?;

        if let Some(stderr) = child.stderr.take() {
            for line in io::BufReader::new(stderr).lines().flatten() {
                if let Some(pct) = parse_download_pct(&line) {
                    let _ = app.emit("download-progress", serde_json::json!({
                        "percent": pct, "stage": "downloading"
                    }));
                } else if line.contains("[ExtractAudio]") {
                    let _ = app.emit("download-progress", serde_json::json!({
                        "percent": 100.0, "stage": "converting"
                    }));
                }
            }
        }

        let status = child.wait()
            .map_err(|e| crate::error::AppError::Extraction(format!("yt-dlp wait: {e}")))?;
        if !status.success() {
            return Err(crate::error::AppError::Extraction(
                format!("yt-dlp failed ({})", status),
            ));
        }

        if !expected_path.exists() {
            return Err(crate::error::AppError::Extraction(
                format!("yt-dlp produced no output at {}", expected_path.display()),
            ));
        }
    } else {
        eprintln!("[sunder] cache hit: {}", expected_path.display());
    }

    let file_len = std::fs::metadata(&expected_path)
        .map(|m| m.len())
        .unwrap_or(0);
    eprintln!("[sunder] audio ready: {} bytes at {}", file_len, expected_path.display());

    let file = std::fs::File::open(&expected_path)
        .map_err(|e| crate::error::AppError::Io(e))?;
    let decoder = Decoder::new(io::BufReader::new(file))
        .map_err(|e| crate::error::AppError::Audio(format!("decoder init failed: {e}")))?;

    let sink = Sink::try_new(stream_handle)
        .map_err(|e| crate::error::AppError::Audio(e.to_string()))?;
    sink.set_volume(volume);
    sink.append(decoder);

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
