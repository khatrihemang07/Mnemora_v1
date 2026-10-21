//! IPC commands. Thin: validate nothing here that core already validates.

use mnemora_core::db::backup;
use mnemora_core::device::DeviceSettings;
use mnemora_core::env::Environment;
use mnemora_core::error::{AppError, ErrorCode};
use mnemora_core::logging::normalize_level;
use mnemora_core::settings::{SettingsChange, SettingsSnapshot};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_opener::OpenerExt;

use crate::state::{AppState, lock};

/// Sidebar width limits in pixels; below the minimum the UI snaps to the rail instead.
const SIDEBAR_MIN: u32 = 200;
const SIDEBAR_MAX: u32 = 360;
/// Cap on forwarded UI error text, so a runaway loop can't flood the log.
const UI_ERROR_MAX_CHARS: usize = 8_000;

/// Static facts about this running app.
#[derive(Debug, Clone, Serialize, Type)]
pub struct AppInfo {
    pub version: String,
    pub environment: Environment,
    pub data_dir: String,
    pub log_dir: String,
    /// First name from the OS account, for the greeting when no display name is set.
    pub os_first_name: String,
}

/// All Settings values plus the schema (defaults) of every key.
#[tauri::command]
#[specta::specta]
pub fn settings_get_all(state: State<'_, AppState>) -> Result<SettingsSnapshot, AppError> {
    state.settings.get_all(&lock(&state.db))
}

/// Saves a batch atomically. Emits `SettingsChangedEvent` on success.
#[tauri::command]
#[specta::specta]
pub fn settings_save(
    state: State<'_, AppState>,
    changes: Vec<SettingsChange>,
) -> Result<(), AppError> {
    state.settings.save_batch(&mut lock(&state.db), &changes)
}

/// Device-only settings (`device.toml`).
#[tauri::command]
#[specta::specta]
pub fn device_get(state: State<'_, AppState>) -> DeviceSettings {
    state.device.get()
}

/// Remembers the sidebar layout. Width is clamped to the allowed range.
#[tauri::command]
#[specta::specta]
pub fn device_set_sidebar(
    state: State<'_, AppState>,
    width: u32,
    rail: bool,
) -> Result<(), AppError> {
    state.device.update(|d| {
        d.sidebar.width = width.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
        d.sidebar.rail = rail;
    })
}

/// Remembers whether Todo's views and Lists are unfolded under "Todo" in the sidebar.
#[tauri::command]
#[specta::specta]
pub fn device_set_todo_open(state: State<'_, AppState>, open: bool) -> Result<(), AppError> {
    state.device.update(|d| d.sidebar.todo_open = open)
}

/// Changes the log level immediately and remembers it.
#[tauri::command]
#[specta::specta]
pub fn device_set_log_level(state: State<'_, AppState>, level: String) -> Result<(), AppError> {
    let level = normalize_level(&level)?;
    state.log.set_level(&level)?;
    state.device.update(|d| d.log_level = level)
}

/// Turns spellcheck in every text editor on or off, and remembers it on this device.
#[tauri::command]
#[specta::specta]
pub fn device_set_spellcheck(state: State<'_, AppState>, enabled: bool) -> Result<(), AppError> {
    state.device.update(|d| d.spellcheck = enabled)
}

/// Version, environment, folders and the OS account's first name.
#[tauri::command]
#[specta::specta]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        environment: state.env,
        data_dir: state.paths.data_dir.display().to_string(),
        log_dir: state.paths.log_dir.display().to_string(),
        os_first_name: os_first_name(),
    }
}

/// Forwards a frontend error into the Rust log file.
#[tauri::command]
#[specta::specta]
pub fn log_ui_error(message: String, stack: Option<String>) {
    let message: String = message.chars().take(UI_ERROR_MAX_CHARS).collect();
    let stack: String = stack
        .unwrap_or_default()
        .chars()
        .take(UI_ERROR_MAX_CHARS)
        .collect();
    tracing::error!(target: "ui", %message, %stack, "unhandled UI error");
}

/// Takes a database backup now and returns its path.
#[tauri::command]
#[specta::specta]
pub fn backup_now(state: State<'_, AppState>) -> Result<String, AppError> {
    let path = backup::backup_now(&lock(&state.db), &state.paths.backup_dir)?;
    tracing::info!(path = %path.display(), "manual backup created");
    Ok(path.display().to_string())
}

/// Opens the log folder in the system file manager.
#[tauri::command]
#[specta::specta]
pub fn open_log_dir<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    app.opener()
        .open_path(state.paths.log_dir.display().to_string(), None::<&str>)
        .map_err(|e| AppError::new(ErrorCode::Io, "Could not open the log folder").with_details(e))
}

/// Names the OS reports when there is no real account name (Android returns "Anonymous").
const PLACEHOLDER_NAMES: [&str; 2] = ["anonymous", "unknown"];

