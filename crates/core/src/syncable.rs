//! The sync contract (ADR 0017): how a Module tells Sync what it keeps in step between devices
//! and takes remote values back. The Sync crate never imports a Module; it only reads this.
//!
//! A Module lists its syncable **entities** (the names it uses in the change log) and classifies
//! every field of each:
//!
//! - [`FieldClass::Text`]: text a person wrote. Newest wins for now; three-way merge later.
//! - [`FieldClass::Value`]: newest wins.
//! - [`FieldClass::Order`]: manual-order keys. Newest wins, and is reported apart from values.
//! - [`FieldClass::Derived`]: recomputed locally from other fields, never synced.
//!
//! Fields that must never be split share a **group** (the When of a Task, its state and closed
//! time): a change to any member sends every member, and a receiver takes the whole group or
//! none of it.

use serde_json::{Map, Value};

use crate::changelog::ChangeOp;

/// How a field merges when two devices changed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldClass {
    /// Text a person wrote.
    Text,
    /// Newest wins; logged as resolved automatically.
    Value,
    /// A manual-order key: newest wins.
    Order,
    /// Recomputed locally; never sent.
    Derived,
}

/// One field of a syncable entity.
#[derive(Debug, Clone, Copy)]
pub struct SyncField {
    /// The key in the entity's change-log snapshot.
    pub name: &'static str,
    /// How it merges.
    pub class: FieldClass,
    /// The atomic group it belongs to, if any.
    pub group: Option<&'static str>,
}

impl SyncField {
    /// A [`FieldClass::Text`] field.
    pub const fn text(name: &'static str) -> Self {
        Self::of(name, FieldClass::Text)
    }

    /// A [`FieldClass::Value`] field.
    pub const fn value(name: &'static str) -> Self {
        Self::of(name, FieldClass::Value)
    }

    /// A [`FieldClass::Order`] field.
    pub const fn order(name: &'static str) -> Self {
        Self::of(name, FieldClass::Order)
    }

    /// A [`FieldClass::Derived`] field.
    pub const fn derived(name: &'static str) -> Self {
        Self::of(name, FieldClass::Derived)
    }

    const fn of(name: &'static str, class: FieldClass) -> Self {
        Self {
            name,
            class,
            group: None,
        }
    }

    /// Puts the field in an atomic group.
    #[must_use]
    pub const fn in_group(mut self, group: &'static str) -> Self {
        self.group = Some(group);
        self
    }
}

/// One syncable kind of record, e.g. a Todo Task.
#[derive(Debug, Clone, Copy)]
pub struct SyncEntity {
    /// The change-log entity name.
    pub name: &'static str,
    /// Every field of its snapshot that Sync should know about. A snapshot key that is not
    /// listed is neither sent nor applied.
    pub fields: &'static [SyncField],
}

impl SyncEntity {
    /// The field called `name`, if it is listed.
    pub fn field(&self, name: &str) -> Option<&SyncField> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// The members of an atomic group.
    pub fn members<'a>(&'a self, group: &'a str) -> impl Iterator<Item = &'a SyncField> + 'a {
        self.fields.iter().filter(move |f| f.group == Some(group))
    }
}

/// Field values from another device for one entity: the ones that won the merge.
#[derive(Debug, Clone, Copy)]
pub struct RemoteChange<'a> {
    /// The change-log entity name.
    pub entity: &'a str,
    /// Id of the entity.
    pub entity_id: &'a str,
    /// What the origin device did.
    pub op: ChangeOp,
    /// `{field: value}` of the winning fields; whole groups only. May be empty when every field
    /// lost: the Module then applies nothing and returns the entity as it is.
    pub fields: &'a Map<String, Value>,
}

/// What applying a [`RemoteChange`] left behind.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// The entity's full state afterwards, in the shape of its change-log snapshots.
    pub snapshot: Value,
    /// Whether anything visible changed (so screens should refresh).
    pub changed: bool,
}

/// The entity called `name` among a Module's declared ones.
pub fn entity_of<'a>(entities: &'a [SyncEntity], name: &str) -> Option<&'a SyncEntity> {
    entities.iter().find(|e| e.name == name)
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn fields_and_groups_are_looked_up_by_name() {
        assert_eq!(
            ENTITY.field("title").map(|f| f.class),
            Some(FieldClass::Text)
        );
        assert!(ENTITY.field("nope").is_none());
        let names: Vec<_> = ENTITY.members("g").map(|f| f.name).collect();
        assert_eq!(names, ["a", "b"]);
        assert_eq!(entity_of(&[ENTITY], "thing").map(|e| e.name), Some("thing"));
    }
}
