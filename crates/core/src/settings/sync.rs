//! Preferences in the sync contract (ADR 0017): entity `setting`, id `module.key`, one VALUE
//! field. The newest write of each key wins, per key. Device settings (look and feel, and
//! anything else registered with `register_device`) never come in from another device, and
//! never go out (see `Registry::local_only`). Values never reach a log line.

use rusqlite::Connection;
use serde_json::{Value, json};

use super::ENTITY;
use crate::error::AppResult;
use crate::syncable::{Applied, RemoteChange, SyncEntity, SyncEvent, SyncField};
use crate::util::now_ms;

/// The Preference entity: the value merges; the time of the write is recomputed here.
pub(crate) const ENTITIES: &[SyncEntity] = &[SyncEntity {
    name: ENTITY,
    fields: &[SyncField::value("value"), SyncField::derived("updated_at")],
}];

/// Takes another device's value for one Preference, if the key is known here, is not a Device
/// setting and the value passes the key's validator; anything else is ignored.
pub(crate) fn apply(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let nothing = |snapshot: Value| Applied {
        snapshot,
        changed: false,
        events: Vec::new(),
    };
    let Some((module, key)) = change.entity_id.split_once('.') else {
        return Ok(nothing(Value::Object(change.fields.clone())));
    };
    let held = super::stored(conn, module, key)?;
    let current = |value: Option<&Value>| {
        json!({
            "module": module, "key": key, "value": value.cloned().unwrap_or(Value::Null),
            "updated_at": now_ms(),
        })
    };
    let Some(def) = change.settings.get(change.entity_id).filter(|d| !d.device) else {
        // A key this build does not know, or one that is per device: kept for relay only.
        return Ok(nothing(current(held.as_ref())));
    };
    let Some(value) = change
        .fields
        .get("value")
        .filter(|v| (def.validator)(v).is_ok())
    else {
        return Ok(nothing(current(held.as_ref())));
    };
    if held.as_ref() == Some(value) {
        return Ok(nothing(current(held.as_ref())));
    }
    let at = now_ms();
    conn.execute(
        "INSERT INTO settings(module, key, value, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(module, key) DO UPDATE
         SET value = excluded.value, updated_at = excluded.updated_at",
        rusqlite::params![module, key, value.to_string(), at],
    )?;
    Ok(Applied {
        snapshot: json!({"module": module, "key": key, "value": value, "updated_at": at}),
        changed: true,
        events: vec![SyncEvent::SettingChanged {
            module: module.to_owned(),
            key: key.to_owned(),
        }],
    })
}
