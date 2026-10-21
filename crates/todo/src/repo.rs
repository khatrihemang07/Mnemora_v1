//! SQL for the Tasks table. No rules here; see `service.rs` and `task.rs`.

use mnemora_core::error::{AppError, AppResult, ErrorCode};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::task::{Task, TaskState};

const COLUMNS: &str = "id, list_id, title, notes, due_date, due_time, repeat_freq, repeat_every,
    repeat_days, repeat_mode, priority, state, closed_at, repeat_of, understood, sort_key,
    created_at, updated_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Task> {
    let state: String = r.get(11)?;
    Ok(Task {
        id: r.get(0)?,
        list_id: r.get(1)?,
        title: r.get(2)?,
        notes: r.get(3)?,
        due_date: r.get(4)?,
        due_time: r.get(5)?,
        repeat_freq: r.get(6)?,
        repeat_every: r.get(7)?,
        repeat_days: r.get(8)?,
        repeat_mode: r.get(9)?,
        priority: r.get(10)?,
        state: TaskState::parse(&state).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                11,
                rusqlite::types::Type::Text,
                "unknown Task state".into(),
            )
        })?,
        closed_at: r.get(12)?,
        repeat_of: r.get(13)?,
        understood: r.get(14)?,
        sort_key: r.get(15)?,
        created_at: r.get(16)?,
        updated_at: r.get(17)?,
    })
}

pub fn insert(conn: &Connection, t: &Task) -> AppResult<()> {
    conn.execute(
        "INSERT INTO todo_tasks(id, list_id, title, notes, due_date, due_time, repeat_freq,
             repeat_every, repeat_days, repeat_mode, priority, state, closed_at, repeat_of,
             understood, sort_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            t.id,
            t.list_id,
            t.title,
            t.notes,
            t.due_date,
            t.due_time,
            t.repeat_freq,
            t.repeat_every,
            t.repeat_days,
            t.repeat_mode,
            t.priority,
            t.state.as_str(),
            t.closed_at,
            t.repeat_of,
            t.understood,
            t.sort_key,
            t.created_at,
            t.updated_at
        ],
    )?;
    Ok(())
}

/// A live (not deleted) Task.
pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Task>> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM todo_tasks WHERE id = ?1 AND deleted_at IS NULL"),
        [id],
        from_row,
    )
    .optional()
    .map_err(corrupt_or_db)
}

/// Writes the state and its close time, and stamps `updated_at`.
pub fn update_state(conn: &Connection, t: &Task) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_tasks SET state = ?2, closed_at = ?3, updated_at = ?4 WHERE id = ?1",
        params![t.id, t.state.as_str(), t.closed_at, t.updated_at],
    )?;
    Ok(())
}

/// The greatest sort key of any Task, deleted ones included: keys are never reused.
pub fn last_sort_key(conn: &Connection) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT MAX(sort_key) FROM todo_tasks", [], |r| r.get(0))?)
}

/// Open, live Tasks with no List, in manual order.
pub fn inbox(conn: &Connection) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM todo_tasks
         WHERE list_id IS NULL AND state = 'open' AND deleted_at IS NULL
         ORDER BY sort_key, id"
    ))?;
    stmt.query_map([], from_row)?
        .collect::<Result<_, _>>()
        .map_err(corrupt_or_db)
}

/// An unreadable stored value is the app's fault, not the user's.
fn corrupt_or_db(e: rusqlite::Error) -> AppError {
    match e {
        rusqlite::Error::FromSqlConversionFailure(..) => {
            AppError::new(ErrorCode::Internal, "Stored Task data is unreadable").with_details(e)
        }
        other => other.into(),
    }
}

/// Open, live Tasks of one List, in manual order.
pub fn open_in_list(conn: &Connection, list_id: &str) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM todo_tasks
         WHERE list_id = ?1 AND state = 'open' AND deleted_at IS NULL
         ORDER BY sort_key, id"
    ))?;
    stmt.query_map([list_id], from_row)?
        .collect::<Result<_, _>>()
        .map_err(corrupt_or_db)
}

/// Open, live Tasks that belong to any List, in manual order.
pub fn open_in_lists(conn: &Connection) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM todo_tasks
         WHERE list_id IS NOT NULL AND state = 'open' AND deleted_at IS NULL
         ORDER BY sort_key, id"
    ))?;
    stmt.query_map([], from_row)?
        .collect::<Result<_, _>>()
        .map_err(corrupt_or_db)
}

