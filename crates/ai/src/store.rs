//! Reads and writes of Conversations and their entries. Rules live in the other modules; this file
//! only moves rows.

use mnemora_core::error::AppResult;
use mnemora_harness::{Call, Message};
use rusqlite::{Connection, OptionalExtension};

use crate::error::AiError;
use crate::types::{ConversationSummary, EntryKind, EntryPayload, EntryView};

fn kind_text(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::User => "user",
        EntryKind::Assistant => "assistant",
        EntryKind::ToolResult => "tool_result",
        EntryKind::Correction => "correction",
        EntryKind::ModelChange => "model_change",
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

/// The header of a live Conversation.
pub struct Header {
    /// Its title.
    pub title: String,
    /// When it was started, in milliseconds.
    pub created_at: i64,
    /// When it last changed, in milliseconds.
    pub updated_at: i64,
    /// The Provider id it last used.
    pub provider_id: Option<String>,
    /// The Provider name it last used.
    pub provider_name: Option<String>,
    /// The Model id it last used.
    pub model: Option<String>,
}

/// The header of a live Conversation, if there is one.
pub fn live_conversation(conn: &Connection, id: &str) -> AppResult<Option<Header>> {
    Ok(conn
        .query_row(
            "SELECT title, created_at, updated_at, provider_id, provider_name, model
             FROM ai_conversations WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| {
                Ok(Header {
                    title: r.get(0)?,
                    created_at: r.get(1)?,
                    updated_at: r.get(2)?,
                    provider_id: r.get(3)?,
                    provider_name: r.get(4)?,
                    model: r.get(5)?,
                })
            },
        )
        .optional()?)
}

/// Records the Provider and Model a Conversation now uses.
pub fn set_model(
    conn: &Connection,
    id: &str,
    provider_id: &str,
    provider_name: &str,
    model: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_conversations SET provider_id = ?2, provider_name = ?3, model = ?4 WHERE id = ?1",
        rusqlite::params![id, provider_id, provider_name, model],
    )?;
    Ok(())
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

/// The branch that ends at `leaf`, as messages for the Model, oldest first: the question, each
/// Step's text and the Tool calls it made, the results read back, and any corrections.
pub fn branch_messages(conn: &Connection, leaf: &str) -> AppResult<Vec<Message>> {
    let mut stmt = conn.prepare("SELECT parent_id, payload FROM ai_entries WHERE id = ?1")?;
    let mut messages = Vec::new();
    let mut current = Some(leaf.to_owned());
    while let Some(id) = current {
        let (parent, payload): (Option<String>, String) =
            stmt.query_row([&id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        match parse(&payload)? {
            EntryPayload::User { text } => messages.push(Message::User(text)),
            // A Step stopped before it wrote anything is kept only to show that it was stopped.
            EntryPayload::Assistant { text, calls, .. } if text.is_empty() && calls.is_empty() => {}
            EntryPayload::Assistant { text, calls, .. } => messages.push(Message::Assistant {
                text,
                calls: calls
                    .into_iter()
                    .map(|c| Call {
                        id: c.id,
                        name: c.name,
                        arguments: c.arguments,
                    })
                    .collect(),
            }),
            EntryPayload::ToolResult {
                call_id,
                name,
                result,
                is_error,
                ..
            } => messages.push(Message::Tool {
                call_id,
                name,
                text: result,
                is_error,
            }),
            EntryPayload::Correction { text } => messages.push(Message::Correction(text)),
            EntryPayload::ModelChange { .. } => {}
        }
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

/// Every live Conversation, newest first. Titles are matched by the caller, which folds them.
pub fn live_conversations(conn: &Connection) -> AppResult<Vec<ConversationSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, created_at, updated_at FROM ai_conversations
         WHERE deleted_at IS NULL ORDER BY updated_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ConversationSummary {
            id: r.get(0)?,
            title: r.get(1)?,
            created_at: r.get(2)?,
            updated_at: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Gives a Conversation a new title. The caller has checked the title and that the Conversation is live.
pub fn set_title(conn: &Connection, id: &str, title: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_conversations SET title = ?2 WHERE id = ?1",
        rusqlite::params![id, title],
    )?;
    Ok(())
}

/// Whether a Conversation exists and is deleted (so that Undo can bring it back).
pub fn is_deleted(conn: &Connection, id: &str) -> AppResult<bool> {
    Ok(conn
        .query_row(
            "SELECT deleted_at IS NOT NULL FROM ai_conversations WHERE id = ?1",
            [id],
            |r| r.get::<_, bool>(0),
        )
        .optional()?
        .unwrap_or(false))
}

/// Marks a Conversation deleted at `at`, or brings it back when `at` is none. Nothing is purged.
pub fn set_deleted(conn: &Connection, id: &str, at: Option<i64>) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_conversations SET deleted_at = ?2 WHERE id = ?1",
        rusqlite::params![id, at],
    )?;
    Ok(())
}

/// The branch that ends at `leaf`, as entries, oldest first (what a copy of the Conversation shows).
pub fn branch_entries(conn: &Connection, leaf: &str) -> AppResult<Vec<EntryView>> {
    let mut stmt =
        conn.prepare("SELECT id, parent_id, created_at, payload FROM ai_entries WHERE id = ?1")?;
    let mut out = Vec::new();
    let mut current = Some(leaf.to_owned());
    while let Some(id) = current {
        let (entry, parent) = stmt.query_row([&id], entry_row)?;
        out.push(entry);
        current = parent;
    }
    out.reverse();
    Ok(out)
}

/// One entry of a Conversation, if it belongs to it.
pub fn entry_in(conn: &Connection, conversation: &str, id: &str) -> AppResult<Option<EntryView>> {
    Ok(conn
        .query_row(
            "SELECT id, parent_id, created_at, payload FROM ai_entries
             WHERE id = ?1 AND conversation_id = ?2",
            rusqlite::params![id, conversation],
            entry_row,
        )
        .optional()?
        .map(|(entry, _)| entry))
}

/// A row of `ai_entries` as an entry and the id it follows.
fn entry_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<(EntryView, Option<String>)> {
    let payload: String = r.get(3)?;
    let payload = serde_json::from_str(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let parent: Option<String> = r.get(1)?;
    Ok((
        EntryView {
            id: r.get(0)?,
            parent_id: parent.clone(),
            created_at: r.get(2)?,
            payload,
        },
        parent,
    ))
}
