//! SQL for the Lists table. No rules here; see `service.rs` and `list.rs`.

use mnemora_core::error::AppResult;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::list::TodoList;

const COLUMNS: &str = "id, name, emoji, sort_key, created_at, updated_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<TodoList> {
    Ok(TodoList {
        id: r.get(0)?,
        name: r.get(1)?,
        emoji: r.get(2)?,
        sort_key: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
    })
}

pub fn insert(conn: &Connection, l: &TodoList, name_key: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO todo_lists(id, name, name_key, emoji, sort_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            l.id,
            l.name,
            name_key,
            l.emoji,
            l.sort_key,
            l.created_at,
            l.updated_at
        ],
    )?;
    Ok(())
}

/// A live (not deleted) List.
pub fn get(conn: &Connection, id: &str) -> AppResult<Option<TodoList>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM todo_lists WHERE id = ?1 AND deleted_at IS NULL"),
            [id],
            from_row,
        )
        .optional()?)
}

/// A List whether or not it is deleted, with its `deleted_at`.
pub fn get_any(conn: &Connection, id: &str) -> AppResult<Option<(TodoList, Option<i64>)>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS}, deleted_at FROM todo_lists WHERE id = ?1"),
            [id],
            |r| Ok((from_row(r)?, r.get(6)?)),
        )
        .optional()?)
}

/// Every live List, first to last.
pub fn all(conn: &Connection) -> AppResult<Vec<TodoList>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM todo_lists WHERE deleted_at IS NULL ORDER BY sort_key, id"
    ))?;
    Ok(stmt.query_map([], from_row)?.collect::<Result<_, _>>()?)
}

/// The greatest sort key of any List, deleted ones included: keys are never reused.
pub fn last_sort_key(conn: &Connection) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT MAX(sort_key) FROM todo_lists", [], |r| r.get(0))?)
}

/// Whether another live List (not `except`) already has this folded name.
pub fn name_taken(conn: &Connection, name_key: &str, except: Option<&str>) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM todo_lists
                       WHERE name_key = ?1 AND deleted_at IS NULL AND id IS NOT ?2)",
        params![name_key, except],
        |r| r.get(0),
    )?)
}

pub fn update_name(conn: &Connection, l: &TodoList, name_key: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_lists SET name = ?2, name_key = ?3, updated_at = ?4 WHERE id = ?1",
        params![l.id, l.name, name_key, l.updated_at],
    )?;
    Ok(())
}

pub fn update_emoji(conn: &Connection, l: &TodoList) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_lists SET emoji = ?2, updated_at = ?3 WHERE id = ?1",
        params![l.id, l.emoji, l.updated_at],
    )?;
    Ok(())
}

/// Sets or clears `deleted_at` (soft delete and Undo).
pub fn set_deleted(
    conn: &Connection,
    id: &str,
    deleted_at: Option<i64>,
    now: i64,
) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_lists SET deleted_at = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, deleted_at, now],
    )?;
    Ok(())
}
