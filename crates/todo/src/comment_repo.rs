//! SQL for the Comments table. No rules here; see `comment_service.rs`.

use mnemora_core::error::AppResult;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::comment::Comment;

const COLUMNS: &str = "id, task_id, body, created_at, updated_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Comment> {
    let (created_at, updated_at): (i64, i64) = (r.get(3)?, r.get(4)?);
    Ok(Comment {
        id: r.get(0)?,
        task_id: r.get(1)?,
        body: r.get(2)?,
        edited: updated_at > created_at,
        created_at,
        updated_at,
    })
}

pub fn insert(conn: &Connection, c: &Comment) -> AppResult<()> {
    conn.execute(
        "INSERT INTO todo_comments(id, task_id, body, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![c.id, c.task_id, c.body, c.created_at, c.updated_at],
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
            |r| Ok((from_row(r)?, r.get(5)?)),
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
        "UPDATE todo_comments SET body = ?2, updated_at = ?3 WHERE id = ?1",
        params![c.id, c.body, c.updated_at],
    )?;
    Ok(())
}

/// Sets or clears `deleted_at` (soft delete and Undo). A deleted Comment keeps its `updated_at`,
/// so deleting and restoring never makes one look edited.
pub fn set_deleted(conn: &Connection, id: &str, deleted_at: Option<i64>) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_comments SET deleted_at = ?2 WHERE id = ?1",
        params![id, deleted_at],
    )?;
    Ok(())
}

/// Ids of Comments to erase: deleted before `cutoff`, or on a Task in `task_ids`.
pub fn purgeable(conn: &Connection, cutoff: i64, task_ids: &[String]) -> AppResult<Vec<String>> {
    let mut ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM todo_comments WHERE deleted_at < ?1")?;
        stmt.query_map([cutoff], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    let mut by_task = conn.prepare("SELECT id FROM todo_comments WHERE task_id = ?1")?;
    for task_id in task_ids {
        for id in by_task.query_map([task_id], |r| r.get::<_, String>(0))? {
            ids.push(id?);
        }
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// Erases a Comment row for good.
pub fn remove(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM todo_comments WHERE id = ?1", [id])?;
    Ok(())
}
