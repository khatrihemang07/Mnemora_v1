//! Append-only schema migrations for Todo. The first ones create the whole agreed shape (Lists,
//! Tasks, Comments and their search indexes) so later slices only add behaviour.

use rusqlite_migration::{M, Migrations};

pub fn all() -> Migrations<'static> {
    Migrations::new(vec![
        // Dates are `YYYY-MM-DD` and times `HH:MM`, both floating local. `sort_key` is text that
        // sorts bytewise (fractional-index keys); a Task's `list_id` of NULL is the Inbox.
        // `name_key` is the List name folded for case and accents, so names are unique among live
        // Lists. `understood` is JSON; `repeat_of` points an occurrence copy at its live Task.
        M::up(
            "CREATE TABLE todo_lists(
                 id TEXT PRIMARY KEY NOT NULL,
                 name TEXT NOT NULL,
                 name_key TEXT NOT NULL,
                 emoji TEXT,
                 sort_key TEXT NOT NULL,
                 task_sort TEXT NOT NULL DEFAULT 'manual',
                 group_by TEXT,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 deleted_at INTEGER
             ) STRICT;
             CREATE UNIQUE INDEX todo_lists_name
                 ON todo_lists(name_key) WHERE deleted_at IS NULL;
             CREATE INDEX todo_lists_order
                 ON todo_lists(sort_key) WHERE deleted_at IS NULL;

             CREATE TABLE todo_tasks(
                 id TEXT PRIMARY KEY NOT NULL,
                 list_id TEXT REFERENCES todo_lists(id),
                 title TEXT NOT NULL,
                 notes TEXT NOT NULL DEFAULT '',
                 due_date TEXT,
                 due_time TEXT,
                 repeat_freq TEXT,
                 repeat_every INTEGER,
                 repeat_days INTEGER,
                 repeat_mode TEXT,
                 priority INTEGER NOT NULL DEFAULT 4 CHECK (priority BETWEEN 1 AND 4),
                 state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'done', 'wont')),
                 closed_at INTEGER,
                 repeat_of TEXT REFERENCES todo_tasks(id),
                 understood TEXT,
                 sort_key TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 deleted_at INTEGER
             ) STRICT;
             CREATE INDEX todo_tasks_open
                 ON todo_tasks(list_id, sort_key) WHERE state = 'open' AND deleted_at IS NULL;
             CREATE INDEX todo_tasks_due
                 ON todo_tasks(due_date) WHERE state = 'open' AND deleted_at IS NULL;
             CREATE INDEX todo_tasks_closed
                 ON todo_tasks(closed_at) WHERE state <> 'open' AND deleted_at IS NULL;
             CREATE INDEX todo_tasks_occurrences
                 ON todo_tasks(repeat_of) WHERE repeat_of IS NOT NULL;

             CREATE TABLE todo_comments(
                 id TEXT PRIMARY KEY NOT NULL,
                 task_id TEXT NOT NULL REFERENCES todo_tasks(id),
                 body TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 deleted_at INTEGER
             ) STRICT;
             CREATE INDEX todo_comments_task
                 ON todo_comments(task_id, created_at) WHERE deleted_at IS NULL;",
        ),
        // Search indexes: external content, kept in step by triggers, same tokenizer and prefix
        // settings as the Journal. Deleted rows stay indexed (Undo); queries filter on
        // `deleted_at`. They are keyed on the tables' implicit rowids, which VACUUM may renumber
        // (the tables have a TEXT primary key): never VACUUM without rebuilding both indexes.
        M::up(
            "CREATE VIRTUAL TABLE todo_tasks_fts USING fts5(
                 title, notes, content='todo_tasks', content_rowid='rowid',
                 tokenize='unicode61 remove_diacritics 2', prefix='2 3'
             );
             CREATE TRIGGER todo_tasks_fts_insert AFTER INSERT ON todo_tasks BEGIN
                 INSERT INTO todo_tasks_fts(rowid, title, notes)
                 VALUES (new.rowid, new.title, new.notes);
             END;
             CREATE TRIGGER todo_tasks_fts_delete AFTER DELETE ON todo_tasks BEGIN
                 INSERT INTO todo_tasks_fts(todo_tasks_fts, rowid, title, notes)
                 VALUES ('delete', old.rowid, old.title, old.notes);
             END;
             CREATE TRIGGER todo_tasks_fts_update AFTER UPDATE OF title, notes ON todo_tasks BEGIN
                 INSERT INTO todo_tasks_fts(todo_tasks_fts, rowid, title, notes)
                 VALUES ('delete', old.rowid, old.title, old.notes);
                 INSERT INTO todo_tasks_fts(rowid, title, notes)
                 VALUES (new.rowid, new.title, new.notes);
             END;

             CREATE VIRTUAL TABLE todo_comments_fts USING fts5(
                 body, content='todo_comments', content_rowid='rowid',
                 tokenize='unicode61 remove_diacritics 2', prefix='2 3'
             );
             CREATE TRIGGER todo_comments_fts_insert AFTER INSERT ON todo_comments BEGIN
                 INSERT INTO todo_comments_fts(rowid, body) VALUES (new.rowid, new.body);
             END;
             CREATE TRIGGER todo_comments_fts_delete AFTER DELETE ON todo_comments BEGIN
                 INSERT INTO todo_comments_fts(todo_comments_fts, rowid, body)
                 VALUES ('delete', old.rowid, old.body);
             END;
             CREATE TRIGGER todo_comments_fts_update AFTER UPDATE OF body ON todo_comments BEGIN
                 INSERT INTO todo_comments_fts(todo_comments_fts, rowid, body)
                 VALUES ('delete', old.rowid, old.body);
                 INSERT INTO todo_comments_fts(rowid, body) VALUES (new.rowid, new.body);
             END;",
        ),
    ])
}
