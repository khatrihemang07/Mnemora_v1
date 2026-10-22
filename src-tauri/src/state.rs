//! Shared application state managed by Tauri.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use mnemora_core::db::Connection;
use mnemora_core::device::DeviceStore;
use mnemora_core::env::Environment;
use mnemora_core::events::EventBus;
use mnemora_core::logging::LogHandle;
use mnemora_core::paths::AppPaths;
use mnemora_core::settings::SettingsStore;
use mnemora_sync::{Modules, SyncRuntime};

use crate::widget::signal::WidgetSignal;

/// Locks a mutex, recovering the data if another thread panicked while holding it.
/// A poisoned lock here would otherwise brick every command for the rest of the session.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
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
    /// Sync's services: nothing runs in them while Sync is off.
    pub sync: SyncRuntime,
    /// Every Module, for the Sync engine to reach through the `Module` contract. Read by the
    /// sync sessions (today only the `TestApp` cluster starts them; the live runtime comes next).
    #[cfg_attr(not(test), allow(dead_code))]
    pub modules: Modules,
    /// The in-process event bus Sync publishes on. Read like `modules`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub bus: EventBus,
}
