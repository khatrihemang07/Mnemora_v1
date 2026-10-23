//! What the engine knows about the Modules that sync: which entities exist and how their fields
//! merge. The engine reaches a Module only through the `Module` trait, never by import.

use std::sync::Arc;

use mnemora_core::module::Module;
use mnemora_core::settings::{self, SettingsRegistry};
use mnemora_core::syncable::{Holdings, SyncEntity};
use rusqlite::Connection;

use crate::SyncError;

/// Every Module of the app, shared with the engine.
pub type Modules = Arc<Vec<Box<dyn Module>>>;

/// Looks up the Module that owns a syncable entity.
#[derive(Clone)]
pub struct Registry {
    modules: Modules,
    settings: Arc<SettingsRegistry>,
}

impl Registry {
    /// A registry over `modules`.
    pub fn new(modules: Modules) -> Self {
        let mut settings = SettingsRegistry::new();
        for module in modules.iter() {
            module.register_settings(&mut settings);
        }
        Self {
            modules,
            settings: Arc::new(settings),
        }
    }

    /// Every Module's registered Preferences.
    pub fn settings(&self) -> &SettingsRegistry {
        &self.settings
    }

    /// Ids of `entity`'s records that stay on this device and are never sent: the Device
    /// settings among the Preferences (look and feel), whose old change-log rows still exist.
    pub fn local_only(&self, entity: &str) -> Vec<String> {
        if entity != settings::ENTITY {
            return Vec::new();
        }
        self.settings
            .iter()
            .filter(|(_, def)| def.device)
            .map(|(full, _)| full.clone())
            .collect()
    }

    /// The Module and field table of the entity called `name`.
    pub fn find(&self, name: &str) -> Option<(&dyn Module, &'static SyncEntity)> {
        self.modules.iter().find_map(|m| {
            let entity = m.sync_entities().iter().find(|e| e.name == name)?;
            Some((m.as_ref(), entity))
        })
    }

    /// What this device holds, every Module's part added up.
    pub fn holdings(&self, conn: &Connection) -> Result<Holdings, SyncError> {
        let mut all = Holdings::default();
        for module in self.modules.iter() {
            all.add(module.holdings(conn)?);
        }
        Ok(all)
    }

    /// Every syncable entity name.
    pub fn entities(&self) -> Vec<&'static str> {
        self.modules
            .iter()
            .flat_map(|m| m.sync_entities().iter().map(|e| e.name))
            .collect()
    }
}

pub use mnemora_core::syncable::wire_fields;

#[cfg(test)]
mod tests {
    use mnemora_core::syncable::SyncField;
    use serde_json::{Map, Value, json};

    use super::*;

    const ENTITY: SyncEntity = SyncEntity {
        name: "thing",
        fields: &[
            SyncField::text("title"),
            SyncField::value("a").in_group("g"),
            SyncField::value("b").in_group("g"),
            SyncField::derived("count"),
        ],
    };

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn a_changed_member_brings_its_whole_group_and_derived_fields_stay_home() {
        let snapshot = json!({"title": "t", "a": 1, "b": 2, "count": 9, "other": 1});
        let sent = wire_fields(
            &ENTITY,
            Some(map(json!({"a": 5, "count": 9, "other": 1}))),
            &snapshot,
            true,
        );
        assert_eq!(Value::Object(sent), json!({"a": 5, "b": 2}));
    }

    #[test]
    fn a_received_record_is_not_completed_from_this_devices_state() {
        let snapshot = json!({"a": 1, "b": 2});
        let sent = wire_fields(&ENTITY, Some(map(json!({"a": 5}))), &snapshot, false);
        assert_eq!(Value::Object(sent), json!({"a": 5}));
    }

    #[test]
    fn a_record_without_fields_means_every_field_of_its_snapshot() {
        let snapshot = json!({"title": "t", "a": 1, "b": 2, "count": 9});
        let sent = wire_fields(&ENTITY, None, &snapshot, true);
        assert_eq!(Value::Object(sent), json!({"title": "t", "a": 1, "b": 2}));
    }
}
