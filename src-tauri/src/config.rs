use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub volume: f64,
    pub eq_enabled: bool,
    pub eq_gains: Vec<f64>,
    pub notifications_enabled: bool,
    pub discord_rpc_enabled: bool,
    pub saved_queue: Vec<String>,
    pub saved_queue_index: i64,
    pub repeat_mode: String,
    pub playback_speed: f64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            volume: 0.8,
            eq_enabled: false,
            eq_gains: vec![0.0; 10],
            notifications_enabled: true,
            discord_rpc_enabled: false,
            saved_queue: Vec::new(),
            saved_queue_index: -1,
            repeat_mode: "off".into(),
            playback_speed: 1.0,
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigPatch {
    pub volume: Option<f64>,
    pub eq_enabled: Option<bool>,
    pub eq_gains: Option<Vec<f64>>,
    pub notifications_enabled: Option<bool>,
    pub discord_rpc_enabled: Option<bool>,
    pub saved_queue: Option<Vec<String>>,
    pub saved_queue_index: Option<i64>,
    pub repeat_mode: Option<String>,
    pub playback_speed: Option<f64>,
}

impl ConfigPatch {
    fn apply(self, config: AppConfig) -> AppConfig {
        AppConfig {
            volume: self.volume.unwrap_or(config.volume),
            eq_enabled: self.eq_enabled.unwrap_or(config.eq_enabled),
            eq_gains: self.eq_gains.unwrap_or(config.eq_gains),
            notifications_enabled: self
                .notifications_enabled
                .unwrap_or(config.notifications_enabled),
            discord_rpc_enabled: self
                .discord_rpc_enabled
                .unwrap_or(config.discord_rpc_enabled),
            saved_queue: self.saved_queue.unwrap_or(config.saved_queue),
            saved_queue_index: self.saved_queue_index.unwrap_or(config.saved_queue_index),
            repeat_mode: self.repeat_mode.unwrap_or(config.repeat_mode),
            playback_speed: self.playback_speed.unwrap_or(config.playback_speed),
        }
    }
}

pub struct ConfigManager {
    config: RwLock<AppConfig>,
    path: PathBuf,
    // Serialize saves without holding up readers while the filesystem syncs.
    writer: Mutex<()>,
}

/// Name pattern of the temp files `replace_config` writes: `config.json.<pid>.<n>.tmp`.
const TEMP_PREFIX: &str = "config.json.";
const TEMP_SUFFIX: &str = ".tmp";

/// Delete temp files orphaned when a process died between writing the temp file
/// and renaming it into place. Best effort: a directory that cannot be read is
/// left alone and per-file errors are ignored. The match is deliberately narrow
/// (prefix AND suffix AND a non-empty middle) so unrelated files in the data
/// directory are never removed.
fn remove_orphaned_temp_configs(data_dir: &Path) {
    let Ok(entries) = fs::read_dir(data_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.len() > TEMP_PREFIX.len() + TEMP_SUFFIX.len()
            && name.starts_with(TEMP_PREFIX)
            && name.ends_with(TEMP_SUFFIX)
        {
            let _ = fs::remove_file(entry.path());
        }
    }
}

