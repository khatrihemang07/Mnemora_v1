//! Append-only schema migrations for the Journal.

use rusqlite_migration::{M, Migrations};

pub fn all() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
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
        ),
        // Search index. External content: the text stays in `journal_entries`, triggers keep the
        // index in step. Deleted Entries stay indexed (Undo); queries filter on `deleted_at`.
        // The vocabulary table lists the distinct words, for fuzzy matching.
        M::up(
            "CREATE VIRTUAL TABLE journal_fts USING fts5(
                 body, content='journal_entries', content_rowid='rowid',
                 tokenize='unicode61 remove_diacritics 2', prefix='2 3'
             );
             CREATE VIRTUAL TABLE journal_fts_vocab USING fts5vocab('journal_fts', 'row');
             CREATE TRIGGER journal_fts_insert AFTER INSERT ON journal_entries BEGIN
                 INSERT INTO journal_fts(rowid, body) VALUES (new.rowid, new.body);
             END;
             CREATE TRIGGER journal_fts_delete AFTER DELETE ON journal_entries BEGIN
                 INSERT INTO journal_fts(journal_fts, rowid, body)
                 VALUES ('delete', old.rowid, old.body);
             END;
             CREATE TRIGGER journal_fts_update AFTER UPDATE OF body ON journal_entries BEGIN
                 INSERT INTO journal_fts(journal_fts, rowid, body)
                 VALUES ('delete', old.rowid, old.body);
                 INSERT INTO journal_fts(rowid, body) VALUES (new.rowid, new.body);
             END;
             INSERT INTO journal_fts(journal_fts) VALUES ('rebuild');",
        ),
    ])
}
