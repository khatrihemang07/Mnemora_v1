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
mod sync;
mod today;
mod upgrade;

use mnemora_core::db::migrate::MigrationReport;
use mnemora_core::device::DeviceStore;
use mnemora_core::error::AppResult;
use mnemora_core::module::Module;
use mnemora_core::settings::{SettingsRegistry, validate};
use mnemora_core::syncable::{Applied, Holdings, RemoteChange, SyncEntity};
use rusqlite::Connection;
use rusqlite_migration::Migrations;
use serde_json::json;

pub use entry::{Entry, NewEntry, Page, PageDirection};
pub use error::JournalError;
pub use mnemora_core::search::Span;
pub use search::{Hit, SearchResults, search};
pub use service::{
    create_entry, delete_entry, list_pages, month_days, move_entry, restore_entry,
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

    fn sync_entities(&self) -> &'static [SyncEntity] {
        sync::ENTITIES
    }

    fn holdings(&self, conn: &Connection) -> AppResult<Holdings> {
        let entries: u32 = conn.query_row(
            "SELECT COUNT(*) FROM journal_entries WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        Ok(Holdings {
            entries,
            ..Holdings::default()
        })
    }

    fn backfill_log(&self, conn: &Connection, _settings: &SettingsRegistry) -> AppResult<()> {
        sync::backfill(conn)
    }

    fn apply_remote(&self, conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
        match change.entity {
            service::ENTITY => sync::apply(conn, change),
            _ => Err(JournalError::Validation("Journal does not sync that kind of record").into()),
        }
    }

    fn on_startup(
        &self,
        _conn: &mut Connection,
        device: &DeviceStore,
        report: &MigrationReport,
    ) -> AppResult<()> {
        upgrade::keep_existing_phone_tabs(device, report)?;
        Ok(())
    }
}
