use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub watch_folders: Vec<PathBuf>,
    pub manual_paths: Vec<PathBuf>,
    pub always_visible: bool,
    pub max_items: usize,
    pub locale_zh: bool,
    #[serde(default)]
    pub dismissed: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub pinned: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        let downloads = dirs::download_dir().unwrap_or_else(|| {
            PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("Downloads")
        });
        Self {
            watch_folders: vec![downloads],
            manual_paths: Vec::new(),
            always_visible: false,
            max_items: 24,
            locale_zh: true,
            dismissed: std::collections::HashMap::new(),
            pinned: Vec::new(),
        }
    }
}

pub fn settings_path() -> PathBuf {
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Clothesline-rust");
    let _ = std::fs::create_dir_all(&base);
    base.join("settings.json")
}

pub fn load() -> Settings {
    let path = settings_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(s) = serde_json::from_str(&data) {
            return s;
        }
    }
    Settings::default()
}

pub fn save(settings: &Settings) {
    let path = settings_path();
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(path, json);
    }
}

pub fn path_to_key(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
