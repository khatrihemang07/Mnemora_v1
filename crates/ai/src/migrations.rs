//! Append-only schema migrations for the AI Module.

use rusqlite_migration::{M, Migrations};

pub fn all() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
            // Entries are insert-only (ADR 0019): the tree is never edited, so there is no updated_at
            // and no deleted_at on them. Conversations are soft-deleted and may be renamed.
            "CREATE TABLE ai_conversations(
                 id TEXT PRIMARY KEY NOT NULL,
                 title TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 deleted_at INTEGER
             ) STRICT;
             CREATE TABLE ai_entries(
                 id TEXT PRIMARY KEY NOT NULL,
                 conversation_id TEXT NOT NULL REFERENCES ai_conversations(id),
                 parent_id TEXT REFERENCES ai_entries(id),
                 kind TEXT NOT NULL,
                 payload TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             ) STRICT;
             CREATE INDEX ai_entries_conversation ON ai_entries(conversation_id, created_at);",
        ),
        M::up(
            // Providers and their usage are per device and never synced (the spec), so they stay
            // out of the sync registry. Keys are not here at all: they live in an owner-only file.
            "CREATE TABLE ai_providers(
                 id TEXT PRIMARY KEY NOT NULL,
                 name TEXT NOT NULL,
                 kind TEXT NOT NULL,
                 address TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 deleted_at INTEGER
             ) STRICT;
             CREATE TABLE ai_model_usage(
                 provider_id TEXT NOT NULL REFERENCES ai_providers(id),
                 model TEXT NOT NULL,
                 uses INTEGER NOT NULL,
                 last_used_at INTEGER NOT NULL,
                 PRIMARY KEY(provider_id, model)
             ) STRICT;
             ALTER TABLE ai_conversations ADD COLUMN provider_id TEXT;
             ALTER TABLE ai_conversations ADD COLUMN provider_name TEXT;
             ALTER TABLE ai_conversations ADD COLUMN model TEXT;",
        ),
        M::up(
            // Saved questions keep their manual order as a fractional-index key (ADR 0012), and are
            // soft-deleted so that Undo can bring one back where it was.
            "CREATE TABLE ai_saved_questions(
                 id TEXT PRIMARY KEY NOT NULL,
                 text TEXT NOT NULL,
                 sort_key TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 deleted_at INTEGER
             ) STRICT;
             CREATE INDEX ai_saved_questions_order ON ai_saved_questions(sort_key);",
        ),
        M::up("ALTER TABLE ai_providers ADD COLUMN allow_paid INTEGER NOT NULL DEFAULT 0;"),
        // The entry each Conversation shows on this device. It is device-local (the spec): never
        // synced, so it has no change-log row and no sync entity. Missing means the newest entry.
        M::up(
            "CREATE TABLE ai_visible_branches(
                 conversation_id TEXT PRIMARY KEY NOT NULL REFERENCES ai_conversations(id),
                 entry_id TEXT NOT NULL REFERENCES ai_entries(id),
                 updated_at INTEGER NOT NULL
             ) STRICT;",
        ),
    ])
}
