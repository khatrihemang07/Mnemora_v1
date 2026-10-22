//! Editing a Task: its text, Priority, Won't do, deleting and bringing it back.
//! Rules, persistence and change-log records in one transaction.
//! Task and Comment text must never reach a log line.

use mnemora_core::changelog::ChangeOp;
use mnemora_core::error::AppResult;
use mnemora_core::util::now_ms;
use rusqlite::Connection;

use crate::error::TodoError;
use crate::list_repo;
use crate::repeat::{rule_of, skip_task};
use crate::repo;
use crate::service::{record, record_at, record_session};
use crate::task::{Task, TaskState, validate_notes, validate_priority, validate_title};
use crate::when::LocalNow;

/// One Task, in any state.
pub fn task(conn: &Connection, id: &str) -> AppResult<Task> {
    Ok(repo::get(conn, id)?.ok_or(TodoError::TaskNotFound)?)
}

/// Replaces a Task's title and notes: the Task sheet's autosave. Every save of one opening of the
/// sheet carries the same `session_id`, so the change log keeps one record for all of them. The
/// same text changes and records nothing.
pub fn update_task(
    conn: &mut Connection,
    id: &str,
    title: &str,
    notes: &str,
    session_id: &str,
) -> AppResult<Task> {
    let title = validate_title(title)?;
    validate_notes(notes)?;
    if session_id.trim().is_empty() {
        return Err(TodoError::Validation("An editing session needs an id").into());
    }
    let tx = conn.transaction()?;
    let mut task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    if task.title == title && task.notes == notes {
        return Ok(task);
    }
    title.clone_into(&mut task.title);
    notes.clone_into(&mut task.notes);
    task.updated_at = now_ms();
    repo::update_text(&tx, &task)?;
    record_session(&tx, &task, ChangeOp::Update, session_id)?;
    tx.commit()?;
    Ok(task)
}

/// Sets an open Task's Priority, 1 (highest) to 4 (none). The same Priority changes nothing.
pub fn set_priority(conn: &mut Connection, id: &str, priority: u32) -> AppResult<Task> {
    let priority = validate_priority(priority)?;
    let tx = conn.transaction()?;
    let mut task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    if task.state != TaskState::Open {
        return Err(TodoError::Validation("That Task is already closed").into());
    }
    if task.priority == priority {
        return Ok(task);
    }
    task.priority = priority;
    task.updated_at = now_ms();
    repo::update_priority(&tx, &task)?;
    record(&tx, &task, ChangeOp::Update)?;
    tx.commit()?;
    Ok(task)
}

/// Marks an open Task Won't do, now: it joins Done but never counts as done. A repeating Task is
/// skipped instead (`skip_task`, which is why this takes the device's day): "Won't do" on one
/// means "not this time". Reopening brings a Won't do Task back.
pub fn wont_do(conn: &mut Connection, id: &str, local: &LocalNow) -> AppResult<Task> {
    let task = repo::get(conn, id)?.ok_or(TodoError::TaskNotFound)?;
    if task.state != TaskState::Open {
        return Err(TodoError::Validation("That Task is already closed").into());
    }
    if rule_of(&task)?.is_some() {
        return skip_task(conn, id, local);
    }
    let tx = conn.transaction()?;
    let mut task = task;
    let now = now_ms();
    task.state = TaskState::Wont;
    task.closed_at = Some(now);
    task.updated_at = now;
    repo::update_state(&tx, &task)?;
    record(&tx, &task, ChangeOp::Tick)?;
    tx.commit()?;
    Ok(task)
}

/// Deletes a Task at once (soft: the row stays for good); `restore_task` is Undo. Only this
/// row goes: the occurrences of a repeating Task stay in Done, and Comments stay with their Task.
pub fn delete_task(conn: &mut Connection, id: &str) -> AppResult<()> {
    let tx = conn.transaction()?;
    let task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    let now = now_ms();
    repo::set_deleted(&tx, id, Some(now), now)?;
    record_at(&tx, &task, ChangeOp::Delete, Some(now))?;
    tx.commit()?;
    Ok(())
}

/// Brings a deleted Task back in its old place. If its List has been deleted meanwhile it returns
/// to the Inbox.
pub fn restore_task(conn: &mut Connection, id: &str) -> AppResult<Task> {
    let tx = conn.transaction()?;
    let mut task = repo::get_any(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    let deleted: Option<i64> = tx.query_row(
        "SELECT deleted_at FROM todo_tasks WHERE id = ?1",
        [id],
        |r| r.get(0),
    )?;
    if deleted.is_none() {
        return Err(TodoError::Validation("That Task isn't deleted").into());
    }
    let now = now_ms();
    repo::set_deleted(&tx, id, None, now)?;
    task.updated_at = now;
    let list_gone = match &task.list_id {
        Some(list_id) => list_repo::get(&tx, list_id)?.is_none(),
        None => false,
    };
    if list_gone {
        task.list_id = None;
        repo::update_list(&tx, &task)?;
    }
    record(&tx, &task, ChangeOp::Restore)?;
    tx.commit()?;
    Ok(repo::get(conn, id)?.unwrap_or(task))
}
