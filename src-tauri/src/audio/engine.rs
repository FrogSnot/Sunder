use std::ffi::c_void;
use std::io::{self, BufRead, Read};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use rodio::{Decoder, OutputStream, Sink, Source};
use tauri::{Emitter, Manager};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};

/// Wrapper to send a raw HWND pointer across threads.
/// SAFETY: The HWND outlives the audio thread (it's the main window).
struct RawHwnd(*mut c_void);
unsafe impl Send for RawHwnd {}

use super::equalizer::{EqSettings, EqSource};
use super::state::PlaybackState;

pub enum AudioCommand {
    Play { video_id: String, duration_ms: u64 },
    Prepared { session_id: usize, sink: rodio::Sink, duration_ms: u64 },
    LoadFailed { session_id: usize, video_id: String, error: String },
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
    Seek(f64),
    UpdateMetadata { video_id: String, title: String, artist: String, thumbnail_url: String },
    ArtReady { title: String, artist: String, path: Option<std::path::PathBuf> },
}

pub struct AudioHandle {
    tx: std::sync::mpsc::Sender<AudioCommand>,
    pub state: Arc<RwLock<PlaybackState>>,
    pub position_ms: Arc<AtomicU64>,
    pub duration_ms: Arc<AtomicU64>,
    pub volume: Arc<RwLock<f32>>,
    pub eq_settings: Arc<RwLock<EqSettings>>,
    #[allow(dead_code)]
    pub current_session: Arc<AtomicUsize>,
}

impl AudioHandle {
    pub fn new(app: tauri::AppHandle) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let state = Arc::new(RwLock::new(PlaybackState::Idle));
        let position_ms = Arc::new(AtomicU64::new(0));
        let duration_ms = Arc::new(AtomicU64::new(0));
        let volume = Arc::new(RwLock::new(0.8_f32));
        let eq_settings = Arc::new(RwLock::new(EqSettings::default()));
        let current_session = Arc::new(AtomicUsize::new(0));

        let handle = Self {
            tx: tx.clone(),
            state: state.clone(),
            position_ms: position_ms.clone(),
            duration_ms: duration_ms.clone(),
            volume: volume.clone(),
            eq_settings: eq_settings.clone(),
            current_session: current_session.clone(),
        };

        let hwnd = Self::extract_hwnd(&app);

        let app_handle = app.app_handle().clone();
        let current_session_clone = current_session.clone();
        let tx_clone = tx.clone();

        std::thread::Builder::new()
            .name("sunder-audio".into())
            .spawn(move || {
                audio_thread(
                    tx_clone,
                    rx,
                    state,
                    position_ms,
                    duration_ms,
                    volume,
                    eq_settings,
                    app_handle,
                    current_session_clone,
                    hwnd,
                );
            })
            .expect("failed to spawn audio thread");
        handle
    }

    pub fn send(&self, cmd: AudioCommand) {
        let _ = self.tx.send(cmd);
    }

    /// Extract the main window's HWND for souvlaki on Windows.
    /// Returns None on Linux/macOS where HWND is not needed.
    fn extract_hwnd(app: &tauri::AppHandle) -> Option<RawHwnd> {
        #[cfg(target_os = "windows")]
        {
            use tauri::Manager;
            app.get_webview_window("main")
                .and_then(|w| w.hwnd().ok())
                .map(|h| RawHwnd(h.0 as *mut c_void))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = app;
            None
        }
    }
}

fn ytdlp_bin() -> String {
    std::env::var("SUNDER_YTDLP_PATH").unwrap_or_else(|_| "yt-dlp".into())
}

