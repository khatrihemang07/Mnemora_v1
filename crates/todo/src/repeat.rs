//! Repeating Tasks: completing one records the occurrence as a done copy and moves the same Task
//! to its next date; reopening the latest occurrence rewinds the Task; skipping moves it on and
//! records no occurrence (only a `tick` marked skipped). Also the Done view, which lists those copies with everything else done. Task
//! text must never reach a log line.

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::error::AppResult;
use mnemora_core::util::{new_id, now_ms};
use rusqlite::Connection;
use serde::Serialize;
use specta::Type;

use crate::error::TodoError;
use crate::recurrence::Rule;
use crate::service::{record, record_at, record_skip};
use crate::task::{Task, TaskState};
use crate::when::{DAY_MS, LocalNow, add_days, epoch_day, validate_date};
use crate::{MODULE_ID, repo};

/// What ticking a Task did.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct Completion {
    /// The Task: done, or for a repeating one still open on its next date.
    pub task: Task,
    /// For a repeating Task, the done copy that records this occurrence. Reopening it (the
    /// latest occurrence) is the Undo.
    pub occurrence: Option<Task>,
}

/// The Tasks done on one day of Done.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct DoneDay {
    /// The device's day, `YYYY-MM-DD`.
    pub date: String,
    /// Newest first.
    pub tasks: Vec<Task>,
}

/// One page of Done: whole days, newest first.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct DoneView {
    /// The days of this page.
    pub days: Vec<DoneDay>,
    /// Hand it back as the cursor for the next page; none when this was the last.
    pub next_cursor: Option<String>,
}

/// The most days one page of Done holds.
const MAX_PAGE_DAYS: u32 = 60;

/// A Task's Repeat, or none. A stored rule that no longer checks is the app's fault.
pub(crate) fn rule_of(task: &Task) -> AppResult<Option<Rule>> {
    let (Some(freq), Some(every), Some(mode)) =
        (&task.repeat_freq, task.repeat_every, &task.repeat_mode)
    else {
        return Ok(None);
    };
    let wire = crate::recurrence::RepeatRule {
        freq: freq.clone(),
        every,
        days: task.repeat_days,
        mode: mode.clone(),
    };
    wire.check().map(Some).map_err(|e| {
        mnemora_core::error::AppError::new(
            mnemora_core::error::ErrorCode::Internal,
            "Stored Task data is unreadable",
        )
        .with_details(e)
    })
}

/// The Checklist lines of some notes with every ticked item unticked (`- [x]`, `* [X]`, nested
/// ones too), so the next occurrence starts fresh.
pub fn reset_checklist(notes: &str) -> String {
    notes
        .split_inclusive('\n')
        .map(|line| {
            let body = line.trim_start();
            let indent = line.len() - body.len();
            let ticked = body.len() >= 5
                && matches!(body.as_bytes()[0], b'-' | b'*' | b'+')
                && body[1..].starts_with(" [")
                && matches!(body.as_bytes()[3], b'x' | b'X')
                && body.as_bytes()[4] == b']';
            if ticked {
                format!("{}{} [ ]{}", &line[..indent], &body[..1], &body[5..])
            } else {
                line.to_owned()
            }
        })
        .collect()
}

/// Ticks a repeating Task: records this occurrence as a done copy that takes the Task's Comments,
/// then moves the Task to its next date with its Checklist reset. The time is kept.
pub(crate) fn complete(
    conn: &Connection,
    mut task: Task,
    rule: Rule,
    local: &LocalNow,
) -> AppResult<Completion> {
    let due = task.due_date.clone().unwrap_or_else(|| local.today.clone());
    let next = rule.next_due(&due, &local.today)?;
    let now = now_ms();
    let occurrence = Task {
        id: new_id(),
        list_id: task.list_id.clone(),
        title: task.title.clone(),
        notes: task.notes.clone(),
        due_date: task.due_date.clone(),
        due_time: task.due_time.clone(),
        repeat_freq: None,
        repeat_every: None,
        repeat_days: None,
        repeat_mode: None,
        priority: task.priority,
        state: TaskState::Done,
        closed_at: Some(now),
        repeat_of: Some(task.id.clone()),
        understood: None,
        sort_key: task.sort_key.clone(),
        created_at: now,
        updated_at: now,
        comment_count: 0,
    };
    repo::insert(conn, &occurrence)?;
    record(conn, &occurrence, ChangeOp::Create)?;
    move_comments(conn, &task.id, &occurrence.id, now)?;
    let mut occurrence = occurrence;
    occurrence.comment_count = repo::count_comments(conn, &occurrence.id)?;
    task.comment_count = 0;
    task.notes = reset_checklist(&task.notes);
    task.due_date = Some(next);
    task.updated_at = now;
    repo::update_notes_and_due(conn, &task)?;
    record(conn, &task, ChangeOp::Tick)?;
    Ok(Completion {
        task,
        occurrence: Some(occurrence),
    })
}

