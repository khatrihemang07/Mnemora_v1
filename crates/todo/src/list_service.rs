//! Lists: rules, persistence and change-log records in one transaction.
//! List and Task text must never reach a log line.

use std::collections::HashMap;

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::error::AppResult;
use mnemora_core::util::{new_id, now_ms};
use rusqlite::Connection;

use crate::MODULE_ID;
use crate::arrange::{
    DoneInLists, GroupBy, GroupContext, GroupKey, ListSort, TaskSort, ViewPrefs, compare,
    group_tasks, sort_lists,
};
use crate::error::TodoError;
use crate::list::{
    AllView, DeleteChoice, ListDeletion, ListDone, ListEntry, ListView, SidebarView, TodoList,
    name_key, validate_emoji, validate_list_name,
};
use crate::order::key_after;
use crate::service::record_at;
use crate::when::LocalNow;
use crate::{list_repo, repo};

/// Change-log entity name for Lists.
pub(crate) const ENTITY: &str = "todo_list";

/// Writes a change-log record for a List; the snapshot carries `deleted_at` like Task ones do.
pub(crate) fn record_list(
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

/// Live Lists with their open counts, in the order `sort` says.
pub(crate) fn ordered_lists(conn: &Connection, sort: ListSort) -> AppResult<Vec<ListEntry>> {
    let counts: HashMap<Option<String>, (u32, u32)> = repo::task_counts(conn)?
        .into_iter()
        .map(|(list, open, all)| (list, (open, all)))
        .collect();
    let mut entries: Vec<ListEntry> = list_repo::all(conn)?
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
    sort_lists(&mut entries, sort);
    Ok(entries)
}

/// The sidebar's data: the Inbox count, Today's count (due today plus Overdue) and every List
/// with its open count, in the order the Lists sort says.
pub fn sidebar(conn: &Connection, local: &LocalNow) -> AppResult<SidebarView> {
    local.check()?;
    let prefs = ViewPrefs::read(conn)?;
    let (overdue_count, due_today) = repo::due_counts(conn, &local.today)?;
    let inbox_count = repo::task_counts(conn)?
        .into_iter()
        .find(|(list, ..)| list.is_none())
        .map_or(0, |(_, open, _)| open);
    Ok(SidebarView {
        inbox_count,
        today_count: overdue_count + due_today,
        overdue_count,
        lists: ordered_lists(conn, prefs.lists_sort)?,
        lists_sort: prefs.lists_sort,
    })
}

/// One List's page: its open Tasks grouped and sorted as the List says, and its Done Tasks as the
/// Done-in-Lists Preference says.
pub fn list_view(conn: &Connection, id: &str, local: &LocalNow) -> AppResult<ListView> {
    local.check()?;
    let list = list_repo::get(conn, id)?.ok_or(TodoError::ListNotFound)?;
    let prefs = ViewPrefs::read(conn)?;
    let ctx = GroupContext {
        lists: &[],
        today: &local.today,
    };
    let mut groups = group_tasks(
        repo::open_in_list(conn, id)?,
        list.group_by,
        &ctx,
        &compare(list.task_sort),
    );
    // Adding inside a List group keeps the Task in this List.
    for group in &mut groups {
        group.adopt.insert(
            0,
            GroupKey::List {
                list_id: Some(id.to_owned()),
            },
        );
    }
    let done = list_done(conn, id, prefs.done_in_lists, local)?;
    Ok(ListView { list, groups, done })
}

/// A List's closed Tasks split the way the Preference says.
fn list_done(
    conn: &Connection,
    id: &str,
    mode: DoneInLists,
    local: &LocalNow,
) -> AppResult<ListDone> {
    let closed = match mode {
        DoneInLists::Hidden => Vec::new(),
        _ => repo::closed_in_list(conn, id)?,
    };
    let (shown, folded) = match mode {
        DoneInLists::Hidden => (Vec::new(), Vec::new()),
        DoneInLists::Shown => (closed, Vec::new()),
        DoneInLists::Folded => (Vec::new(), closed),
        DoneInLists::TodayShown => {
            let (start, end) = local.today_range_ms()?;
            closed
                .into_iter()
                .partition(|t| t.closed_at.is_some_and(|at| at >= start && at < end))
        }
    };
    Ok(ListDone {
        mode,
        shown,
        folded,
    })
}

/// Open Tasks, grouped as the Preference says (by List unless changed): with List groups the
/// Inbox first, then Lists in the sidebar's order. Empty groups are left out.
pub fn all(conn: &Connection, local: &LocalNow) -> AppResult<AllView> {
    local.check()?;
    let prefs = ViewPrefs::read(conn)?;
    let lists: Vec<TodoList> = ordered_lists(conn, prefs.lists_sort)?
        .into_iter()
        .map(|e| e.list)
        .collect();
    let ctx = GroupContext {
        lists: &lists,
        today: &local.today,
    };
    let mut tasks = repo::inbox(conn)?;
    tasks.extend(repo::open_in_lists(conn)?);
    let mut groups = group_tasks(tasks, prefs.group_all, &ctx, &compare(TaskSort::Manual));
    groups.retain(|g| !g.tasks.is_empty());
    Ok(AllView {
        group_by: prefs.group_all,
        groups,
    })
}

/// Adds a List at the bottom of the order.
pub fn create_list(conn: &mut Connection, name: &str) -> AppResult<TodoList> {
    let name = validate_list_name(name)?;
    let key = name_key(name);
    let tx = conn.transaction()?;
    if list_repo::name_taken(&tx, &key, None)? {
        return Err(TodoError::NameTaken(name.to_owned()).into());
    }
    let now = now_ms();
    let list = TodoList {
        id: new_id(),
        name: name.to_owned(),
        emoji: None,
        sort_key: key_after(list_repo::last_sort_key(&tx)?.as_deref()),
        task_sort: TaskSort::Manual,
        group_by: GroupBy::None,
        created_at: now,
        updated_at: now,
    };
    list_repo::insert(&tx, &list, &key)?;
    record_list(&tx, &list, ChangeOp::Create, None)?;
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
        return Err(TodoError::NameTaken(name.to_owned()).into());
    }
    if list.name == name {
        return Ok(list);
    }
    name.clone_into(&mut list.name);
    list.updated_at = now_ms();
    list_repo::update_name(&tx, &list, &key)?;
    record_list(&tx, &list, ChangeOp::Update, None)?;
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
    record_list(&tx, &list, ChangeOp::Update, None)?;
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
    record_list(&tx, &list, ChangeOp::Delete, Some(now))?;
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
        return Err(TodoError::NameTaken(list.name).into());
    }
    let now = now_ms();
    list.updated_at = now;
    list_repo::set_deleted(&tx, &list.id, None, now)?;
    record_list(&tx, &list, ChangeOp::Restore, None)?;
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

