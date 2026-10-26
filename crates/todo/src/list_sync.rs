//! Todo's side of the sync contract for Lists (ADR 0017). List and Task text must never reach a
//! log line.
//!
//! Merge rules (the engine applies them; this file declares them):
//! - `name`, `emoji`, `task_sort` (sort), `group_by` (grouping) and `deleted_at` are Values:
//!   newest wins. `sort_key` is the manual order. The folded name and `updated_at` are derived.
//!
//! **Name clash.** Two live Lists may not share a name once case and accents are ignored, but two
//! devices can each make one (or rename one into the other's name). When a received change would
//! leave two, the older List by id survives with all its own properties and the newer one is
//! soft-deleted; its Tasks join the end of the survivor's manual order. Every device makes the
//! same choice because it depends on the two ids alone, not on which arrived first. The merge is
//! recorded as ordinary changes (a Move for each Task, a Delete for the List), made after the
//! received record is stored, so they are stamped later and win everywhere; devices that make
//! the same merge on their own write the same values wherever they hold the same Tasks, and
//! where they differ the newest write wins like any other. A Task that names the merged-away
//! List later (made on a device that had not heard) goes to the survivor too: see
//! `list_repo::home`.
//!
//! To let the unique name index hold while a change is applied, a List that is to be live under a
//! name another live List has is first written parked (`deleted_at` = `PARKED`), and `settle`
//! decides.

use mnemora_core::changelog::{self, ChangeOp};
use mnemora_core::error::AppResult;
use mnemora_core::syncable::{Applied, RemoteChange, Settled, SyncEntity, SyncEvent, SyncField};
use mnemora_core::util::now_ms;
use rusqlite::Connection;
use serde_json::{Map, Value};

use crate::arrange::{GroupBy, TaskSort};
use crate::list::{TodoList, name_key, validate_emoji, validate_list_name};
use crate::list_repo::{self, PARKED};
use crate::list_service::{ENTITY, record_list, snapshot_of};
use crate::repo;
use crate::service::record_at;
use crate::sync::{Set, number, strip_updated, text};
use mnemora_core::order::key_after;

/// The List entity's fields and how each merges.
pub(crate) const LIST: SyncEntity = SyncEntity {
    name: ENTITY,
    fields: &[
        SyncField::value("name"),
        SyncField::value("emoji"),
        SyncField::value("task_sort"),
        SyncField::value("group_by"),
        SyncField::order("sort_key"),
        SyncField::value("created_at"),
        SyncField::value("deleted_at"),
        SyncField::derived("updated_at"),
    ],
};

/// What a set of received fields would write, every field already checked. A field that fails
/// its checks (a bug or a hostile peer) is left out.
#[derive(Default)]
struct Patch {
    name: Option<String>,
    emoji: Option<Set<String>>,
    task_sort: Option<TaskSort>,
    group_by: Option<GroupBy>,
    sort_key: Option<String>,
    created_at: Option<i64>,
    deleted_at: Option<Set<i64>>,
}

impl Patch {
    fn read(fields: &Map<String, Value>) -> Self {
        let get = |k: &str| fields.get(k);
        Self {
            name: get("name")
                .and_then(Value::as_str)
                .and_then(|n| validate_list_name(n).ok())
                .map(str::to_owned),
            emoji: get("emoji").and_then(text).and_then(|Set(emoji)| {
                let checked = validate_emoji(emoji.as_deref()).ok()?;
                Some(Set(checked.map(str::to_owned)))
            }),
            task_sort: get("task_sort")
                .and_then(Value::as_str)
                .and_then(TaskSort::parse),
            // A List groups by Priority or Due date, or not at all.
            group_by: get("group_by")
                .and_then(Value::as_str)
                .and_then(GroupBy::parse)
                .filter(|g| matches!(g, GroupBy::None | GroupBy::Priority | GroupBy::Due)),
            sort_key: get("sort_key")
                .and_then(Value::as_str)
                .filter(|k| !k.is_empty())
                .map(str::to_owned),
            created_at: get("created_at").and_then(Value::as_i64),
            // Zero is the parked marker, not a time anything was deleted.
            deleted_at: get("deleted_at")
                .and_then(number)
                .filter(|Set(at)| at.is_none_or(|at| at > PARKED)),
        }
    }
}

