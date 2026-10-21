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
