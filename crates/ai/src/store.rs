//! Reads and writes of Conversations and their entries. Rules live in the other modules; this file
//! only moves rows.

use mnemora_core::error::AppResult;
use mnemora_harness::Message;
use rusqlite::{Connection, OptionalExtension};

use crate::error::AiError;
use crate::types::{EntryKind, EntryPayload, EntryView};

fn kind_text(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::User => "user",
        EntryKind::Assistant => "assistant",
    }
}

fn parse(payload: &str) -> AppResult<EntryPayload> {
    Ok(serde_json::from_str(payload).map_err(AiError::Payload)?)
}

/// Starts a Conversation titled `title`.
pub fn insert_conversation(conn: &Connection, id: &str, title: &str, now: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ai_conversations(id, title, created_at, updated_at) VALUES(?1, ?2, ?3, ?3)",
        rusqlite::params![id, title, now],
    )?;
    Ok(())
}

/// Marks a Conversation as changed at `now`.
pub fn touch_conversation(conn: &Connection, id: &str, now: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_conversations SET updated_at = ?2 WHERE id = ?1",
        rusqlite::params![id, now],
    )?;
    Ok(())
}

/// The title, created and updated times of a live Conversation, if there is one.
pub fn live_conversation(conn: &Connection, id: &str) -> AppResult<Option<(String, i64, i64)>> {
    Ok(conn
        .query_row(
            "SELECT title, created_at, updated_at FROM ai_conversations
             WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?)
}

/// The newest entry of a Conversation: what a new entry follows.
pub fn leaf(conn: &Connection, conversation: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT id FROM ai_entries WHERE conversation_id = ?1
             ORDER BY created_at DESC, rowid DESC LIMIT 1",
            [conversation],
            |r| r.get(0),
        )
        .optional()?)
}

/// Writes one entry, once. Entries are never updated (ADR 0019).
pub fn insert_entry(conn: &Connection, entry: NewRow<'_>, payload: &EntryPayload) -> AppResult<()> {
    let json = serde_json::to_string(payload).map_err(AiError::Payload)?;
    conn.execute(
        "INSERT INTO ai_entries(id, conversation_id, parent_id, kind, payload, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            entry.id,
            entry.conversation,
            entry.parent,
            kind_text(entry.kind),
            json,
            entry.now
        ],
    )?;
    Ok(())
}

/// The fields of one new entry, bundled so the write stays readable.
pub struct NewRow<'a> {
    /// The new entry's id.
    pub id: &'a str,
    /// The Conversation it belongs to.
    pub conversation: &'a str,
    /// The entry it follows, if any.
    pub parent: Option<&'a str>,
    /// Who wrote it.
    pub kind: EntryKind,
    /// When it was written, in milliseconds.
    pub now: i64,
}

/// The branch that ends at `leaf`, as messages for the Model, oldest first.
pub fn branch_messages(conn: &Connection, leaf: &str) -> AppResult<Vec<Message>> {
    let mut stmt = conn.prepare("SELECT parent_id, payload FROM ai_entries WHERE id = ?1")?;
    let mut messages = Vec::new();
    let mut current = Some(leaf.to_owned());
    while let Some(id) = current {
        let (parent, payload): (Option<String>, String) =
            stmt.query_row([&id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        messages.push(match parse(&payload)? {
            EntryPayload::User { text } => Message::User(text),
            EntryPayload::Assistant { text, .. } => Message::Assistant(text),
        });
        current = parent;
    }
    messages.reverse();
    Ok(messages)
}

/// Every entry of a Conversation, oldest first.
pub fn entries(conn: &Connection, conversation: &str) -> AppResult<Vec<EntryView>> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, created_at, payload FROM ai_entries
         WHERE conversation_id = ?1 ORDER BY created_at, rowid",
    )?;
    let rows = stmt.query_map([conversation], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, parent_id, created_at, payload) = row?;
        out.push(EntryView {
            id,
            parent_id,
            created_at,
            payload: parse(&payload)?,
        });
    }
    Ok(out)
}
