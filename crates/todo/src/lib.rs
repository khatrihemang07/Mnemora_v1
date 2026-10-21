//! The Todo Module: Tasks in Lists, with Comments. Rules and storage live here; the app crate
//! only exposes them as commands.

#![forbid(unsafe_code)]

mod capture;
mod comment;
mod comment_repo;
mod comment_service;
mod error;
mod history;
mod list;
mod list_repo;
mod list_service;
mod migrations;
mod parse;
mod recurrence;
mod repeat;
mod repo;
mod schedule;
mod service;
mod task;
mod task_service;
mod when;

use mnemora_core::db::migrate::MigrationReport;
use mnemora_core::error::AppResult;
use mnemora_core::module::Module;
use mnemora_core::settings::{SettingsRegistry, validate};
use rusqlite::Connection;
use rusqlite_migration::Migrations;
use serde_json::json;

pub use capture::{QuickEdit, parse_text, quick_edit};
pub use comment::{Comment, MAX_COMMENT_CHARS};
pub use comment_service::{add_comment, comments, delete_comment, edit_comment, restore_comment};
pub use error::TodoError;
pub use history::{HistoryLine, task_history};
pub use list::{
    AllGroup, AllView, DeleteChoice, ListDeletion, ListEntry, ListView, MAX_LIST_NAME_CHARS,
    SidebarView, TodoList,
};
pub use list_service::{
    all, create_list, delete_list, list_view, rename_list, restore_list, set_list_emoji, sidebar,
};
pub use parse::{ParseAlternative, ParseRequest, ParseSpan, Parsed, TextRange};
pub use recurrence::{MAX_EVERY, RepeatRule};
pub use repeat::{Completion, DoneDay, DoneView, done, skip_task};
pub use schedule::{
    DayCount, DueChange, OverdueMove, TodayView, UpcomingDay, UpcomingView, day_counts,
    move_overdue, restore_when, set_when, today, upcoming,
};
pub use service::{complete_task, create_task, inbox, move_task, reopen_task};
pub use task::{InboxView, MAX_TITLE_CHARS, NewTaskDetails, Task, TaskState};
pub use task_service::{
    Purged, delete_task, purge_deleted, restore_task, set_priority, task, update_task, wont_do,
};
pub use when::{LocalNow, NewWhen};

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

    fn register_settings(&self, reg: &mut SettingsRegistry) {
        // "Recognise dates in titles": turns the capture box's reading of typed text off.
        reg.register(MODULE_ID, "parse_dates", json!(true), validate::boolean());
    }

    fn on_startup(&self, conn: &mut Connection, _report: &MigrationReport) -> AppResult<()> {
        let purged = purge_deleted(conn)?;
        if purged.tasks + purged.lists + purged.comments > 0 {
            tracing::info!(
                tasks = purged.tasks,
                lists = purged.lists,
                comments = purged.comments,
                "purged Todo items deleted over 30 days ago"
            );
        }
        Ok(())
    }
}
