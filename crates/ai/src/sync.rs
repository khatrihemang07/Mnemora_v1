//! The AI Module's side of the sync contract (ADR 0017, ADR 0019): which entities sync and how each
//! field merges, the change-log record every write makes, and the backfill of rows that predate
//! the log. Providers, keys, usage and the visible Branch are not here at all: they stay on the
//! device. Question and answer text must never reach a log line.

use mnemora_core::changelog::{self, Change, ChangeOp, Unlogged};
use mnemora_core::error::AppResult;
use mnemora_core::syncable::{SyncEntity, SyncField};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};

use crate::MODULE_ID;

/// The change-log entity of a Conversation's header.
pub const CONVERSATION: &str = "ai_conversation";
/// The change-log entity of one entry of a Conversation.
pub const ENTRY: &str = "ai_entry";
/// The change-log entity of a Saved question.
pub const SAVED: &str = "ai_saved_question";

const PROVIDER: &str = "provider";

/// A Conversation's header. The title is text. The Provider and Model it last used travel as one
/// group, so the name and Model a device shows always come from the same record. Provider ids
/// name this device's own Providers; a device that lacks one asks for a Provider of its own.
pub(crate) const CONVERSATION_SCHEMA: SyncEntity = SyncEntity {
    name: CONVERSATION,
    fields: &[
        SyncField::text("title"),
        SyncField::value("provider_id").in_group(PROVIDER),
        SyncField::value("provider_name").in_group(PROVIDER),
        SyncField::value("model").in_group(PROVIDER),
        SyncField::value("created_at"),
        SyncField::value("deleted_at"),
        SyncField::derived("updated_at"),
    ],
};

/// An entry is written once and never changes (ADR 0019): every field is fixed at creation.
pub(crate) const ENTRY_SCHEMA: SyncEntity = SyncEntity {
    name: ENTRY,
    fields: &[
        SyncField::value("conversation_id"),
        SyncField::value("parent_id"),
        SyncField::value("kind"),
        SyncField::value("payload"),
        SyncField::value("created_at"),
    ],
};

/// A Saved question: its text is text, its place is an order key, and deletion is a value so
/// that Undo can bring it back.
pub(crate) const SAVED_SCHEMA: SyncEntity = SyncEntity {
    name: SAVED,
    fields: &[
        SyncField::text("text"),
        SyncField::order("sort_key"),
        SyncField::value("created_at"),
        SyncField::value("deleted_at"),
        SyncField::derived("updated_at"),
    ],
};

/// Records what a write did to a Conversation's header: its full state after the write.
pub(crate) fn log_conversation(conn: &Connection, id: &str, op: ChangeOp) -> AppResult<()> {
    let snapshot = conversation_snapshot(conn, id)?;
    record(conn, CONVERSATION, id, op, snapshot)
}

/// Records a new entry. Entries are only ever created.
pub(crate) fn log_entry(conn: &Connection, id: &str) -> AppResult<()> {
    let snapshot = entry_snapshot(conn, id)?;
    record(conn, ENTRY, id, ChangeOp::Create, snapshot)
}

/// Records what a write did to a Saved question: its full state after the write.
pub(crate) fn log_saved(conn: &Connection, id: &str, op: ChangeOp) -> AppResult<()> {
    let snapshot = saved_snapshot(conn, id)?;
    record(conn, SAVED, id, op, snapshot)
}

fn record(
    conn: &Connection,
    entity: &str,
    id: &str,
    op: ChangeOp,
    snapshot: Value,
) -> AppResult<()> {
    changelog::record(
        conn,
        &Change {
            module: MODULE_ID.into(),
            entity: entity.into(),
            entity_id: id.into(),
            op,
            session_id: None,
            snapshot,
        },
    )
}

/// A Conversation's header as the change log holds it, deleted or not.
pub(crate) fn conversation_snapshot(conn: &Connection, id: &str) -> AppResult<Value> {
    Ok(conn.query_row(
        "SELECT title, provider_id, provider_name, model, created_at, updated_at, deleted_at
         FROM ai_conversations WHERE id = ?1",
        [id],
        |r| {
            Ok(json!({
                "title": r.get::<_, String>(0)?,
                "provider_id": r.get::<_, Option<String>>(1)?,
                "provider_name": r.get::<_, Option<String>>(2)?,
                "model": r.get::<_, Option<String>>(3)?,
                "created_at": r.get::<_, i64>(4)?,
                "updated_at": r.get::<_, i64>(5)?,
                "deleted_at": r.get::<_, Option<i64>>(6)?,
            }))
        },
    )?)
}

