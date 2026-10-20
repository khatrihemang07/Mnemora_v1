//! IPC commands. Thin: validate nothing here that core already validates.

use mnemora_core::db::backup;
use mnemora_core::device::DeviceSettings;
use mnemora_core::env::Environment;
use mnemora_core::error::{AppError, ErrorCode};
use mnemora_core::logging::normalize_level;
use mnemora_core::settings::{SettingsChange, SettingsSnapshot};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};
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

/// Changes the log level immediately and remembers it.
#[tauri::command]
#[specta::specta]
pub fn device_set_log_level(state: State<'_, AppState>, level: String) -> Result<(), AppError> {
    let level = normalize_level(&level)?;
    state.log.set_level(&level)?;
    state.device.update(|d| d.log_level = level)
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
pub fn open_log_dir(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    app.opener()
        .open_path(state.paths.log_dir.display().to_string(), None::<&str>)
        .map_err(|e| AppError::new(ErrorCode::Io, "Could not open the log folder").with_details(e))
}

/// First token of the OS real name, else the account name, else a friendly fallback.
fn os_first_name() -> String {
    let first = |s: String| s.split_whitespace().next().map(str::to_owned);
    whoami::realname()
        .ok()
        .and_then(first)
        .or_else(|| whoami::username().ok().and_then(first))
        .unwrap_or_else(|| "friend".to_owned())
}
