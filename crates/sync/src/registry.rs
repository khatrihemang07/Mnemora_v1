//! What the engine knows about the Modules that sync: which entities exist and how their fields
//! merge. The engine reaches a Module only through the `Module` trait, never by import.

use std::sync::Arc;

use mnemora_core::module::Module;
use mnemora_core::settings::{self, SettingsRegistry};
use mnemora_core::syncable::{FieldClass, SyncEntity};
use serde_json::{Map, Value};

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

    /// Every syncable entity name.
    pub fn entities(&self) -> Vec<&'static str> {
        self.modules
            .iter()
            .flat_map(|m| m.sync_entities().iter().map(|e| e.name))
            .collect()
    }
}

/// The fields of a record that travel and count, with their values.
///
/// `fields` is what the record says it changed (`None` for a record from before the change log
/// kept that: every field of the snapshot). Derived fields and keys the Module does not list
/// are dropped. With `complete_groups`, a group with any member present gets all its members,
/// the missing ones read from `snapshot`: a change to the Due date sends the whole When. Do
/// that only for records made on this device; a received record already carries whole groups,
/// and its snapshot holds this device's merged values, not the sender's.
pub fn wire_fields(
    entity: &SyncEntity,
    fields: Option<Map<String, Value>>,
    snapshot: &Value,
    complete_groups: bool,
) -> Map<String, Value> {
    let base = fields.unwrap_or_else(|| snapshot.as_object().cloned().unwrap_or_default());
    let mut out = Map::new();
    for field in entity.fields {
        if field.class != FieldClass::Derived
            && let Some(value) = base.get(field.name)
        {
            out.insert(field.name.to_owned(), value.clone());
        }
    }
    if complete_groups {
        let groups: Vec<&str> = entity
            .fields
            .iter()
            .filter(|f| f.class != FieldClass::Derived && out.contains_key(f.name))
            .filter_map(|f| f.group)
            .collect();
        for group in groups {
            for member in entity.members(group) {
                if !out.contains_key(member.name)
                    && let Some(value) = snapshot.get(member.name)
                {
                    out.insert(member.name.to_owned(), value.clone());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use mnemora_core::syncable::SyncField;
    use serde_json::json;

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
