//! Editing a Task: its text, Priority, Won't do, deleting and bringing it back, and the purge of
//! what was deleted long ago. Rules, persistence and change-log records in one transaction.
//! Task and Comment text must never reach a log line.

use mnemora_core::changelog::{self, ChangeOp};
use mnemora_core::error::AppResult;
use mnemora_core::util::now_ms;
use rusqlite::Connection;

use crate::comment_service;
use crate::error::TodoError;
use crate::list_repo;
use crate::repeat::rule_of;
use crate::service::{ENTITY, record, record_at, record_session};
use crate::task::{Task, TaskState, validate_priority, validate_title};
use crate::{comment_repo, repo};

/// Deleted Tasks, Lists and Comments are erased this long after deletion (30 days).
const PURGE_AFTER_MS: i64 = 30 * 24 * 60 * 60 * 1000;

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
/// skipped instead (see `skip_task`). Reopening brings it back.
pub fn wont_do(conn: &mut Connection, id: &str) -> AppResult<Task> {
    let tx = conn.transaction()?;
    let mut task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    if task.state != TaskState::Open {
        return Err(TodoError::Validation("That Task is already closed").into());
    }
    if rule_of(&task)?.is_some() {
        return Err(TodoError::Validation("A repeating Task is skipped instead").into());
    }
    let now = now_ms();
    task.state = TaskState::Wont;
    task.closed_at = Some(now);
    task.updated_at = now;
    repo::update_state(&tx, &task)?;
    record(&tx, &task, ChangeOp::Tick)?;
    tx.commit()?;
    Ok(task)
}

/// Deletes a Task at once (soft: it is erased after 30 days); `restore_task` is Undo. Only this
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

/// What a purge erased.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Purged {
    /// Tasks erased.
    pub tasks: usize,
    /// Lists erased.
    pub lists: usize,
    /// Comments erased.
    pub comments: usize,
}

/// Permanently erases Tasks, Lists and Comments deleted over 30 days ago, with their change-log
/// records, in one transaction. A Task takes its Comments with it; its occurrences stay as plain
/// done Tasks. Run once at app start.
pub fn purge_deleted(conn: &mut Connection) -> AppResult<Purged> {
    let cutoff = now_ms() - PURGE_AFTER_MS;
    let tx = conn.transaction()?;
    let task_ids = repo::deleted_before(&tx, cutoff)?;
    let list_ids = list_repo::deleted_before(&tx, cutoff)?;
    let comment_ids = comment_repo::purgeable(&tx, cutoff, &task_ids)?;
    for id in &comment_ids {
        comment_repo::remove(&tx, id)?;
    }
    for id in &task_ids {
        repo::remove(&tx, id)?;
    }
    for id in &list_ids {
        list_repo::remove(&tx, id)?;
    }
    changelog::purge(&tx, ENTITY, &task_ids)?;
    changelog::purge(&tx, comment_service::ENTITY, &comment_ids)?;
    changelog::purge(&tx, crate::list_service::ENTITY, &list_ids)?;
    tx.commit()?;
    Ok(Purged {
        tasks: task_ids.len(),
        lists: list_ids.len(),
        comments: comment_ids.len(),
    })
}
