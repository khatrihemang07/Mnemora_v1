//! The Sync Module: keeps a person's own devices in step, directly between them (ADR 0017). Off by
//! default and inert while off: its startup returns at once and nothing listens, advertises or
//! runs. It never imports the Journal or Todo.

#![forbid(unsafe_code)]

mod error;
mod migrations;
mod name;
mod peers;
mod runtime;
mod state;

use mnemora_core::module::Module;
use mnemora_core::settings::SettingsRegistry;
use rusqlite_migration::Migrations;

pub use error::SyncError;
pub use name::{MAX_NAME_CHARS, default_device_name, validate_name};
pub use peers::{forget_all, paired_count};
pub use runtime::{RuntimeProbe, SyncRuntime};
pub use state::{SyncOption, SyncState, current, rename, set_enabled, set_option};

/// The Sync Module.
pub struct SyncModule;

/// Module id: also the migration namespace and the change-log module name.
pub const MODULE_ID: &str = "sync";

impl Module for SyncModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    /// Off until the user turns it on, on each device separately.
    fn default_enabled(&self) -> bool {
        false
    }

    fn migrations(&self) -> Migrations<'static> {
        migrations::all()
    }

    /// Sync has no Preferences: its choices are Device settings (`DeviceSettings::sync`).
    fn register_settings(&self, _reg: &mut SettingsRegistry) {}

    // `on_startup` stays the trait's no-op: with Sync off there is nothing to start, and the app
    // shell starts `SyncRuntime` only when the Device setting says it is on.
}
