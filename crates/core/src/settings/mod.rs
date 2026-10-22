//! Typed Settings: a schema registry plus validated, atomic persistence.

mod schema;

use std::collections::BTreeMap;
use std::sync::Arc;

use rusqlite::Connection;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use specta::Type;

pub use schema::{JsonValue, SettingDef, SettingSchema, SettingsRegistry, Validator, validate};

use crate::changelog::{self, Change, ChangeOp};
use crate::device::{DeviceStore, LOOK_KEYS};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::events::{EventBus, SettingsChanged};
use crate::util::now_ms;

/// One requested change; `key` is the full `module.key`.
#[derive(Debug, Clone, PartialEq, Deserialize, Type)]
pub struct SettingsChange {
    /// Full key, `module.key`.
    pub key: String,
    /// New value.
    #[specta(type = JsonValue)]
    pub value: Value,
}

/// Everything the UI needs to render Settings.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SettingsSnapshot {
    /// Current value of every registered key (defaults filled in).
    #[specta(type = BTreeMap<String, JsonValue>)]
    pub values: BTreeMap<String, Value>,
    /// Registered keys with their defaults.
    pub schema: Vec<SettingSchema>,
}

/// Change-log entity of a Preference; its id is the full `module.key`.
pub const ENTITY: &str = "setting";

/// Records a Preference as it now stands (ADR 0008). Every write to the `settings` table is
/// followed by this, in the same transaction, so a sync sees it.
pub fn record(conn: &Connection, module: &str, key: &str, value: &Value, at: i64) -> AppResult<()> {
    changelog::record(
        conn,
        &Change {
            module: module.to_owned(),
            entity: ENTITY.to_owned(),
            entity_id: format!("{module}.{key}"),
            op: ChangeOp::Update,
            session_id: None,
            snapshot: serde_json::json!({
                "module": module, "key": key, "value": value, "updated_at": at,
            }),
        },
    )
}

/// Saves `value` for `module.key` only when nothing is saved for it yet; true when it wrote.
/// For Modules pinning a value at upgrade time, so a new registered default cannot reach
/// existing installs. The value is trusted: it is not validated and announces no event. A write
/// is recorded in the change log like any other.
pub fn save_if_unset(conn: &Connection, module: &str, key: &str, value: &Value) -> AppResult<bool> {
    let at = now_ms();
    let written = conn.execute(
        "INSERT OR IGNORE INTO settings(module, key, value, updated_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![module, key, value.to_string(), at],
    )?;
    if written > 0 {
        record(conn, module, key, value, at)?;
    }
    Ok(written > 0)
}