impl ConfigManager {
    pub fn new(data_dir: &Path) -> Self {
        // A previous process may have died between writing a temp file and the
        // rename, which would otherwise leave it orphaned forever.
        remove_orphaned_temp_configs(data_dir);
        let path = data_dir.join("config.json");
        let config = fs::read_to_string(&path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_default();

        Self {
            config: RwLock::new(config),
            path,
            writer: Mutex::new(()),
        }
    }

    pub fn get(&self) -> AppConfig {
        self.config.read().unwrap().clone()
    }

    /// An error means replacement failed. A warning means it succeeded but
    /// synchronization of the directory entry failed.
    pub fn update(
        &self,
        patch: ConfigPatch,
        after_save: impl FnOnce(),
    ) -> io::Result<Option<io::Error>> {
        self.update_with_sync(patch, after_save, sync_config_directory)
    }

    fn update_with_sync(
        &self,
        patch: ConfigPatch,
        after_save: impl FnOnce(),
        sync_directory: impl FnOnce(&Path) -> io::Result<()>,
    ) -> io::Result<Option<io::Error>> {
        let _writer = self.writer.lock().unwrap();
        let next = patch.apply(self.get());
        let contents = serde_json::to_vec_pretty(&next)?;
        replace_config(&self.path, |file| file.write_all(&contents))?;
        let warning = sync_directory(&self.path.with_file_name(".")).err();
        *self.config.write().unwrap() = next;
        // Apply live settings in the same order as their saved values.
        after_save();
        Ok(warning)
    }
}

#[cfg(unix)]
fn sync_config_directory(directory: &Path) -> io::Result<()> {
    let directory = File::open(directory)?;
    #[cfg(target_os = "macos")]
    {
        use std::os::fd::AsRawFd;
        // File::sync_all uses F_FULLFSYNC on macOS. Use fsync for the directory.
        // SAFETY: The open directory owns the descriptor for this call.
        if unsafe { libc::fsync(directory.as_raw_fd()) } == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    directory.sync_all()
}

#[cfg(not(unix))]
fn sync_config_directory(_directory: &Path) -> io::Result<()> {
    // There is no portable directory sync on Windows. Keep file sync + rename.
    Ok(())
}

fn replace_config(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let (temp, mut file) = loop {
        let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let temp = path.with_extension(format!("json.{}.{id}.tmp", std::process::id()));
        match options.open(&temp) {
            Ok(file) => break (temp, file),
            // A previous process with the same PID may have left a temp file.
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    };
    let result = write(&mut file).and_then(|()| file.sync_all());
    drop(file);
    let result = result.and_then(|()| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            static NEXT_DIR: AtomicU64 = AtomicU64::new(0);
            loop {
                let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir()
                    .join(format!("sunder-config-test-{}-{id}", std::process::id()));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(err) => panic!("could not create test directory: {err}"),
                }
            }
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn construction_removes_orphaned_temp_files() {
        let dir = TestDir::new();
        let config_path = dir.0.join("config.json");
        let contents = br#"{"volume":0.2}"#;
        fs::write(&config_path, contents).unwrap();
        let orphan = dir.0.join("config.json.12345.0.tmp");
        fs::write(&orphan, b"partial write").unwrap();
        // Not the temp-file shape: must survive the sweep.
        let unrelated = dir.0.join("config.json.bak");
        fs::write(&unrelated, b"keep me").unwrap();

        let manager = ConfigManager::new(&dir.0);

        assert!(!orphan.exists(), "orphaned temp file should be removed");
        assert!(unrelated.exists(), "unrelated file must be left alone");
        assert_eq!(fs::read(&config_path).unwrap(), contents);
        assert_eq!(manager.get().volume, 0.2);
    }

    #[test]
    fn loads_existing_config() {
        let dir = TestDir::new();
        fs::write(
            dir.0.join("config.json"),
            r#"{
                "volume": 0.2, "eq_enabled": true, "eq_gains": [1.0],
                "notifications_enabled": false, "discord_rpc_enabled": true,
                "saved_queue": ["track"], "saved_queue_index": 0,
                "repeat_mode": "queue", "playback_speed": 1.5
            }"#,
        )
        .unwrap();
        let config = ConfigManager::new(&dir.0).get();
        assert_eq!(config.volume, 0.2);
        assert!(config.eq_enabled);
        assert_eq!(config.eq_gains, [1.0]);
        assert!(!config.notifications_enabled);
        assert!(config.discord_rpc_enabled);
        assert_eq!(config.saved_queue, ["track"]);
        assert_eq!(config.saved_queue_index, 0);
        assert_eq!(config.repeat_mode, "queue");
        assert_eq!(config.playback_speed, 1.5);
    }

    #[test]
    fn defaults_missing_fields() {
        let dir = TestDir::new();
        assert_eq!(ConfigManager::new(&dir.0).get().volume, 0.8);
        fs::write(dir.0.join("config.json"), r#"{"volume":0.3}"#).unwrap();
        let config = ConfigManager::new(&dir.0).get();
        assert_eq!(config.volume, 0.3);
        assert_eq!(config.eq_gains, [0.0; 10]);
        assert!(config.notifications_enabled);
        assert!(!config.discord_rpc_enabled);
        assert!(config.saved_queue.is_empty());
        assert_eq!(config.saved_queue_index, -1);
        assert_eq!(config.repeat_mode, "off");
        assert_eq!(config.playback_speed, 1.0);
    }

    #[test]
    fn patches_preserve_unrelated_settings() {
        let dir = TestDir::new();
        let manager = ConfigManager::new(&dir.0);
        for json in [
            r#"{"volume":0.4,"eq_enabled":true,"playback_speed":1.5}"#,
            r#"{"discord_rpc_enabled":true}"#,
            r#"{"saved_queue":["track"],"saved_queue_index":0,"repeat_mode":"track"}"#,
            r#"{"volume":0.7,"notifications_enabled":false}"#,
        ] {
            manager
                .update(serde_json::from_str(json).unwrap(), || {})
                .unwrap();
        }
        let config = ConfigManager::new(&dir.0).get();
        assert_eq!(config.volume, 0.7);
        assert!(config.eq_enabled);
        assert!(!config.notifications_enabled);
        assert!(config.discord_rpc_enabled);
        assert_eq!(config.saved_queue, ["track"]);
        assert_eq!(config.saved_queue_index, 0);
        assert_eq!(config.repeat_mode, "track");
        assert_eq!(config.playback_speed, 1.5);
    }

    #[test]
    fn concurrent_patches_merge_before_persisting() {
        let dir = TestDir::new();
        let manager = ConfigManager::new(&dir.0);
        let start = Barrier::new(2);
        std::thread::scope(|scope| {
            for json in [r#"{"volume":0.3}"#, r#"{"discord_rpc_enabled":true}"#] {
                let manager = &manager;
                let start = &start;
                scope.spawn(move || {
                    start.wait();
                    manager
                        .update(serde_json::from_str(json).unwrap(), || {})
                        .unwrap();
                });
            }
        });
        for config in [manager.get(), ConfigManager::new(&dir.0).get()] {
            assert_eq!(config.volume, 0.3);
            assert!(config.discord_rpc_enabled);
        }
    }

    #[test]
    fn rapid_updates_persist_in_order() {
        let dir = TestDir::new();
        let manager = ConfigManager::new(&dir.0);
        for volume in [0.2, 0.7, 0.1, 0.8] {
            let patch = ConfigPatch {
                volume: Some(volume),
                ..ConfigPatch::default()
            };
            manager
                .update(patch, || {
                    assert_eq!(manager.get().volume, volume);
                    assert_eq!(ConfigManager::new(&dir.0).get().volume, volume);
                })
                .unwrap();
        }
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[test]
    fn replacement_keeps_old_config_visible_until_write_completes() {
        let dir = TestDir::new();
        let path = dir.0.join("config.json");
        let old = br#"{"volume":0.3}"#;
        fs::write(&path, old).unwrap();
        replace_config(&path, |file| {
            file.write_all(br#"{"volume":"#)?;
            assert_eq!(fs::read(&path).unwrap(), old);
            file.write_all(b"0.5}")
        })
        .unwrap();
        assert_eq!(ConfigManager::new(&dir.0).get().volume, 0.5);
    }

    #[test]
    fn partial_write_failure_keeps_old_config() {
        let dir = TestDir::new();
        let path = dir.0.join("config.json");
        let old = br#"{"volume":0.3}"#;
        fs::write(&path, old).unwrap();
        let error = replace_config(&path, |file| {
            file.write_all(b"{")?;
            Err(io::Error::other("injected write failure"))
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "injected write failure");
        assert_eq!(fs::read(&path).unwrap(), old);
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[test]
    fn failed_replacement_does_not_publish_update() {
        let dir = TestDir::new();
        let path = dir.0.join("config.json");
        fs::create_dir(&path).unwrap();
        let manager = ConfigManager::new(&dir.0);
        let result = manager.update(
            ConfigPatch {
                discord_rpc_enabled: Some(true),
                ..ConfigPatch::default()
            },
            || panic!("failed save applied a live change"),
        );
        assert!(result.is_err());
        assert!(path.is_dir());
        assert!(!manager.get().discord_rpc_enabled);
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[test]
    fn directory_sync_failure_keeps_committed_settings() {
        let dir = TestDir::new();
        fs::write(dir.0.join("config.json"), r#"{"volume":0.3}"#).unwrap();
        let manager = ConfigManager::new(&dir.0);
        let patch = serde_json::from_str(r#"{"discord_rpc_enabled":true}"#).unwrap();
        let mut applied = false;
        let warning = manager
            .update_with_sync(
                patch,
                || applied = true,
                |path| {
                    assert!(ConfigManager::new(path).get().discord_rpc_enabled);
                    Err(io::Error::other("injected directory sync failure"))
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(warning.to_string(), "injected directory sync failure");
        assert!(applied && manager.get().discord_rpc_enabled);
    }
}
