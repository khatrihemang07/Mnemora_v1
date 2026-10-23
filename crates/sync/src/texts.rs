//! The text of a record is the same on every device because it is a pure function of the
//! records that touched it: replayed in clock order, each record's change is carried onto what
//! the earlier ones left.
//!
//! A record that replaced the very text the replay holds simply takes over. A record made
//! from an older text (the author had not seen what came in between) is merged three-way: the
//! text it replaced is the base, what the replay holds is `ours`, its own text is `theirs`, and
//! the clash, if any, goes to the newer side so that the newest version shows. Because the
//! replay depends only on the records, in their clock order, delivery order cannot matter: a
//! record that turns up late is simply replayed in its place.
//!
//! A record without the text it replaced (older than this feature, or a first record) replaces
//! whatever is there.
//!
//! Never log text.

use mnemora_core::changelog::ChangeOp;
use mnemora_core::hlc::Stamp;
use std::collections::BTreeSet;

use mnemora_core::syncable::{FieldClass, RemoteChange, SyncEntity, SyncEvent};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};

use crate::conflicts;
use crate::error::SyncError;
use crate::registry::{Registry, wire_fields};
use crate::textmerge;

/// A unit of fields with text in it: the text field and what travels with it.
#[derive(Debug, Clone)]
pub(crate) struct TextUnit {
    /// The text field.
    pub(crate) text: &'static str,
    /// The other members of the group (Comment `edited`, Task `understood`).
    pub(crate) others: Vec<&'static str>,
}

/// The text unit called `key`, if that unit has a text field.
pub(crate) fn unit(schema: &SyncEntity, key: &str) -> Option<TextUnit> {
    let members: Vec<_> = schema
        .fields
        .iter()
        .filter(|f| f.class != FieldClass::Derived && f.group.unwrap_or(f.name) == key)
        .collect();
    let text = members.iter().find(|f| f.class == FieldClass::Text)?;
    Some(TextUnit {
        text: text.name,
        others: members
            .iter()
            .filter(|f| f.class != FieldClass::Text)
            .map(|f| f.name)
            .collect(),
    })
}

/// One text change of a unit.
#[derive(Debug, Clone)]
struct Rec {
    stamp: Stamp,
    origin_seq: i64,
    op: ChangeOp,
    at: i64,
    values: Map<String, Value>,
    text: String,
    prev: Option<String>,
}

/// Two texts that clashed, with the base they came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Clashed {
    /// What both started from.
    pub(crate) base: String,
    /// The older side's text (what the replay held).
    pub(crate) ours: String,
    /// Who made it.
    pub(crate) ours_device: String,
    /// And when (wall clock).
    pub(crate) ours_at: i64,
    /// The newer side's text.
    pub(crate) theirs: String,
    /// Who made it.
    pub(crate) theirs_device: String,
    /// And when (wall clock).
    pub(crate) theirs_at: i64,
}

/// What the replay of a unit comes to.
#[derive(Debug, Clone)]
pub(crate) struct Folded {
    /// The unit's values: the text, and its group's other members.
    pub(crate) values: Map<String, Value>,
    /// The newest record's operation, and its moment: what derived times follow.
    pub(crate) op: ChangeOp,
    pub(crate) at: i64,
    /// Every clash met on the way.
    pub(crate) clashes: Vec<Clashed>,
}

/// Replays every stored record that touched `unit` of one entity, in clock order. `None` when
/// no record has text for it.
pub(crate) fn fold(
    conn: &Connection,
    schema: &SyncEntity,
    me: &str,
    entity: &str,
    entity_id: &str,
    unit: &TextUnit,
) -> Result<Option<Folded>, SyncError> {
    let mut recs = history(conn, schema, me, entity, entity_id, unit)?;
    recs.sort_by(|a, b| (&a.stamp, a.origin_seq).cmp(&(&b.stamp, b.origin_seq)));
    Ok(replay(&recs, unit))
}

