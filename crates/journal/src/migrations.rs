//! Append-only schema migrations for the Journal.

use rusqlite_migration::{M, Migrations};

pub fn all() -> Migrations<'static> {
    Migrations::new(vec![M::up(
        "CREATE TABLE journal_entries(
             id TEXT PRIMARY KEY NOT NULL,
             page_date TEXT NOT NULL,
             body TEXT NOT NULL,
             written_at INTEGER NOT NULL,
             written_offset_min INTEGER NOT NULL,
             written_tz TEXT NOT NULL,
             edited_at INTEGER,
             word_count INTEGER NOT NULL,
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL,
             deleted_at INTEGER
         ) STRICT;
         CREATE INDEX journal_entries_page
             ON journal_entries(page_date, written_at) WHERE deleted_at IS NULL;",
    )])
}
