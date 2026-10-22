//! SQL for the Comments table. No rules here; see `comment_service.rs`.

use mnemora_core::error::AppResult;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::comment::Comment;

const COLUMNS: &str = "id, task_id, body, edited, created_at, updated_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Comment> {
    Ok(Comment {
        id: r.get(0)?,
        task_id: r.get(1)?,
        body: r.get(2)?,
        edited: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
    })
}

pub fn insert(conn: &Connection, c: &Comment) -> AppResult<()> {
    conn.execute(
        "INSERT INTO todo_comments(id, task_id, body, edited, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            c.id,
            c.task_id,
            c.body,
            c.edited,
            c.created_at,
            c.updated_at
        ],
    )?;
    Ok(())
}

/// A live (not deleted) Comment.
pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Comment>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM todo_comments WHERE id = ?1 AND deleted_at IS NULL"),
            [id],
            from_row,
        )
        .optional()?)
}

/// A Comment whether or not it is deleted, with its `deleted_at`.
pub fn get_any(conn: &Connection, id: &str) -> AppResult<Option<(Comment, Option<i64>)>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS}, deleted_at FROM todo_comments WHERE id = ?1"),
            [id],
            |r| Ok((from_row(r)?, r.get(6)?)),
        )
        .optional()?)
}

/// A Task's live Comments, oldest first.
pub fn for_task(conn: &Connection, task_id: &str) -> AppResult<Vec<Comment>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM todo_comments
         WHERE task_id = ?1 AND deleted_at IS NULL ORDER BY created_at, id"
    ))?;
    Ok(stmt
        .query_map([task_id], from_row)?
        .collect::<Result<_, _>>()?)
}

pub fn update_body(conn: &Connection, c: &Comment) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_comments SET body = ?2, edited = 1, updated_at = ?3 WHERE id = ?1",
        params![c.id, c.body, c.updated_at],
    )?;
    Ok(())
}

/// Sets or clears `deleted_at` (soft delete and Undo) and stamps `updated_at`, so a sync sees the
/// newer state. `edited` is its own flag: deleting and restoring never makes a Comment look edited.
pub fn set_deleted(
    conn: &Connection,
    id: &str,
    deleted_at: Option<i64>,
    now: i64,
) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_comments SET deleted_at = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, deleted_at, now],
    )?;
    Ok(())
}