/// Live Task counts per List, `(list, open, all states)`; the `None` entry is the Inbox.
pub fn task_counts(conn: &Connection) -> AppResult<Vec<(Option<String>, u32, u32)>> {
    let mut stmt = conn.prepare(
        "SELECT list_id, COUNT(*) FILTER (WHERE state = 'open'), COUNT(*) FROM todo_tasks
         WHERE deleted_at IS NULL GROUP BY list_id",
    )?;
    Ok(stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?)
}

/// Every live Task of a List, whatever its state, in manual order.
pub fn in_list(conn: &Connection, list_id: &str) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM todo_tasks
         WHERE list_id = ?1 AND deleted_at IS NULL ORDER BY sort_key, id"
    ))?;
    stmt.query_map([list_id], from_row)?
        .collect::<Result<_, _>>()
        .map_err(corrupt_or_db)
}

/// A Task whether or not it is deleted.
pub fn get_any(conn: &Connection, id: &str) -> AppResult<Option<Task>> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM todo_tasks WHERE id = ?1"),
        [id],
        from_row,
    )
    .optional()
    .map_err(corrupt_or_db)
}

/// Puts a Task in a List (none is the Inbox), and stamps `updated_at`.
pub fn update_list(conn: &Connection, t: &Task) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_tasks SET list_id = ?2, updated_at = ?3 WHERE id = ?1",
        params![t.id, t.list_id, t.updated_at],
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
        "UPDATE todo_tasks SET deleted_at = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, deleted_at, now],
    )?;
    Ok(())
}

/// Writes the Due date and time, and stamps `updated_at`. Without a date there is no Repeat
/// either: a repeating Task always has a Due date.
pub fn update_when(conn: &Connection, t: &Task) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_tasks SET due_date = ?2, due_time = ?3, updated_at = ?4,
             repeat_freq = CASE WHEN ?2 IS NULL THEN NULL ELSE repeat_freq END,
             repeat_every = CASE WHEN ?2 IS NULL THEN NULL ELSE repeat_every END,
             repeat_days = CASE WHEN ?2 IS NULL THEN NULL ELSE repeat_days END,
             repeat_mode = CASE WHEN ?2 IS NULL THEN NULL ELSE repeat_mode END
         WHERE id = ?1",
        params![t.id, t.due_date, t.due_time, t.updated_at],
    )?;
    Ok(())
}

/// Open, live Tasks that have a Due date, by date then manual order. The views regroup them.
pub fn open_dated(conn: &Connection) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM todo_tasks
         WHERE due_date IS NOT NULL AND state = 'open' AND deleted_at IS NULL
         ORDER BY due_date, sort_key, id"
    ))?;
    stmt.query_map([], from_row)?
        .collect::<Result<_, _>>()
        .map_err(corrupt_or_db)
}

/// Open, live Task counts `(due before today, due today)`.
pub fn due_counts(conn: &Connection, today: &str) -> AppResult<(u32, u32)> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FILTER (WHERE due_date < ?1), COUNT(*) FILTER (WHERE due_date = ?1)
         FROM todo_tasks
         WHERE due_date IS NOT NULL AND state = 'open' AND deleted_at IS NULL",
        [today],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?)
}

/// Open, live Tasks due before `today`.
pub fn overdue(conn: &Connection, today: &str) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM todo_tasks
         WHERE due_date < ?1 AND state = 'open' AND deleted_at IS NULL
         ORDER BY due_date, sort_key, id"
    ))?;
    stmt.query_map([today], from_row)?
        .collect::<Result<_, _>>()
        .map_err(corrupt_or_db)
}

/// Live Tasks done in the UTC millisecond range `[start, end)`, earliest first. Won't do does not
/// count as done.
pub fn done_between(conn: &Connection, start: i64, end: i64) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM todo_tasks
         WHERE state = 'done' AND closed_at >= ?1 AND closed_at < ?2 AND deleted_at IS NULL
         ORDER BY closed_at, sort_key, id"
    ))?;
    stmt.query_map([start, end], from_row)?
        .collect::<Result<_, _>>()
        .map_err(corrupt_or_db)
}

/// How many open, live Tasks are due on each date in `[from, to]`, leaving out `exclude`.
pub fn day_counts(
    conn: &Connection,
    from: &str,
    to: &str,
    exclude: Option<&str>,
) -> AppResult<Vec<(String, u32)>> {
    let mut stmt = conn.prepare(
        "SELECT due_date, COUNT(*) FROM todo_tasks
         WHERE due_date BETWEEN ?1 AND ?2 AND state = 'open' AND deleted_at IS NULL
           AND (?3 IS NULL OR id <> ?3)
         GROUP BY due_date ORDER BY due_date",
    )?;
    Ok(stmt
        .query_map(params![from, to, exclude], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?)
}