/// First token of the name, or `None` when it is empty or a placeholder.
fn sanitize_first_name(name: &str) -> Option<String> {
    let first = name.split_whitespace().next()?;
    let lower = first.to_lowercase();
    (!PLACEHOLDER_NAMES.contains(&lower.as_str())).then(|| first.to_owned())
}

/// First token of the OS real name, else the account name; empty on phones or placeholders so
/// the UI shows a plain greeting.
fn os_first_name() -> String {
    if cfg!(mobile) {
        return String::new();
    }
    whoami::realname()
        .ok()
        .and_then(|n| sanitize_first_name(&n))
        .or_else(|| {
            whoami::username()
                .ok()
                .and_then(|n| sanitize_first_name(&n))
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::sanitize_first_name;
    use crate::test_support::TestApp;

    fn save_theme(app: &TestApp, value: Value) -> Result<(), Value> {
        app.invoke(
            "settings_save",
            json!({ "changes": [{ "key": "core.theme", "value": value }] }),
        )
    }

    #[test]
    fn settings_get_all_returns_defaults_and_schema() {
        let app = TestApp::new();
        let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(snapshot["values"]["core.theme"], "system");
        assert!(!snapshot["schema"].as_array().unwrap().is_empty());
    }

    #[test]
    fn saved_setting_reads_back() {
        let app = TestApp::new();
        save_theme(&app, json!("dark")).unwrap();
        let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(snapshot["values"]["core.theme"], "dark");
    }

    #[test]
    fn invalid_setting_is_rejected_with_the_validation_code() {
        let app = TestApp::new();
        let err = save_theme(&app, json!("neon")).unwrap_err();
        assert_eq!(err["code"], "validation");
        let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(snapshot["values"]["core.theme"], "system");
    }

    #[test]
    fn unknown_setting_key_is_rejected() {
        let app = TestApp::new();
        let err = app
            .invoke::<()>(
                "settings_save",
                json!({ "changes": [{ "key": "core.nope", "value": 1 }] }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "unknown_key");
    }

    #[test]
    fn spellcheck_is_on_by_default_and_round_trips() {
        let app = TestApp::new();
        let device: Value = app.invoke("device_get", json!({})).unwrap();
        assert_eq!(device["spellcheck"], true);
        app.invoke::<()>("device_set_spellcheck", json!({ "enabled": false }))
            .unwrap();
        let device: Value = app.invoke("device_get", json!({})).unwrap();
        assert_eq!(device["spellcheck"], false);
        app.invoke::<()>("device_set_spellcheck", json!({ "enabled": true }))
            .unwrap();
        let device: Value = app.invoke("device_get", json!({})).unwrap();
        assert_eq!(device["spellcheck"], true);
    }

    #[test]
    fn the_todo_fold_state_is_a_device_setting_that_starts_folded() {
        let app = TestApp::new();
        let device: Value = app.invoke("device_get", json!({})).unwrap();
        assert_eq!(device["sidebar"]["todo_open"], false);
        app.invoke::<()>("device_set_todo_open", json!({ "open": true }))
            .unwrap();
        let device: Value = app.invoke("device_get", json!({})).unwrap();
        assert_eq!(device["sidebar"]["todo_open"], true);
        // The sidebar width and rail are saved separately and leave the fold state alone.
        app.invoke::<()>("device_set_sidebar", json!({ "width": 300, "rail": true }))
            .unwrap();
        let device: Value = app.invoke("device_get", json!({})).unwrap();
        assert_eq!(device["sidebar"]["todo_open"], true);
    }

    #[test]
    fn phone_tabs_default_to_home_and_journal() {
        let app = TestApp::new();
        let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(
            snapshot["values"]["core.phone_tabs"],
            json!(["home", "journal"])
        );
    }

    #[test]
    fn a_saved_phone_tabs_choice_is_kept() {
        let app = TestApp::new();
        app.invoke::<()>(
            "settings_save",
            json!({ "changes": [{ "key": "core.phone_tabs", "value": ["home"] }] }),
        )
        .unwrap();
        let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(snapshot["values"]["core.phone_tabs"], json!(["home"]));
    }

    #[test]
    fn keeps_a_real_first_name() {
        assert_eq!(
            sanitize_first_name("  Hemang Khatri"),
            Some("Hemang".into())
        );
    }

    #[test]
    fn root_and_user_may_be_real_names() {
        assert_eq!(sanitize_first_name("root"), Some("root".into()));
        assert_eq!(sanitize_first_name("User"), Some("User".into()));
    }

    #[test]
    fn placeholders_and_blanks_are_no_name() {
        for n in ["Anonymous", "UNKNOWN", "", "   "] {
            assert_eq!(sanitize_first_name(n), None, "{n:?}");
        }
    }
}