/// Applies the winning field values of one List, creating it when it is new here. A List that
/// would clash with a live one by name is left parked for `settle`.
pub(crate) fn apply(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let id = change.entity_id;
    let patch = Patch::read(change.fields);
    let before = list_repo::get_any(conn, id)?;
    let before_snapshot = match &before {
        Some((list, deleted_at)) => Some(snapshot_of(list, *deleted_at)?),
        None => None,
    };

    if before.is_none() {
        // A List can only be made from its first record, which carries these.
        let (Some(name), Some(sort_key), Some(created_at)) =
            (&patch.name, &patch.sort_key, patch.created_at)
        else {
            tracing::debug!(target: "sync", list = id, "record for an unknown List without its creation, kept for relay only");
            return Ok(Applied {
                snapshot: Value::Object(change.fields.clone()),
                changed: false,
                events: Vec::new(),
            });
        };
        list_repo::insert_remote(
            conn,
            id,
            name,
            &name_key(name),
            sort_key,
            created_at,
            now_ms(),
        )?;
    }
    let was_live = matches!(&before, Some((_, None)));
    let was_deleted = match &before {
        Some((_, deleted_at)) => *deleted_at,
        None => None,
    };

    // Where the List stands once the winners are written, and whether that name is free.
    let deleted_at = match &patch.deleted_at {
        Some(Set(at)) => *at,
        None => was_deleted,
    };
    let name = match (&patch.name, &before) {
        (Some(name), _) => name.clone(),
        (None, Some((list, _))) => list.name.clone(),
        (None, None) => String::new(),
    };
    let clashes = deleted_at.is_none()
        && !name.is_empty()
        && list_repo::live_twin(conn, &name_key(&name), id)?.is_some();
    let stored_deleted = if clashes { Some(PARKED) } else { deleted_at };
    write(conn, id, &patch, stored_deleted)?;

    let mut events = Vec::new();
    if was_live && deleted_at.is_some() {
        send_tasks_home(conn, id, &mut events)?;
    }

    // The record's own state: a parked List reads as live, which is what the record says.
    let state_now = |conn: &Connection| -> AppResult<Value> {
        match list_repo::get_any(conn, id)? {
            Some((list, _)) => snapshot_of(&list, deleted_at),
            None => Ok(Value::Null),
        }
    };
    let snapshot = state_now(conn)?;
    let changed = before_snapshot.as_ref().map(strip_updated) != Some(strip_updated(&snapshot));
    if changed {
        let mut keys = vec![name_key(&name)];
        if let Some((old, _)) = &before {
            keys.push(name_key(&old.name));
        }
        let revived = was_deleted.is_some() && deleted_at.is_none();
        rehome(conn, id, &keys, revived)?;
        // `updated_at` is derived: it says when this device last changed the row.
        list_repo::update_columns(conn, id, &[("updated_at", &now_ms())])?;
    }
    Ok(Applied {
        snapshot: state_now(conn)?,
        changed,
        events,
    })
}

/// Writes every checked field of `patch` and the stored `deleted_at`.
fn write(conn: &Connection, id: &str, patch: &Patch, stored_deleted: Option<i64>) -> AppResult<()> {
    let key = patch.name.as_deref().map(name_key);
    let sort = patch.task_sort.map(TaskSort::as_str);
    // The `group_by` column holds none as NULL.
    let group = patch
        .group_by
        .map(|g| (g != GroupBy::None).then(|| g.as_str()));
    let mut sets: Vec<(&str, &dyn rusqlite::ToSql)> = Vec::new();
    if let (Some(name), Some(key)) = (&patch.name, &key) {
        sets.push(("name", name));
        sets.push(("name_key", key));
    }
    if let Some(emoji) = &patch.emoji {
        sets.push(("emoji", &emoji.0));
    }
    if let Some(sort) = &sort {
        sets.push(("task_sort", sort));
    }
    if let Some(group) = &group {
        sets.push(("group_by", group));
    }
    if let Some(sort_key) = &patch.sort_key {
        sets.push(("sort_key", sort_key));
    }
    if let Some(created_at) = &patch.created_at {
        sets.push(("created_at", created_at));
    }
    sets.push(("deleted_at", &stored_deleted));
    list_repo::update_columns(conn, id, &sets)
}