/// Sets how a List's page sorts the Tasks inside each group. Records an `update` change.
pub fn set_list_sort(conn: &mut Connection, id: &str, sort: TaskSort) -> AppResult<TodoList> {
    let tx = conn.transaction()?;
    let mut list = list_repo::get(&tx, id)?.ok_or(TodoError::ListNotFound)?;
    if list.task_sort != sort {
        list.task_sort = sort;
        list.updated_at = now_ms();
        list_repo::update_arrangement(&tx, &list)?;
        record_list(&tx, &list, ChangeOp::Update, None)?;
    }
    tx.commit()?;
    Ok(list)
}

/// Sets how a List's page groups its Tasks: None, Priority or Due date. Records an `update`.
pub fn set_list_group(conn: &mut Connection, id: &str, group_by: GroupBy) -> AppResult<TodoList> {
    if !matches!(group_by, GroupBy::None | GroupBy::Priority | GroupBy::Due) {
        return Err(TodoError::Validation("A List groups by Priority or Due date").into());
    }
    let tx = conn.transaction()?;
    let mut list = list_repo::get(&tx, id)?.ok_or(TodoError::ListNotFound)?;
    if list.group_by != group_by {
        list.group_by = group_by;
        list.updated_at = now_ms();
        list_repo::update_arrangement(&tx, &list)?;
        record_list(&tx, &list, ChangeOp::Update, None)?;
    }
    tx.commit()?;
    Ok(list)
}
