//! SQL for the Tasks table. No rules here; see `service.rs` and `task.rs`.

use mnemora_core::error::{AppError, AppResult, ErrorCode};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::task::{Task, TaskState};

pub(crate) const COLUMNS: &str =
    "id, list_id, title, notes, due_date, due_time, repeat_freq, repeat_every,
    repeat_days, repeat_mode, priority, state, closed_at, repeat_of, understood, sort_key,
    created_at, updated_at,
    (SELECT COUNT(*) FROM todo_comments c
      WHERE c.task_id = todo_tasks.id AND c.deleted_at IS NULL)";

pub(crate) fn from_row(r: &Row<'_>) -> rusqlite::Result<Task> {
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
        comment_count: r.get(18)?,
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

/// Inserts an occurrence copy, or, when its derived id already exists (a reopened occurrence being
/// completed again), rewrites that row and clears its deletion. Returns whether the row was there.
pub fn upsert_occurrence(conn: &Connection, t: &Task) -> AppResult<bool> {
    let existed = conn
        .query_row(
            "SELECT 1 FROM todo_tasks WHERE id = ?1",
            [&t.id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if existed {
        conn.execute(
            "UPDATE todo_tasks SET list_id = ?2, title = ?3, notes = ?4, due_date = ?5,
                 due_time = ?6, priority = ?7, state = 'done', closed_at = ?8, sort_key = ?9,
                 updated_at = MAX(?10, updated_at + 1), deleted_at = NULL
             WHERE id = ?1",
            params![
                t.id,
                t.list_id,
                t.title,
                t.notes,
                t.due_date,
                t.due_time,
                t.priority,
                t.closed_at,
                t.sort_key,
                t.updated_at
            ],
        )?;
    } else {
        insert(conn, t)?;
    }
    Ok(existed)
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
pub(crate) fn corrupt_or_db(e: rusqlite::Error) -> AppError {
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

/// Writes the Due date and time and the Repeat, and stamps `updated_at`. The caller has already
/// settled the rules: without a date there is no Repeat.
pub fn update_when(conn: &Connection, t: &Task) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_tasks SET due_date = ?2, due_time = ?3, repeat_freq = ?4, repeat_every = ?5,
             repeat_days = ?6, repeat_mode = ?7, updated_at = ?8
         WHERE id = ?1",
        params![
            t.id,
            t.due_date,
            t.due_time,
            t.repeat_freq,
            t.repeat_every,
            t.repeat_days,
            t.repeat_mode,
            t.updated_at
        ],
    )?;
    Ok(())
}

/// Writes a repeating Task's notes (Checklist reset or put back) and Due date, and stamps
/// `updated_at`. The Repeat itself stays as it is.
pub fn update_notes_and_due(conn: &Connection, t: &Task) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_tasks SET notes = ?2, due_date = ?3, due_time = ?4, updated_at = ?5
         WHERE id = ?1",
        params![t.id, t.notes, t.due_date, t.due_time, t.updated_at],
    )?;
    Ok(())
}

/// Gives the Comments of one Task to another and stamps their `updated_at` (at least a
/// millisecond past what it was, so the move always reads as newer). Returns their ids (deleted
/// ones too: they come back with Undo).
pub fn move_comments(conn: &Connection, from: &str, to: &str, now: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM todo_comments WHERE task_id = ?1 ORDER BY id")?;
    let ids: Vec<String> = stmt
        .query_map([from], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    conn.execute(
        "UPDATE todo_comments SET task_id = ?2, updated_at = MAX(?3, updated_at + 1)
         WHERE task_id = ?1",
        params![from, to, now],
    )?;
    Ok(ids)
}

/// The newest done occurrence copy of a repeating Task, if any is left.
pub fn latest_occurrence(conn: &Connection, task_id: &str) -> AppResult<Option<Task>> {
    conn.query_row(
        &format!(
            "SELECT {COLUMNS} FROM todo_tasks
             WHERE repeat_of = ?1 AND state = 'done' AND deleted_at IS NULL
             ORDER BY closed_at DESC, due_date DESC, id DESC LIMIT 1"
        ),
        [task_id],
        from_row,
    )
    .optional()
    .map_err(corrupt_or_db)
}

/// The days on which something was done or marked Won't do, newest first, strictly before `before` (a local day,
/// `YYYY-MM-DD`) when given. Days are the device's: closing times are UTC, shifted by the offset.
pub fn done_days(
    conn: &Connection,
    before: Option<&str>,
    offset_min: i32,
    limit: u32,
) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT date((closed_at + ?1 * 60000) / 1000, 'unixepoch') FROM todo_tasks
         WHERE state IN ('done', 'wont') AND closed_at IS NOT NULL AND deleted_at IS NULL
           AND (?2 IS NULL OR date((closed_at + ?1 * 60000) / 1000, 'unixepoch') < ?2)
         ORDER BY 1 DESC LIMIT ?3",
    )?;
    Ok(stmt
        .query_map(params![offset_min, before, limit], |r| r.get(0))?
        .collect::<Result<_, _>>()?)
}

/// Live Tasks done or marked Won't do in the UTC millisecond range `[start, end)`, newest first.
pub fn done_between_newest_first(conn: &Connection, start: i64, end: i64) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM todo_tasks
         WHERE state IN ('done', 'wont') AND closed_at >= ?1 AND closed_at < ?2 AND deleted_at IS NULL
         ORDER BY closed_at DESC, sort_key DESC, id DESC"
    ))?;
    stmt.query_map([start, end], from_row)?
        .collect::<Result<_, _>>()
        .map_err(corrupt_or_db)
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

