//! Lists: rules, persistence and change-log records in one transaction.
//! List and Task text must never reach a log line.

use std::collections::HashMap;

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::error::AppResult;
use mnemora_core::util::{new_id, now_ms};
use rusqlite::Connection;

use crate::MODULE_ID;
use crate::error::TodoError;
use crate::list::{
    AllGroup, AllView, DeleteChoice, ListDeletion, ListEntry, ListView, SidebarView, TodoList,
    name_key, validate_emoji, validate_list_name,
};
use crate::service::record_at;
use crate::task::key_after;
use crate::{list_repo, repo};

/// Change-log entity name for Lists.
const ENTITY: &str = "todo_list";

/// Writes a change-log record for a List; the snapshot carries `deleted_at` like Task ones do.
fn record(
    conn: &Connection,
    list: &TodoList,
    op: ChangeOp,
    deleted_at: Option<i64>,
) -> AppResult<()> {
    let mut snapshot = serde_json::to_value(list).map_err(TodoError::Snapshot)?;
    if let Some(fields) = snapshot.as_object_mut() {
        fields.insert("deleted_at".into(), deleted_at.into());
    }
    changelog::record(
        conn,
        &Change {
            module: MODULE_ID.into(),
            entity: ENTITY.into(),
            entity_id: list.id.clone(),
            op,
            session_id: None,
            snapshot,
        },
    )
}

const NAME_TAKEN: TodoError = TodoError::Validation("There’s already a List with that name");

/// The sidebar's data: the Inbox count and every List with its open count.
pub fn sidebar(conn: &Connection) -> AppResult<SidebarView> {
    let counts: HashMap<Option<String>, (u32, u32)> = repo::task_counts(conn)?
        .into_iter()
        .map(|(list, open, all)| (list, (open, all)))
        .collect();
    let lists = list_repo::all(conn)?
        .into_iter()
        .map(|list| {
            let (open_count, task_count) = counts
                .get(&Some(list.id.clone()))
                .copied()
                .unwrap_or_default();
            ListEntry {
                list,
                open_count,
                task_count,
            }
        })
        .collect();
    Ok(SidebarView {
        inbox_count: counts.get(&None).map_or(0, |c| c.0),
        lists,
    })
}

/// One List and its open Tasks.
pub fn list_view(conn: &Connection, id: &str) -> AppResult<ListView> {
    let list = list_repo::get(conn, id)?.ok_or(TodoError::ListNotFound)?;
    Ok(ListView {
        tasks: repo::open_in_list(conn, id)?,
        list,
    })
}

/// Open Tasks grouped by List: the Inbox first, then Lists in order. Empty groups are left out.
pub fn all(conn: &Connection) -> AppResult<AllView> {
    let mut by_list: HashMap<String, Vec<_>> = HashMap::new();
    for task in repo::open_in_lists(conn)? {
        if let Some(id) = task.list_id.clone() {
            by_list.entry(id).or_default().push(task);
        }
    }
    let mut groups = vec![AllGroup {
        list: None,
        tasks: repo::inbox(conn)?,
    }];
    for list in list_repo::all(conn)? {
        let tasks = by_list.remove(&list.id).unwrap_or_default();
        groups.push(AllGroup {
            list: Some(list),
            tasks,
        });
    }
    groups.retain(|g| !g.tasks.is_empty());
    Ok(AllView { groups })
}

/// Adds a List at the bottom of the order.
pub fn create_list(conn: &mut Connection, name: &str) -> AppResult<TodoList> {
    let name = validate_list_name(name)?;
    let key = name_key(name);
    let tx = conn.transaction()?;
    if list_repo::name_taken(&tx, &key, None)? {
        return Err(NAME_TAKEN.into());
    }
    let now = now_ms();
    let list = TodoList {
        id: new_id(),
        name: name.to_owned(),
        emoji: None,
        sort_key: key_after(list_repo::last_sort_key(&tx)?.as_deref()),
        created_at: now,
        updated_at: now,
    };
    list_repo::insert(&tx, &list, &key)?;
    record(&tx, &list, ChangeOp::Create, None)?;
    tx.commit()?;
    Ok(list)
}

