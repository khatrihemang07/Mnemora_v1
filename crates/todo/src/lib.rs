//! The Todo Module: Tasks in Lists, with Comments. Rules and storage live here; the app crate
//! only exposes them as commands.

#![forbid(unsafe_code)]

mod arrange;
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
mod order;
mod parse;
mod quick_add;
mod recurrence;
mod reorder;
mod repeat;
mod repo;
mod schedule;
mod search;
mod search_repo;
mod service;
mod steps;
mod sync;
mod task;
mod task_service;
mod upgrade;
mod when;
mod widget;
mod widget_text;

use mnemora_core::db::migrate::MigrationReport;
use mnemora_core::device::DeviceStore;
use mnemora_core::error::AppResult;
use mnemora_core::module::Module;
use mnemora_core::settings::{SettingsRegistry, validate};
use mnemora_core::syncable::{Applied, RemoteChange, SyncEntity};
use rusqlite::Connection;
use rusqlite_migration::Migrations;
use serde_json::json;

pub use arrange::{
    DoneInLists, DueBucket, GroupBy, GroupKey, ListSort, TaskGroup, TaskSort, ViewPrefs,
};
pub use capture::{QuickEdit, parse_text, quick_edit};
pub use comment::{Comment, MAX_COMMENT_CHARS};
pub use comment_service::{add_comment, comments, delete_comment, edit_comment, restore_comment};
pub use error::TodoError;
pub use history::{HistoryLine, task_history};
pub use list::{
    AllView, DeleteChoice, ListDeletion, ListDone, ListEntry, ListView, MAX_LIST_NAME_CHARS,
    SidebarView, TodoList,
};
pub use list_service::{
    all, create_list, delete_list, list_view, rename_list, restore_list, set_list_emoji,
    set_list_group, set_list_sort, sidebar,
};
pub use parse::{ParseAlternative, ParseRequest, ParseSpan, Parsed, TextRange};
pub use quick_add::{
    Added, Base, CaptureContext, Chip, Fields, Hand, Marked, Place, QuickRequest, Reading,
    add_quick, capture_context, read_quick, resolve_fields,
};
pub use recurrence::{MAX_EVERY, RepeatRule};
pub use reorder::{reorder_list, reorder_task};
pub use repeat::{Completion, DoneDay, DoneView, done, skip_task};
pub use schedule::{
    DayCount, DueChange, OverdueMove, TodayView, TodoCounts, UpcomingView, counts, day_counts,
    move_overdue, restore_when, set_when, today, upcoming,
};
pub use search::{SnippetSource, TaskHit, TaskSearchResults, search};
pub use service::{complete_task, create_task, inbox, move_task, reopen_task};
pub use steps::{open_steps, tick_all_steps};
pub use task::{InboxView, MAX_TITLE_CHARS, NewTaskDetails, Task, TaskState};
pub use task_service::{delete_task, restore_task, set_priority, task, update_task, wont_do};
pub use when::{LocalNow, NewWhen};
pub use widget::{
    EmptyState, HeaderCount, Tone, WidgetGroup, WidgetList, WidgetLists, WidgetRow, WidgetSnapshot,
    WidgetView, lists as widget_lists, settings_affect_widget, snapshot,
};
pub use widget_text::{completed_message, completed_repeat_message, plain_title};

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
        // How each view groups and sorts, and what Done Tasks do in a List. The first value of
        // each is the default.
        for (key, allowed) in [
            arrange::GROUP_TODAY,
            arrange::GROUP_UPCOMING,
            arrange::GROUP_INBOX,
            arrange::GROUP_ALL,
            arrange::INBOX_SORT,
            arrange::LISTS_SORT,
            arrange::DONE_IN_LISTS,
        ] {
            reg.register(MODULE_ID, key, json!(allowed[0]), validate::one_of(allowed));
        }
    }

    fn sync_entities(&self) -> &'static [SyncEntity] {
        sync::ENTITIES
    }

    fn apply_remote(&self, conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
        match change.entity {
            service::ENTITY => sync::apply(conn, change),
            _ => Err(TodoError::Validation("Todo does not sync that kind of record").into()),
        }
    }

    fn on_startup(
        &self,
        conn: &mut Connection,
        device: &DeviceStore,
        report: &MigrationReport,
    ) -> AppResult<()> {
        upgrade::append_todo_tab(conn, device, report)?;
        Ok(())
    }
}
