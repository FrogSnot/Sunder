use std::sync::{mpsc, Arc, RwLock};

const APPLICATION_ID: &str = "1494440534427828354";

pub enum PresenceCommand {
    SetActivity { title: String, artist: String, thumbnail: String },
    Pause,
    Resume,
    Clear,
}

pub struct DiscordPresence {
    tx: mpsc::Sender<PresenceCommand>,
    pub enabled: Arc<RwLock<bool>>,
}

impl Default for DiscordPresence {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscordPresence {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let enabled = Arc::new(RwLock::new(false));

        if !APPLICATION_ID.starts_with("REPLACE") {
            let enabled_clone = enabled.clone();
            std::thread::Builder::new()
                .name("discord-rpc".into())
                .spawn(move || run_loop(rx, enabled_clone))
                .ok();
        }

        Self { tx, enabled }
    }

    pub fn send(&self, cmd: PresenceCommand) {
        let _ = self.tx.send(cmd);
    }

    pub fn set_enabled(&self, val: bool) {
        *self.enabled.write().unwrap() = val;
        if !val {
            let _ = self.tx.send(PresenceCommand::Clear);
        }
    }

    pub fn is_enabled(&self) -> bool {
        *self.enabled.read().unwrap()
    }
}

// ── Platform stream type ────────────────────────────────────────────────────

#[cfg(unix)]
type IpcStream = std::os::unix::net::UnixStream;
#[cfg(windows)]
type IpcStream = std::fs::File;

// ── Shared loop (unix + windows) ────────────────────────────────────────────

