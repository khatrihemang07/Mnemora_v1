//! The AI Module: Conversations, their entries and the Runs that answer questions. Read-only: it
//! never changes Journal or Task data. The loop and the Provider contract live in
//! `mnemora-harness` (ADR 0018); storage follows ADR 0019.

#![forbid(unsafe_code)]

mod conversations;
mod error;
mod keys;
mod markdown;
mod migrations;
mod prompt;
mod provider;
mod provider_store;
mod run;
mod saved;
mod saved_store;
mod service;
mod steering;
mod store;
mod sync;
mod sync_apply;
mod title;
mod tools;
mod types;

use mnemora_core::db::Connection;
use mnemora_core::error::AppResult;
use mnemora_core::module::Module;
use mnemora_core::settings::SettingsRegistry;
use mnemora_core::syncable::{Applied, RemoteChange, SyncEntity};
use rusqlite_migration::Migrations;

pub use conversations::{
    PAGE_MAX, delete as delete_conversation, list as list_conversations,
    markdown as conversation_markdown, question_text, rename as rename_conversation,
    restore as restore_conversation, set_leaf, show_branch,
};
pub use error::AiError;
pub use keys::KeyStore;
pub use mnemora_providers::WrapperProvider;
pub use prompt::Wording;
pub use provider::{
    ConnectionTest, KeyState, ModelList, ModelUsage, ModelUse, NewProvider, ProviderKind,
    ProviderRecord, ProviderUpdate, ProviderView, build,
};
pub use run::{Ask, Choice, Handles, Toolset, begin, execute, failure_message};
pub use saved::{
    create as create_saved, delete as delete_saved, list as list_saved, reorder as reorder_saved,
    restore as restore_saved, update as update_saved,
};
pub use service::{
    Prepared, clear_key, connection_test, create as create_provider, delete as delete_provider,
    list as list_providers, matching_models, prepare_run, reachable, set_key,
    update as update_provider, usage as model_usage,
};
pub use steering::{QueuedNote, Steering};
pub use title::{TITLE_CHARS, title_of, validate_local, validate_question};
pub use tools::{UNSEEN_CITATION, cited_ids};
pub use types::{
    CallRecord, ConversationPage, ConversationSummary, ConversationView, EntryKind, EntryPayload,
    EntryView, LocalContext, RunEvent, RunOutcome, SavedQuestion, Source,
};

/// Module id: the migration namespace.
pub const MODULE_ID: &str = "ai";

/// The AI Module. Off until the person turns it on (`default_enabled`).
pub struct AiModule;

impl Module for AiModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn default_enabled(&self) -> bool {
        false
    }

    fn migrations(&self) -> Migrations<'static> {
        migrations::all()
    }

    fn register_settings(&self, _reg: &mut SettingsRegistry) {}

    /// Conversations, their entries and Saved questions (ADR 0019). Providers, keys, usage and the
    /// visible Branch are never listed: they stay on the device.
    fn sync_entities(&self) -> &'static [SyncEntity] {
        &[
            sync::CONVERSATION_SCHEMA,
            sync::ENTRY_SCHEMA,
            sync::SAVED_SCHEMA,
        ]
    }

    fn apply_remote(&self, conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
        sync_apply::apply(conn, change)
    }

    fn backfill_log(&self, conn: &Connection, _settings: &SettingsRegistry) -> AppResult<()> {
        sync::backfill(conn)
    }
}

/// Fails with "not found" unless the Conversation exists and is not deleted.
pub fn ensure_conversation(conn: &Connection, id: &str) -> AppResult<()> {
    store::live_conversation(conn, id)?
        .map(|_| ())
        .ok_or_else(|| AiError::NotFound.into())
}

/// A Conversation with the entries of the Branch it shows on this device, oldest first, and each
/// question's place among its siblings. The other Branches are not loaded. `running` says whether
/// a Run is answering in it.
pub fn conversation(conn: &Connection, id: &str, running: bool) -> AppResult<ConversationView> {
    let header = store::live_conversation(conn, id)?.ok_or(AiError::NotFound)?;
    let mut entries = match store::visible(conn, id)? {
        Some(leaf) => store::branch_entries(conn, &leaf)?,
        None => Vec::new(),
    };
    let places = store::question_places(conn, id)?;
    for entry in &mut entries {
        if let Some((index, count)) = places.get(&entry.id) {
            entry.sibling_index = *index;
            entry.sibling_count = *count;
        }
    }
    Ok(ConversationView {
        id: id.to_owned(),
        title: header.title,
        created_at: header.created_at,
        updated_at: header.updated_at,
        provider_id: header.provider_id,
        provider_name: header.provider_name,
        model: header.model,
        running,
        queued: Vec::new(),
        entries,
    })
}