fn replay(recs: &[Rec], unit: &TextUnit) -> Option<Folded> {
    let mut held: Option<(String, Map<String, Value>, Stamp, i64)> = None;
    let mut clashes = Vec::new();
    for rec in recs {
        held = Some(match held.take() {
            None => (
                rec.text.clone(),
                rec.values.clone(),
                rec.stamp.clone(),
                rec.at,
            ),
            Some((ours, ours_values, ours_stamp, ours_at)) => {
                match &rec.prev {
                    // It replaced what is held (or says the same): it takes over.
                    Some(base) if *base != ours && rec.text != ours => {
                        let merged = textmerge::merge(base, &ours, &rec.text);
                        if merged.clashes() > 0 {
                            clashes.push(Clashed {
                                base: base.clone(),
                                ours: ours.clone(),
                                ours_device: ours_stamp.device.clone(),
                                ours_at,
                                theirs: rec.text.clone(),
                                theirs_device: rec.stamp.device.clone(),
                                theirs_at: rec.at,
                            });
                        }
                        let text = merged.text();
                        let values = merged_values(unit, &text, &ours_values, rec);
                        (text, values, rec.stamp.clone(), rec.at)
                    }
                    _ => (
                        rec.text.clone(),
                        rec.values.clone(),
                        rec.stamp.clone(),
                        rec.at,
                    ),
                }
            }
        });
    }
    let (_, values, _, _) = held.clone()?;
    let newest = recs.last()?;
    Some(Folded {
        values,
        op: newest.op,
        at: newest.at,
        clashes,
    })
}

/// The unit's values after a merge: the merged text, and the group's other members from the
/// newer record, except that a flag stays set if either side set it and a member that
/// described the newer text (recognised words) is dropped when the text is no longer it.
fn merged_values(
    unit: &TextUnit,
    text: &str,
    ours: &Map<String, Value>,
    theirs: &Rec,
) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert(unit.text.to_owned(), Value::String(text.to_owned()));
    for name in &unit.others {
        let newer = theirs.values.get(*name).cloned().unwrap_or(Value::Null);
        let value = match (ours.get(*name), &newer) {
            (Some(Value::Bool(a)), Value::Bool(b)) => Value::Bool(*a || *b),
            _ if text == theirs.text => newer,
            (_, Value::Bool(_)) => newer,
            _ => Value::Null,
        };
        out.insert((*name).to_owned(), value);
    }
    out
}

fn unit_values(fields: &Map<String, Value>, unit: &TextUnit) -> Map<String, Value> {
    std::iter::once(unit.text)
        .chain(unit.others.iter().copied())
        .filter_map(|k| Some((k.to_owned(), fields.get(k)?.clone())))
        .collect()
}

/// The stored records that touched the unit's text.
fn history(
    conn: &Connection,
    schema: &SyncEntity,
    me: &str,
    entity: &str,
    entity_id: &str,
    unit: &TextUnit,
) -> Result<Vec<Rec>, SyncError> {
    let mut stmt = conn.prepare(
        "SELECT origin_device, hlc_ms, hlc_counter, origin_seq, op, at, fields, snapshot, prev
         FROM change_log WHERE entity = ?1 AND entity_id = ?2",
    )?;
    let mut rows = stmt.query(params![entity, entity_id])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let device: String = row.get(0)?;
        let origin_seq: i64 = row.get(3)?;
        let fields: Option<String> = row.get(6)?;
        let snapshot: String = row.get(7)?;
        let prev: Option<String> = row.get(8)?;
        let fields = fields
            .and_then(|f| serde_json::from_str::<Value>(&f).ok())
            .and_then(|v| match v {
                Value::Object(map) => Some(map),
                _ => None,
            });
        let snapshot: Value = serde_json::from_str(&snapshot).unwrap_or(Value::Null);
        let wire = wire_fields(schema, fields, &snapshot, device == me);
        let Some(text) = wire.get(unit.text).and_then(Value::as_str) else {
            continue;
        };
        let Some(op) = ChangeOp::parse(&row.get::<_, String>(4)?) else {
            continue;
        };
        let prev = prev
            .and_then(|p| serde_json::from_str::<Value>(&p).ok())
            .and_then(|p| p.get(unit.text).and_then(Value::as_str).map(str::to_owned));
        out.push(Rec {
            stamp: Stamp {
                ms: row.get(1)?,
                counter: row.get(2)?,
                device,
            },
            origin_seq,
            op,
            at: row.get(5)?,
            values: unit_values(&wire, unit),
            text: text.to_owned(),
            prev,
        });
    }
    Ok(out)
}

