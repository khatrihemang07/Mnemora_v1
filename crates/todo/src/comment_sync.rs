//! Todo's side of the sync contract for Comments (ADR 0017). Comment text must never reach a
//! log line.
//!
//! Merge rules (the engine applies them; this file declares them):
//! - `body` is Text: newest wins for now, three-way merge later. `edited` (the flag that shows
//!   "edited") travels with it, so the flag always comes from the same device as the text.
//! - The Comment's Task (`task_id`) and `deleted_at` are Values.
//! - `updated_at` is derived and recomputed here (a Comment's always moves forward). The Comment
//!   count is never stored.
//!
//! A Comment stays on its Task whatever state the Task is in, deleted ones included: when the
//! Task comes back, so does the Comment. Only a Task this device has never heard of leaves a
//! Comment unapplied (kept for relay only).

use mnemora_core::error::AppResult;
use mnemora_core::syncable::{Applied, RemoteChange, SyncEntity, SyncField};
use mnemora_core::util::now_ms;
use rusqlite::{Connection, params};
use serde_json::{Map, Value};

use crate::comment::validate_body;
use crate::comment_repo;
use crate::comment_service::{ENTITY, snapshot_of};
use crate::repo;
use crate::sync::{Set, number, strip_updated};

const BODY: &str = "body";

/// The Comment entity's fields and how each merges.
pub(crate) const COMMENT: SyncEntity = SyncEntity {
    name: ENTITY,
    fields: &[
        SyncField::text("body").in_group(BODY),
        SyncField::value("edited").in_group(BODY),
        SyncField::value("task_id"),
        SyncField::value("created_at"),
        SyncField::value("deleted_at"),
        SyncField::derived("updated_at"),
    ],
};

/// What a set of received fields would write, every unit already checked.
#[derive(Default)]
struct Patch {
    body: Option<(String, bool)>,
    task_id: Option<String>,
    created_at: Option<i64>,
    deleted_at: Option<Set<i64>>,
}

impl Patch {
    fn read(fields: &Map<String, Value>) -> Self {
        let get = |k: &str| fields.get(k);
        Self {
            body: get("body")
                .and_then(Value::as_str)
                .and_then(|b| validate_body(b).ok())
                .zip(get("edited").and_then(Value::as_bool))
                .map(|(body, edited)| (body.to_owned(), edited)),
            task_id: get("task_id").and_then(Value::as_str).map(str::to_owned),
            created_at: get("created_at").and_then(Value::as_i64),
            deleted_at: get("deleted_at").and_then(number),
        }
    }
}

/// Applies the winning field values of one Comment, creating it when it is new here.
pub(crate) fn apply(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let id = change.entity_id;
    let mut patch = Patch::read(change.fields);
    // The Task must be there (deleted is fine), or the Comment has nowhere to be attached.
    if let Some(task) = &patch.task_id
        && repo::get_any(conn, task)?.is_none()
    {
        patch.task_id = None;
    }
    let before = comment_repo::get_any(conn, id)?;
    let before_snapshot = match &before {
        Some((comment, deleted_at)) => Some(snapshot_of(comment, *deleted_at)?),
        None => None,
    };

    if before.is_none() {
        // A Comment can only be made from its first record, which carries these.
        let (Some((body, edited)), Some(task_id), Some(created_at)) =
            (&patch.body, &patch.task_id, patch.created_at)
        else {
            tracing::debug!(target: "sync", comment = id, "record for an unknown Comment without its creation or Task, kept for relay only");
            return Ok(Applied {
                snapshot: Value::Object(change.fields.clone()),
                changed: false,
                events: Vec::new(),
            });
        };
        // Hidden until its `deleted_at` has been applied.
        let now = now_ms();
        conn.execute(
            "INSERT INTO todo_comments(id, task_id, body, edited, created_at, updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![id, task_id, body, edited, created_at, now],
        )?;
    }

    write(conn, id, &patch)?;

    let snapshot_now = |conn: &Connection| -> AppResult<Value> {
        match comment_repo::get_any(conn, id)? {
            Some((comment, deleted_at)) => snapshot_of(&comment, deleted_at),
            None => Ok(Value::Null),
        }
    };
    let changed =
        before_snapshot.as_ref().map(strip_updated) != Some(strip_updated(&snapshot_now(conn)?));
    if changed {
        // `updated_at` is derived and only ever moves forward.
        conn.execute(
            "UPDATE todo_comments SET updated_at = MAX(?2, updated_at + 1) WHERE id = ?1",
            params![id, now_ms()],
        )?;
    }
    Ok(Applied {
        snapshot: snapshot_now(conn)?,
        changed,
        events: Vec::new(),
    })
}

/// Writes every checked unit of `patch`.
fn write(conn: &Connection, id: &str, patch: &Patch) -> AppResult<()> {
    let mut sets: Vec<(&str, &dyn rusqlite::ToSql)> = Vec::new();
    if let Some((body, edited)) = &patch.body {
        sets.push(("body", body));
        sets.push(("edited", edited));
    }
    if let Some(task_id) = &patch.task_id {
        sets.push(("task_id", task_id));
    }
    if let Some(created_at) = &patch.created_at {
        sets.push(("created_at", created_at));
    }
    if let Some(deleted_at) = &patch.deleted_at {
        sets.push(("deleted_at", &deleted_at.0));
    }
    comment_repo::update_columns(conn, id, &sets)
}
