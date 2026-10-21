//! Moving things in the manual order: a Task between two neighbours (possibly into another group
//! of the view, which changes the Task's Due date, Priority or List), and a List between two.
//! One row changes per move (ADR 0012). Task text never reaches a log line.

use mnemora_core::changelog::ChangeOp;
use mnemora_core::error::AppResult;
use mnemora_core::util::now_ms;
use rusqlite::Connection;

use crate::arrange::GroupKey;
use crate::error::TodoError;
use crate::list::TodoList;
use crate::order::key_between;
use crate::repeat::rule_of;
use crate::schedule::assign_when;
use crate::service::record_at;
use crate::task::{Task, TaskState, validate_priority};
use crate::when::{LocalNow, validate_pickable};
use crate::{list_repo, repo};

/// The sort key that puts something between its neighbours, or none to leave it where it is
/// (no neighbours). `before` and `after` are the neighbours' keys: the one directly above and the
/// one directly below the new place. The views can disagree with the manual order (a sort, or
/// Priority inside a time block), so the neighbours may be out of order or not adjacent: the new
/// key then goes just after `before`, or just before `after`, in the whole order.
pub(crate) fn place_key(
    before: Option<&str>,
    after: Option<&str>,
    key_before: impl Fn(&str) -> AppResult<Option<String>>,
    key_following: impl Fn(&str) -> AppResult<Option<String>>,
) -> AppResult<Option<String>> {
    let (lo, hi) = match (before, after) {
        (None, None) => return Ok(None),
        (Some(b), Some(a)) if b.trim_end_matches('0') < a.trim_end_matches('0') => {
            (Some(b.to_owned()), Some(a.to_owned()))
        }
        (Some(b), _) => (Some(b.to_owned()), key_following(b)?),
        (None, Some(a)) => (key_before(a)?, Some(a.to_owned())),
    };
    // Deleted rows keep their keys, so the neighbour may be the one that is moving: a key between
    // `lo` and it is still on the right side of everything else.
    Ok(Some(key_between(lo.as_deref(), hi.as_deref())?))
}

/// Moves an open Task between two neighbours, and into a group: `target` is what the group
/// adopts (see `TaskGroup::adopt`). A day sets the Due date (never a past one: that is refused,
/// and Overdue adopts nothing), a Priority sets the Priority, a List moves it there. One change
/// is recorded: `move` when the List changed, `update` otherwise; nothing when nothing changed.
pub fn reorder_task(
    conn: &mut Connection,
    id: &str,
    before: Option<&str>,
    after: Option<&str>,
    target: &[GroupKey],
    local: &LocalNow,
) -> AppResult<Task> {
    local.check()?;
    for key in target {
        if let GroupKey::Day { date } = key {
            validate_pickable(date, &local.today)?;
        }
        if let GroupKey::Priority { priority } = key {
            validate_priority(*priority)?;
        }
    }
    let tx = conn.transaction()?;
    let mut task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    if task.state != TaskState::Open {
        return Err(TodoError::Validation("That Task is already closed").into());
    }
    let neighbour = |n: Option<&str>| -> AppResult<Option<String>> {
        match n.filter(|n| *n != id) {
            None => Ok(None),
            Some(n) => Ok(Some(
                repo::get_any(&tx, n)?
                    .ok_or(TodoError::TaskNotFound)?
                    .sort_key,
            )),
        }
    };
    let (before_key, after_key) = (neighbour(before)?, neighbour(after)?);
    let (mut key_changed, mut when_changed, mut priority_changed, mut list_changed) =
        (false, false, false, false);
    if let Some(key) = place_key(
        before_key.as_deref(),
        after_key.as_deref(),
        |k| repo::key_before(&tx, k),
        |k| repo::key_following(&tx, k),
    )? && key != task.sort_key
    {
        task.sort_key = key;
        key_changed = true;
    }
    for key in target {
        match key {
            GroupKey::Day { date } if task.due_date.as_deref() != Some(date.as_str()) => {
                let (time, rule) = (task.due_time.clone(), rule_of(&task)?);
                when_changed |= assign_when(&mut task, Some(date.clone()), time, rule, false)?;
            }
            GroupKey::Priority { priority } if task.priority != *priority => {
                task.priority = *priority;
                priority_changed = true;
            }
            GroupKey::List { list_id } if task.list_id != *list_id => {
                if let Some(list_id) = list_id {
                    list_repo::get(&tx, list_id)?.ok_or(TodoError::ListNotFound)?;
                }
                task.list_id.clone_from(list_id);
                list_changed = true;
            }
            _ => {}
        }
    }
    if key_changed || when_changed || priority_changed || list_changed {
        task.updated_at = now_ms();
        if key_changed {
            repo::update_sort_key(&tx, &task)?;
        }
        if when_changed {
            repo::update_when(&tx, &task)?;
        }
        if priority_changed {
            repo::update_priority(&tx, &task)?;
        }
        if list_changed {
            repo::update_list(&tx, &task)?;
        }
        let op = if list_changed {
            ChangeOp::Move
        } else {
            ChangeOp::Update
        };
        record_at(&tx, &task, op, None)?;
    }
    tx.commit()?;
    Ok(task)
}

/// Moves a List between two neighbours in the manual order. Records an `update` change.
pub fn reorder_list(
    conn: &mut Connection,
    id: &str,
    before: Option<&str>,
    after: Option<&str>,
) -> AppResult<TodoList> {
    let tx = conn.transaction()?;
    let mut list = list_repo::get(&tx, id)?.ok_or(TodoError::ListNotFound)?;
    let neighbour = |n: Option<&str>| -> AppResult<Option<String>> {
        match n.filter(|n| *n != id) {
            None => Ok(None),
            Some(n) => Ok(Some(
                list_repo::get(&tx, n)?
                    .ok_or(TodoError::ListNotFound)?
                    .sort_key,
            )),
        }
    };
    let (before_key, after_key) = (neighbour(before)?, neighbour(after)?);
    if let Some(key) = place_key(
        before_key.as_deref(),
        after_key.as_deref(),
        |k| list_repo::key_before(&tx, k),
        |k| list_repo::key_following(&tx, k),
    )? && key != list.sort_key
    {
        list.sort_key = key;
        list.updated_at = now_ms();
        list_repo::update_sort_key(&tx, &list)?;
        crate::list_service::record_list(&tx, &list, ChangeOp::Update, None)?;
    }
    tx.commit()?;
    Ok(list)
}
