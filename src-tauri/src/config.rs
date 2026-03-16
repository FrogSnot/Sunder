use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Default volume (0.0 - 1.0)
    pub volume: f64,
    /// Whether the equalizer is enabled by default
    pub eq_enabled: bool,
    /// Equalizer gains per band (10 bands)
    pub eq_gains: Vec<f64>,
    /// Equalizer preset name
    pub eq_preset: String,
    /// Whether to auto-fetch lyrics when a track starts
    pub lyrics_auto_fetch: bool,
    /// Maximum search results to show
    pub search_limit: usize,
    /// Audio quality for yt-dlp (0 = best, 9 = worst)
    pub audio_quality: u8,
    /// Seek step in seconds (for arrow key seeking)
    pub seek_step_secs: f64,
    /// Volume step (for arrow key volume changes)
    pub volume_step: f64,
    /// Preferred subtitle language for lyrics fallback
    pub subtitle_lang: String,
    /// Prefetch ahead count
    pub prefetch_ahead: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            volume: 0.8,
            eq_enabled: false,
            eq_gains: vec![0.0; 10],
            eq_preset: "Flat".into(),
            lyrics_auto_fetch: false,
            search_limit: 10,
            audio_quality: 2,
            seek_step_secs: 5.0,
            volume_step: 0.05,
            subtitle_lang: "en".into(),
            prefetch_ahead: 2,
        }
    }
}

pub struct ConfigManager {
    config: RwLock<AppConfig>,
    path: PathBuf,
}

impl ConfigManager {
    pub fn new(data_dir: &std::path::Path) -> Self {
        let path = data_dir.join("config.json");
        let config = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
                Err(_) => AppConfig::default(),
            }
        } else {
            AppConfig::default()
        };

        let mgr = Self {
            config: RwLock::new(config),
            path,
        };
        mgr.save(); // Ensure file exists with defaults
        mgr
    }

    pub fn get(&self) -> AppConfig {
        self.config.read().unwrap().clone()
    }

    pub fn update(&self, new_config: AppConfig) {
        *self.config.write().unwrap() = new_config;
        self.save();
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&*self.config.read().unwrap()) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}
