//! Shared application state managed by Tauri.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use mnemora_core::db::Connection;
use mnemora_core::device::DeviceSettings;
use mnemora_core::env::Environment;
use mnemora_core::error::AppResult;
use mnemora_core::logging::LogHandle;
use mnemora_core::paths::AppPaths;
use mnemora_core::settings::SettingsStore;

use crate::widget::signal::WidgetSignal;

/// Locks a mutex, recovering the data if another thread panicked while holding it.
/// A poisoned lock here would otherwise brick every command for the rest of the session.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
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
        lock(&self.inner).clone()
    }

    /// Applies `change` and saves atomically. The in-memory copy is updated even if the
    /// write fails, so the session stays consistent; the error is still reported.
    pub fn update(&self, change: impl FnOnce(&mut DeviceSettings)) -> AppResult<()> {
        let mut guard = lock(&self.inner);
        change(&mut guard);
        guard.save(&self.path)
    }
}

/// Everything commands need.
pub struct AppState {
    pub env: Environment,
    pub paths: AppPaths,
    pub db: Mutex<Connection>,
    pub settings: SettingsStore,
    pub device: Arc<DeviceStore>,
    pub log: LogHandle,
    /// Tells the Todo widget that Todo data changed.
    pub widgets: Arc<WidgetSignal>,
}