/// What settling a text unit did.
#[derive(Debug, Default)]
pub(crate) struct Settled {
    /// Modules whose data visibly changed (Sync itself when a Conflict was raised).
    pub(crate) changed: BTreeSet<String>,
    /// What the Modules and the Conflicts report.
    pub(crate) events: Vec<SyncEvent>,
    /// A Module recorded changes of its own, so field clocks must be read again.
    pub(crate) recorded: bool,
}

/// Brings one text unit of a record to what the replay of all its stored records comes to:
/// writes it through the Module if the record holds something else, raises a Conflict for each
/// clash met on the way, and closes the Conflicts that replay no longer produces. Called once the
/// records of a batch are stored, and when a hold ends, so the text never depends on the
/// order records arrived in.
pub(crate) fn settle_unit(
    conn: &Connection,
    registry: &Registry,
    me: &str,
    entity: &str,
    entity_id: &str,
    key: &str,
) -> Result<Settled, SyncError> {
    let mut out = Settled::default();
    let Some((module, schema)) = registry.find(entity) else {
        return Ok(out);
    };
    let Some(unit) = unit(schema, key) else {
        return Ok(out);
    };
    let Some(folded) = fold(conn, schema, me, entity, entity_id, &unit)? else {
        return Ok(out);
    };
    let snapshot = current(conn, entity, entity_id)?;
    if !already(snapshot.as_ref(), &folded.values) {
        let change = RemoteChange {
            entity,
            entity_id,
            op: folded.op,
            at: folded.at,
            fields: &folded.values,
            settings: registry.settings(),
        };
        let outcome = module.apply_remote(conn, &change)?;
        if outcome.changed {
            // The newest log row says what the record looks like now (the next local change is
            // compared with it to see what it touched).
            conn.execute(
                "UPDATE change_log SET snapshot = ?3
                 WHERE entity = ?1 AND entity_id = ?2
                   AND seq = (SELECT MAX(seq) FROM change_log WHERE entity = ?1 AND entity_id = ?2)",
                params![entity, entity_id, outcome.snapshot.to_string()],
            )?;
            out.changed.insert(module.id().to_owned());
        }
        out.events.extend(outcome.events);
        let settled = module.settle_remote(conn, &change)?;
        out.recorded = settled.recorded;
        if settled.recorded || !settled.events.is_empty() {
            out.changed.insert(module.id().to_owned());
        }
        out.events.extend(settled.events);
    }
    let mut live = Vec::new();
    for clash in &folded.clashes {
        live.push(conflicts::conflict_id(
            entity,
            entity_id,
            unit.text,
            &clash.base,
            &clash.ours,
            &clash.theirs,
        ));
        if let Some(raised) = conflicts::raise(conn, entity, entity_id, unit.text, clash)? {
            out.changed.insert(crate::MODULE_ID.to_owned());
            out.events.push(raised);
        }
    }
    conflicts::supersede(conn, entity, entity_id, unit.text, &live)?;
    Ok(out)
}

