//! Conflict commands: the Conflicts tab, Compare, and settling a Conflict. Thin: the rules live
//! in `mnemora-sync`; what is written to an Entry, Task or Comment goes through that Module's own
//! service, as any edit does, so it validates, records an ordinary change and syncs like one.

use mnemora_core::db::Connection;
use mnemora_core::error::{AppError, ErrorCode};
use mnemora_core::util::{new_id, now_ms};
use mnemora_sync::{
    ClashChoice, ConflictChoice, ConflictList, ConflictView, ConflictWrite, default_device_name,
};
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
