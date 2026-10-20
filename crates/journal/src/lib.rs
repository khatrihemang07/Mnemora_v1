//! The Journal Module: diary Pages of Entries. Rules and storage live here; the app crate only
//! exposes them as commands.

#![forbid(unsafe_code)]

mod entry;
mod migrations;
mod repo;
mod service;
mod words;

use mnemora_core::module::Module;
use mnemora_core::settings::SettingsRegistry;
use rusqlite_migration::Migrations;

pub use entry::{Entry, NewEntry, Page};
pub use service::{create_entry, list_pages, update_entry};

/// The Journal Module.
pub struct JournalModule;

/// Module id: also the migration namespace and the change-log module name.
pub const MODULE_ID: &str = "journal";

impl Module for JournalModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn migrations(&self) -> Migrations<'static> {
        migrations::all()
    }

    fn register_settings(&self, _reg: &mut SettingsRegistry) {
        // Journal Preferences arrive with its Settings section.
    }
}
