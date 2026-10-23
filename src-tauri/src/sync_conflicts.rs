//! Conflict commands: the Conflicts tab, Compare, and settling a Conflict. Thin: the rules live
//! in `mnemora-sync`; what is written to an Entry, Task or Comment goes through that Module's own
//! service, as any edit does, so it validates, records an ordinary change and syncs like one.

use mnemora_core::db::Connection;
use mnemora_core::error::{AppError, ErrorCode};
use mnemora_core::util::{new_id, now_ms};
use mnemora_sync::{
    Attention, ClashChoice, ConflictChoice, ConflictList, ConflictView, ConflictWrite, UsePlan,
    default_device_name,
};
use serde_json::Value;
use tauri::State;

use crate::state::{AppState, lock};

/// This device's name as the Conflicts tab shows it.
fn my_name(state: &AppState) -> String {
    state
        .device
        .get()
        .sync
        .name
        .unwrap_or_else(default_device_name)
}

/// "Needs you": the Conflicts that wait, newest first.
#[tauri::command]
#[specta::specta]
pub fn sync_conflicts(state: State<'_, AppState>) -> Result<ConflictList, AppError> {
    mnemora_sync::conflict_list(
        &lock(&state.db),
        &state.device.device_id(),
        &my_name(&state),
    )
}

/// One Conflict for Compare, or nothing when there is none with that id.
#[tauri::command]
#[specta::specta]
pub fn sync_conflict(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<ConflictView>, AppError> {
    mnemora_sync::conflict_view(
        &lock(&state.db),
        &id,
        &state.device.device_id(),
        &my_name(&state),
    )
}

/// The text Edit merged starts from: the agreed parts, each clash as chosen (in order; a clash
/// without a choice takes the showing version).
#[tauri::command]
#[specta::specta]
pub fn sync_conflict_draft(
    state: State<'_, AppState>,
    id: String,
    choices: Vec<ClashChoice>,
) -> Result<String, AppError> {
    mnemora_sync::conflict_draft(&lock(&state.db), &id, &choices)
}

/// Settles a Conflict: keep what shows, write the other version, or write an edited merge. The
/// text is written as an ordinary edit; the Conflict then reads resolved on every device.
#[tauri::command]
#[specta::specta]
pub fn sync_conflict_resolve(
    state: State<'_, AppState>,
    id: String,
    choice: ConflictChoice,
) -> Result<(), AppError> {
    let me = state.device.device_id();
    let mut conn = lock(&state.db);
    let (write, resolution) = mnemora_sync::conflict_plan(&conn, &id, &choice)?;
    if let Some(write) = write {
        write_text(&mut conn, &write)?;
        if write.entity != "journal_entry" {
            state.widgets.todo_changed();
        }
    }
    mnemora_sync::conflict_finish(&conn, &id, resolution, &me, now_ms())?;
    Ok(())
}

/// Writes `text` into the field through the owning Module's edit, as a new editing session.
fn write_text(conn: &mut Connection, write: &ConflictWrite) -> Result<(), AppError> {
    let id = write.entity_id.as_str();
    match write.entity.as_str() {
        "journal_entry" => {
            mnemora_journal::update_entry(conn, id, write.text.clone(), &new_id())?;
        }
        "todo_task" => {
            let task = mnemora_todo::task(conn, id)?;
            let (title, notes) = if write.field == "notes" {
                (task.title, write.text.clone())
            } else {
                (write.text.clone(), task.notes)
            };
            mnemora_todo::update_task(conn, id, &title, &notes, &new_id())?;
        }
        "todo_comment" => {
            mnemora_todo::edit_comment(conn, id, &write.text)?;
        }
        _ => {
            return Err(AppError::new(
                ErrorCode::Internal,
                "That kind of text cannot be written back",
            ));
        }
    }
    Ok(())
}

/// What needs attention now: the Home card, the sidebar badge and the palette hint read this.
#[tauri::command]
#[specta::specta]
pub fn sync_attention(state: State<'_, AppState>) -> Result<Attention, AppError> {
    mnemora_sync::attention(&lock(&state.db), &state.sync.peers_live(), now_ms())
}

/// "Use ‹other› instead": writes the value that gave way as an ordinary new edit through the
/// owning Module (so it validates, reads as a normal History line and syncs like any change),
/// and marks the row used on every device.
#[tauri::command]
#[specta::specta]
pub fn sync_resolution_use_other(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let me = state.device.device_id();
    let mut conn = lock(&state.db);
    let plan = mnemora_sync::resolution_plan_use(&conn, &id)?;
    write_value(&mut conn, &plan)?;
    if plan.entity.starts_with("todo_") {
        state.widgets.todo_changed();
    }
    mnemora_sync::resolution_finish_use(&conn, &id, &me)?;
    Ok(())
}

/// The person looked at "Resolved automatically": its rows stop being new.
#[tauri::command]
#[specta::specta]
pub fn sync_resolutions_visited(state: State<'_, AppState>) -> Result<(), AppError> {
    mnemora_sync::resolutions_visited(&lock(&state.db))?;
    Ok(())
}

fn invalid(what: &'static str) -> AppError {
    AppError::new(ErrorCode::Validation, what)
}

/// Puts the replaced value(s) of `plan` back through the owning Module's own edit.
fn write_value(conn: &mut Connection, plan: &UsePlan) -> Result<(), AppError> {
    let id = plan.entity_id.as_str();
    let text = |k: &str| plan.values.get(k).and_then(Value::as_str);
    match (plan.entity.as_str(), plan.unit.as_str()) {
        ("todo_task", "priority") => {
            let priority = plan
                .values
                .get("priority")
                .and_then(Value::as_u64)
                .and_then(|p| u32::try_from(p).ok())
                .ok_or_else(|| invalid("That priority cannot be set"))?;
            mnemora_todo::set_priority(conn, id, priority)?;
        }
        ("todo_task", "list_id") => {
            mnemora_todo::move_task(conn, id, text("list_id"))?;
        }
        ("todo_task", "when") => {
            let repeat = text("repeat_freq").map(|freq| mnemora_todo::RepeatRule {
                freq: freq.to_owned(),
                every: plan
                    .values
                    .get("repeat_every")
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or(1),
                days: plan
                    .values
                    .get("repeat_days")
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok()),
                mode: text("repeat_mode").unwrap_or("fixed").to_owned(),
            });
            mnemora_todo::restore_when(
                conn,
                &[mnemora_todo::DueChange {
                    id: id.to_owned(),
                    due_date: text("due_date").map(str::to_owned),
                    due_time: text("due_time").map(str::to_owned),
                    repeat,
                }],
            )?;
        }
        ("todo_list", "name") => {
            mnemora_todo::rename_list(conn, id, text("name").unwrap_or_default())?;
        }
        ("todo_list", "emoji") => {
            mnemora_todo::set_list_emoji(conn, id, text("emoji"))?;
        }
        ("todo_list", "task_sort") => {
            let sort = text("task_sort")
                .and_then(mnemora_todo::TaskSort::parse)
                .ok_or_else(|| invalid("That sort cannot be set"))?;
            mnemora_todo::set_list_sort(conn, id, sort)?;
        }
        ("todo_list", "group_by") => {
            let group = text("group_by")
                .and_then(mnemora_todo::GroupBy::parse)
                .ok_or_else(|| invalid("That grouping cannot be set"))?;
            mnemora_todo::set_list_group(conn, id, group)?;
        }
        ("journal_entry", "page_date") => {
            mnemora_journal::move_entry(conn, id, text("page_date").unwrap_or_default())?;
        }
        _ => return Err(invalid("That cannot be changed back")),
    }
    Ok(())
}
