//! The capture box's two commands: reading typed text against the Lists, and Quick edit, which
//! turns a row into the box and saves it in one step. Task text must never reach a log line.

use mnemora_core::changelog::ChangeOp;
use mnemora_core::error::AppResult;
use mnemora_core::util::now_ms;
use rusqlite::Connection;
use serde::Deserialize;
use specta::Type;

use crate::error::TodoError;
use crate::parse::{ListRef, ParseRequest, Parsed, parse};
use crate::recurrence::{RepeatRule, Rule};
use crate::repeat::rule_of;
use crate::schedule::{assign_when, settle_when};
use crate::service::record_at;
use crate::task::{Task, TaskState, validate_priority, validate_title, validate_understood};
use crate::when::{LocalNow, validate_date, validate_time};
use crate::{list_repo, repo};

/// Reads typed text. `#Name` is matched against the live Lists, in their order.
pub fn parse_text(conn: &Connection, request: &ParseRequest) -> AppResult<Parsed> {
    let lists: Vec<ListRef> = list_repo::all(conn)?
        .into_iter()
        .map(|l| ListRef {
            id: l.id,
            name: l.name,
        })
        .collect();
    Ok(parse(request, &lists))
}

/// Everything Quick edit can change, as the Task should be afterwards. What the box left alone is
/// passed back as it was.
#[derive(Debug, Clone, Deserialize, Type)]
pub struct QuickEdit {
    /// The title as typed.
    pub title: String,
    /// What the parser recognised in it, as JSON.
    pub understood: Option<String>,
    /// Due date, `YYYY-MM-DD`.
    pub due_date: Option<String>,
    /// Due time, `HH:MM`.
    pub due_time: Option<String>,
    /// The Repeat after the edit.
    pub repeat: Option<RepeatRule>,
    /// 1 to 4.
    pub priority: u32,
    /// The List, or none for the Inbox.
    pub list_id: Option<String>,
}

/// Saves a Quick edit in one write. A When that differs from the Task's goes through the same
/// rules as `set_when` (past dates refused); one that is the same is left alone, so an Overdue
/// Task can have its title edited. `restoring` is Undo: it puts the old values back, a past date
/// included. Nothing changed records nothing.
pub fn quick_edit(
    conn: &mut Connection,
    id: &str,
    edit: &QuickEdit,
    local: &LocalNow,
    restoring: bool,
) -> AppResult<Task> {
    let title = validate_title(&edit.title)?;
    let priority = validate_priority(edit.priority)?;
    let understood = validate_understood(edit.understood.as_deref())?.map(str::to_owned);
    let tx = conn.transaction()?;
    let mut task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    let wanted = edit.repeat.as_ref().map(RepeatRule::check).transpose()?;
    let same_when = task.due_date == edit.due_date
        && task.due_time == edit.due_time
        && rule_of(&task)?.map(Rule::to_wire) == wanted.map(Rule::to_wire);
    let changes_more = !same_when || task.priority != priority || task.list_id != edit.list_id;
    if changes_more && task.state != TaskState::Open {
        return Err(TodoError::Validation("That Task is already closed").into());
    }
    if let Some(list_id) = edit.list_id.as_deref() {
        list_repo::get(&tx, list_id)?.ok_or(TodoError::ListNotFound)?;
    }
    let before = task.clone();
    title.clone_into(&mut task.title);
    task.priority = priority;
    task.list_id.clone_from(&edit.list_id);
    task.understood = understood;
    if !same_when {
        let (date, time, rule) = if restoring {
            let rule = edit.repeat.as_ref().map(RepeatRule::check).transpose()?;
            edit.due_date.as_deref().map(validate_date).transpose()?;
            edit.due_time.as_deref().map(validate_time).transpose()?;
            (edit.due_date.clone(), edit.due_time.clone(), rule)
        } else {
            settle_when(
                edit.due_date.as_deref(),
                edit.due_time.as_deref(),
                edit.repeat.as_ref(),
                local,
            )?
        };
        let keep_day = task.due_date == date;
        assign_when(&mut task, date, time, rule, keep_day || restoring)?;
    }
    if task == before {
        return Ok(task);
    }
    task.updated_at = now_ms();
    repo::update_quick(&tx, &task)?;
    record_at(&tx, &task, ChangeOp::Update, None)?;
    tx.commit()?;
    Ok(task)
}
