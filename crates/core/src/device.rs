//! Device-only values stored in `device.toml` (never synced, never in the database).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
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
    /// clicking the Todo row or its chevron flips it; it is restored as left. None: never chosen,
    /// so the tree follows the page (unfolded when the app opens in Todo).
    pub todo_open: Option<bool>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            width: 248,
            rail: false,
            todo_open: None,
        }
    }
}

/// Look and feel (ADR 0005, amended): per device so a phone and a Mac can look different. Each
/// field is `None` until chosen on this device, and then the registered default of its Setting
/// applies; that is how an upgrade tells "never saved" from "saved the default". Values are
/// validated by the Setting's own validator on the way in, never here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct LookAndFeel {
    /// `system`, `light`, `dark` or `oled`.
    pub theme: Option<String>,
    /// `comfortable` or `compact`.
    pub density: Option<String>,
    /// `s`, `m` or `l`.
    pub text_size: Option<String>,
    /// Turn animations off.
    pub reduce_motion: Option<bool>,
    /// Ids of the phone tab bar's tabs, in order.
    pub phone_tabs: Option<Vec<String>>,
    /// The tabs were saved from Settings (an upgrade only pins them).
    pub phone_tabs_edited: Option<bool>,
}

/// The `core` keys that live in [`LookAndFeel`] instead of the `settings` table.
pub const LOOK_KEYS: [&str; 6] = [
    "theme",
    "density",
    "text_size",
    "reduce_motion",
    "phone_tabs",
    "phone_tabs_edited",
];

impl LookAndFeel {
    /// The saved value of `key` (a [`LOOK_KEYS`] name) as JSON, or `None` when never saved.
    pub fn get(&self, key: &str) -> Option<Value> {
        match key {
            "theme" => self.theme.as_ref().map(|v| json!(v)),
            "density" => self.density.as_ref().map(|v| json!(v)),
            "text_size" => self.text_size.as_ref().map(|v| json!(v)),
            "reduce_motion" => self.reduce_motion.map(|v| json!(v)),
            "phone_tabs" => self.phone_tabs.as_ref().map(|v| json!(v)),
            "phone_tabs_edited" => self.phone_tabs_edited.map(|v| json!(v)),
            _ => None,
        }
    }

    /// Saves `value` for `key`. False (and nothing changed) for an unknown key or a value of
    /// the wrong JSON type; the caller has already validated the value's content.
    pub fn set(&mut self, key: &str, value: &Value) -> bool {
        let text = || value.as_str().map(str::to_owned);
        match key {
            "theme" => text().map(|v| self.theme = Some(v)).is_some(),
            "density" => text().map(|v| self.density = Some(v)).is_some(),
            "text_size" => text().map(|v| self.text_size = Some(v)).is_some(),
            "reduce_motion" => value
                .as_bool()
                .map(|v| self.reduce_motion = Some(v))
                .is_some(),
            "phone_tabs_edited" => value
                .as_bool()
                .map(|v| self.phone_tabs_edited = Some(v))
                .is_some(),
            "phone_tabs" => value
                .as_array()
                .and_then(|a| {
                    a.iter()
                        .map(|t| t.as_str().map(str::to_owned))
                        .collect::<Option<Vec<_>>>()
                })
                .map(|v| self.phone_tabs = Some(v))
                .is_some(),
            _ => false,
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
    /// Theme, density, text size, reduce motion and phone tabs.
    pub look: LookAndFeel,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            data_dir: None,
            window: WindowState::default(),
            sidebar: SidebarState::default(),
            log_level: "info".to_owned(),
            spellcheck: true,
            look: LookAndFeel::default(),
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

/// `device.toml` held in memory, written through on every change.
pub struct DeviceStore {
    path: PathBuf,
    inner: Mutex<DeviceSettings>,
}

impl DeviceStore {
    /// Wraps already-loaded settings that live at `path`.
    pub fn new(path: PathBuf, settings: DeviceSettings) -> Self {
        Self {
            path,
            inner: Mutex::new(settings),
        }
    }

    /// A copy of the current settings.
    pub fn get(&self) -> DeviceSettings {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Applies `change` and saves atomically. The in-memory copy is updated even if the
    /// write fails, so the session stays consistent; the error is still reported.
    pub fn update(&self, change: impl FnOnce(&mut DeviceSettings)) -> AppResult<()> {
        let mut guard = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        change(&mut guard);
        guard.save(&self.path)
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
        s.sidebar.todo_open = Some(true);
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
        assert_eq!(
            s.sidebar.todo_open, None,
            "a file from before the fold state has no choice"
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

    #[test]
    fn look_and_feel_is_unset_until_chosen_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("device.toml");
        assert_eq!(DeviceSettings::default().look.get("theme"), None);
        let mut s = DeviceSettings::default();
        assert!(s.look.set("theme", &json!("oled")));
        assert!(s.look.set("reduce_motion", &json!(true)));
        assert!(s.look.set("phone_tabs", &json!(["home", "todo"])));
        assert!(!s.look.set("theme", &json!(3)), "wrong type is refused");
        assert!(!s.look.set("bogus", &json!("x")));
        s.save(&path).unwrap();
        let back = DeviceSettings::load(&path);
        assert_eq!(back.look.get("theme"), Some(json!("oled")));
        assert_eq!(back.look.get("reduce_motion"), Some(json!(true)));
        assert_eq!(back.look.get("phone_tabs"), Some(json!(["home", "todo"])));
        assert_eq!(back.look.get("density"), None, "unchosen stays unset");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("density"), "{text}");
    }

    #[test]
    fn a_file_from_before_look_and_feel_has_none_set() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("device.toml");
        std::fs::write(&path, "log_level = \"debug\"\n").unwrap();
        let s = DeviceSettings::load(&path);
        assert_eq!(s.look, LookAndFeel::default());
        for key in LOOK_KEYS {
            assert_eq!(s.look.get(key), None, "{key}");
        }
    }
}
