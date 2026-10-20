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

/// Saves `value` for `module.key` only when nothing is saved for it yet; true when it wrote.
/// For Modules pinning a value at upgrade time, so a new registered default cannot reach
/// existing installs. The value is trusted: it is not validated and announces no event.
pub fn save_if_unset(
    conn: &Connection,
    module: &str,
    key: &str,
    value: &Value,
) -> AppResult<bool> {
    let written = conn.execute(
        "INSERT OR IGNORE INTO settings(module, key, value, updated_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![module, key, value.to_string(), now_ms()],
    )?;
    Ok(written > 0)
}

/// Reads and writes Settings against a registry, announcing changes on the event bus.
pub struct SettingsStore {
    registry: Arc<SettingsRegistry>,
    bus: EventBus,
}

impl SettingsStore {
    /// Builds a store over a finished registry.
    pub fn new(registry: Arc<SettingsRegistry>, bus: EventBus) -> Self {
        Self { registry, bus }
    }

    /// Current values with defaults for anything never saved. Stored values that are unknown
    /// or no longer valid are ignored (logged) rather than failing the whole read.
    pub fn get_all(&self, conn: &Connection) -> AppResult<SettingsSnapshot> {
        let mut values: BTreeMap<String, Value> = self
            .registry
            .iter()
            .map(|(k, d)| (k.clone(), d.default.clone()))
            .collect();

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
            let Some(def) = self.registry.get(&full) else {
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
            rows.push((
                def.module.as_str(),
                def.key.as_str(),
                change.value.to_string(),
            ));
        }

        let now = now_ms();
        let tx = conn.transaction()?;
        {
            let mut upsert = tx.prepare(
                "INSERT INTO settings(module, key, value, updated_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(module, key) DO UPDATE
                 SET value = excluded.value, updated_at = excluded.updated_at",
            )?;
            for (module, key, json) in &rows {
                upsert.execute(rusqlite::params![module, key, json, now])?;
            }
        }
        tx.commit()?;

        for (module, keys) in touched {
            self.bus.publish(&SettingsChanged {
                module: module.to_owned(),
                keys: keys.into_iter().map(str::to_owned).collect(),
            });
        }
        Ok(())
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
        events: Arc<Mutex<Vec<SettingsChanged>>>,
        _dir: tempfile::TempDir,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open_in_memory().unwrap();
        crate::db::migrate::run(&mut conn, &[&CoreModule], dir.path()).unwrap();
        let mut reg = SettingsRegistry::new();
        CoreModule.register_settings(&mut reg);
        let bus = EventBus::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        bus.subscribe(move |e: &SettingsChanged| sink.lock().unwrap().push(e.clone()));
        Fixture {
            conn,
            store: SettingsStore::new(Arc::new(reg), bus),
            events,
            _dir: dir,
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
            "core.phone_tabs": ["home", "journal"],
            "core.disabled_modules": []
        });
        assert_eq!(serde_json::to_value(&snap.values).unwrap(), expected);
        assert_eq!(snap.schema.len(), 10);
    }

    #[test]
    fn save_if_unset_never_overwrites() {
        let f = fixture();
        assert!(save_if_unset(&f.conn, "core", "phone_tabs", &json!(["home"])).unwrap());
        assert!(!save_if_unset(&f.conn, "core", "phone_tabs", &json!(["a"])).unwrap());
        assert_eq!(
            f.store.get_all(&f.conn).unwrap().values["core.phone_tabs"],
            json!(["home"])
        );
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
                "SELECT updated_at FROM settings WHERE key = 'theme'",
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
                "INSERT INTO settings(module, key, value, updated_at) VALUES ('core','theme','\"neon\"',0),
                 ('core','removed_key','1',0)",
                [],
            )
            .unwrap();
        let snap = f.store.get_all(&f.conn).unwrap();
        assert_eq!(snap.values["core.theme"], json!("system"));
        assert!(!snap.values.contains_key("core.removed_key"));
    }
}
