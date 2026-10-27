//! Managing Conversations after they exist: the history list, rename, delete and restore, and
//! copying as Markdown. Asking questions is in `run.rs`.

use mnemora_core::error::AppResult;
use mnemora_core::search::fold::fold;
use rusqlite::Connection;

use crate::error::AiError;
use crate::markdown;
use crate::store;
use crate::title::validate_title;
use crate::types::{ConversationPage, EntryPayload};

/// The most Conversations one history page holds.
pub const PAGE_MAX: u32 = 100;

/// One page of the history, newest first, whose titles contain `query` (any case, any accent).
/// Only titles are searched, not the text of the entries.
pub fn list(
    conn: &Connection,
    query: &str,
    offset: u32,
    limit: u32,
) -> AppResult<ConversationPage> {
    let needle = fold(query.trim());
    let matching: Vec<_> = store::live_conversations(conn)?
        .into_iter()
        .filter(|c| needle.is_empty() || fold(&c.title).contains(&needle))
        .collect();
    let limit = limit.clamp(1, PAGE_MAX) as usize;
    let offset = offset as usize;
    Ok(ConversationPage {
        more: matching.len() > offset + limit,
        items: matching.into_iter().skip(offset).take(limit).collect(),
    })
}

/// Gives a live Conversation a new title, checked first.
pub fn rename(conn: &Connection, id: &str, raw: &str) -> AppResult<()> {
    live(conn, id)?;
    let title = validate_title(raw)?;
    store::set_title(conn, id, &title)
}

/// Deletes a live Conversation. Nothing is purged, so Undo can bring it back.
pub fn delete(conn: &Connection, id: &str, now: i64) -> AppResult<()> {
    live(conn, id)?;
    store::set_deleted(conn, id, Some(now))
}

/// Brings a deleted Conversation back, with every entry it had.
pub fn restore(conn: &Connection, id: &str) -> AppResult<()> {
    if !store::is_deleted(conn, id)? {
        return Err(AiError::NotFound.into());
    }
    store::set_deleted(conn, id, None)
}

/// A live Conversation as Markdown: its visible branch (the one ending at its newest entry), or,
/// with `entry`, that one answer. Only an answer copies on its own.
pub fn markdown(conn: &Connection, id: &str, entry: Option<&str>) -> AppResult<String> {
    let header = live(conn, id)?;
    let Some(entry) = entry else {
        let branch = match store::leaf(conn, id)? {
            Some(leaf) => store::branch_entries(conn, &leaf)?,
            None => Vec::new(),
        };
        return Ok(markdown::conversation(&header.title, &branch));
    };
    let found = store::entry_in(conn, id, entry)?.ok_or(AiError::NotFound)?;
    match found.payload {
        EntryPayload::Assistant { text, calls, .. } if calls.is_empty() => {
            Ok(markdown::answer(&text))
        }
        _ => Err(AiError::Validation("Only an answer copies on its own.").into()),
    }
}

/// The header of a live Conversation, or not found.
fn live(conn: &Connection, id: &str) -> AppResult<store::Header> {
    store::live_conversation(conn, id)?.ok_or_else(|| AiError::NotFound.into())
}
