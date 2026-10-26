//! Takes AI records received from another device (Sync's side of the Module contract). The values
//! are written straight to the tables and never logged again: the record is already in the log.
//! Every field is checked as the app's own writes check it; a field that fails is left out.
//! Question and answer text must never reach a log line.

use mnemora_core::error::{AppError, AppResult, ErrorCode};
use mnemora_core::syncable::{Applied, RemoteChange};
use mnemora_core::util::now_ms;
use rusqlite::{Connection, ToSql, params};
use serde_json::{Map, Value};

use crate::sync::{self, CONVERSATION, ENTRY, SAVED};
use crate::title::{validate_saved, validate_title};
use crate::types::EntryPayload;

/// Applies the winning fields of one AI record, creating its row when it is new here.
pub(crate) fn apply(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    match change.entity {
        CONVERSATION => conversation(conn, change),
        ENTRY => entry(conn, change),
        SAVED => saved(conn, change),
        _ => Err(AppError::new(
            ErrorCode::Internal,
            "The AI Module does not sync that kind of record",
        )),
    }
}

/// What a record that cannot be made here yet leaves behind: nothing in the tables, the fields
/// kept for relay only (the change log has them).
fn relay(change: &RemoteChange<'_>) -> Applied {
    Applied {
        snapshot: Value::Object(change.fields.clone()),
        changed: false,
        events: Vec::new(),
    }
}

/// A text field that is present and checked.
fn text(fields: &Map<String, Value>, key: &str) -> Option<String> {
    fields.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// A whole number that is present.
fn number(fields: &Map<String, Value>, key: &str) -> Option<i64> {
    fields.get(key).and_then(Value::as_i64)
}

/// A value a record sends for a column: the value itself, or null, which clears the column.
enum Sent<T> {
    Null,
    Value(T),
}

impl<T> Sent<T> {
    /// The column's new value: `None` clears it.
    fn into_column(self) -> Option<T> {
        match self {
            Self::Null => None,
            Self::Value(value) => Some(value),
        }
    }
}

/// A nullable whole number the record sends, or none when it sends nothing usable.
fn nullable_number(fields: &Map<String, Value>, key: &str) -> Option<Sent<i64>> {
    match fields.get(key)? {
        Value::Null => Some(Sent::Null),
        other => other.as_i64().map(Sent::Value),
    }
}

/// A nullable text the record sends, or none when it sends nothing usable.
fn nullable_text(fields: &Map<String, Value>, key: &str) -> Option<Sent<String>> {
    match fields.get(key)? {
        Value::Null => Some(Sent::Null),
        Value::String(s) => Some(Sent::Value(s.clone())),
        _ => None,
    }
}

/// Writes one column of a row.
fn set<T: ToSql>(
    conn: &Connection,
    table: &str,
    id: &str,
    column: &str,
    value: T,
) -> AppResult<()> {
    conn.execute(
        &format!("UPDATE {table} SET {column} = ?2 WHERE id = ?1"),
        params![id, value],
    )?;
    Ok(())
}

/// Moves a changed row's derived `updated_at` forward, never backwards.
fn touch(conn: &Connection, table: &str, id: &str) -> AppResult<()> {
    conn.execute(
        &format!("UPDATE {table} SET updated_at = MAX(?2, updated_at + 1) WHERE id = ?1"),
        params![id, now_ms()],
    )?;
    Ok(())
}

/// The snapshot of a record without its derived `updated_at`, to tell whether it changed.
fn comparable(mut snapshot: Value) -> Value {
    if let Some(fields) = snapshot.as_object_mut() {
        fields.remove("updated_at");
    }
    snapshot
}

fn conversation(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let id = change.entity_id;
    let f = change.fields;
    let title = text(f, "title").and_then(|t| validate_title(&t).ok());
    let created_at = number(f, "created_at");
    let existed = sync::exists(conn, "ai_conversations", id)?;
    let before = if existed {
        Some(comparable(sync::conversation_snapshot(conn, id)?))
    } else {
        None
    };
    if !existed {
        // A Conversation can only be made from a record that carries its title and start.
        let (Some(title), Some(created_at)) = (&title, created_at) else {
            tracing::debug!(target: "sync", "an AI Conversation record without its creation, kept for relay only");
            return Ok(relay(change));
        };
        // Its last change is its start until an entry or a header change says otherwise, as it is
        // on the device that made it.
        conn.execute(
            "INSERT INTO ai_conversations(id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![id, title, created_at],
        )?;
    }
    if let Some(title) = &title {
        set(conn, "ai_conversations", id, "title", title)?;
    }
    if let Some(created_at) = created_at {
        set(conn, "ai_conversations", id, "created_at", created_at)?;
    }
    if let Some(deleted_at) = nullable_number(f, "deleted_at") {
        set(
            conn,
            "ai_conversations",
            id,
            "deleted_at",
            deleted_at.into_column(),
        )?;
    }
    // The Provider group arrives whole or not at all.
    let provider = (|| {
        Some((
            nullable_text(f, "provider_id")?.into_column(),
            nullable_text(f, "provider_name")?.into_column(),
            nullable_text(f, "model")?.into_column(),
        ))
    })();
    if let Some((provider_id, provider_name, model)) = provider {
        conn.execute(
            "UPDATE ai_conversations SET provider_id = ?2, provider_name = ?3, model = ?4 WHERE id = ?1",
            params![id, provider_id, provider_name, model],
        )?;
    }
    let after = sync::conversation_snapshot(conn, id)?;
    // A header change does not move `updated_at`: the device that made it does not either.
    let changed = before.as_ref() != Some(&comparable(after.clone()));
    Ok(Applied {
        snapshot: after,
        changed,
        events: Vec::new(),
    })
}

/// An entry as received and checked: its kind must be the kind its payload says, and the payload
/// must be one the app can read.
struct NewEntry {
    conversation: String,
    parent: Option<String>,
    kind: String,
    payload: String,
    created_at: i64,
}

fn read_entry(fields: &Map<String, Value>) -> Option<NewEntry> {
    let payload = text(fields, "payload")?;
    let value: Value = serde_json::from_str(&payload).ok()?;
    let kind = text(fields, "kind")?;
    if value.get("kind")?.as_str()? != kind {
        return None;
    }
    serde_json::from_str::<EntryPayload>(&payload).ok()?;
    Some(NewEntry {
        conversation: text(fields, "conversation_id")?,
        parent: nullable_text(fields, "parent_id")?.into_column(),
        kind,
        payload,
        created_at: number(fields, "created_at")?,
    })
}

fn entry(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let Some(new) = read_entry(change.fields) else {
        tracing::debug!(target: "sync", "an AI entry record that does not read, kept for relay only");
        return Ok(relay(change));
    };
    // The Conversation (deleted is fine) and the entry it follows must be here first.
    let parent_here = match &new.parent {
        Some(parent) => sync::exists(conn, "ai_entries", parent)?,
        None => true,
    };
    if !parent_here || !sync::exists(conn, "ai_conversations", &new.conversation)? {
        tracing::debug!(target: "sync", "an AI entry whose Conversation or entry is not here yet, kept for relay only");
        return Ok(relay(change));
    }
    let written = conn.execute(
        "INSERT OR IGNORE INTO ai_entries(id, conversation_id, parent_id, kind, payload, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            change.entity_id,
            new.conversation,
            new.parent,
            new.kind,
            new.payload,
            new.created_at
        ],
    )?;
    if written > 0 {
        // An answer moves its Conversation to the top of the history, as it does on the device that
        // wrote it (where the Conversation is touched with the entry's time).
        conn.execute(
            "UPDATE ai_conversations SET updated_at = MAX(updated_at, ?2) WHERE id = ?1",
            params![new.conversation, new.created_at],
        )?;
    }
    Ok(Applied {
        snapshot: sync::entry_snapshot(conn, change.entity_id)?,
        changed: written > 0,
        events: Vec::new(),
    })
}