#[cfg(any(unix, windows))]
fn run_loop(rx: mpsc::Receiver<PresenceCommand>, enabled: Arc<RwLock<bool>>) {
    let mut stream: Option<IpcStream> = None;
    let mut nonce: u64 = 0;
    let mut last_title = String::new();
    let mut last_artist = String::new();
    let mut last_thumb = String::new();

    while let Ok(cmd) = rx.recv() {
        if !*enabled.read().unwrap() {
            stream = None;
            continue;
        }

        if stream.is_none() {
            stream = connect();
            if let Some(ref mut s) = stream {
                eprintln!("[sunder] discord: connected to IPC socket");
                let handshake = format!(r#"{{"v":1,"client_id":"{APPLICATION_ID}"}}"#);
                if let Err(e) = write_frame(s, 0, &handshake) {
                    eprintln!("[sunder] discord: handshake write failed: {e}");
                    stream = None;
                    continue;
                }
                match read_frame_body(s) {
                    Ok(body) => {
                        if body.contains("\"ERROR\"") {
                            eprintln!("[sunder] discord: handshake rejected: {body}");
                            stream = None;
                            continue;
                        }
                    }
                    Err(e) => {
                        eprintln!("[sunder] discord: handshake read failed: {e}");
                        stream = None;
                        continue;
                    }
                }
            } else {
                eprintln!("[sunder] discord: could not find IPC socket");
                continue;
            }
        }

        nonce += 1;
        let s = stream.as_mut().unwrap();

        let ok = match cmd {
            PresenceCommand::SetActivity { title, artist, thumbnail } => {
                last_title = title;
                last_artist = artist;
                last_thumb = thumbnail;
                send_activity(s, &last_title, &last_artist, &last_thumb, false, nonce)
            }
            PresenceCommand::Pause => {
                if last_title.is_empty() { continue; }
                send_activity(s, &last_title, &last_artist, &last_thumb, true, nonce)
            }
            PresenceCommand::Resume => {
                if last_title.is_empty() { continue; }
                send_activity(s, &last_title, &last_artist, &last_thumb, false, nonce)
            }
            PresenceCommand::Clear => {
                last_title.clear();
                last_artist.clear();
                last_thumb.clear();
                let pid = std::process::id();
                write_frame(s, 1, &format!(
                    r#"{{"cmd":"SET_ACTIVITY","args":{{"pid":{pid},"activity":null}},"nonce":"{nonce}"}}"#
                ))
            }
        };

        match ok {
            Ok(()) => {
                match read_frame_body(s) {
                    Ok(body) if body.contains("\"ERROR\"") => {
                        eprintln!("[sunder] discord: activity rejected: {body}");
                    }
                    Err(e) => {
                        eprintln!("[sunder] discord: response read failed: {e}");
                        stream = None;
                    }
                    _ => {}
                }
            }
            Err(e) => {
                eprintln!("[sunder] discord: write failed: {e}");
                stream = None;
            }
        }
    }
}

// ── Connection ──────────────────────────────────────────────────────────────

#[cfg(unix)]
fn connect() -> Option<IpcStream> {
    use std::time::Duration;

    // Discord-compatible clients use different IPC socket filename prefixes.
    // Standard Discord: "discord-ipc-". Vesktop (Vencord-based): "vesktop-ipc-".
    // Try each prefix per directory; first successful connect wins.
    const PREFIXES: &[&str] = &["discord-ipc-", "vesktop-ipc-"];

    let base_dirs: Vec<String> = [
        std::env::var("XDG_RUNTIME_DIR").ok(),
        std::env::var("TMPDIR").ok(),
        Some("/tmp".into()),
    ]
    .into_iter()
    .flatten()
    .collect();

    // Also check Flatpak and Snap subdirectories for both Discord and Vesktop.
    let mut dirs = base_dirs.clone();
    if let Some(xdg) = base_dirs.first() {
        dirs.push(format!("{xdg}/app/com.discordapp.Discord"));
        dirs.push(format!("{xdg}/snap.discord"));
        dirs.push(format!("{xdg}/app/dev.vencord.vesktop"));
        dirs.push(format!("{xdg}/snap.vesktop"));
    }

    for dir in &dirs {
        for prefix in PREFIXES {
            for i in 0..10 {
                let path = format!("{dir}/{prefix}{i}");
                if let Ok(s) = IpcStream::connect(&path) {
                    s.set_read_timeout(Some(Duration::from_secs(5))).ok();
                    return Some(s);
                }
            }
        }
    }
    None
}

#[cfg(windows)]
fn connect() -> Option<IpcStream> {
    for i in 0..10 {
        let path = format!(r"\\.\pipe\discord-ipc-{}", i);
        if let Ok(f) = std::fs::OpenOptions::new().read(true).write(true).open(&path) {
            return Some(f);
        }
    }
    None
}

// ── Frame IO ────────────────────────────────────────────────────────────────

#[cfg(any(unix, windows))]
fn write_frame(s: &mut IpcStream, op: u32, payload: &str) -> std::io::Result<()> {
    use std::io::Write;
    let b = payload.as_bytes();
    let mut hdr = [0u8; 8];
    hdr[..4].copy_from_slice(&op.to_le_bytes());
    hdr[4..].copy_from_slice(&(b.len() as u32).to_le_bytes());
    s.write_all(&hdr)?;
    s.write_all(b)?;
    s.flush()
}

#[cfg(any(unix, windows))]
fn read_frame_body(s: &mut IpcStream) -> std::io::Result<String> {
    use std::io::Read;
    let mut hdr = [0u8; 8];
    s.read_exact(&mut hdr)?;
    let len = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
    let mut body = vec![0u8; len];
    s.read_exact(&mut body)?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

#[cfg(any(unix, windows))]
fn send_activity(
    s: &mut IpcStream,
    title: &str,
    artist: &str,
    thumb: &str,
    paused: bool,
    nonce: u64,
) -> std::io::Result<()> {
    let pid = std::process::id();
    // Discord requires details to be at least 2 chars; fall back if empty.
    let raw_title = if title.is_empty() { "Unknown" } else { title };
    let t = esc(raw_title);
    let a = esc(artist);
    // One unobtrusive "using sunder" mention, on the artist line only.
    let state = if paused {
        "Paused".into()
    } else if artist.is_empty() {
        "Playing".into()
    } else {
        format!("by {a}")
    };
    let ts = if paused {
        String::new()
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!(r#","timestamps":{{"start":{now}}}"#)
    };
    // Discord accepts direct HTTPS URLs in `large_image` per the current
    // schema ("To use an external image via media proxy, specify the URL as
    // the field's value when sending"; Discord proxies and re-emits as
    // `mp:image_id` over the gateway). No pre-upload, bot, or Developer
    // Portal asset required for large_image. Source:
    // https://discord.com/developers/docs/events/gateway-events#activity-object-activity-asset-image
    //
    // Per-track behavior: use the YouTube thumbnail when present; fall back
    // to the repo's icon.png (raw.githubusercontent.com serves the file
    // directly) so the activity still shows the Sunder logo when the track
    // metadata lacks a thumbnail.
    let image_url = if !thumb.is_empty() {
        thumb.to_string()
    } else {
        "https://raw.githubusercontent.com/FrogSnot/Sunder/main/src-tauri/icons/icon.png".to_string()
    };
    // small_image is the uploaded `sunder-logo` asset (the only one we keep
    // in the Developer Portal; track_art and sunder-mark are gone).
    let assets = format!(
        r#","assets":{{"large_image":"{}","small_image":"mp:sunder-logo"}}"#,
        esc(&image_url)
    );
    write_frame(s, 1, &format!(
        r#"{{"cmd":"SET_ACTIVITY","args":{{"pid":{pid},"activity":{{"type":2,"details":"{t}","state":"{state}"{ts}{assets}}}}},"nonce":"{nonce}"}}"#
    ))
}

#[cfg(any(unix, windows))]
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

// ── Unsupported platform stub ───────────────────────────────────────────────

#[cfg(not(any(unix, windows)))]
fn run_loop(rx: mpsc::Receiver<PresenceCommand>, _enabled: Arc<RwLock<bool>>) {
    while rx.recv().is_ok() {}
}