#[allow(clippy::too_many_arguments)]
fn audio_thread(
    tx: std::sync::mpsc::Sender<AudioCommand>,
    rx: std::sync::mpsc::Receiver<AudioCommand>,
    state: Arc<RwLock<PlaybackState>>,
    position_ms: Arc<AtomicU64>,
    duration_ms: Arc<AtomicU64>,
    volume: Arc<RwLock<f32>>,
    eq_settings: Arc<RwLock<EqSettings>>,
    app: tauri::AppHandle,
    current_session: Arc<AtomicUsize>,
    hwnd: Option<RawHwnd>,
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

    let mut controls = match MediaControls::new(PlatformConfig {
        dbus_name: "sunder",
        display_name: "Sunder",
        hwnd: hwnd.map(|h| h.0),
    }) {
        Ok(mut c) => {
            let tx_clone = tx.clone();
            let app_clone = app.clone();
            let _ = c.attach(move |event| {
                match event {
                    MediaControlEvent::Pause => { let _ = tx_clone.send(AudioCommand::Pause); }
                    MediaControlEvent::Play => { let _ = tx_clone.send(AudioCommand::Resume); }
                    MediaControlEvent::Toggle => {
                        // Toggle routes through the frontend so it can update UI state
                        let _ = app_clone.emit("media-toggle", ());
                    }
                    MediaControlEvent::Next => {
                        let _ = app_clone.emit("media-next", ());
                    }
                    MediaControlEvent::Previous => {
                        let _ = app_clone.emit("media-previous", ());
                    }
                    MediaControlEvent::Stop => { let _ = tx_clone.send(AudioCommand::Stop); }
                    MediaControlEvent::Seek(_) => {}
                    MediaControlEvent::SeekBy(_, _) => {}
                    MediaControlEvent::SetPosition(pos) => { let _ = tx_clone.send(AudioCommand::Seek(pos.0.as_secs_f64())); }
                    MediaControlEvent::SetVolume(v) => { let _ = tx_clone.send(AudioCommand::SetVolume(v as f32)); }
                    _ => {}
                }
            });
            Some(c)
        }
        Err(_) => None,
    };

    let mut last_mpris_state: Option<PlaybackState> = None;
    let mut last_mpris_pos: u64 = 0;

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
                    let session_id = current_session.fetch_add(1, Ordering::SeqCst) + 1;
                    *state.write().unwrap() = PlaybackState::Loading;
                    duration_ms.store(dur, Ordering::Release);
                    position_ms.store(0, Ordering::Release);
                    emit_state(&app, &state, &position_ms, &duration_ms);

                    let app_clone = app.clone();
                    let state_clone = state.clone();
                    let stream_handle_clone = stream_handle.clone();
                    let volume_clone = volume.clone();
                    let eq_settings_clone = eq_settings.clone();
                    let tx_clone = tx.clone();
                    let video_id_clone = video_id.clone();
                    let session_clone = current_session.clone();

                    std::thread::spawn(move || {
                        // Early exit if session was already superseded (rapid skip)
                        if session_clone.load(Ordering::SeqCst) != session_id {
                            return;
                        }
                        let vol = *volume_clone.read().unwrap();
                        match start_streaming(&video_id_clone, &state_clone, &stream_handle_clone, vol, &eq_settings_clone, &app_clone) {
                            Ok(new_sink) => {
                                let _ = tx_clone.send(AudioCommand::Prepared {
                                    session_id,
                                    sink: new_sink,
                                    duration_ms: dur,
                                });
                            }
                            Err(e) => {
                                let _ = tx_clone.send(AudioCommand::LoadFailed {
                                    session_id,
                                    video_id: video_id_clone,
                                    error: e.to_string(),
                                });
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
                AudioCommand::LoadFailed { session_id, video_id, error } => {
                    if session_id == current_session.load(Ordering::SeqCst) {
                        *state.write().unwrap() = PlaybackState::Idle;
                        emit_state(&app, &state, &position_ms, &duration_ms);
                        let _ = app.emit("playback-error", serde_json::json!({
                            "video_id": video_id,
                            "error": error,
                        }));
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
                        if let Err(e) = s.try_seek(d) {
                            eprintln!("[sunder] seek failed: {e}");
                        } else {
                            position_ms.store((secs * 1000.0) as u64, Ordering::Release);
                        }
                    }
                }
                AudioCommand::UpdateMetadata { video_id, title, artist, thumbnail_url } => {
                    if let Some(ref mut c) = controls {
                        let _ = c.set_metadata(MediaMetadata {
                            title: Some(&title),
                            artist: Some(&artist),
                            album: None,
                            cover_url: None,
                            duration: Some(Duration::from_millis(duration_ms.load(Ordering::Relaxed))),
                        });
                    }

                    // Spawn worker to fetch art
                    let tx_clone = tx.clone();
                    let video_id_clone = video_id.clone();
                    let title_clone = title.clone();
                    let artist_clone = artist.clone();
                    std::thread::spawn(move || {
                        let path = super::art_worker::download_art(video_id_clone.clone(), thumbnail_url);
                        let _ = tx_clone.send(AudioCommand::ArtReady { 
                            title: title_clone, 
                            artist: artist_clone, 
                            path 
                        });
                    });
                }
                AudioCommand::ArtReady { title, artist, path } => {
                    // Update MPRIS with local art path
                    let path_buf = path.clone();
                    if let Some(ref mut c) = controls {
                        let mut metadata = MediaMetadata {
                            title: Some(&title),
                            artist: Some(&artist),
                            album: None,
                            cover_url: None,
                            duration: Some(Duration::from_millis(duration_ms.load(Ordering::Relaxed))),
                        };
                        
                        let path_str;
                        if let Some(ref p) = path_buf {
                            if let Some(s) = p.to_str() {
                                path_str = format!("file://{}", s);
                                metadata.cover_url = Some(&path_str);
                            }
                        }
                        let _ = c.set_metadata(metadata);
                    }

                    // Trigger system notification
                    super::art_worker::trigger_notification(&app, &title, &artist, path);
                }
            }
        }

        // Only update MPRIS when playback state or position actually changed
        if let Some(ref mut c) = controls {
            let st = state.read().unwrap().clone();
            let pos = position_ms.load(Ordering::Relaxed);
            let dur = duration_ms.load(Ordering::Relaxed);
            let state_changed = last_mpris_state.as_ref() != Some(&st);
            let pos_changed = pos.abs_diff(last_mpris_pos) > 500; // debounce to 500ms

            if state_changed || pos_changed {
                let progress = if dur > 0 {
                    Some(souvlaki::MediaPosition(Duration::from_millis(pos)))
                } else {
                    None
                };

                match st {
                    PlaybackState::Playing => {
                        let _ = c.set_playback(MediaPlayback::Playing { progress });
                    }
                    PlaybackState::Paused => {
                        let _ = c.set_playback(MediaPlayback::Paused { progress });
                    }
                    PlaybackState::Stopped | PlaybackState::Idle => {
                        let _ = c.set_playback(MediaPlayback::Stopped);
                    }
                    _ => {}
                }

                last_mpris_state = Some(st);
                last_mpris_pos = pos;
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

/// Download audio via yt-dlp to a temp MP3 file, then decode with rodio.
/// symphonia 0.5 cannot decode YouTube's M4A containers (SeekError on init),
/// so we let yt-dlp + ffmpeg convert to MP3 which symphonia handles perfectly.
fn start_streaming(
    video_id: &str,
    state: &Arc<RwLock<PlaybackState>>,
    stream_handle: &rodio::OutputStreamHandle,
    volume: f32,
    eq_settings: &Arc<RwLock<EqSettings>>,
    app: &tauri::AppHandle,
) -> Result<Sink, crate::error::AppError> {
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let bin = ytdlp_bin();

    let cache_dir = std::env::temp_dir().join("sunder");
    std::fs::create_dir_all(&cache_dir)
        .map_err(crate::error::AppError::Io)?;

    let out_template = cache_dir.join(format!("{video_id}.%(ext)s"));
    let expected_path = cache_dir.join(format!("{video_id}.mp3"));

    *state.write().unwrap() = PlaybackState::Buffering;

    if !expected_path.exists() {
        let _ = app.emit("download-progress", serde_json::json!({
            "percent": 0.0, "stage": "preparing"
        }));

        let out_path_str = out_template.to_str().unwrap_or_default();
        let base_args: Vec<&str> = vec![
            url.as_str(),
            "--extract-audio",
            "--audio-format", "mp3",
            "--audio-quality", "2",
            "-o", out_path_str,
            "--no-playlist",
            "--newline",
            "--concurrent-fragments", "4",
        ];
        let fallback_args: &[&str] = &["--force-ipv4", "--geo-bypass", "--extractor-retries", "3"];
        let mut last_error = String::new();

        for attempt in 0..2u8 {
            if attempt > 0 {
                eprintln!("[sunder] retrying download (attempt {})", attempt + 1);
                for ext in ["mp3", "webm", "m4a", "opus", "part", "webm.part", "m4a.part"] {
                    let _ = std::fs::remove_file(cache_dir.join(format!("{video_id}.{ext}")));
                }
            }

            let mut args = base_args.clone();
            if attempt > 0 {
                args.extend_from_slice(fallback_args);
            }

            let mut child = match Command::new(&bin)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => return Err(crate::error::AppError::Extraction(format!("failed to spawn yt-dlp: {e}"))),
            };

            if let Some(stdout) = child.stdout.take() {
                for line in io::BufReader::new(stdout).lines().map_while(Result::ok) {
                    if let Some(pct) = parse_download_pct(&line) {
                        let _ = app.emit("download-progress", serde_json::json!({
                            "percent": pct, "stage": "downloading"
                        }));
                    } else if line.contains("[ExtractAudio]") {
                        let _ = app.emit("download-progress", serde_json::json!({
                            "percent": 100.0, "stage": "converting"
                        }));
                    } else if line.contains("[youtube]") || line.contains("[info]") {
                        let _ = app.emit("download-progress", serde_json::json!({
                            "percent": 0.0, "stage": "extracting"
                        }));
                    }
                }
            }

            let status = match child.wait() {
                Ok(s) => s,
                Err(e) => return Err(crate::error::AppError::Extraction(format!("yt-dlp wait: {e}"))),
            };

            if status.success() && expected_path.exists() {
                last_error.clear();
                break;
            }

            let stderr_out = child.stderr.take()
                .map(|mut s| {
                    let mut buf = String::new();
                    let _ = s.read_to_string(&mut buf);
                    buf
                })
                .unwrap_or_default();

            last_error = if !stderr_out.is_empty() {
                let trimmed = stderr_out.trim();
                eprintln!("[sunder] yt-dlp stderr (attempt {}): {}", attempt + 1, trimmed);
                format!("yt-dlp failed ({}): {}", status, trimmed.lines().last().unwrap_or(trimmed))
            } else {
                format!("yt-dlp failed ({})", status)
            };
        }

        if !last_error.is_empty() {
            return Err(crate::error::AppError::Extraction(last_error));
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
        .map_err(crate::error::AppError::Io)?;
    let decoder = Decoder::new(io::BufReader::with_capacity(512 * 1024, file))
        .map_err(|e| crate::error::AppError::Audio(format!("decoder init failed: {e}")))?;

    let sink = Sink::try_new(stream_handle)
        .map_err(|e| crate::error::AppError::Audio(e.to_string()))?;
    sink.set_volume(volume);
    sink.append(EqSource::new(decoder.convert_samples(), eq_settings.clone()));

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
