//! Todo's side of the sync contract for Tasks (ADR 0017): which Task fields merge how, and how
//! values received from another device are written. Task text must never reach a log line.
//!
//! Merge rules (the engine applies them; this file declares them):
//! - `title` and `notes` are Text: newest wins for now, three-way merge later.
//! - `understood` (the recognised words) travels with `title`, so the words always come from the
//!   same device as the newest title.
//! - The **When** (Due date, time and every Repeat field) and the **state** (state and closed
//!   time) are atomic groups.
//! - `sort_key` is the manual order; `deleted_at` is an ordinary value, so a delete on one
//!   device and an edit on another both stand.
//! - `skipped` is not a column: it marks the `tick` of a Skip in the change log, and travels so
//!   that History reads "Skipped" on every device, not "Done".
//! - `updated_at` is recomputed here when values arrive.

use mnemora_core::changelog::ChangeOp;
use mnemora_core::error::AppResult;
use mnemora_core::syncable::{Applied, RemoteChange, Settled, SyncEntity, SyncEvent, SyncField};
use mnemora_core::util::now_ms;
use rusqlite::Connection;
use serde_json::{Map, Value};

use crate::order::key_after;
use crate::recurrence::RepeatRule;
use crate::service::{ENTITY, record_at, snapshot_of};
use crate::task::{
    TaskState, validate_notes, validate_priority, validate_title, validate_understood,
};
use crate::when::{validate_date, validate_time};
use crate::{list_repo, repo};

const TITLE: &str = "title";
const WHEN: &str = "when";
const STATE: &str = "state";

/// Everything Todo syncs: Tasks, Lists and Comments.
pub(crate) const ENTITIES: &[SyncEntity] =
    &[TASK, crate::list_sync::LIST, crate::comment_sync::COMMENT];

/// The Task entity's fields and how each merges.
pub(crate) const TASK: SyncEntity = SyncEntity {
    name: ENTITY,
    fields: &[
        SyncField::value("list_id"),
        SyncField::text("title").in_group(TITLE),
        SyncField::value("understood").in_group(TITLE),
        SyncField::text("notes"),
        SyncField::value("due_date").in_group(WHEN),
        SyncField::value("due_time").in_group(WHEN),
        SyncField::value("repeat_freq").in_group(WHEN),
        SyncField::value("repeat_every").in_group(WHEN),
        SyncField::value("repeat_days").in_group(WHEN),
        SyncField::value("repeat_mode").in_group(WHEN),
        SyncField::value("priority"),
        SyncField::value("state").in_group(STATE),
        SyncField::value("closed_at").in_group(STATE),
        SyncField::value("repeat_of"),
        SyncField::value("created_at"),
        SyncField::order("sort_key"),
        SyncField::value("deleted_at"),
        SyncField::value("skipped"),
        SyncField::derived("updated_at"),
    ],
};

/// A value to write that may be NULL (`Set(None)`), apart from "no value was sent".
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Set<T>(pub(crate) Option<T>);

/// A string or null.
pub(crate) fn text(v: &Value) -> Option<Set<String>> {
    match v {
        Value::Null => Some(Set(None)),
        Value::String(s) => Some(Set(Some(s.clone()))),
        _ => None,
    }
}

/// A whole number or null.
pub(crate) fn number(v: &Value) -> Option<Set<i64>> {
    match v {
        Value::Null => Some(Set(None)),
        other => other.as_i64().map(|n| Set(Some(n))),
    }
}

/// The When group, checked as `set_when` would check it (the date itself may be in the past: it
/// was valid where it was made).
struct When {
    due_date: Option<String>,
    due_time: Option<String>,
    repeat: Option<RepeatRule>,
}

/// What a set of received fields would write, every unit already checked. A unit that fails its
/// checks (a bug or a hostile peer) is left out whole.
#[derive(Default)]
struct Patch {
    list_id: Option<Set<String>>,
    title: Option<(String, Option<String>)>,
    notes: Option<String>,
    when: Option<When>,
    priority: Option<u32>,
    state: Option<(TaskState, Option<i64>)>,
    repeat_of: Option<Set<String>>,
    created_at: Option<i64>,
    sort_key: Option<String>,
    deleted_at: Option<Set<i64>>,
}

impl Patch {
    fn read(fields: &Map<String, Value>) -> Self {
        let get = |k: &str| fields.get(k);
        Self {
            list_id: get("list_id").and_then(text),
            title: title_group(fields),
            notes: get("notes")
                .and_then(Value::as_str)
                .filter(|n| validate_notes(n).is_ok())
                .map(str::to_owned),
            when: when_group(fields),
            priority: get("priority")
                .and_then(Value::as_u64)
                .and_then(|p| u32::try_from(p).ok())
                .filter(|p| validate_priority(*p).is_ok()),
            state: state_group(fields),
            repeat_of: get("repeat_of").and_then(text),
            created_at: get("created_at").and_then(Value::as_i64),
            sort_key: get("sort_key")
                .and_then(Value::as_str)
                .filter(|k| !k.is_empty())
                .map(str::to_owned),
            deleted_at: get("deleted_at").and_then(number),
        }
    }
}

