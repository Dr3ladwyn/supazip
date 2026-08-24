//! Application settings persisted as JSON (WS-F, milestone 0.5.0).
//!
//! Settings live at `dirs::config_local_dir()/supazip/settings.json`. The
//! struct is loaded once at startup and saved whenever the user clicks
//! "Save" in the settings window or closes it.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// User-facing theme choice persisted in `settings.json`.
///
/// `System` follows the OS preference via egui's `ThemePreference`.
/// Default is `Dark` so existing 1.0 installs keep the terminal palette
/// until the user opts into light or system.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    #[default]
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// `None` = system default language.
    pub language: Option<String>,
    /// Maximum archive size in bytes. Default 1 GiB.
    pub max_archive_size: u64,
    /// How many recent files to keep. Default 10.
    pub recent_files_limit: usize,
    /// Show the debug overlay in the GUI. Default false.
    pub show_debug_overlay: bool,
    /// Dark / light / follow OS. Missing key in old JSON → [`ThemePreference::Dark`].
    #[serde(default)]
    pub theme: ThemePreference,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: None,
            max_archive_size: 1024 * 1024 * 1024,
            recent_files_limit: 10,
            show_debug_overlay: false,
            theme: ThemePreference::Dark,
        }
    }
}

impl Settings {
    /// Where the settings file should live. Returns `None` when the
    /// platform has no suitable config directory (e.g. some sandboxed
    /// environments).
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_local_dir().map(|p| p.join("supazip").join("settings.json"))
    }

    /// Load settings from disk, falling back to defaults on any error.
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Persist settings to disk. Creates the parent directory if needed.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = Self::config_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let s = Settings::default();
        assert!(s.language.is_none());
        assert_eq!(s.max_archive_size, 1024 * 1024 * 1024);
        assert_eq!(s.recent_files_limit, 10);
        assert!(!s.show_debug_overlay);
        assert_eq!(s.theme, ThemePreference::Dark);
    }

    #[test]
    fn config_path_returns_some() {
        // On CI and dev machines `dirs::config_local_dir()` should resolve.
        let path = Settings::config_path();
        assert!(path.is_some(), "config_local_dir unavailable");
        let p = path.unwrap();
        assert!(p.ends_with("supazip/settings.json"), "got: {p:?}");
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        // Even if the file does not exist yet, `load` must return sane
        // defaults — it must not panic.
        let s = Settings::load();
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Override the config path for this test by writing and reading
        // manually (Settings::config_path is not injectable, so we test
        // the serialization logic directly).
        let s = Settings {
            language: Some("ru".into()),
            max_archive_size: 512 * 1024 * 1024,
            recent_files_limit: 5,
            show_debug_overlay: true,
            theme: ThemePreference::Light,
        };

        let json = serde_json::to_string_pretty(&s).expect("serialize");
        let path = dir.path().join("settings.json");
        std::fs::write(&path, &json).expect("write");

        let loaded: Settings =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
        assert_eq!(loaded, s);
    }

    #[test]
    fn load_ignores_malformed_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "{not valid json!!!").expect("write");
        // Simulate load logic: must fall back to default.
        let loaded: Settings = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn missing_theme_field_defaults_to_dark() {
        let json = r#"{
            "language": null,
            "max_archive_size": 1,
            "recent_files_limit": 10,
            "show_debug_overlay": false
        }"#;
        let loaded: Settings = serde_json::from_str(json).expect("parse legacy settings");
        assert_eq!(loaded.theme, ThemePreference::Dark);
    }
}
