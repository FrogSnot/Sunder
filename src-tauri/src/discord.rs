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
                let handshake = format!(r#"{{"v":1,"client_id":"{APPLICATION_ID}"}}"#);
                if write_frame(s, 0, &handshake).is_err() || read_frame(s).is_err() {
                    stream = None;
                    continue;
                }
            } else {
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
            Ok(()) => { let _ = read_frame(s); }
            Err(_) => { stream = None; }
        }
    }
}

// ── Connection ──────────────────────────────────────────────────────────────

#[cfg(unix)]
fn connect() -> Option<IpcStream> {
    use std::time::Duration;

    let dirs: Vec<String> = [
        std::env::var("XDG_RUNTIME_DIR").ok(),
        std::env::var("TMPDIR").ok(),
        Some("/tmp".into()),
    ]
    .into_iter()
    .flatten()
    .collect();

    for dir in &dirs {
        for i in 0..10 {
            if let Ok(s) = IpcStream::connect(format!("{dir}/discord-ipc-{i}")) {
                s.set_read_timeout(Some(Duration::from_secs(5))).ok();
                return Some(s);
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
fn read_frame(s: &mut IpcStream) -> std::io::Result<()> {
    use std::io::Read;
    let mut hdr = [0u8; 8];
    s.read_exact(&mut hdr)?;
    let len = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
    let mut body = vec![0u8; len];
    s.read_exact(&mut body)?;
    Ok(())
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
    let t = esc(title);
    let a = esc(artist);
    let state = if paused { "Paused".into() } else { format!("by {a}") };
    let ts = if paused {
        String::new()
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!(r#","timestamps":{{"start":{now}}}"#)
    };
    let img = if thumb.is_empty() {
        String::new()
    } else {
        format!(r#","assets":{{"large_image":"{}","large_text":"Sunder"}}"#, esc(thumb))
    };
    write_frame(s, 1, &format!(
        r#"{{"cmd":"SET_ACTIVITY","args":{{"pid":{pid},"activity":{{"details":"{t}","state":"{state}"{img}{ts}}}}},"nonce":"{nonce}"}}"#
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