/// An entry as the change log holds it.
pub(crate) fn entry_snapshot(conn: &Connection, id: &str) -> AppResult<Value> {
    Ok(conn.query_row(
        "SELECT conversation_id, parent_id, kind, payload, created_at FROM ai_entries WHERE id = ?1",
        [id],
        |r| {
            Ok(json!({
                "conversation_id": r.get::<_, String>(0)?,
                "parent_id": r.get::<_, Option<String>>(1)?,
                "kind": r.get::<_, String>(2)?,
                "payload": r.get::<_, String>(3)?,
                "created_at": r.get::<_, i64>(4)?,
            }))
        },
    )?)
}

/// A Saved question as the change log holds it, deleted or not.
pub(crate) fn saved_snapshot(conn: &Connection, id: &str) -> AppResult<Value> {
    Ok(conn.query_row(
        "SELECT text, sort_key, created_at, updated_at, deleted_at FROM ai_saved_questions
         WHERE id = ?1",
        [id],
        |r| {
            Ok(json!({
                "text": r.get::<_, String>(0)?,
                "sort_key": r.get::<_, String>(1)?,
                "created_at": r.get::<_, i64>(2)?,
                "updated_at": r.get::<_, i64>(3)?,
                "deleted_at": r.get::<_, Option<i64>>(4)?,
            }))
        },
    )?)
}

/// Logs every Conversation, entry and Saved question (deleted ones included) whose state the log
/// lacks: rows from before the AI Module synced. Conversations first, then entries in the order
/// they were written (so a parent is met before its follower), then Saved questions. Sync calls
/// this before it first reads the log.
pub(crate) fn backfill(conn: &Connection) -> AppResult<()> {
    let conversations = ids_with_time(
        conn,
        "SELECT id, updated_at FROM ai_conversations ORDER BY created_at, rowid",
    )?;
    let mut rows = Vec::new();
    for (id, ms) in &conversations {
        rows.push(unlogged(
            CONVERSATION,
            id,
            conversation_snapshot(conn, id)?,
            *ms,
            Vec::new(),
        ));
    }
    let mut written = changelog::backfill(conn, &CONVERSATION_SCHEMA, rows)?;

    let entries = entries_in_order(conn)?;
    let mut rows = Vec::new();
    for (id, conversation, parent, created_at) in &entries {
        let mut after = vec![(CONVERSATION, conversation.as_str())];
        if let Some(parent) = parent {
            after.push((ENTRY, parent.as_str()));
        }
        rows.push(unlogged(
            ENTRY,
            id,
            entry_snapshot(conn, id)?,
            *created_at,
            after,
        ));
    }
    written += changelog::backfill(conn, &ENTRY_SCHEMA, rows)?;

    let saved = ids_with_time(
        conn,
        "SELECT id, updated_at FROM ai_saved_questions ORDER BY created_at, rowid",
    )?;
    let mut rows = Vec::new();
    for (id, ms) in &saved {
        rows.push(unlogged(
            SAVED,
            id,
            saved_snapshot(conn, id)?,
            *ms,
            Vec::new(),
        ));
    }
    written += changelog::backfill(conn, &SAVED_SCHEMA, rows)?;

    if written > 0 {
        tracing::info!(target: "sync", records = written, "AI rows missing from the change log were added");
    }
    Ok(())
}

fn unlogged<'a>(
    entity: &str,
    id: &str,
    snapshot: Value,
    ms: i64,
    after: Vec<(&'a str, &'a str)>,
) -> Unlogged<'a> {
    Unlogged {
        change: Change {
            module: MODULE_ID.into(),
            entity: entity.into(),
            entity_id: id.into(),
            op: ChangeOp::Update,
            session_id: None,
            snapshot,
        },
        ms,
        after,
        needs_create: true,
    }
}

fn ids_with_time(conn: &Connection, sql: &str) -> AppResult<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// An entry's id, Conversation, the entry it follows, and when it was written.
type EntryRow = (String, String, Option<String>, i64);

/// Every entry as `(id, conversation, parent, created_at)`, in the order they were written.
fn entries_in_order(conn: &Connection) -> AppResult<Vec<EntryRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, parent_id, created_at FROM ai_entries ORDER BY created_at, rowid",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Whether a row with this id exists in `table` (deleted rows count).
pub(crate) fn exists(conn: &Connection, table: &str, id: &str) -> AppResult<bool> {
    Ok(conn
        .query_row(&format!("SELECT 1 FROM {table} WHERE id = ?1"), [id], |r| {
            r.get::<_, i64>(0)
        })
        .optional()?
        .is_some())
}