fn title_group(fields: &Map<String, Value>) -> Option<(String, Option<String>)> {
    let title = fields.get("title")?.as_str()?;
    let title = validate_title(title).ok()?;
    let understood = text(fields.get("understood")?)?.0;
    validate_understood(understood.as_deref()).ok()?;
    Some((title.to_owned(), understood))
}

fn when_group(fields: &Map<String, Value>) -> Option<When> {
    let due_date = text(fields.get("due_date")?)?.0;
    let due_time = text(fields.get("due_time")?)?.0;
    let freq = text(fields.get("repeat_freq")?)?.0;
    let every = number(fields.get("repeat_every")?)?.0;
    let days = number(fields.get("repeat_days")?)?.0;
    let mode = text(fields.get("repeat_mode")?)?.0;
    if let Some(d) = &due_date {
        validate_date(d).ok()?;
    }
    if let Some(t) = &due_time {
        validate_time(t).ok()?;
    }
    let repeat = match (freq, every, mode) {
        (None, None, None) => None,
        (Some(freq), Some(every), Some(mode)) => {
            let rule = RepeatRule {
                freq,
                every: u32::try_from(every).ok()?,
                days: days.map(u32::try_from).transpose().ok()?,
                mode,
            };
            rule.check().ok()?;
            Some(rule)
        }
        _ => return None,
    };
    // No time and no Repeat without a date.
    if due_date.is_none() && (due_time.is_some() || repeat.is_some()) {
        return None;
    }
    Some(When {
        due_date,
        due_time,
        repeat,
    })
}

fn state_group(fields: &Map<String, Value>) -> Option<(TaskState, Option<i64>)> {
    let state = TaskState::parse(fields.get("state")?.as_str()?)?;
    let closed_at = number(fields.get("closed_at")?)?.0;
    // Open Tasks have no close time; closed ones always do.
    (matches!(state, TaskState::Open) == closed_at.is_none()).then_some((state, closed_at))
}

/// Applies the winning field values of one Task, creating it when it is new here.
pub(crate) fn apply(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let id = change.entity_id;
    let patch = Patch::read(change.fields);
    let before = repo::get_any(conn, id)?;
    let before_snapshot = match &before {
        Some(task) => Some(snapshot_of(task, repo::deleted_at(conn, id)?, false)?),
        None => None,
    };

    if before.is_none() {
        // A Task can only be made from its first record, which carries these.
        let (Some((title, _)), Some(sort_key), Some(created_at)) =
            (&patch.title, &patch.sort_key, patch.created_at)
        else {
            tracing::debug!(target: "sync", task = id, "record for an unknown Task without its creation, kept for relay only");
            return Ok(Applied {
                snapshot: Value::Object(change.fields.clone()),
                changed: false,
                events: Vec::new(),
            });
        };
        repo::insert_remote(conn, id, title, sort_key, created_at, now_ms())?;
    }

    let mut events = Vec::new();
    write(conn, id, patch, &mut events)?;
    keep_out_of_missing_lists(conn, id, &mut events)?;

    // The record's own marker, not state: it shows in this record's snapshot only.
    let skipped = change.fields.get("skipped") == Some(&Value::Bool(true));
    let snapshot_now = |conn: &Connection, skipped: bool| -> AppResult<Value> {
        match repo::get_any(conn, id)? {
            Some(task) => snapshot_of(&task, repo::deleted_at(conn, id)?, skipped),
            None => Ok(Value::Null),
        }
    };
    let changed = before_snapshot.as_ref().map(strip_updated)
        != Some(strip_updated(&snapshot_now(conn, false)?));
    if changed {
        // `updated_at` is derived: it says when this device last changed the row.
        let now = now_ms();
        repo::update_columns(conn, id, &[("updated_at", &now)])?;
    }
    Ok(Applied {
        snapshot: snapshot_now(conn, skipped)?,
        changed,
        events,
    })
}

/// A snapshot without `updated_at`, which is derived and says nothing about what changed.
pub(crate) fn strip_updated(snapshot: &Value) -> Value {
    let mut copy = snapshot.clone();
    if let Some(fields) = copy.as_object_mut() {
        fields.remove("updated_at");
    }
    copy
}

