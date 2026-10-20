//! Settings schema registry and reusable validators.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;
use specta::Type;

/// TypeScript-facing description of any JSON value. Only used for type generation: on the wire
/// fields stay `serde_json::Value`, which specta cannot export (recursive, i64 numbers).
#[derive(Debug, Serialize, Type)]
#[serde(untagged)]
#[allow(dead_code, missing_docs)]
pub enum JsonValue {
    Null(()),
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

/// Checks a JSON value; the error string explains what is wrong.
pub type Validator = Box<dyn Fn(&Value) -> Result<(), String> + Send + Sync>;

/// A registered key.
pub struct SettingDef {
    /// Owning Module id.
    pub module: String,
    /// Key within the Module.
    pub key: String,
    /// Value used until the user saves one.
    pub default: Value,
    /// Gatekeeper for every write.
    pub validator: Validator,
}

/// Wire form of a registered key, so the UI can render defaults and reset buttons.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SettingSchema {
    /// Full key, `module.key`.
    pub key: String,
    /// Owning Module id.
    pub module: String,
    /// Default value.
    #[specta(type = JsonValue)]
    pub default: Value,
}

/// All keys Modules have declared, indexed by full key (`module.key`).
#[derive(Default)]
pub struct SettingsRegistry {
    defs: BTreeMap<String, SettingDef>,
}

impl SettingsRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares `module.key`. A duplicate is a programming error: it is logged and the
    /// first registration wins, so one Module cannot silently override another.
    pub fn register(&mut self, module: &str, key: &str, default: Value, validator: Validator) {
        let full = format!("{module}.{key}");
        if self.defs.contains_key(&full) {
            tracing::error!(key = %full, "duplicate settings key ignored");
            return;
        }
        self.defs.insert(
            full,
            SettingDef {
                module: module.to_owned(),
                key: key.to_owned(),
                default,
                validator,
            },
        );
    }

    /// Looks up a key by its full name.
    pub fn get(&self, full_key: &str) -> Option<&SettingDef> {
        self.defs.get(full_key)
    }

    /// All definitions in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &SettingDef)> {
        self.defs.iter()
    }

    /// Wire schema for every key.
    pub fn schema(&self) -> Vec<SettingSchema> {
        self.defs
            .iter()
            .map(|(full, d)| SettingSchema {
                key: full.clone(),
                module: d.module.clone(),
                default: d.default.clone(),
            })
            .collect()
    }
}

/// Reusable validators.
pub mod validate {
    use serde_json::Value;

    use super::Validator;

    /// Accepts a JSON boolean.
    pub fn boolean() -> Validator {
        Box::new(|v| {
            v.is_boolean()
                .then_some(())
                .ok_or_else(|| "expected a boolean".to_owned())
        })
    }

    /// Accepts one of the listed strings.
    pub fn one_of(allowed: &'static [&'static str]) -> Validator {
        Box::new(move |v| match v.as_str() {
            Some(s) if allowed.contains(&s) => Ok(()),
            _ => Err(format!("expected one of {allowed:?}")),
        })
    }

    /// Accepts a string of at most `max_len` characters.
    pub fn text(max_len: usize) -> Validator {
        Box::new(move |v| match v.as_str() {
            Some(s) if s.chars().count() <= max_len => Ok(()),
            Some(_) => Err(format!("at most {max_len} characters")),
            None => Err("expected a string".to_owned()),
        })
    }

    /// Accepts an array of strings with at most `max_items` entries.
    pub fn string_list(max_items: usize) -> Validator {
        Box::new(move |v| {
            let items = v.as_array().ok_or_else(|| "expected an array".to_owned())?;
            if items.len() > max_items {
                return Err(format!("at most {max_items} entries"));
            }
            if items.iter().all(Value::is_string) {
                Ok(())
            } else {
                Err("entries must be strings".to_owned())
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn registered_keys_are_found_by_full_name() {
        let mut reg = SettingsRegistry::new();
        reg.register(
            "core",
            "theme",
            json!("system"),
            validate::one_of(&["system", "dark"]),
        );
        let def = reg.get("core.theme").unwrap();
        assert_eq!((def.module.as_str(), def.key.as_str()), ("core", "theme"));
        assert!(reg.get("core.nope").is_none());
        assert_eq!(
            reg.schema(),
            vec![SettingSchema {
                key: "core.theme".into(),
                module: "core".into(),
                default: json!("system")
            }]
        );
    }

    #[test]
    fn duplicate_registration_keeps_the_first() {
        let mut reg = SettingsRegistry::new();
        reg.register("m", "k", json!(1), validate::text(5));
        reg.register("m", "k", json!(2), validate::text(5));
        assert_eq!(reg.get("m.k").unwrap().default, json!(1));
    }

    #[test]
    fn boolean_validator() {
        let v = validate::boolean();
        assert!(v(&json!(true)).is_ok());
        assert!(v(&json!("true")).is_err());
    }

    #[test]
    fn one_of_validator() {
        let v = validate::one_of(&["a", "b"]);
        assert!(v(&json!("a")).is_ok());
        assert!(v(&json!("c")).is_err());
        assert!(v(&json!(1)).is_err());
    }

    #[test]
    fn text_validator_counts_characters_not_bytes() {
        let v = validate::text(3);
        assert!(v(&json!("héé")).is_ok());
        assert!(v(&json!("abcd")).is_err());
        assert!(v(&json!(null)).is_err());
    }

    #[test]
    fn string_list_validator() {
        let v = validate::string_list(2);
        assert!(v(&json!([])).is_ok());
        assert!(v(&json!(["a", "b"])).is_ok());
        assert!(v(&json!(["a", "b", "c"])).is_err());
        assert!(v(&json!(["a", 1])).is_err());
        assert!(v(&json!("a")).is_err());
    }
}
