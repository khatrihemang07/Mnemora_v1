//! Todo's operations: rules, persistence and change-log records in one transaction.
//! Task titles and snapshots must never reach a log line.

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::error::AppResult;
use mnemora_core::util::{new_id, now_ms};
use rusqlite::Connection;

use crate::MODULE_ID;
use crate::error::TodoError;
use crate::repeat::{self, Completion};
use crate::task::{InboxView, Task, TaskState, key_after, validate_title};
use crate::when::{LocalNow, NewWhen, resolve_when};
use crate::{list_repo, repo};

/// Change-log entity name for Tasks.
pub(crate) const ENTITY: &str = "todo_task";

/// Writes a change-log record. The snapshot is the Task plus its `deleted_at`, which the Task
/// type does not carry: a delete record shows when it was deleted, every other one shows null.
pub(crate) fn record(conn: &Connection, task: &Task, op: ChangeOp) -> AppResult<()> {
    record_in(conn, task, op, None, None)
}

/// Like `record`, with the `deleted_at` the snapshot shows.
pub(crate) fn record_at(
    conn: &Connection,
    task: &Task,
    op: ChangeOp,
    deleted_at: Option<i64>,
) -> AppResult<()> {
    record_in(conn, task, op, deleted_at, None)
}

/// Like `record`, as part of an editing session: the session keeps a single record.
pub(crate) fn record_session(
    conn: &Connection,
    task: &Task,
    op: ChangeOp,
    session_id: &str,
) -> AppResult<()> {
    record_in(conn, task, op, None, Some(session_id))
}

fn record_in(
    conn: &Connection,
    task: &Task,
    op: ChangeOp,
    deleted_at: Option<i64>,
    session_id: Option<&str>,
) -> AppResult<()> {
    let mut snapshot = serde_json::to_value(task).map_err(TodoError::Snapshot)?;
    if let Some(fields) = snapshot.as_object_mut() {
        fields.remove("comment_count");
        fields.insert("deleted_at".into(), deleted_at.into());
    }
    changelog::record(
        conn,
        &Change {
            module: MODULE_ID.into(),
            entity: ENTITY.into(),
            entity_id: task.id.clone(),
            op,
            session_id: session_id.map(str::to_owned),
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

/// Adds an open Task at the bottom of the order, in a List or (with none) the Inbox. `when`
/// gives it a Due date and time under the same rules as `set_when`.
pub fn create_task(
    conn: &mut Connection,
    title: &str,
    list_id: Option<&str>,
    when: Option<&NewWhen>,
) -> AppResult<Task> {
    let title = validate_title(title)?;
    let (due_date, due_time) = match when {
        Some(w) => resolve_when(w.date.as_deref(), w.time.as_deref(), &w.local)?,
        None => (None, None),
    };
    let tx = conn.transaction()?;
    if let Some(list_id) = list_id {
        list_repo::get(&tx, list_id)?.ok_or(TodoError::ListNotFound)?;
    }
    let now = now_ms();
    let task = Task {
        id: new_id(),
        list_id: list_id.map(str::to_owned),
        title: title.to_owned(),
        notes: String::new(),
        due_date,
        due_time,
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
        comment_count: 0,
    };
    repo::insert(&tx, &task)?;
    record(&tx, &task, ChangeOp::Create)?;
    tx.commit()?;
    Ok(task)
}

/// Marks an open Task done, now. A repeating Task is not closed: that occurrence is recorded as a
/// done copy and the Task moves to its next date (`local` says what today is for that).
pub fn complete_task(conn: &mut Connection, id: &str, local: &LocalNow) -> AppResult<Completion> {
    local.check()?;
    let tx = conn.transaction()?;
    let mut task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    if task.state != TaskState::Open {
        return Err(TodoError::Validation("That Task is already closed").into());
    }
    if let Some(rule) = repeat::rule_of(&task)? {
        let completion = repeat::complete(&tx, task, rule, local)?;
        tx.commit()?;
        return Ok(completion);
    }
    let now = now_ms();
    task.state = TaskState::Done;
    task.closed_at = Some(now);
    task.updated_at = now;
    repo::update_state(&tx, &task)?;
    record(&tx, &task, ChangeOp::Tick)?;
    tx.commit()?;
    Ok(Completion {
        task,
        occurrence: None,
    })
}

/// Brings a done (or Won't do) Task back to open, in its old place. Reopening an occurrence of a
/// repeating Task works only for the latest one and rewinds the Task: the Task is returned.
pub fn reopen_task(conn: &mut Connection, id: &str) -> AppResult<Task> {
    let tx = conn.transaction()?;
    let mut task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    if task.state == TaskState::Open {
        return Err(TodoError::Validation("That Task is already open").into());
    }
    if let (Some(parent), TaskState::Done) = (task.repeat_of.clone(), task.state) {
        let rewound = repeat::rewind(&tx, task, &parent)?;
        tx.commit()?;
        return Ok(rewound);
    }
    task.state = TaskState::Open;
    task.closed_at = None;
    task.updated_at = now_ms();
    repo::update_state(&tx, &task)?;
    record(&tx, &task, ChangeOp::Tick)?;
    tx.commit()?;
    Ok(task)
}

/// Puts a Task in a List, or in the Inbox when `list_id` is none. Undo is moving it back.
pub fn move_task(conn: &mut Connection, id: &str, list_id: Option<&str>) -> AppResult<Task> {
    let tx = conn.transaction()?;
    let mut task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    if let Some(list_id) = list_id {
        list_repo::get(&tx, list_id)?.ok_or(TodoError::ListNotFound)?;
    }
    if task.list_id.as_deref() == list_id {
        return Ok(task);
    }
    task.list_id = list_id.map(str::to_owned);
    task.updated_at = now_ms();
    repo::update_list(&tx, &task)?;
    record(&tx, &task, ChangeOp::Move)?;
    tx.commit()?;
    Ok(task)
}
