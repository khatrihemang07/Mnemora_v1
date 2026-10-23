//! The sync contract (ADR 0017): how a Module tells Sync what it keeps in step between devices
//! and takes remote values back. The Sync crate never imports a Module; it only reads this.
//!
//! A Module lists its syncable **entities** (the names it uses in the change log) and classifies
//! every field of each:
//!
//! - [`FieldClass::Text`]: text a person wrote. Merged three-way; a clash becomes a Conflict.
//! - [`FieldClass::Value`]: newest wins.
//! - [`FieldClass::Order`]: manual-order keys. Newest wins, and is reported apart from values.
//! - [`FieldClass::Derived`]: recomputed locally from other fields, never synced.
//!
//! Fields that must never be split share a **group** (the When of a Task, its state and closed
//! time): a change to any member sends every member, and a receiver takes the whole group or
//! none of it.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use specta::Type;

use crate::changelog::ChangeOp;
use crate::settings::SettingsRegistry;

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
#[derive(Clone, Copy)]
pub struct RemoteChange<'a> {
    /// The change-log entity name.
    pub entity: &'a str,
    /// Id of the entity.
    pub entity_id: &'a str,
    /// What the origin device did.
    pub op: ChangeOp,
    /// When the origin made it: its wall-clock milliseconds (for times derived from it).
    pub at: i64,
    /// `{field: value}` of the winning fields; whole groups only. May be empty when every field
    /// lost: the Module then applies nothing and returns the entity as it is.
    pub fields: &'a Map<String, Value>,
    /// Every Module's registered Preferences: what a Preference may be set to, and which are
    /// Device settings (never taken from another device).
    pub settings: &'a SettingsRegistry,
}

/// What applying a [`RemoteChange`] left behind.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// The entity's full state afterwards, in the shape of its change-log snapshots.
    pub snapshot: Value,
    /// Whether anything visible changed (so screens should refresh).
    pub changed: bool,
    /// What happened that Sync reports on: the hook for the Activity log and for Resolved
    /// automatically (never carries more than names a screen already shows).
    pub events: Vec<SyncEvent>,
}

/// What a Module's handling of a remote change tells Sync, beyond the data itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    /// A Task was put in the Inbox because the List it was moved to is gone on this device.
    TaskToInbox {
        /// The Task.
        task_id: String,
        /// The List it was meant for.
        list_id: String,
    },
    /// Two Lists with the same name became one: `from` was soft-deleted and its Tasks joined
    /// the end of `into`.
    ListsMerged {
        /// The List that went away.
        from_id: String,
        /// Its name.
        from_name: String,
        /// The device that made it.
        from_device: Option<String>,
        /// The List that survives.
        into_id: String,
        /// Its name.
        into_name: String,
        /// The device that made it.
        into_device: Option<String>,
    },
    /// Two devices rewrote the same words of a text: a Conflict now waits in the Sync Module
    /// (raised by the engine itself when it merges text, never by a Module). The newest version
    /// is what shows.
    ConflictRaised {
        /// The Conflict's id.
        id: String,
        /// The kind of record it is about (its change-log entity).
        entity: String,
        /// Its id.
        entity_id: String,
        /// The text field.
        field: String,
    },
    /// A text clash on a record that is deleted was resolved without asking: nothing needs the
    /// person. The hook for Resolved automatically.
    ConflictAutoResolved {
        /// The Conflict's id.
        id: String,
        /// The kind of record it is about.
        entity: String,
        /// Its id.
        entity_id: String,
        /// The text field.
        field: String,
    },
    /// A Preference took another device's value.
    SettingChanged {
        /// Owning Module id.
        module: String,
        /// Key within the Module.
        key: String,
    },
    /// Another device renamed THIS device (Sync's `sync_device` record): the app changes the name
    /// in its device file.
    DeviceRenamed {
        /// The new name.
        name: String,
    },
}

/// What [`Module::settle_remote`](crate::module::Module::settle_remote) did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settled {
    /// It recorded changes of its own in the change log (so clocks must be read again).
    pub recorded: bool,
    /// What it reports.
    pub events: Vec<SyncEvent>,
}

/// What one device holds of the things a person counts, for the first-merge preview and to tell an
/// empty device from one with data. Each Module fills in its own part; they are added together.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Holdings {
    /// Journal Entries (not deleted).
    pub entries: u32,
    /// Tasks (not deleted).
    pub tasks: u32,
    /// Names of the Lists (not deleted), in no particular order.
    pub lists: Vec<String>,
}

impl Holdings {
    /// Nothing at all: a fresh device.
    pub fn is_empty(&self) -> bool {
        self.entries == 0 && self.tasks == 0 && self.lists.is_empty()
    }

    /// Adds another Module's part.
    pub fn add(&mut self, other: Self) {
        self.entries += other.entries;
        self.tasks += other.tasks;
        self.lists.extend(other.lists);
    }
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
