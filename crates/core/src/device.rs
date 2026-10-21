//! Device-only values stored in `device.toml` (never synced, never in the database).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::{AppError, AppResult, ErrorCode};

/// Last known window geometry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct WindowState {
    /// Inner width in logical pixels.
    pub w: u32,
    /// Inner height in logical pixels.
    pub h: u32,
    /// Left edge, if ever saved.
    pub x: Option<i32>,
    /// Top edge, if ever saved.
    pub y: Option<i32>,
    /// Whether the window was maximized.
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            w: 1200,
            h: 780,
            x: None,
            y: None,
            maximized: false,
        }
    }
}

/// Sidebar layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct SidebarState {
    /// Expanded width in pixels (200 to 360).
    pub width: u32,
    /// Collapsed to the icon rail.
    pub rail: bool,
    /// The Todo views and Lists under "Todo" are unfolded. Entering and leaving Todo set it;
    /// clicking the Todo row or its chevron flips it; it is restored as left.
    pub todo_open: bool,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            width: 248,
            rail: false,
            todo_open: false,
        }
    }
}

/// Everything stored per device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct DeviceSettings {
    /// Custom data folder; `None` means the platform default.
    pub data_dir: Option<String>,
    /// Window geometry.
    pub window: WindowState,
    /// Sidebar layout.
    pub sidebar: SidebarState,
    /// `tracing` level: error, warn, info, debug or trace.
    pub log_level: String,
    /// Spellcheck in every text editor. Per device: input methods and dictionaries differ.
    pub spellcheck: bool,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            data_dir: None,
            window: WindowState::default(),
            sidebar: SidebarState::default(),
            log_level: "info".to_owned(),
            spellcheck: true,
        }
    }
}

impl DeviceSettings {
    /// Loads from `path`. A missing file yields defaults silently; a corrupt one is
    /// logged and also yields defaults, so a bad edit can never block startup.
    pub fn load(path: &Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "device settings unreadable, using defaults");
                return Self::default();
            }
        };
        toml::from_str(&text).unwrap_or_else(|e| {
            tracing::warn!(path = %path.display(), error = %e, "device settings corrupt, using defaults");
            Self::default()
        })
    }

    /// Saves atomically (temp file in the same directory, then rename).
    pub fn save(&self, path: &Path) -> AppResult<()> {
        let text = toml::to_string_pretty(self).map_err(|e| {
            AppError::new(ErrorCode::Internal, "Could not encode device settings").with_details(e)
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Same directory as the target so the rename never crosses filesystems.
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// The data-dir override as a path.
    pub fn data_dir_override(&self) -> Option<PathBuf> {
        self.data_dir
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let s = DeviceSettings::load(&dir.path().join("device.toml"));
        assert_eq!(s, DeviceSettings::default());
        assert_eq!(s.log_level, "info");
        assert!(s.spellcheck, "spellcheck is on by default");
        assert_eq!((s.window.w, s.window.h), (1200, 780));
    }

    #[test]
    fn save_then_load_round_trips_and_creates_parents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config/device.toml");
        let mut s = DeviceSettings::default();
        s.sidebar.width = 300;
        s.sidebar.rail = true;
        s.sidebar.todo_open = true;
        s.window = WindowState {
            w: 900,
            h: 700,
            x: Some(-20),
            y: Some(40),
            maximized: true,
        };
        s.data_dir = Some("/somewhere".into());
        s.save(&path).unwrap();
        assert_eq!(DeviceSettings::load(&path), s);
        assert_eq!(s.data_dir_override(), Some(PathBuf::from("/somewhere")));
    }

    #[test]
    fn partial_file_fills_missing_fields_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("device.toml");
        std::fs::write(&path, "log_level = \"debug\"\n[sidebar]\nrail = true\n").unwrap();
        let s = DeviceSettings::load(&path);
        assert_eq!(s.log_level, "debug");
        assert!(s.sidebar.rail);
        assert_eq!(s.sidebar.width, SidebarState::default().width);
        assert!(
            !s.sidebar.todo_open,
            "a file from before the fold state keeps it folded"
        );
        assert_eq!(s.window, WindowState::default());
        assert!(s.spellcheck, "a file from before the flag keeps it on");
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("device.toml");
        std::fs::write(&path, "this is = = not toml [[[").unwrap();
        assert_eq!(DeviceSettings::load(&path), DeviceSettings::default());
    }

    #[test]
    fn save_leaves_no_temp_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("device.toml");
        DeviceSettings::default().save(&path).unwrap();
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["device.toml"]);
    }

    #[test]
    fn empty_data_dir_string_means_no_override() {
        let s = DeviceSettings {
            data_dir: Some(String::new()),
            ..DeviceSettings::default()
        };
        assert_eq!(s.data_dir_override(), None);
    }
}
