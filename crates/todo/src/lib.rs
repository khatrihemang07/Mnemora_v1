//! The Todo Module: Tasks in Lists, with Comments. Rules and storage live here; the app crate
//! only exposes them as commands.

#![forbid(unsafe_code)]

mod error;
mod migrations;
mod repo;
mod service;
mod task;

use mnemora_core::module::Module;
use mnemora_core::settings::SettingsRegistry;
use rusqlite_migration::Migrations;

pub use error::TodoError;
pub use service::{complete_task, create_task, inbox, reopen_task};
pub use task::{InboxView, MAX_TITLE_CHARS, Task, TaskState};

/// The Todo Module.
pub struct TodoModule;

/// Module id: also the migration namespace and the change-log module name.
pub const MODULE_ID: &str = "todo";

impl Module for TodoModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn migrations(&self) -> Migrations<'static> {
        migrations::all()
    }

    // Todo's Preferences arrive with the slices that use them.
    fn register_settings(&self, _reg: &mut SettingsRegistry) {}
}