/// The value saved for `module.key`, if any: raw, so the caller validates it. For a Module that
/// reads its own Preferences inside a view, where it needs no registry or event bus.
pub fn stored(conn: &Connection, module: &str, key: &str) -> AppResult<Option<Value>> {
    use rusqlite::OptionalExtension;
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE module = ?1 AND key = ?2",
            rusqlite::params![module, key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(raw.and_then(|raw| serde_json::from_str(&raw).ok()))
}

/// One-time upgrade: look and feel moves from the `settings` table to the device file (ADR 0005,
/// amended). A value the device file already has wins; the rows are deleted either way, so it is
/// safe to run on every start. The rows go only after the file is saved, so a failed write is
/// retried next start. Not change-logged: it is housekeeping, not a user edit.
pub fn move_look_and_feel_to_device(conn: &Connection, device: &DeviceStore) -> AppResult<()> {
    let list = LOOK_KEYS.map(|k| format!("'{k}'")).join(",");
    let mut stmt = conn.prepare(&format!(
        "SELECT key, value FROM settings WHERE module = 'core' AND key IN ({list})"
    ))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    if rows.is_empty() {
        return Ok(());
    }
    device.update(|d| {
        for (key, raw) in &rows {
            if d.look.get(key).is_some() {
                continue;
            }
            // A value that no longer parses would only have read as the default.
            if let Ok(value) = serde_json::from_str::<Value>(raw) {
                d.look.set(key, &value);
            }
        }
    })?;
    conn.execute(
        &format!("DELETE FROM settings WHERE module = 'core' AND key IN ({list})"),
        [],
    )?;
    tracing::info!(moved = rows.len(), "look and feel moved to the device file");
    Ok(())
}

/// Reads and writes Settings against a registry, announcing changes on the event bus. Device
/// settings (see [`SettingsRegistry::register_device`]) go to the device file instead of the
/// database, through the same calls.
pub struct SettingsStore {
    registry: Arc<SettingsRegistry>,
    bus: EventBus,
    device: Arc<DeviceStore>,
}

impl SettingsStore {
    /// Builds a store over a finished registry and the device file.
    pub fn new(registry: Arc<SettingsRegistry>, bus: EventBus, device: Arc<DeviceStore>) -> Self {
        Self {
            registry,
            bus,
            device,
        }
    }

    /// Current values of the Device settings alone (defaults filled in, invalid values ignored),
    /// without touching the database: what the window needs before first paint.
    pub fn device_values(&self) -> BTreeMap<String, Value> {
        let look = self.device.get().look;
        self.registry
            .iter()
            .filter(|(_, d)| d.device)
            .map(|(full, def)| {
                let value = look
                    .get(&def.key)
                    .filter(|v| (def.validator)(v).is_ok())
                    .unwrap_or_else(|| def.default.clone());
                (full.clone(), value)
            })
            .collect()
    }

    /// Current values with defaults for anything never saved. Stored values that are unknown
    /// or no longer valid are ignored (logged) rather than failing the whole read.
    pub fn get_all(&self, conn: &Connection) -> AppResult<SettingsSnapshot> {
        let mut values: BTreeMap<String, Value> = self
            .registry
            .iter()
            .map(|(k, d)| (k.clone(), d.default.clone()))
            .collect();
        values.extend(self.device_values());

        let mut stmt = conn.prepare("SELECT module, key, value FROM settings")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (module, key, raw) = row?;
            let full = format!("{module}.{key}");
            let Some(def) = self.registry.get(&full).filter(|d| !d.device) else {
                tracing::debug!(key = %full, "ignoring stored setting with no registered key");
                continue;
            };
            match serde_json::from_str::<Value>(&raw) {
                Ok(v) if (def.validator)(&v).is_ok() => {
                    values.insert(full, v);
                }
                _ => tracing::warn!(key = %full, "stored setting invalid, using default"),
            }
        }
        Ok(SettingsSnapshot {
            values,
            schema: self.registry.schema(),
        })
    }

    /// Validates every change, then writes them all in one transaction (all or nothing).
    /// Unknown keys are rejected. Publishes one `SettingsChanged` per Module after commit.
    pub fn save_batch(&self, conn: &mut Connection, changes: &[SettingsChange]) -> AppResult<()> {
        if changes.is_empty() {
            return Ok(());
        }
        // Validate everything before touching the database.
        let mut touched: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut rows = Vec::with_capacity(changes.len());
        let mut device_rows: Vec<(&str, Value)> = Vec::new();
        for change in changes {
            let def = self.registry.get(&change.key).ok_or_else(|| {
                AppError::new(ErrorCode::UnknownKey, "Unknown setting").with_details(&change.key)
            })?;
            (def.validator)(&change.value).map_err(|why| {
                AppError::new(
                    ErrorCode::Validation,
                    format!("Invalid value for {}", change.key),
                )
                .with_details(why)
            })?;
            let keys = touched.entry(def.module.as_str()).or_default();
            if !keys.contains(&def.key.as_str()) {
                keys.push(def.key.as_str());
            }
            if def.device {
                device_rows.push((def.key.as_str(), change.value.clone()));
                if change.key == "core.phone_tabs" {
                    device_rows.push(("phone_tabs_edited", Value::Bool(true)));
                }
            } else {
                rows.push((def.module.as_str(), def.key.as_str(), change.value.clone()));
            }
        }

        if !rows.is_empty() {
            let now = now_ms();
            let tx = conn.transaction()?;
            {
                let mut upsert = tx.prepare(
                    "INSERT INTO settings(module, key, value, updated_at) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(module, key) DO UPDATE
                     SET value = excluded.value, updated_at = excluded.updated_at",
                )?;
                for (module, key, value) in &rows {
                    upsert.execute(rusqlite::params![module, key, value.to_string(), now])?;
                    record(&tx, module, key, value, now)?;
                }
            }
            tx.commit()?;
        }
        // The device file after the database, so a refused batch never half-applies. If the file
        // cannot be written the session still has the values, so the UI is told either way.
        let written = if device_rows.is_empty() {
            Ok(())
        } else {
            self.device.update(|d| {
                for (key, value) in &device_rows {
                    d.look.set(key, value);
                }
            })
        };
        self.announce(&touched);
        written
    }

    fn announce(&self, touched: &BTreeMap<&str, Vec<&str>>) {
        for (module, keys) in touched {
            self.bus.publish(&SettingsChanged {
                module: (*module).to_owned(),
                keys: keys.iter().map(|k| (*k).to_owned()).collect(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;
    use crate::error::ErrorCode;
    use crate::events::SettingsChanged;
    use crate::module::{CoreModule, Module};

    struct Fixture {
        conn: Connection,
        store: SettingsStore,
        device: Arc<DeviceStore>,
        events: Arc<Mutex<Vec<SettingsChanged>>>,
        dir: tempfile::TempDir,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open_in_memory().unwrap();
        crate::db::migrate::run(&mut conn, &[&CoreModule], dir.path()).unwrap();
        let mut reg = SettingsRegistry::new();
        CoreModule.register_settings(&mut reg);
        let device = Arc::new(DeviceStore::new(
            dir.path().join("device.toml"),
            crate::device::DeviceSettings::default(),
        ));
        let bus = EventBus::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        bus.subscribe(move |e: &SettingsChanged| sink.lock().unwrap().push(e.clone()));
        Fixture {
            conn,
            store: SettingsStore::new(Arc::new(reg), bus, Arc::clone(&device)),
            device,
            events,
            dir,
        }
    }

    fn change(key: &str, value: Value) -> SettingsChange {
        SettingsChange {
            key: key.into(),
            value,
        }
    }

    #[test]
    fn fresh_database_returns_core_defaults() {
        let f = fixture();
        let snap = f.store.get_all(&f.conn).unwrap();
        let expected = json!({
            "core.theme": "system",
            "core.density": "comfortable",
            "core.text_size": "m",
            "core.reduce_motion": false,
            "core.display_name": "",
            "core.time_format": "24h",
            "core.date_format": "long",
            "core.week_start": "mon",
            "core.phone_tabs": ["home", "journal", "todo"],
            "core.phone_tabs_edited": false,
            "core.disabled_modules": [],
            "core.enabled_optional_modules": []
        });
        assert_eq!(serde_json::to_value(&snap.values).unwrap(), expected);
        assert_eq!(snap.schema.len(), 12);
    }

    #[test]
    fn save_if_unset_never_overwrites() {
        let f = fixture();
        assert!(save_if_unset(&f.conn, "core", "week_start", &json!("sun")).unwrap());
        assert!(!save_if_unset(&f.conn, "core", "week_start", &json!("mon")).unwrap());
        assert_eq!(
            f.store.get_all(&f.conn).unwrap().values["core.week_start"],
            json!("sun")
        );
    }

    #[test]
    fn save_if_unset_records_only_when_it_writes() {
        let f = fixture();
        let records = |f: &Fixture| -> i64 {
            f.conn
                .query_row(
                    "SELECT COUNT(*) FROM change_log WHERE entity = 'setting' AND entity_id = 'core.week_start'",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
        };
        save_if_unset(&f.conn, "core", "week_start", &json!("sun")).unwrap();
        save_if_unset(&f.conn, "core", "week_start", &json!("mon")).unwrap();
        assert_eq!(records(&f), 1);
    }

    #[test]
    fn every_core_default_passes_its_own_validator() {
        let f = fixture();
        for (key, def) in f.store.registry.iter() {
            assert!((def.validator)(&def.default).is_ok(), "{key}");
        }
    }

    #[test]
    fn saved_values_persist_and_overlay_defaults() {
        let mut f = fixture();
        f.store
            .save_batch(
                &mut f.conn,
                &[
                    change("core.theme", json!("oled")),
                    change("core.display_name", json!("Hemang")),
                ],
            )
            .unwrap();
        let snap = f.store.get_all(&f.conn).unwrap();
        assert_eq!(snap.values["core.theme"], json!("oled"));
        assert_eq!(snap.values["core.display_name"], json!("Hemang"));
        assert_eq!(snap.values["core.density"], json!("comfortable"));
        let stamp: i64 = f
            .conn
            .query_row(
                "SELECT updated_at FROM settings WHERE key = 'display_name'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(stamp > 1_704_067_200_000);
    }

    #[test]
    fn later_save_overwrites() {
        let mut f = fixture();
        f.store
            .save_batch(&mut f.conn, &[change("core.theme", json!("dark"))])
            .unwrap();
        f.store
            .save_batch(&mut f.conn, &[change("core.theme", json!("light"))])
            .unwrap();
        assert_eq!(
            f.store.get_all(&f.conn).unwrap().values["core.theme"],
            json!("light")
        );
    }

    #[test]
    fn unknown_key_is_rejected_and_nothing_is_saved() {
        let mut f = fixture();
        let err = f
            .store
            .save_batch(
                &mut f.conn,
                &[
                    change("core.theme", json!("dark")),
                    change("core.bogus", json!(1)),
                ],
            )
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::UnknownKey);
        assert_eq!(err.details.as_deref(), Some("core.bogus"));
        assert_eq!(
            f.store.get_all(&f.conn).unwrap().values["core.theme"],
            json!("system")
        );
        assert!(f.events.lock().unwrap().is_empty());
    }

    #[test]
    fn one_invalid_value_rejects_the_whole_batch() {
        let mut f = fixture();
        let err = f
            .store
            .save_batch(
                &mut f.conn,
                &[
                    change("core.density", json!("compact")),
                    change("core.theme", json!("neon")),
                ],
            )
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Validation);
        assert_eq!(
            f.store.get_all(&f.conn).unwrap().values["core.density"],
            json!("comfortable")
        );
    }

    #[test]
    fn phone_tabs_allow_at_most_four() {
        let mut f = fixture();
        let five = json!(["a", "b", "c", "d", "e"]);
        let err = f
            .store
            .save_batch(&mut f.conn, &[change("core.phone_tabs", five)])
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Validation);
        f.store
            .save_batch(
                &mut f.conn,
                &[change("core.phone_tabs", json!(["home", "todo"]))],
            )
            .unwrap();
    }

    #[test]
    fn success_publishes_one_event_per_module() {
        let mut f = fixture();
        f.store
            .save_batch(
                &mut f.conn,
                &[
                    change("core.theme", json!("dark")),
                    change("core.density", json!("compact")),
                ],
            )
            .unwrap();
        let events = f.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].module, "core");
        assert_eq!(events[0].keys, vec!["theme", "density"]);
    }

    #[test]
    fn empty_batch_does_nothing() {
        let mut f = fixture();
        f.store.save_batch(&mut f.conn, &[]).unwrap();
        assert!(f.events.lock().unwrap().is_empty());
    }

    #[test]
    fn stale_invalid_stored_value_falls_back_to_default() {
        let f = fixture();
        f.conn
            .execute(
                "INSERT INTO settings(module, key, value, updated_at) VALUES ('core','week_start','\"neon\"',0),
                 ('core','removed_key','1',0)",
                [],
            )
            .unwrap();
        let snap = f.store.get_all(&f.conn).unwrap();
        assert_eq!(snap.values["core.week_start"], json!("mon"));
        assert!(!snap.values.contains_key("core.removed_key"));
    }

    #[test]
    fn a_stray_row_for_a_device_setting_is_ignored() {
        let f = fixture();
        f.conn
            .execute(
                "INSERT INTO settings(module, key, value, updated_at) VALUES ('core','theme','\"dark\"',0)",
                [],
            )
            .unwrap();
        assert_eq!(
            f.store.get_all(&f.conn).unwrap().values["core.theme"],
            json!("system")
        );
    }

    #[test]
    fn device_values_come_from_the_file_with_defaults_and_without_the_database() {
        let f = fixture();
        f.device
            .update(|d| {
                d.look.set("theme", &json!("oled"));
                d.look.set("density", &json!("neon"));
            })
            .unwrap();
        let values = f.store.device_values();
        assert_eq!(values["core.theme"], json!("oled"));
        assert_eq!(
            values["core.density"],
            json!("comfortable"),
            "invalid falls back"
        );
        assert_eq!(values["core.text_size"], json!("m"));
        assert_eq!(values.len(), 6, "only Device settings: {values:?}");
    }

    #[test]
    fn saving_a_device_setting_writes_the_file_and_not_the_database() {
        let mut f = fixture();
        f.store
            .save_batch(&mut f.conn, &[change("core.theme", json!("dark"))])
            .unwrap();
        let reread = crate::device::DeviceSettings::load(&f.dir.path().join("device.toml"));
        assert_eq!(reread.look.theme.as_deref(), Some("dark"));
        let rows: i64 = f
            .conn
            .query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))
            .unwrap();
        let logged: i64 = f
            .conn
            .query_row("SELECT COUNT(*) FROM change_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!((rows, logged), (0, 0));
    }

    #[test]
    fn moving_look_and_feel_copies_missing_values_deletes_rows_and_is_repeatable() {
        let f = fixture();
        f.conn
            .execute(
                "INSERT INTO settings(module, key, value, updated_at) VALUES
                 ('core','theme','\"dark\"',0), ('core','reduce_motion','true',0),
                 ('core','text_size','not json',0), ('core','week_start','\"sun\"',0)",
                [],
            )
            .unwrap();
        f.device
            .update(|d| d.look.theme = Some("light".into()))
            .unwrap();
        for _ in 0..2 {
            move_look_and_feel_to_device(&f.conn, &f.device).unwrap();
        }
        let look = f.device.get().look;
        assert_eq!(
            look.theme.as_deref(),
            Some("light"),
            "the file's own value wins"
        );
        assert_eq!(look.reduce_motion, Some(true));
        assert_eq!(look.text_size, None, "unreadable rows count as unset");
        let left: Vec<String> = f
            .conn
            .prepare("SELECT key FROM settings")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(left, vec!["week_start"]);
    }

    #[test]
    fn a_failed_file_write_keeps_the_rows_for_the_next_start() {
        let f = fixture();
        f.conn
            .execute(
                "INSERT INTO settings(module, key, value, updated_at) VALUES ('core','theme','\"dark\"',0)",
                [],
            )
            .unwrap();
        // A folder where the file should be: the atomic rename fails.
        let broken = Arc::new(DeviceStore::new(
            f.dir.path().to_path_buf(),
            crate::device::DeviceSettings::default(),
        ));
        assert!(move_look_and_feel_to_device(&f.conn, &broken).is_err());
        let rows: i64 = f
            .conn
            .query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }
}
