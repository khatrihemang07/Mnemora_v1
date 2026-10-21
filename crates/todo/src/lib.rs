//! The Todo Module: Tasks in Lists, with Comments. Rules and storage live here; the app crate
//! only exposes them as commands.

#![forbid(unsafe_code)]

mod error;
mod list;
mod list_repo;
mod list_service;
mod migrations;
mod repo;
mod service;
mod task;

use mnemora_core::module::Module;
use mnemora_core::settings::SettingsRegistry;
use rusqlite_migration::Migrations;

pub use error::TodoError;
pub use list::{
    AllGroup, AllView, DeleteChoice, ListDeletion, ListEntry, ListView, MAX_LIST_NAME_CHARS,
    SidebarView, TodoList,
};
pub use list_service::{
    all, create_list, delete_list, list_view, rename_list, restore_list, set_list_emoji, sidebar,
};
pub use service::{complete_task, create_task, inbox, move_task, reopen_task};
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