/// Reopens an occurrence copy: only the latest one, and only while its Task is still open. The
/// Task goes back to that occurrence's date and time, takes the Comments back and gets its
/// Checklist ticks back unless the notes were edited since. The copy is deleted.
pub(crate) fn rewind(conn: &Connection, occurrence: Task, parent_id: &str) -> AppResult<Task> {
    let Some(mut task) = repo::get(conn, parent_id)? else {
        return Err(TodoError::Validation(
            "This Task was deleted, so its past occurrences can't be reopened",
        )
        .into());
    };
    let latest = repo::latest_occurrence(conn, parent_id)?;
    if latest.as_ref().map(|t| &t.id) != Some(&occurrence.id) {
        return Err(TodoError::Validation("Only the latest occurrence can be reopened").into());
    }
    if task.state != TaskState::Open {
        return Err(TodoError::Validation(
            "This Task has changed since, so the occurrence can't be reopened",
        )
        .into());
    }
    let now = now_ms();
    move_comments(conn, &occurrence.id, &task.id, now)?;
    task.comment_count = repo::count_comments(conn, &task.id)?;
    if task.notes == reset_checklist(&occurrence.notes) {
        task.notes.clone_from(&occurrence.notes);
    }
    task.due_date.clone_from(&occurrence.due_date);
    task.due_time.clone_from(&occurrence.due_time);
    task.updated_at = now;
    repo::update_notes_and_due(conn, &task)?;
    record(conn, &task, ChangeOp::Tick)?;
    repo::set_deleted(conn, &occurrence.id, Some(now), now)?;
    record_at(conn, &occurrence, ChangeOp::Delete, Some(now))?;
    Ok(task)
}

/// Hands Comments to another Task and records each move (ids only, never text).
fn move_comments(conn: &Connection, from: &str, to: &str, now: i64) -> AppResult<()> {
    for id in repo::move_comments(conn, from, to)? {
        changelog::record(
            conn,
            &Change {
                module: MODULE_ID.into(),
                entity: "todo_comment".into(),
                entity_id: id.clone(),
                op: ChangeOp::Move,
                session_id: None,
                snapshot: serde_json::json!({ "id": id, "task_id": to, "updated_at": now }),
            },
        )?;
    }
    Ok(())
}

/// Moves a repeating Task to its next date without recording anything done: "Skip to ‹date›".
/// The change log gets a `tick` marked `skipped` (not an occurrence copy), see `record_skip`.
pub fn skip_task(conn: &mut Connection, id: &str, local: &LocalNow) -> AppResult<Task> {
    local.check()?;
    let tx = conn.transaction()?;
    let mut task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    if task.state != TaskState::Open {
        return Err(TodoError::Validation("That Task is already closed").into());
    }
    let Some(rule) = rule_of(&task)? else {
        return Err(TodoError::Validation("Only a repeating Task can be skipped").into());
    };
    let due = task.due_date.clone().unwrap_or_else(|| local.today.clone());
    task.due_date = Some(rule.next_due(&due, &local.today)?);
    task.updated_at = now_ms();
    repo::update_when(&tx, &task)?;
    record_skip(&tx, &task)?;
    tx.commit()?;
    Ok(task)
}

/// The device's day of a UTC instant, `YYYY-MM-DD`.
fn local_day(ms: i64, offset_min: i32) -> Option<String> {
    add_days(
        "1970-01-01",
        (ms + i64::from(offset_min) * 60_000).div_euclid(DAY_MS),
    )
}

/// One page of Done: up to `days` days that have Tasks done, newest first, strictly before the
/// `before` day when given. Won't do is not listed yet. `local` carries the device's offset, which
/// decides what day a closing time falls on.
pub fn done(
    conn: &Connection,
    before: Option<&str>,
    days: u32,
    local: &LocalNow,
) -> AppResult<DoneView> {
    local.check()?;
    before.map(validate_date).transpose()?;
    let days = days.clamp(1, MAX_PAGE_DAYS);
    let mut found = repo::done_days(conn, before, local.offset_min, days + 1)?;
    let more = found.len() > days as usize;
    found.truncate(days as usize);
    let (Some(newest), Some(oldest)) = (found.first(), found.last()) else {
        return Ok(DoneView {
            days: Vec::new(),
            next_cursor: None,
        });
    };
    let shift = i64::from(local.offset_min) * 60_000;
    let start =
        epoch_day(oldest).ok_or(TodoError::Validation("Not a valid date"))? * DAY_MS - shift;
    let end =
        (epoch_day(newest).ok_or(TodoError::Validation("Not a valid date"))? + 1) * DAY_MS - shift;
    let mut groups: Vec<DoneDay> = Vec::new();
    for task in repo::done_between_newest_first(conn, start, end)? {
        let Some(date) = task
            .closed_at
            .and_then(|ms| local_day(ms, local.offset_min))
        else {
            continue;
        };
        match groups.last_mut() {
            Some(g) if g.date == date => g.tasks.push(task),
            _ => groups.push(DoneDay {
                date,
                tasks: vec![task],
            }),
        }
    }
    Ok(DoneView {
        next_cursor: more.then(|| oldest.clone()),
        days: groups,
    })
}

#[cfg(test)]
mod tests {
    use super::reset_checklist;

    #[test]
    fn ticked_checklist_items_are_unticked_and_the_rest_is_left_alone() {
        let notes =
            "Buy:\n- [x] milk\n- [ ] eggs\n  * [X] nested\n+ [x]\nplain - [x] text\n-[x] no space";
        assert_eq!(
            reset_checklist(notes),
            "Buy:\n- [ ] milk\n- [ ] eggs\n  * [ ] nested\n+ [ ]\nplain - [x] text\n-[x] no space"
        );
        assert_eq!(reset_checklist(""), "");
        assert_eq!(
            reset_checklist("- [x] a\r\n- [x] b\r\n"),
            "- [ ] a\r\n- [ ] b\r\n"
        );
    }
}
