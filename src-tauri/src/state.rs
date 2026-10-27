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
    /// What lets Sync's network run: in front or flushing, and the kind of network (#74).
    pub power: crate::sync_power::Power,
    /// The app opens a real port and announces itself when Sync is on. False in tests, which
    /// start their own network on loopback.
    pub live_network: bool,
    /// Every Module, for the Sync engine to reach through the `Module` contract.
    pub modules: Modules,
    /// The in-process event bus Sync publishes on.
    pub bus: EventBus,
    /// The AI Module's Provider and the Conversations with a Run answering in them.
    pub ai: crate::ai::AiRuntime,
}
