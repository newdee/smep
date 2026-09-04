//! User settings, kept in a small TOML file under the platform config dir.

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::theme::PreviewTheme;

/// Overrides the config directory (mostly for tests and portable setups).
pub const DIR_ENV: &str = "SMEP_CONFIG_DIR";

/// Which panes the window shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewMode {
    /// The Markdown source alone.
    Source,
    /// Source on the left, rendered preview on the right.
    #[default]
    Split,
    /// The rendered document alone.
    Rendered,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Split => "Split",
            Self::Rendered => "Rendered",
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub preview_theme: PreviewTheme,
    pub view_mode: ViewMode,
    /// Whether the menu bar in the title bar is shown.
    #[serde(default = "default_true")]
    pub menu_bar: bool,
    /// Where these settings live; `None` means they are never written.
    #[serde(skip)]
    pub path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preview_theme: PreviewTheme::default(),
            view_mode: ViewMode::default(),
            menu_bar: true,
            path: None,
        }
    }
}

impl Settings {
    /// The settings file: `$SMEP_CONFIG_DIR/settings.toml`, else the
    /// platform config dir plus `smep/settings.toml`.
    pub fn default_path() -> Option<PathBuf> {
        let dir = match std::env::var_os(DIR_ENV) {
            Some(dir) => PathBuf::from(dir),
            None => dirs::config_dir()?.join("smep"),
        };
        Some(dir.join("settings.toml"))
    }

    /// Read the settings at the default path. A missing or unreadable file
    /// gives the defaults; a malformed one is reported and also ignored.
    pub fn load() -> Self {
        match Self::default_path() {
            Some(path) => Self::load_from(path),
            None => Self::default(),
        }
    }

    pub fn load_from(path: PathBuf) -> Self {
        let mut settings = match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str::<Settings>(&text).unwrap_or_else(|err| {
                eprintln!("smep: ignoring {}: {err}", path.display());
                Settings::default()
            }),
            Err(_) => Settings::default(),
        };
        settings.path = Some(path);
        settings
    }

    /// Write the settings to their path, creating the directory if needed.
    pub fn save(&self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string(self).map_err(io::Error::other)?;
        std::fs::write(path, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("smep-settings-{tag}-{}", std::process::id()))
            .join("settings.toml")
    }

    #[test]
    fn a_missing_file_gives_defaults_with_the_path_kept() {
        let path = temp_path("missing");
        let settings = Settings::load_from(path.clone());
        assert_eq!(settings.preview_theme, PreviewTheme::System);
        assert_eq!(settings.path, Some(path));
    }

    #[test]
    fn save_then_load_round_trips_and_creates_the_directory() {
        let path = temp_path("roundtrip");
        let settings = Settings {
            preview_theme: PreviewTheme::Night,
            path: Some(path.clone()),
            ..Settings::default()
        };
        settings.save().unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap().trim(),
            "preview_theme = \"night\"\nview_mode = \"split\"\nmenu_bar = true"
        );
        assert_eq!(Settings::load_from(path.clone()), settings);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn older_files_without_the_new_keys_still_load() {
        let settings: Settings = toml::from_str("preview_theme = \"sepia\"\n").unwrap();
        assert_eq!(settings.preview_theme, PreviewTheme::Sepia);
        assert_eq!(settings.view_mode, ViewMode::Split);
        assert!(settings.menu_bar);
        let settings: Settings =
            toml::from_str("view_mode = \"rendered\"\nmenu_bar = false\n").unwrap();
        assert_eq!(settings.view_mode, ViewMode::Rendered);
        assert!(!settings.menu_bar);
    }

    #[test]
    fn a_malformed_file_is_ignored() {
        let path = temp_path("malformed");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "preview_theme = 42\n").unwrap();
        let settings = Settings::load_from(path.clone());
        assert_eq!(settings.preview_theme, PreviewTheme::System);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn unknown_keys_are_tolerated() {
        let settings: Settings = toml::from_str("future_key = true\n").unwrap();
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn no_path_means_save_is_a_no_op() {
        Settings::default().save().unwrap();
    }
}