/// A List was deleted elsewhere: Tasks still in it go to the List it merged into, or the Inbox.
/// The column only, with nothing recorded: every device does the same, so there is nothing to
/// tell the others. (The deleter moved the Tasks it knew of with recorded changes first.)
fn send_tasks_home(conn: &Connection, id: &str, events: &mut Vec<SyncEvent>) -> AppResult<()> {
    let home = list_repo::home(conn, id)?;
    let tasks = repo::in_list(conn, id)?;
    if tasks.is_empty() && home.is_none() {
        return Ok(());
    }
    if let (Some(into), Some((list, _))) = (&home, list_repo::get_any(conn, id)?)
        && let Some((survivor, _)) = list_repo::get_any(conn, into)?
    {
        events.push(merged_event(conn, &list, &survivor)?);
    }
    for task in tasks {
        repo::update_columns(conn, &task.id, &[("list_id", &home)])?;
        if home.is_none() {
            events.push(SyncEvent::TaskToInbox {
                task_id: task.id,
                list_id: id.to_owned(),
            });
        }
    }
    Ok(())
}

fn merged_event(conn: &Connection, from: &TodoList, into: &TodoList) -> AppResult<SyncEvent> {
    Ok(SyncEvent::ListsMerged {
        from_id: from.id.clone(),
        from_name: from.name.clone(),
        from_device: changelog::creator(conn, ENTITY, &from.id)?,
        into_id: into.id.clone(),
        into_name: into.name.clone(),
        into_device: changelog::creator(conn, ENTITY, &into.id)?,
    })
}

/// Decides a parked List: if its name is free it goes live; if a live List has the name, the
/// older of the two survives and the newer is merged into it.
pub(crate) fn settle(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Settled> {
    let Some((list, Some(PARKED))) = list_repo::get_any(conn, change.entity_id)? else {
        return Ok(Settled::default());
    };
    let mut settled = Settled::default();
    match list_repo::live_twin(conn, &name_key(&list.name), &list.id)? {
        Some(other) if other.id < list.id => {
            // The parked List is the newer: it goes away.
            merge(conn, &list, &other, &mut settled)?;
        }
        Some(other) => {
            merge(conn, &other, &list, &mut settled)?;
            list_repo::unpark(conn, &list.id)?;
        }
        None => list_repo::unpark(conn, &list.id)?,
    }
    // Which List a gone List's Tasks belong in may have changed with the decision.
    rehome(conn, &list.id, &[name_key(&list.name)], true)?;
    Ok(settled)
}

/// Tasks sit where `list_repo::home` says their List's place is. That depends on the Lists, which
/// change after a Task was placed (a rename into the name of a deleted List, a deleted List
/// restored), so when a List changes its Tasks are placed again: every live Task whose newest
/// List choice (from the change log, the same on every device) names a List this change can
/// affect: the List itself and the deleted Lists with its old or new name. Only the column is
/// written, so nothing is recorded and every device ends with the same place for every Task,
/// whatever order the Lists and the Tasks arrived in. `revived`: the List was deleted and is
/// live again.
fn rehome(conn: &Connection, id: &str, keys: &[String], revived: bool) -> AppResult<()> {
    let mut affected = vec![id.to_owned()];
    let mut gone = revived;
    for key in keys {
        affected.extend(list_repo::with_name_key(conn, key)?);
    }
    affected.sort();
    affected.dedup();
    for list in &affected {
        gone |= list_repo::get(conn, list)?.is_none();
    }
    if !gone {
        return Ok(());
    }
    for list in &affected {
        for (task, column) in repo::chosen_list_is(conn, list)? {
            let place = list_repo::home(conn, list)?;
            if column != place {
                repo::update_columns(conn, &task, &[("list_id", &place)])?;
            }
        }
    }
    Ok(())
}

/// Merges `from` into `into`: its Tasks go to the end of `into`'s manual order, keeping their
/// own order, and it is soft-deleted. All recorded as ordinary changes.
fn merge(
    conn: &Connection,
    from: &TodoList,
    into: &TodoList,
    settled: &mut Settled,
) -> AppResult<()> {
    let now = now_ms();
    // Keys only grow: a Task whose key is already past the end keeps it.
    let mut end = repo::in_list(conn, &into.id)?
        .last()
        .map(|t| t.sort_key.clone());
    for mut task in repo::in_list(conn, &from.id)? {
        task.list_id = Some(into.id.clone());
        if end
            .as_deref()
            .is_some_and(|end| task.sort_key.as_str() <= end)
        {
            task.sort_key = key_after(end.as_deref());
        }
        end = Some(task.sort_key.clone());
        task.updated_at = now;
        repo::update_list(conn, &task)?;
        repo::update_sort_key(conn, &task)?;
        record_at(conn, &task, ChangeOp::Move, None)?;
    }
    list_repo::set_deleted(conn, &from.id, Some(now), now)?;
    record_list(conn, from, ChangeOp::Delete, Some(now))?;
    settled.recorded = true;
    settled.events.push(merged_event(conn, from, into)?);
    Ok(())
}