/// What the newest change-log snapshot of a record says it holds now: the Module's state as
/// last written.
pub(crate) fn current(
    conn: &Connection,
    entity: &str,
    entity_id: &str,
) -> Result<Option<Value>, SyncError> {
    let snapshot: Option<String> = conn
        .query_row(
            "SELECT snapshot FROM change_log WHERE entity = ?1 AND entity_id = ?2
             ORDER BY seq DESC LIMIT 1",
            [entity, entity_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(snapshot.and_then(|s| serde_json::from_str(&s).ok()))
}

/// Whether the unit's values already stand in `snapshot`.
pub(crate) fn already(snapshot: Option<&Value>, values: &Map<String, Value>) -> bool {
    snapshot.is_some_and(|s| {
        values
            .iter()
            .all(|(k, v)| s.get(k).unwrap_or(&Value::Null) == v)
    })
}

#[cfg(test)]
mod tests {
    use mnemora_core::syncable::SyncField;

    use super::*;

    const THING: SyncEntity = SyncEntity {
        name: "thing",
        fields: &[
            SyncField::text("body").in_group("body"),
            SyncField::value("edited").in_group("body"),
            SyncField::text("notes"),
            SyncField::value("n"),
        ],
    };

    fn rec(ms: i64, device: &str, prev: Option<&str>, text: &str, edited: bool) -> Rec {
        let mut values = Map::new();
        values.insert("body".into(), Value::String(text.into()));
        values.insert("edited".into(), Value::Bool(edited));
        Rec {
            stamp: Stamp {
                ms,
                counter: 0,
                device: device.into(),
            },
            origin_seq: ms,
            op: ChangeOp::Update,
            at: ms,
            values,
            text: text.into(),
            prev: prev.map(str::to_owned),
        }
    }

    fn body() -> TextUnit {
        unit(&THING, "body").unwrap()
    }

    #[test]
    fn units_are_found_by_key_and_list_each_text_once() {
        let u = body();
        assert_eq!(
            (u.text, u.others.as_slice()),
            ("body", ["edited"].as_slice())
        );
        assert!(unit(&THING, "n").is_none());
    }

    #[test]
    fn records_made_one_from_the_other_take_over_in_turn() {
        let recs = [
            rec(1, "a", None, "one", false),
            rec(2, "b", Some("one"), "two", false),
            rec(3, "a", Some("two"), "three", true),
        ];
        let f = replay(&recs, &body()).unwrap();
        assert_eq!(f.values["body"], "three");
        assert_eq!(f.values["edited"], true);
        assert!(f.clashes.is_empty());
    }

    #[test]
    fn concurrent_edits_of_different_words_merge_and_of_the_same_words_clash() {
        let base = "alpha beta gamma";
        let recs = [
            rec(1, "a", Some(base), "ALPHA beta gamma", false),
            rec(2, "b", Some(base), "alpha beta GAMMA", false),
        ];
        let f = replay(&recs, &body()).unwrap();
        assert_eq!(f.values["body"], "ALPHA beta GAMMA");
        assert!(f.clashes.is_empty());

        let recs = [
            rec(1, "a", Some(base), "alpha BETA gamma", false),
            rec(2, "b", Some(base), "alpha Beta gamma", false),
        ];
        let f = replay(&recs, &body()).unwrap();
        assert_eq!(f.values["body"], "alpha Beta gamma", "the newer side shows");
        assert_eq!(
            f.clashes,
            [Clashed {
                base: base.into(),
                ours: "alpha BETA gamma".into(),
                ours_device: "a".into(),
                ours_at: 1,
                theirs: "alpha Beta gamma".into(),
                theirs_device: "b".into(),
                theirs_at: 2,
            }]
        );
    }

    #[test]
    fn the_order_the_records_were_received_in_does_not_matter() {
        let base = "x y z";
        let all = [
            rec(1, "a", Some(base), "X y z", false),
            rec(2, "b", Some(base), "x Y z", false),
            rec(3, "c", Some(base), "x y Z", false),
            rec(4, "a", Some("X Y Z"), "X Y Z!", false),
        ];
        let mut sorted = all.to_vec();
        sorted.sort_by(|a, b| a.stamp.cmp(&b.stamp));
        let want = replay(&sorted, &body()).unwrap().values["body"].clone();
        assert_eq!(want, "X Y Z!");
        // `fold` sorts, so any arrival order replays the same.
        let mut shuffled = all.to_vec();
        shuffled.reverse();
        shuffled.sort_by(|a, b| a.stamp.cmp(&b.stamp));
        assert_eq!(replay(&shuffled, &body()).unwrap().values["body"], want);
    }

    #[test]
    fn a_record_without_the_text_it_replaced_replaces() {
        let recs = [
            rec(1, "a", None, "old", false),
            rec(2, "b", None, "new", false),
        ];
        assert_eq!(replay(&recs, &body()).unwrap().values["body"], "new");
    }

    #[test]
    fn the_edited_flag_stays_set_when_either_side_set_it() {
        let recs = [
            rec(1, "a", Some("a b"), "A b", true),
            rec(2, "b", Some("a b"), "a B", false),
        ];
        let f = replay(&recs, &body()).unwrap();
        assert_eq!(
            (f.values["body"].as_str(), f.values["edited"].as_bool()),
            (Some("A B"), Some(true))
        );
    }
}
