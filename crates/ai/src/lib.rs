//! The AI Module: Conversations, their entries and the Runs that answer questions. Read-only: it
//! never changes Journal or Task data. The loop and the Provider contract live in
//! `mnemora-harness` (ADR 0018); storage follows ADR 0019.

#![forbid(unsafe_code)]

mod error;
mod keys;
mod migrations;
mod prompt;
mod provider;
mod provider_store;
mod run;
mod service;
mod store;
mod title;
mod types;

use mnemora_core::db::Connection;
use mnemora_core::error::AppResult;
use mnemora_core::module::Module;
use mnemora_core::settings::SettingsRegistry;
use rusqlite_migration::Migrations;

pub use error::AiError;
pub use keys::KeyStore;
pub use provider::{
    ConnectionTest, KeyState, ModelList, ModelUsage, ModelUse, NewProvider, ProviderKind,
    ProviderRecord, ProviderUpdate, ProviderView, build,
};
pub use run::{Ask, Choice, begin, execute, failure_message};
pub use service::{
    Prepared, clear_key, connection_test, create as create_provider, delete as delete_provider,
    list as list_providers, matching_models, prepare_run, reachable, set_key,
    update as update_provider, usage as model_usage,
};
pub use title::{TITLE_CHARS, title_of, validate_local, validate_question};
pub use types::{
    ConversationView, EntryKind, EntryPayload, EntryView, LocalContext, RunEvent, RunOutcome,
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
}

/// Fails with "not found" unless the Conversation exists and is not deleted.
pub fn ensure_conversation(conn: &Connection, id: &str) -> AppResult<()> {
    store::live_conversation(conn, id)?
        .map(|_| ())
        .ok_or_else(|| AiError::NotFound.into())
}

/// A Conversation with every entry, oldest first. `running` says whether a Run is answering in it.
pub fn conversation(conn: &Connection, id: &str, running: bool) -> AppResult<ConversationView> {
    let header = store::live_conversation(conn, id)?.ok_or(AiError::NotFound)?;
    Ok(ConversationView {
        id: id.to_owned(),
        title: header.title,
        created_at: header.created_at,
        updated_at: header.updated_at,
        provider_name: header.provider_name,
        model: header.model,
        running,
        entries: store::entries(conn, id)?,
    })
}
