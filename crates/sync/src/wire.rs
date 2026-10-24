//! What two devices say to each other. JSON on the wire (the network transport frames it); the
//! types here are the whole protocol of a session.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The version vector: for each entity, the highest origin sequence held per origin device.
pub type Vector = BTreeMap<String, BTreeMap<String, i64>>;

/// One change, as it travels: who made it, when, and the fields it changed with the origin's
/// values. Any device that holds it can pass it on unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireRecord {
    /// Owning Module id.
    pub module: String,
    /// Entity kind.
    pub entity: String,
    /// Id of the changed entity.
    pub entity_id: String,
    /// What the origin did (`create`, `update`, ...).
    pub op: String,
    /// The origin's editing session. The same session arriving again replaces the earlier
    /// record: the origin rewrote it.
    pub session_id: Option<String>,
    /// The origin's wall-clock milliseconds.
    pub at: i64,
    /// The device that made the change.
    pub origin_device: String,
    /// Its sequence on that device.
    pub origin_seq: i64,
    /// Hybrid logical clock, milliseconds.
    pub hlc_ms: i64,
    /// Hybrid logical clock, counter.
    pub hlc_counter: i64,
    /// Changed fields only (whole groups), `{field: value}`.
    pub fields: Map<String, Value>,
    /// For the fields among `fields`: what this change replaced on the origin device. For a
    /// text it is what a receiver merges two devices' edits against (absent for a first record
    /// or one whose earlier text is unknown, and then the text simply replaces); for any other
    /// field it tells a change made after another one from a change made beside it.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub prev: Map<String, Value>,
    /// For a delete: what the origin held of the other devices' records of this kind of entity,
    /// `{device: highest origin sequence}`. An edit above it was made without knowing of the
    /// delete. Absent for any other record, and for deletes made by older versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen: Option<BTreeMap<String, i64>>,
}

/// A message of a sync session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// First message each way: who this is and what it already holds.
    Hello {
        /// Sender's device id.
        device: String,
        /// Sender's version vector.
        vector: Vector,
    },
    /// A run of records the receiver lacks, oldest first by clock. The receiver applies it in
    /// one transaction and answers with [`Message::Ack`].
    Batch {
        /// The records.
        records: Vec<WireRecord>,
        /// Nothing follows this batch.
        last: bool,
        /// On the first batch: how many records the whole stream holds, for progress.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        total: Option<usize>,
    },
    /// The batch before was applied and stored.
    Ack,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn messages_have_a_stable_json_shape() {
        let hello = Message::Hello {
            device: "a".into(),
            vector: Vector::from([("task".into(), BTreeMap::from([("b".into(), 4)]))]),
        };
        assert_eq!(
            serde_json::to_value(&hello).unwrap(),
            json!({"type": "hello", "device": "a", "vector": {"task": {"b": 4}}})
        );
        assert_eq!(
            serde_json::to_value(Message::Ack).unwrap(),
            json!({"type": "ack"})
        );
        let back: Message = serde_json::from_value(serde_json::to_value(&hello).unwrap()).unwrap();
        assert_eq!(back, hello);
    }
}
