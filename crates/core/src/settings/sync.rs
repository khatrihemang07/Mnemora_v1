//! Preferences in the sync contract (ADR 0017): entity `setting`, id `module.key`, one VALUE
//! field. The newest write of each key wins, per key. Device settings (look and feel, and
//! anything else registered with `register_device`) never come in from another device, and
//! never go out (see `Registry::local_only`). Values never reach a log line.

use rusqlite::Connection;
use serde_json::{Value, json};

use super::ENTITY;
use crate::changelog::{self, Change, ChangeOp, Unlogged};
use crate::error::AppResult;
use crate::settings::SettingsRegistry;
use crate::syncable::{Applied, RemoteChange, SyncEntity, SyncEvent, SyncField};
use crate::util::now_ms;

/// The Preference entity: the value merges; the time of the write is recomputed here.
pub(crate) const ENTITIES: &[SyncEntity] = &[SyncEntity {
    name: ENTITY,
    fields: &[SyncField::value("value"), SyncField::derived("updated_at")],
}];

/// Logs every shared Preference that is saved but not (as saved) in the change log: ones set
/// before Preferences were logged, or around the log. Stamped with the time the row says it was
/// written, so a later real write on any device still wins. Device settings stay home.
pub(crate) fn backfill(conn: &Connection, registry: &SettingsRegistry) -> AppResult<()> {
    let mut stmt =
        conn.prepare("SELECT module, key, value, updated_at FROM settings ORDER BY module, key")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    let mut unlogged = Vec::new();
    for (module, key, raw, at) in rows {
        let id = format!("{module}.{key}");
        if registry.get(&id).is_some_and(|d| d.device) {
            continue;
        }
        // A value that no longer parses reads as the default; there is nothing to send.
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        unlogged.push(Unlogged {
            change: Change {
                module: module.clone(),
                entity: ENTITY.to_owned(),
                entity_id: id,
                op: ChangeOp::Update,
                session_id: None,
                snapshot: json!({"module": module, "key": key, "value": value, "updated_at": at}),
            },
            ms: at,
            after: Vec::new(),
        });
    }
    changelog::backfill(conn, &ENTITIES[0], unlogged)?;
    Ok(())
}

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

#[cfg(test)]
mod tests {
    use serde_json::{Map, json};

    use super::*;
    use crate::module::{CoreModule, Module};
    use crate::settings::SettingsRegistry;

    /// What a hostile or newer peer may send: only a registered, non-device key with a value its
    /// validator accepts is ever stored.
    #[test]
    fn a_received_preference_is_stored_only_for_a_registered_shared_key_with_a_valid_value() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open_in_memory().unwrap();
        crate::db::migrate::run(&mut conn, &[&CoreModule], dir.path()).unwrap();
        let mut registry = SettingsRegistry::new();
        CoreModule.register_settings(&mut registry);
        let send = |id: &str, value: Value| {
            let mut fields = Map::new();
            fields.insert("value".into(), value);
            let applied = apply(
                &conn,
                &RemoteChange {
                    entity: ENTITY,
                    entity_id: id,
                    op: crate::changelog::ChangeOp::Update,
                    at: 0,
                    fields: &fields,
                    settings: &registry,
                },
            )
            .unwrap();
            applied.changed
        };
        assert!(!send("core.theme", json!("dark")), "a Device setting");
        assert!(!send("core.density", json!("compact")), "a Device setting");
        assert!(!send("core.no_such_key", json!("x")), "an unregistered key");
        assert!(
            !send("nomodule.week_start", json!("sun")),
            "an unknown Module"
        );
        assert!(!send("noseparator", json!("sun")), "a malformed id");
        assert!(
            !send("core.week_start", json!("someday")),
            "an invalid value"
        );
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 0, "nothing was stored");
        assert!(send("core.week_start", json!("sun")));
    }
}