/// Writes every checked unit of `patch`.
fn write(conn: &Connection, id: &str, patch: Patch, events: &mut Vec<SyncEvent>) -> AppResult<()> {
    // Whether the Task is visible once this is written: only a visible Task is moved out of a
    // List that is gone. A deleted one keeps the List it was in (its row is there, deleted or
    // not), so that every device holds the same, whichever of the Task and the List it heard of
    // first, and Restore finds it where it was.
    let visible = match &patch.deleted_at {
        Some(Set(deleted)) => deleted.is_none(),
        None => repo::deleted_at(conn, id)?.is_none(),
    };
    let list_id = match patch.list_id {
        Some(Set(Some(list))) if visible => Some(Set(home_or_inbox(conn, id, &list, events)?)),
        Some(Set(Some(list))) => Some(Set(list_repo::get_any(conn, &list)?.map(|_| list))),
        other => other,
    };
    let repeat_of = match patch.repeat_of {
        Some(Set(Some(parent))) if repo::get_any(conn, &parent)?.is_none() => Some(Set(None)),
        other => other,
    };
    let mut sets: Vec<(&str, &dyn rusqlite::ToSql)> = Vec::new();
    if let Some((title, understood)) = &patch.title {
        sets.push(("title", title));
        sets.push(("understood", understood));
    }
    if let Some(notes) = &patch.notes {
        sets.push(("notes", notes));
    }
    let when_values;
    if let Some(when) = &patch.when {
        when_values = (
            when.repeat.as_ref().map(|r| r.freq.clone()),
            when.repeat.as_ref().map(|r| r.every),
            when.repeat.as_ref().and_then(|r| r.days),
            when.repeat.as_ref().map(|r| r.mode.clone()),
        );
        sets.push(("due_date", &when.due_date));
        sets.push(("due_time", &when.due_time));
        sets.push(("repeat_freq", &when_values.0));
        sets.push(("repeat_every", &when_values.1));
        sets.push(("repeat_days", &when_values.2));
        sets.push(("repeat_mode", &when_values.3));
    }
    let state_text;
    if let Some((state, closed_at)) = &patch.state {
        state_text = state.as_str();
        sets.push(("state", &state_text));
        sets.push(("closed_at", closed_at));
    }
    if let Some(list_id) = &list_id {
        sets.push(("list_id", &list_id.0));
    }
    if let Some(priority) = &patch.priority {
        sets.push(("priority", priority));
    }
    if let Some(repeat_of) = &repeat_of {
        sets.push(("repeat_of", &repeat_of.0));
    }
    if let Some(created_at) = &patch.created_at {
        sets.push(("created_at", created_at));
    }
    if let Some(sort_key) = &patch.sort_key {
        sets.push(("sort_key", sort_key));
    }
    if let Some(deleted_at) = &patch.deleted_at {
        sets.push(("deleted_at", &deleted_at.0));
    }
    repo::update_columns(conn, id, &sets)
}

/// After a Task record is stored: a Task that was meant for a List that has since been merged
/// away is in the surviving List now (see `list_repo::home`), and joins the end of its manual
/// order like the Tasks that were in the merged List when it merged. Recorded as an ordinary
/// change, so every device ends with the newest such key.
pub(crate) fn settle(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Settled> {
    let id = change.entity_id;
    let Some(named) = change.fields.get("list_id").and_then(Value::as_str) else {
        return Ok(Settled::default());
    };
    let Some(mut task) = repo::get_any(conn, id)? else {
        return Ok(Settled::default());
    };
    let Some(home) = list_repo::home(conn, named)? else {
        return Ok(Settled::default());
    };
    if home == named || task.list_id.as_deref() != Some(home.as_str()) {
        return Ok(Settled::default());
    }
    let end = repo::in_list(conn, &home)?
        .into_iter()
        .filter(|t| t.id != task.id)
        .map(|t| t.sort_key)
        .max();
    if end
        .as_deref()
        .is_none_or(|end| task.sort_key.as_str() > end)
    {
        return Ok(Settled::default());
    }
    task.sort_key = key_after(end.as_deref());
    task.updated_at = now_ms();
    repo::update_sort_key(conn, &task)?;
    record_at(conn, &task, ChangeOp::Move, repo::deleted_at(conn, id)?)?;
    Ok(Settled {
        recorded: true,
        events: Vec::new(),
    })
}

/// Where a Task meant for `list` goes here: that List while it is live, the List it was merged
/// into, else the Inbox (none), which is reported as an event (the Activity log will say
/// "moved to the Inbox").
fn home_or_inbox(
    conn: &Connection,
    task: &str,
    list: &str,
    events: &mut Vec<SyncEvent>,
) -> AppResult<Option<String>> {
    let home = list_repo::home(conn, list)?;
    if home.is_none() {
        events.push(SyncEvent::TaskToInbox {
            task_id: task.to_owned(),
            list_id: list.to_owned(),
        });
    }
    Ok(home)
}

/// A Task never points at a List that is gone: it sits in the List that took its place, or in
/// the Inbox (as Undo of a delete does).
fn keep_out_of_missing_lists(
    conn: &Connection,
    id: &str,
    events: &mut Vec<SyncEvent>,
) -> AppResult<()> {
    let Some(task) = repo::get_any(conn, id)? else {
        return Ok(());
    };
    if let Some(list) = &task.list_id
        && repo::deleted_at(conn, id)?.is_none()
        && list_repo::get(conn, list)?.is_none()
    {
        let home = home_or_inbox(conn, id, list, events)?;
        repo::update_columns(conn, id, &[("list_id", &home)])?;
    }
    Ok(())
}