/// How many live Comments a Task has.
pub fn count_comments(conn: &Connection, task_id: &str) -> AppResult<u32> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM todo_comments WHERE task_id = ?1 AND deleted_at IS NULL",
        [task_id],
        |r| r.get(0),
    )?)
}

/// Writes the title and notes, and stamps `updated_at`.
pub fn update_text(conn: &Connection, t: &Task) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_tasks SET title = ?2, notes = ?3, updated_at = ?4 WHERE id = ?1",
        params![t.id, t.title, t.notes, t.updated_at],
    )?;
    Ok(())
}

/// Writes the Priority, and stamps `updated_at`.
pub fn update_priority(conn: &Connection, t: &Task) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_tasks SET priority = ?2, updated_at = ?3 WHERE id = ?1",
        params![t.id, t.priority, t.updated_at],
    )?;
    Ok(())
}

/// Writes everything Quick edit can change at once: title, List, When, Repeat, Priority and the
/// record of what was understood, and stamps `updated_at`.
pub fn update_quick(conn: &Connection, t: &Task) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_tasks SET title = ?2, list_id = ?3, due_date = ?4, due_time = ?5,
             repeat_freq = ?6, repeat_every = ?7, repeat_days = ?8, repeat_mode = ?9,
             priority = ?10, understood = ?11, updated_at = ?12
         WHERE id = ?1",
        params![
            t.id,
            t.title,
            t.list_id,
            t.due_date,
            t.due_time,
            t.repeat_freq,
            t.repeat_every,
            t.repeat_days,
            t.repeat_mode,
            t.priority,
            t.understood,
            t.updated_at
        ],
    )?;
    Ok(())
}

/// The greatest sort key below `key`, deleted Tasks included: the neighbour a move goes after.
pub fn key_before(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row(
        "SELECT MAX(sort_key) FROM todo_tasks WHERE sort_key < ?1",
        [key],
        |r| r.get(0),
    )?)
}

/// The least sort key above `key`, deleted Tasks included: the neighbour a move goes before.
pub fn key_following(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row(
        "SELECT MIN(sort_key) FROM todo_tasks WHERE sort_key > ?1",
        [key],
        |r| r.get(0),
    )?)
}

/// Writes the sort key, and stamps `updated_at`.
pub fn update_sort_key(conn: &Connection, t: &Task) -> AppResult<()> {
    conn.execute(
        "UPDATE todo_tasks SET sort_key = ?2, updated_at = ?3 WHERE id = ?1",
        params![t.id, t.sort_key, t.updated_at],
    )?;
    Ok(())
}

/// Live Tasks of one List that were done or marked Won't do, newest first.
pub fn closed_in_list(conn: &Connection, list_id: &str) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM todo_tasks
         WHERE list_id = ?1 AND state IN ('done', 'wont') AND closed_at IS NOT NULL
           AND deleted_at IS NULL
         ORDER BY closed_at DESC, sort_key DESC, id DESC"
    ))?;
    stmt.query_map([list_id], from_row)?
        .collect::<Result<_, _>>()
        .map_err(corrupt_or_db)
}

/// Writes the named columns of a Task, whatever its state or deletion (applying values received
/// from another device). The names are the caller's own constants, never user input.
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
            "UPDATE todo_tasks SET {} WHERE id = ?1",
            assignments.join(", ")
        ),
        values.as_slice(),
    )?;
    Ok(())
}

/// Inserts a Task received from another device with only what it must have; the rest of what
/// it sent is written by `update_columns`. It starts deleted (hidden) until its `deleted_at`
/// has been applied, so a half-built Task is never visible.
pub fn insert_remote(
    conn: &Connection,
    id: &str,
    title: &str,
    sort_key: &str,
    created_at: i64,
    now: i64,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO todo_tasks(id, title, sort_key, created_at, updated_at, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, title, sort_key, created_at, now],
    )?;
    Ok(())
}

/// A Task's `deleted_at`, `None` when it is live or not there.
pub fn deleted_at(conn: &Connection, id: &str) -> AppResult<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT deleted_at FROM todo_tasks WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .optional()?
        .flatten())
}

/// Live Tasks whose newest choice of List (the latest change-log record that set it, by clock)
/// is `list_id`, with the List each is in now: `(task id, column)`. The log is the same on every
/// device once synced, so is the answer.
pub fn chosen_list_is(
    conn: &Connection,
    list_id: &str,
) -> AppResult<Vec<(String, Option<String>)>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.list_id FROM todo_tasks t
         JOIN (
             SELECT entity_id,
                    CASE WHEN fields IS NULL THEN json_extract(snapshot, '$.list_id')
                         ELSE json_extract(fields, '$.list_id') END AS chosen,
                    ROW_NUMBER() OVER (
                        PARTITION BY entity_id
                        ORDER BY hlc_ms DESC, hlc_counter DESC, origin_device DESC,
                                 origin_seq DESC) AS newest
             FROM change_log
             WHERE entity = ?2
               AND (fields IS NULL
                    OR EXISTS (SELECT 1 FROM json_each(change_log.fields) WHERE key = 'list_id'))
         ) c ON c.entity_id = t.id AND c.newest = 1
         WHERE t.deleted_at IS NULL AND c.chosen = ?1",
    )?;
    stmt.query_map(params![list_id, crate::service::ENTITY], |r| {
        Ok((r.get(0)?, r.get(1)?))
    })?
    .collect::<Result<_, _>>()
    .map_err(corrupt_or_db)
}
