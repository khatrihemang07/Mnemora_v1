//! Reads and writes of Saved question rows. Rules live in `saved.rs`; this file only moves rows.

use mnemora_core::changelog::ChangeOp;
use mnemora_core::error::AppResult;
use rusqlite::{Connection, OptionalExtension};

use crate::sync;
use crate::types::SavedQuestion;

/// The live Saved questions in their manual order.
pub fn live(conn: &Connection) -> AppResult<Vec<SavedQuestion>> {
    let mut stmt = conn.prepare(
        "SELECT id, text FROM ai_saved_questions WHERE deleted_at IS NULL ORDER BY sort_key, id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(SavedQuestion {
            id: r.get(0)?,
            text: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The manual-order key of a live Saved question, if there is one.
pub fn live_key(conn: &Connection, id: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT sort_key FROM ai_saved_questions WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| r.get(0),
        )
        .optional()?)
}

/// Whether a Saved question exists and is deleted.
pub fn is_deleted(conn: &Connection, id: &str) -> AppResult<bool> {
    Ok(conn
        .query_row(
            "SELECT deleted_at IS NOT NULL FROM ai_saved_questions WHERE id = ?1",
            [id],
            |r| r.get::<_, bool>(0),
        )
        .optional()?
        .unwrap_or(false))
}

/// The greatest key of any Saved question, deleted ones included, so that a new one goes last and
/// a deleted one restored keeps its place.
pub fn last_key(conn: &Connection) -> AppResult<Option<String>> {
    Ok(
        conn.query_row("SELECT MAX(sort_key) FROM ai_saved_questions", [], |r| {
            r.get(0)
        })?,
    )
}

/// Writes a new Saved question.
pub fn insert(conn: &Connection, id: &str, text: &str, key: &str, now: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ai_saved_questions(id, text, sort_key, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![id, text, key, now],
    )?;
    sync::log_saved(conn, id, ChangeOp::Create)
}

/// Changes the text of a Saved question.
pub fn set_text(conn: &Connection, id: &str, text: &str, now: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_saved_questions SET text = ?2, updated_at = ?3 WHERE id = ?1",
        rusqlite::params![id, text, now],
    )?;
    sync::log_saved(conn, id, ChangeOp::Update)
}

/// Moves a Saved question to a new key.
pub fn set_key(conn: &Connection, id: &str, key: &str, now: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_saved_questions SET sort_key = ?2, updated_at = ?3 WHERE id = ?1",
        rusqlite::params![id, key, now],
    )?;
    sync::log_saved(conn, id, ChangeOp::Update)
}

/// Marks a Saved question deleted at `at`, or brings it back when `at` is none.
pub fn set_deleted(conn: &Connection, id: &str, at: Option<i64>, now: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_saved_questions SET deleted_at = ?2, updated_at = ?3 WHERE id = ?1",
        rusqlite::params![id, at, now],
    )?;
    let op = if at.is_some() {
        ChangeOp::Delete
    } else {
        ChangeOp::Restore
    };
    sync::log_saved(conn, id, op)
}
