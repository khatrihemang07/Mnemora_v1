//! Todo's operations: rules, persistence and change-log records in one transaction.
//! Task titles and snapshots must never reach a log line.

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::error::AppResult;
use mnemora_core::util::{new_id, now_ms};
use rusqlite::Connection;

use crate::MODULE_ID;
use crate::arrange::{GroupContext, GroupKey, TaskGroup, ViewPrefs, compare, group_tasks};
use crate::error::TodoError;
use crate::order::key_after;
use crate::repeat::{self, Completion};
use crate::schedule::{assign_when, settle_when};
use crate::task::{
    InboxView, NewTaskDetails, Task, TaskState, validate_notes, validate_priority, validate_title,
    validate_understood,
};
use crate::when::{LocalNow, NewWhen};
use crate::{list_repo, repo};

/// Change-log entity name for Tasks.
pub(crate) const ENTITY: &str = "todo_task";

/// Writes a change-log record. The snapshot is the Task plus its `deleted_at`, which the Task
/// type does not carry: a delete record shows when it was deleted, every other one shows null.
pub(crate) fn record(conn: &Connection, task: &Task, op: ChangeOp) -> AppResult<()> {
    record_in(conn, task, op, None, None, false)
}

/// Like `record`, with the `deleted_at` the snapshot shows.
pub(crate) fn record_at(
    conn: &Connection,
    task: &Task,
    op: ChangeOp,
    deleted_at: Option<i64>,
) -> AppResult<()> {
    record_in(conn, task, op, deleted_at, None, false)
}

/// Like `record`, as part of an editing session: the session keeps a single record.
pub(crate) fn record_session(
    conn: &Connection,
    task: &Task,
    op: ChangeOp,
    session_id: &str,
) -> AppResult<()> {
    record_in(conn, task, op, None, Some(session_id), false)
}

/// Records a Skip: a `tick` (the Task moved on without closing) whose snapshot carries
/// `skipped: true`, which is how History tells it from completing an occurrence.
pub(crate) fn record_skip(conn: &Connection, task: &Task) -> AppResult<()> {
    record_in(conn, task, ChangeOp::Tick, None, None, true)
}

/// The change-log snapshot of a Task: the Task type plus its `deleted_at`, without the Comment
/// count (counted when read, never stored).
pub(crate) fn snapshot_of(
    task: &Task,
    deleted_at: Option<i64>,
    skipped: bool,
) -> AppResult<serde_json::Value> {
    let mut snapshot = serde_json::to_value(task).map_err(TodoError::Snapshot)?;
    if let Some(fields) = snapshot.as_object_mut() {
        fields.remove("comment_count");
        fields.insert("deleted_at".into(), deleted_at.into());
        if skipped {
            fields.insert("skipped".into(), true.into());
        }
    }
    Ok(snapshot)
}

fn record_in(
    conn: &Connection,
    task: &Task,
    op: ChangeOp,
    deleted_at: Option<i64>,
    session_id: Option<&str>,
    skipped: bool,
) -> AppResult<()> {
    let snapshot = snapshot_of(task, deleted_at, skipped)?;
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

/// The Inbox: open Tasks with no List, grouped as the Preference says and sorted inside each
/// group, then "No date, in a List" for the undated Tasks that sit in a List. Those do not count
/// toward the Inbox badge.
pub fn inbox(conn: &Connection, local: &LocalNow) -> AppResult<InboxView> {
    local.check()?;
    let prefs = ViewPrefs::read(conn)?;
    let lists = list_repo::all(conn)?;
    let ctx = GroupContext {
        lists: &lists,
        today: &local.today,
    };
    let cmp = compare(prefs.inbox_sort);
    let mut groups = group_tasks(repo::inbox(conn)?, prefs.group_inbox, &ctx, &cmp);
    let mut undated: Vec<Task> = repo::open_in_lists(conn)?
        .into_iter()
        .filter(|t| t.due_date.is_none())
        .collect();
    if !undated.is_empty() {
        undated.sort_by(cmp);
        groups.push(TaskGroup {
            key: GroupKey::NoDateInList,
            adopt: Vec::new(),
            tasks: undated,
        });
    }
    Ok(InboxView {
        group_by: prefs.group_inbox,
        sort: prefs.inbox_sort,
        groups,
    })
}

/// Adds an open Task at the bottom of the order, in a List or (with none) the Inbox. `when`
/// gives it a Due date and time under the same rules as `set_when`; `details` adds notes, a
/// Priority, a Repeat (which needs `when` for the clock) and the record of understood words.
pub fn create_task(
    conn: &mut Connection,
    title: &str,
    list_id: Option<&str>,
    when: Option<&NewWhen>,
    details: Option<&NewTaskDetails>,
) -> AppResult<Task> {
    let title = validate_title(title)?;
    let details = details.cloned().unwrap_or_default();
    validate_notes(&details.notes)?;
    let priority = validate_priority(details.priority.unwrap_or(4))?;
    let understood = validate_understood(details.understood.as_deref())?.map(str::to_owned);
    let (due_date, due_time, rule) = match (when, details.repeat.as_ref()) {
        (Some(w), repeat) => {
            let (date, time, rule) =
                settle_when(w.date.as_deref(), w.time.as_deref(), repeat, &w.local)?;
            (date, time, rule)
        }
        (None, Some(_)) => return Err(TodoError::Validation("A Repeat needs a date").into()),
        (None, None) => (None, None, None),
    };
    let tx = conn.transaction()?;
    if let Some(list_id) = list_id {
        list_repo::get(&tx, list_id)?.ok_or(TodoError::ListNotFound)?;
    }
    let now = now_ms();
    let mut task = Task {
        id: new_id(),
        list_id: list_id.map(str::to_owned),
        title: title.to_owned(),
        notes: details.notes,
        due_date: None,
        due_time: None,
        repeat_freq: None,
        repeat_every: None,
        repeat_days: None,
        repeat_mode: None,
        priority,
        state: TaskState::Open,
        closed_at: None,
        repeat_of: None,
        understood,
        sort_key: key_after(repo::last_sort_key(&tx)?.as_deref()),
        created_at: now,
        updated_at: now,
        comment_count: 0,
    };
    assign_when(&mut task, due_date, due_time, rule, false)?;
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
