//! Append-only schema migrations for the AI Module.

use rusqlite_migration::{M, Migrations};

pub fn all() -> Migrations<'static> {
    Migrations::new(vec![M::up(
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
    )])
}