/// Renames a List. Changing only the case or accents of its own name is fine.
pub fn rename_list(conn: &mut Connection, id: &str, name: &str) -> AppResult<TodoList> {
    let name = validate_list_name(name)?;
    let key = name_key(name);
    let tx = conn.transaction()?;
    let mut list = list_repo::get(&tx, id)?.ok_or(TodoError::ListNotFound)?;
    if list_repo::name_taken(&tx, &key, Some(id))? {
        return Err(NAME_TAKEN.into());
    }
    if list.name == name {
        return Ok(list);
    }
    name.clone_into(&mut list.name);
    list.updated_at = now_ms();
    list_repo::update_name(&tx, &list, &key)?;
    record(&tx, &list, ChangeOp::Update, None)?;
    tx.commit()?;
    Ok(list)
}

/// Sets a List's emoji; none (or blank) goes back to the neutral icon.
pub fn set_list_emoji(conn: &mut Connection, id: &str, emoji: Option<&str>) -> AppResult<TodoList> {
    let emoji = validate_emoji(emoji)?;
    let tx = conn.transaction()?;
    let mut list = list_repo::get(&tx, id)?.ok_or(TodoError::ListNotFound)?;
    if list.emoji.as_deref() == emoji {
        return Ok(list);
    }
    list.emoji = emoji.map(str::to_owned);
    list.updated_at = now_ms();
    list_repo::update_emoji(&tx, &list)?;
    record(&tx, &list, ChangeOp::Update, None)?;
    tx.commit()?;
    Ok(list)
}

/// Deletes a List (soft). Its Tasks are deleted with it or moved, as `choice` says; an empty
/// List ignores the choice. The result is what Undo needs.
pub fn delete_list(
    conn: &mut Connection,
    id: &str,
    choice: &DeleteChoice,
) -> AppResult<ListDeletion> {
    let tx = conn.transaction()?;
    let list = list_repo::get(&tx, id)?.ok_or(TodoError::ListNotFound)?;
    let target = match choice {
        DeleteChoice::DeleteTasks | DeleteChoice::MoveToInbox => None,
        DeleteChoice::MoveToList { list_id } => {
            if list_id == id {
                return Err(TodoError::Validation("Pick a different List to move them to").into());
            }
            Some(
                list_repo::get(&tx, list_id)?
                    .ok_or(TodoError::ListNotFound)?
                    .id,
            )
        }
    };
    let now = now_ms();
    let mut deletion = ListDeletion {
        list_id: list.id.clone(),
        deleted_task_ids: Vec::new(),
        moved_task_ids: Vec::new(),
    };
    for mut task in repo::in_list(&tx, id)? {
        if matches!(choice, DeleteChoice::DeleteTasks) {
            repo::set_deleted(&tx, &task.id, Some(now), now)?;
            record_at(&tx, &task, ChangeOp::Delete, Some(now))?;
            deletion.deleted_task_ids.push(task.id);
        } else {
            task.list_id.clone_from(&target);
            task.updated_at = now;
            repo::update_list(&tx, &task)?;
            record_at(&tx, &task, ChangeOp::Move, None)?;
            deletion.moved_task_ids.push(task.id);
        }
    }
    list_repo::set_deleted(&tx, id, Some(now), now)?;
    record(&tx, &list, ChangeOp::Delete, Some(now))?;
    tx.commit()?;
    Ok(deletion)
}

/// Undoes `delete_list`: the List returns, its deleted Tasks come back and its moved Tasks go
/// home again. Fails if a newer List has taken the name meanwhile.
pub fn restore_list(conn: &mut Connection, deletion: &ListDeletion) -> AppResult<TodoList> {
    let tx = conn.transaction()?;
    let (mut list, deleted_at) =
        list_repo::get_any(&tx, &deletion.list_id)?.ok_or(TodoError::ListNotFound)?;
    if deleted_at.is_none() {
        return Err(TodoError::Validation("That List isn't deleted").into());
    }
    if list_repo::name_taken(&tx, &name_key(&list.name), None)? {
        return Err(NAME_TAKEN.into());
    }
    let now = now_ms();
    list.updated_at = now;
    list_repo::set_deleted(&tx, &list.id, None, now)?;
    record(&tx, &list, ChangeOp::Restore, None)?;
    for id in &deletion.deleted_task_ids {
        if let Some(task) = repo::get_any(&tx, id)? {
            repo::set_deleted(&tx, id, None, now)?;
            record_at(&tx, &task, ChangeOp::Restore, None)?;
        }
    }
    for id in &deletion.moved_task_ids {
        if let Some(mut task) = repo::get(&tx, id)? {
            task.list_id = Some(list.id.clone());
            task.updated_at = now;
            repo::update_list(&tx, &task)?;
            record_at(&tx, &task, ChangeOp::Move, None)?;
        }
    }
    tx.commit()?;
    Ok(list)
}
