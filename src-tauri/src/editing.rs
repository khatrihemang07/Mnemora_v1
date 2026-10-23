//! Editing holds: the Entry editor and the Task sheet's text fields tell Rust what they are
//! editing so that changes from other devices wait until they are done (`mnemora_core::editing`,
//! `mnemora_sync` for the rest). Thin: the rules live in those crates.

use mnemora_core::db::Connection;
use mnemora_core::editing;
use mnemora_core::error::{AppError, ErrorCode};
use mnemora_core::events::{EventBus, RemoteChanged};
use mnemora_core::util::now_ms;
use mnemora_sync::{Modules, Registry, SyncError};
use tauri::State;

use crate::state::{AppState, lock};

/// Longest entity id or scope accepted.
const MAX_NAME: usize = 64;

fn invalid() -> AppError {
    AppError::new(ErrorCode::Validation, "Not something that can be edited")
}

/// Whether `scope` names a unit of a syncable `entity`: the whole of it, or one field or group.
fn known(modules: &Modules, entity: &str, scope: &str) -> bool {
    if scope.len() > MAX_NAME {
        return false;
    }
    let registry = Registry::new(std::sync::Arc::clone(modules));
    registry.find(entity).is_some_and(|(_, schema)| {
        scope == editing::WHOLE
            || schema
                .fields
                .iter()
                .any(|f| f.group.unwrap_or(f.name) == scope)
    })
}

/// An editor opened or is still open on `scope` (a field, a group of fields, or `*` for the whole
/// record) of one record: what other devices send for it waits. Call again about every 30
/// seconds; the hold lapses after two minutes without a call, so an editor that vanished cannot
/// hold changes back for ever. Answers whether Sync is on: with it off nothing is stored, the editor
/// has nothing to renew and starts no timer.
#[tauri::command]
#[specta::specta]
pub fn editing_hold(
    state: State<'_, AppState>,
    entity: String,
    entity_id: String,
    scope: String,
) -> Result<bool, AppError> {
    if entity_id.len() > MAX_NAME || !known(&state.modules, &entity, &scope) {
        return Err(invalid());
    }
    // With Sync off nothing arrives to be held back: write nothing (Sync is inert when off).
    if !state.device.get().sync.enabled {
        return Ok(false);
    }
    editing::hold(&lock(&state.db), &entity, &entity_id, &scope, now_ms())?;
    Ok(true)
}

/// The editor is done (its text is saved): what was held back is merged now, and the screens
/// refresh silently.
#[tauri::command]
#[specta::specta]
pub fn editing_release(
    state: State<'_, AppState>,
    entity: String,
    entity_id: String,
    scope: String,
) -> Result<(), AppError> {
    if entity_id.len() > MAX_NAME || !known(&state.modules, &entity, &scope) {
        return Err(invalid());
    }
    // Holds exist only while Sync is on; whatever a switched-off Sync left held is merged at the
    // next start.
    if !state.device.get().sync.enabled {
        return Ok(());
    }
    let me = state.device.device_id();
    let changed = {
        let mut conn = lock(&state.db);
        let tx = conn.transaction()?;
        editing::release(&tx, &entity, &entity_id, &scope)?;
        let released = settle(&tx, &state.modules, &me)?;
        tx.commit()?;
        released
    };
    announce(&state.bus, changed);
    Ok(())
}

/// Applies the held changes whose hold has ended, and says which Modules changed.
pub fn settle(conn: &Connection, modules: &Modules, me: &str) -> Result<Vec<String>, SyncError> {
    let registry = Registry::new(std::sync::Arc::clone(modules));
    let released = mnemora_sync::release_held(conn, &registry, me, now_ms())?;
    Ok(released.changed.into_iter().collect())
}

/// Tells the screens of each Module in `changed` to refresh.
pub fn announce(bus: &EventBus, changed: Vec<String>) {
    for module in changed {
        bus.publish(&RemoteChanged { module });
    }
}

/// Releases whatever is waiting on a hold that lapsed (the app came back to the foreground).
pub fn release_lapsed(state: &AppState) {
    let me = state.device.device_id();
    let result = {
        let mut conn = lock(&state.db);
        conn.transaction()
            .map_err(SyncError::from)
            .and_then(|tx| Ok((settle(&tx, &state.modules, &me)?, tx.commit()?)))
    };
    match result {
        Ok((changed, ())) => announce(&state.bus, changed),
        Err(e) => tracing::warn!(target: "sync", error = %e, "could not apply held changes"),
    }
}

/// At app start, after every Module's startup work (which dropped all holds): merges whatever a
/// previous run was still holding back, because its editor died with it.
pub fn release_at_start(conn: &mut Connection, modules: &Modules, me: &str) {
    let result = conn
        .transaction()
        .map_err(SyncError::from)
        .and_then(|tx| Ok((settle(&tx, modules, me)?, tx.commit()?)));
    if let Err(e) = result {
        tracing::warn!(target: "sync", error = %e, "could not apply held changes at start");
    }
}
