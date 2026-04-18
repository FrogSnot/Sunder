use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;

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

        Self {
            config: RwLock::new(config),
            path,
        }
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
