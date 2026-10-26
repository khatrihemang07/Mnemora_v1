//! Saved questions: the person's one-tap questions, kept in a manual order. Running one is an
//! ordinary Run; nothing here asks a Provider.

use mnemora_core::error::AppResult;
use mnemora_core::order::{key_after, key_between};
use mnemora_core::util::new_id;
use rusqlite::Connection;

use crate::error::AiError;
use crate::saved_store as store;
use crate::title::validate_saved;
use crate::types::SavedQuestion;

/// Every live Saved question, in its manual order.
pub fn list(conn: &Connection) -> AppResult<Vec<SavedQuestion>> {
    store::live(conn)
}

/// Adds a Saved question at the end of the list.
pub fn create(conn: &Connection, raw: &str, now: i64) -> AppResult<SavedQuestion> {
    let text = validate_saved(raw)?;
    let id = new_id();
    let key = key_after(store::last_key(conn)?.as_deref());
    store::insert(conn, &id, &text, &key, now)?;
    Ok(SavedQuestion { id, text })
}

/// Changes a Saved question's text.
pub fn update(conn: &Connection, id: &str, raw: &str, now: i64) -> AppResult<SavedQuestion> {
    live(conn, id)?;
    let text = validate_saved(raw)?;
    store::set_text(conn, id, &text, now)?;
    Ok(SavedQuestion {
        id: id.to_owned(),
        text,
    })
}

/// Deletes a Saved question. Its key stays, so Undo puts it back in its place.
pub fn delete(conn: &Connection, id: &str, now: i64) -> AppResult<()> {
    live(conn, id)?;
    store::set_deleted(conn, id, Some(now), now)
}

/// Brings back a deleted Saved question in its old place.
pub fn restore(conn: &Connection, id: &str, now: i64) -> AppResult<()> {
    if !store::is_deleted(conn, id)? {
        return Err(AiError::NotFound.into());
    }
    store::set_deleted(conn, id, None, now)
}

/// Puts a Saved question between two neighbours the person saw: `after` above it and `before`
/// below it (either may be none, for the top or the bottom). Only its own key changes.
pub fn reorder(
    conn: &Connection,
    id: &str,
    after: Option<&str>,
    before: Option<&str>,
    now: i64,
) -> AppResult<()> {
    live(conn, id)?;
    let lo = after.map(|a| neighbour_key(conn, a)).transpose()?;
    let hi = before.map(|b| neighbour_key(conn, b)).transpose()?;
    let key = key_between(lo.as_deref(), hi.as_deref()).map_err(AiError::Validation)?;
    store::set_key(conn, id, &key, now)
}

/// The key of a live neighbour; one that has gone since the screen showed it is not found.
fn neighbour_key(conn: &Connection, id: &str) -> AppResult<String> {
    store::live_key(conn, id)?.ok_or_else(|| AiError::NotFound.into())
}

/// The key of a live Saved question, or not found.
fn live(conn: &Connection, id: &str) -> AppResult<String> {
    store::live_key(conn, id)?.ok_or_else(|| AiError::NotFound.into())
}