fn saved(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let id = change.entity_id;
    let f = change.fields;
    let text_value = text(f, "text").and_then(|t| validate_saved(&t).ok());
    let sort_key = text(f, "sort_key").filter(|k| !k.is_empty());
    let created_at = number(f, "created_at");
    let existed = sync::exists(conn, "ai_saved_questions", id)?;
    let before = if existed {
        Some(comparable(sync::saved_snapshot(conn, id)?))
    } else {
        None
    };
    if !existed {
        // A Saved question can only be made from a record that carries its text, place and start.
        let (Some(text_value), Some(sort_key), Some(created_at)) =
            (&text_value, &sort_key, created_at)
        else {
            tracing::debug!(target: "sync", "an AI Saved question record without its creation, kept for relay only");
            return Ok(relay(change));
        };
        conn.execute(
            "INSERT INTO ai_saved_questions(id, text, sort_key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, text_value, sort_key, created_at, now_ms()],
        )?;
    }
    if let Some(text_value) = &text_value {
        set(conn, "ai_saved_questions", id, "text", text_value)?;
    }
    if let Some(sort_key) = &sort_key {
        set(conn, "ai_saved_questions", id, "sort_key", sort_key)?;
    }
    if let Some(created_at) = created_at {
        set(conn, "ai_saved_questions", id, "created_at", created_at)?;
    }
    if let Some(deleted_at) = nullable_number(f, "deleted_at") {
        set(
            conn,
            "ai_saved_questions",
            id,
            "deleted_at",
            deleted_at.into_column(),
        )?;
    }
    let after = sync::saved_snapshot(conn, id)?;
    let changed = before.as_ref() != Some(&comparable(after.clone()));
    if changed && existed {
        touch(conn, "ai_saved_questions", id)?;
    }
    Ok(Applied {
        snapshot: after,
        changed,
        events: Vec::new(),
    })
}
