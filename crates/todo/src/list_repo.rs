//! SQL for the Lists table. No rules here; see `service.rs` and `list.rs`.

use mnemora_core::error::AppResult;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::arrange::{GroupBy, TaskSort};
use crate::list::TodoList;

const COLUMNS: &str = "id, name, emoji, sort_key, created_at, updated_at, task_sort, group_by";

/// An unreadable stored value is the app's fault; the row's column tells which.
fn unreadable(column: usize, what: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        format!("unknown List {what}").into(),
    )
}

fn from_row(r: &Row<'_>) -> rusqlite::Result<TodoList> {
    let sort: String = r.get(6)?;
    let group: Option<String> = r.get(7)?;
    Ok(TodoList {
        id: r.get(0)?,
        name: r.get(1)?,
        emoji: r.get(2)?,
        sort_key: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
        task_sort: TaskSort::parse(&sort).ok_or_else(|| unreadable(6, "sort"))?,
        group_by: group
            .as_deref()
            .map_or(Some(GroupBy::None), GroupBy::parse)
            .ok_or_else(|| unreadable(7, "grouping"))?,
    })
}

/// The `group_by` column: none is NULL.
fn stored_group(group_by: GroupBy) -> Option<&'static str> {
    (group_by != GroupBy::None).then(|| group_by.as_str())
}

pub fn insert(conn: &Connection, l: &TodoList, name_key: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO todo_lists(id, name, name_key, emoji, sort_key, created_at, updated_at,
             task_sort, group_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            l.id,
            l.name,
            name_key,
            l.emoji,
            l.sort_key,
            l.created_at,
            l.updated_at,
            l.task_sort.as_str(),
            stored_group(l.group_by)
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
            |r| Ok((from_row(r)?, r.get(8)?)),
        )
        .optional()?)
}

/// Every List whether or not it is deleted, with its `deleted_at`, by id. Lists parked while a
/// received change is settled are left out.
pub fn every(conn: &Connection) -> AppResult<Vec<(TodoList, Option<i64>)>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS}, deleted_at FROM todo_lists WHERE deleted_at IS NOT ?1 ORDER BY id"
    ))?;
    Ok(stmt
        .query_map([PARKED], |r| Ok((from_row(r)?, r.get(8)?)))?
        .collect::<Result<_, _>>()?)
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

/// The greatest sort key below `key`, deleted Lists included: the neighbour a move goes after.
pub fn key_before(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row(
        "SELECT MAX(sort_key) FROM todo_lists WHERE sort_key < ?1",
        [key],
        |r| r.get(0),
    )?)
}

/// The least sort key above `key`, deleted Lists included: the neighbour a move goes before.
pub fn key_following(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row(
        "SELECT MIN(sort_key) FROM todo_lists WHERE sort_key > ?1",
        [key],
        |r| r.get(0),
    )?)
}

/// Writes the sort key, and stamps `updated_at`.
pub fn update_sort_key(conn: &Connection, l: &TodoList) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_lists SET sort_key = ?2, updated_at = ?3 WHERE id = ?1",
        params![l.id, l.sort_key, l.updated_at],
    )?;
    Ok(())
}

/// Writes how the List's page sorts and groups, and stamps `updated_at`.
pub fn update_arrangement(conn: &Connection, l: &TodoList) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_lists SET task_sort = ?2, group_by = ?3, updated_at = ?4 WHERE id = ?1",
        params![
            l.id,
            l.task_sort.as_str(),
            stored_group(l.group_by),
            l.updated_at
        ],
    )?;
    Ok(())
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

/// `deleted_at` of a List received from another device that is waiting to find out whether its
/// name clashes with a live List (see `list_sync`): hidden like a deleted one, and never stored
/// past the transaction that wrote it. No real deletion happens at the Unix epoch.
pub const PARKED: i64 = 0;

/// Inserts a List received from another device with only what it must have, hidden (parked)
/// until the rest of what it sent is written by `update_columns`.
pub fn insert_remote(
    conn: &Connection,
    id: &str,
    name: &str,
    name_key: &str,
    sort_key: &str,
    created_at: i64,
    now: i64,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO todo_lists(id, name, name_key, sort_key, created_at, updated_at, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, name, name_key, sort_key, created_at, now, PARKED],
    )?;
    Ok(())
}

/// Writes the named columns of a List, whatever its deletion (applying values received from
/// another device). The names are the caller's own constants, never user input.
pub fn update_columns(
    conn: &Connection,
    id: &str,
    sets: &[(&str, &dyn rusqlite::ToSql)],
) -> AppResult<()> {
    if sets.is_empty() {
        return Ok(());
    }
    let assignments: Vec<String> = sets
        .iter()
        .enumerate()
        .map(|(i, (column, _))| format!("{column} = ?{}", i + 2))
        .collect();
    let mut values: Vec<&dyn rusqlite::ToSql> = vec![&id];
    values.extend(sets.iter().map(|(_, v)| *v));
    conn.execute(
        &format!(
            "UPDATE todo_lists SET {} WHERE id = ?1",
            assignments.join(", ")
        ),
        values.as_slice(),
    )?;
    Ok(())
}

/// The other live List that has this folded name, if any (there is at most one).
pub fn live_twin(conn: &Connection, name_key: &str, except: &str) -> AppResult<Option<TodoList>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM todo_lists
                 WHERE name_key = ?1 AND deleted_at IS NULL AND id <> ?2"
            ),
            params![name_key, except],
            from_row,
        )
        .optional()?)
}

/// Makes a parked List live.
pub fn unpark(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_lists SET deleted_at = NULL WHERE id = ?1 AND deleted_at = ?2",
        params![id, PARKED],
    )?;
    Ok(())
}

/// Where a List's Tasks belong now: the List itself while it is live; the live List it merged
/// into when it is gone and an older live List has its name; otherwise nowhere (the Inbox). Two
/// Lists with one name merge into the older, so this reads the same on every device from the
/// Lists alone.
pub fn home(conn: &Connection, id: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT COALESCE(
                 (SELECT l.id FROM todo_lists l WHERE l.id = ?1 AND l.deleted_at IS NULL),
                 (SELECT s.id FROM todo_lists n JOIN todo_lists s
                      ON s.name_key = n.name_key AND s.deleted_at IS NULL AND s.id < n.id
                  WHERE n.id = ?1 AND n.deleted_at IS NOT NULL))",
            [id],
            |r| r.get(0),
        )
        .optional()?
        .flatten())
}

/// Ids of every List, live or not, that has this folded name.
pub fn with_name_key(conn: &Connection, name_key: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM todo_lists WHERE name_key = ?1")?;
    Ok(stmt
        .query_map([name_key], |r| r.get(0))?
        .collect::<Result<_, _>>()?)
}
