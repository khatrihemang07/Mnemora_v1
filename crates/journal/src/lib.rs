//! The Journal Module: diary Pages of Entries. Rules and storage live here; the app crate only
//! exposes them as commands.

#![forbid(unsafe_code)]

mod entry;
mod error;
mod migrations;
mod repo;
mod search;
mod service;
mod snippet;
mod today;
mod upgrade;

use mnemora_core::db::migrate::MigrationReport;
use mnemora_core::error::AppResult;
use mnemora_core::module::Module;
use mnemora_core::settings::{SettingsRegistry, validate};
use rusqlite::Connection;
use rusqlite_migration::Migrations;
use serde_json::json;

pub use entry::{Entry, NewEntry, Page, PageDirection};
pub use error::JournalError;
pub use mnemora_core::search::Span;
pub use search::{Hit, SearchResults, search};
pub use service::{
    create_entry, delete_entry, list_pages, month_days, move_entry, purge_deleted, restore_entry,
    toggle_task_line, update_entry,
};
pub use today::{TodayLatest, TodaySummary, today};

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

    fn register_settings(&self, reg: &mut SettingsRegistry) {
        reg.register(
            MODULE_ID,
            "font",
            json!("serif"),
            validate::one_of(&["serif", "sans"]),
        );
        reg.register(
            MODULE_ID,
            "width",
            json!("default"),
            validate::one_of(&["narrow", "default", "wide"]),
        );
        reg.register(MODULE_ID, "page_summary", json!(true), validate::boolean());
    }

    fn on_startup(&self, conn: &mut Connection, report: &MigrationReport) -> AppResult<()> {
        upgrade::keep_existing_phone_tabs(conn, report)?;
        match purge_deleted(conn)? {
            0 => {}
            n => tracing::info!(
                entries = n,
                "purged Journal Entries deleted over 30 days ago"
            ),
        }
        Ok(())
    }
}
