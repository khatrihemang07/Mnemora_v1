//! Reads and writes of Conversations and their entries. Rules live in the other modules; this file
//! only moves rows.

use std::collections::HashMap;

use mnemora_core::changelog::ChangeOp;
use mnemora_core::error::AppResult;
use mnemora_harness::{Call, Message};
use rusqlite::{Connection, OptionalExtension};

use crate::error::AiError;
use crate::sync;
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
    sync::log_conversation(conn, id, ChangeOp::Create)
}

/// Marks a Conversation as changed at `now`. Never moves it back: an entry another device wrote
/// later than this device's clock can already have moved it forward.
pub fn touch_conversation(conn: &Connection, id: &str, now: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_conversations SET updated_at = MAX(updated_at, ?2) WHERE id = ?1",
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
    sync::log_conversation(conn, id, ChangeOp::Update)
}

/// The entry a Conversation shows on this device: the one set, else its newest entry.
pub fn visible(conn: &Connection, conversation: &str) -> AppResult<Option<String>> {
    let set: Option<String> = conn
        .query_row(
            "SELECT entry_id FROM ai_visible_branches WHERE conversation_id = ?1",
            [conversation],
            |r| r.get(0),
        )
        .optional()?;
    match set {
        Some(entry) => Ok(Some(entry)),
        None => leaf(conn, conversation),
    }
}

/// Makes `entry` the one a Conversation shows on this device. Device-local: nothing is recorded to
/// the change log, so it is never synced.
pub fn show(conn: &Connection, conversation: &str, entry: &str, now: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ai_visible_branches(conversation_id, entry_id, updated_at) VALUES(?1, ?2, ?3)
         ON CONFLICT(conversation_id) DO UPDATE SET entry_id = excluded.entry_id,
             updated_at = excluded.updated_at",
        rusqlite::params![conversation, entry, now],
    )?;
    Ok(())
}

/// The newest entry that follows `entry`, then the newest after that, down to an entry with no
/// follower: the end of the Branch that continues from `entry`.
pub fn newest_under(conn: &Connection, entry: &str) -> AppResult<String> {
    let mut stmt = conn.prepare(
        "SELECT id FROM ai_entries WHERE parent_id = ?1 ORDER BY created_at DESC, rowid DESC LIMIT 1",
    )?;
    let mut current = entry.to_owned();
    while let Some(next) = stmt
        .query_row([&current], |r| r.get::<_, String>(0))
        .optional()?
    {
        current = next;
    }
    Ok(current)
}

/// The questions a question sits beside, oldest first, including itself. Questions are siblings
/// when they follow the same answer or the same start; a Model change between a question and what
/// it follows does not separate them. Reads ids, parents and kinds only: no payloads, so other
/// Branches are not loaded.
pub fn question_groups(conn: &Connection, conversation: &str) -> AppResult<Vec<Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, kind FROM ai_entries WHERE conversation_id = ?1 ORDER BY created_at, id",
    )?;
    let rows: Vec<(String, Option<String>, String)> = stmt
        .query_map([conversation], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let parent: HashMap<&str, Option<&str>> = rows
        .iter()
        .map(|(id, parent, _)| (id.as_str(), parent.as_deref()))
        .collect();
    let kind: HashMap<&str, &str> = rows
        .iter()
        .map(|(id, _, kind)| (id.as_str(), kind.as_str()))
        .collect();
    // The first entry above a question that is not a Model change.
    let anchor = |id: &str| -> Option<String> {
        let mut current = parent.get(id).copied().flatten();
        while let Some(above) = current {
            if kind.get(above) != Some(&"model_change") {
                return Some(above.to_owned());
            }
            current = parent.get(above).copied().flatten();
        }
        None
    };
    let mut groups: HashMap<Option<String>, Vec<String>> = HashMap::new();
    for (id, _, entry_kind) in &rows {
        if entry_kind == "user" {
            groups.entry(anchor(id)).or_default().push(id.clone());
        }
    }
    Ok(groups.into_values().collect())
}

/// Every question of a Conversation with its place among the questions it sits beside: `(index,
/// count)`, from 1.
pub fn question_places(
    conn: &Connection,
    conversation: &str,
) -> AppResult<HashMap<String, (u32, u32)>> {
    let mut places = HashMap::new();
    for members in question_groups(conn, conversation)? {
        let count = u32::try_from(members.len()).unwrap_or(u32::MAX);
        for (index, id) in members.into_iter().enumerate() {
            let index = u32::try_from(index + 1).unwrap_or(u32::MAX);
            places.insert(id, (index, count));
        }
    }
    Ok(places)
}

/// The questions that sit beside `question` (itself included), oldest first, or none when it is
/// not a question of this Conversation.
pub fn siblings_of(
    conn: &Connection,
    conversation: &str,
    question: &str,
) -> AppResult<Vec<String>> {
    Ok(question_groups(conn, conversation)?
        .into_iter()
        .find(|members| members.iter().any(|id| id == question))
        .unwrap_or_default())
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
    sync::log_entry(conn, entry.id)
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
    sync::log_conversation(conn, id, ChangeOp::Update)
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
    let op = if at.is_some() {
        ChangeOp::Delete
    } else {
        ChangeOp::Restore
    };
    sync::log_conversation(conn, id, op)
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
            // The places among siblings are filled in by `question_places`, not per row.
            sibling_count: 1,
            sibling_index: 1,
        },
        parent,
    ))
}
