//! Todo's operations: rules, persistence and change-log records in one transaction.
//! Task titles and snapshots must never reach a log line.

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::error::AppResult;
use mnemora_core::util::{new_id, now_ms};
use rusqlite::Connection;

use crate::MODULE_ID;
use crate::error::TodoError;
use crate::repo;
use crate::task::{InboxView, Task, TaskState, key_after, validate_title};

/// Change-log entity name for Tasks.
const ENTITY: &str = "todo_task";

/// Writes a change-log record. The snapshot is the Task plus its `deleted_at`, which the Task
/// type does not carry: a delete record shows when it was deleted, every other one shows null.
fn record(conn: &Connection, task: &Task, op: ChangeOp) -> AppResult<()> {
    let mut snapshot = serde_json::to_value(task).map_err(TodoError::Snapshot)?;
    if let Some(fields) = snapshot.as_object_mut() {
        fields.insert("deleted_at".into(), serde_json::Value::Null);
    }
    changelog::record(
        conn,
        &Change {
            module: MODULE_ID.into(),
            entity: ENTITY.into(),
            entity_id: task.id.clone(),
            op,
            session_id: None,
            snapshot,
        },
    )
}

/// The Inbox: open Tasks with no List, in manual order (new Tasks last).
pub fn inbox(conn: &Connection) -> AppResult<InboxView> {
    Ok(InboxView {
        tasks: repo::inbox(conn)?,
    })
}

/// Adds an open Task with no List at the bottom of the order.
pub fn create_task(conn: &mut Connection, title: &str) -> AppResult<Task> {
    let title = validate_title(title)?;
    let tx = conn.transaction()?;
    let now = now_ms();
    let task = Task {
        id: new_id(),
        list_id: None,
        title: title.to_owned(),
        notes: String::new(),
        due_date: None,
        due_time: None,
        repeat_freq: None,
        repeat_every: None,
        repeat_days: None,
        repeat_mode: None,
        priority: 4,
        state: TaskState::Open,
        closed_at: None,
        repeat_of: None,
        understood: None,
        sort_key: key_after(repo::last_sort_key(&tx)?.as_deref()),
        created_at: now,
        updated_at: now,
    };
    repo::insert(&tx, &task)?;
    record(&tx, &task, ChangeOp::Create)?;
    tx.commit()?;
    Ok(task)
}

/// Marks an open Task done, now.
pub fn complete_task(conn: &mut Connection, id: &str) -> AppResult<Task> {
    let tx = conn.transaction()?;
    let mut task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    if task.state != TaskState::Open {
        return Err(TodoError::Validation("That Task is already closed").into());
    }
    let now = now_ms();
    task.state = TaskState::Done;
    task.closed_at = Some(now);
    task.updated_at = now;
    repo::update_state(&tx, &task)?;
    record(&tx, &task, ChangeOp::Tick)?;
    tx.commit()?;
    Ok(task)
}

/// Brings a done (or Won't do) Task back to open, in its old place.
pub fn reopen_task(conn: &mut Connection, id: &str) -> AppResult<Task> {
    let tx = conn.transaction()?;
    let mut task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    if task.state == TaskState::Open {
        return Err(TodoError::Validation("That Task is already open").into());
    }
    task.state = TaskState::Open;
    task.closed_at = None;
    task.updated_at = now_ms();
    repo::update_state(&tx, &task)?;
    record(&tx, &task, ChangeOp::Tick)?;
    tx.commit()?;
    Ok(task)
}
