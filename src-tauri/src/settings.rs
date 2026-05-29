//! Persistent user settings, stored as JSON in the platform config dir.
//!
//! Mirrors the `@AppStorage`-backed settings in the Swift app (connection,
//! refresh interval, theme, status-bar detail toggle, update checks, etc.).

use cch_core::CchConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEFAULT_BASE_URL: &str = "http://localhost:3000";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub cch_base_url: String,
    pub cch_token: String,
    pub cch_env_path: String,
    pub active_session_user_filter: String,
    pub refresh_interval: f64,
    pub show_status_bar_details: bool,
    pub check_for_updates_enabled: bool,
    pub selected_theme: String,
    pub leaderboard_period: String,
    pub leaderboard_scope: String,
    pub launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cch_base_url: DEFAULT_BASE_URL.to_string(),
            cch_token: String::new(),
            cch_env_path: String::new(),
            active_session_user_filter: String::new(),
            refresh_interval: 15.0,
            show_status_bar_details: true,
            check_for_updates_enabled: true,
            selected_theme: "liquidGlass".to_string(),
            leaderboard_period: "daily".to_string(),
            leaderboard_scope: "user".to_string(),
            launch_at_login: false,
        }
    }
}

impl Settings {
    pub fn config(&self) -> CchConfig {
        CchConfig {
            base_url: self.cch_base_url.clone(),
            token: self.cch_token.clone(),
            env_path: self.cch_env_path.clone(),
        }
    }

    fn config_path() -> PathBuf {
        let mut dir = config_dir();
        dir.push("CCHBar");
        let _ = std::fs::create_dir_all(&dir);
        dir.push("settings.json");
        dir
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self).unwrap_or_else(|_| "".to_string());
        std::fs::write(path, json)
    }
}

fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata);
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join("Library").join("Application Support");
        }
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config");
    }
    std::env::temp_dir()
}
