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
use std::fmt::Write;

use mnemora_core::syncable::{FieldClass, RemoteChange, SyncEntity, SyncEvent};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::conflicts;
use crate::error::SyncError;
use crate::registry::Registry;
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// The newest record replayed: its clock, and its sequence on the device that made it.
    stamp: Stamp,
    seq: i64,
}

/// Where a replay got to, as kept in `sync_text_replay`.
struct Memo {
    rows: Rows,
    folded: Folded,
}

/// Replays every stored record that touched `unit` of one entity, in clock order. `None` when
/// no record has text for it.
///
/// The replay resumes from where the last one got to (`sync_text_replay`) when the records
/// before that point are still exactly the ones it replayed, so settling one more record of a
/// text edited for months costs one record, not all of them. The result is the same either way.
pub(crate) fn fold(
    conn: &Connection,
    schema: &SyncEntity,
    me: &str,
    entity: &str,
    entity_id: &str,
    unit: &TextUnit,
) -> Result<Option<Folded>, SyncError> {
    let memo = load_memo(conn, entity, entity_id, unit.text)?.filter(|m| {
        count_upto(conn, entity, entity_id, &m.folded).is_ok_and(|rows| rows == m.rows)
    });
    let after = memo.as_ref().map(|m| &m.folded);
    let mut recs = history(conn, schema, me, entity, entity_id, unit, after)?;
    recs.sort_by(|a, b| (&a.stamp, a.origin_seq).cmp(&(&b.stamp, b.origin_seq)));
    let folded = replay(&recs, unit, memo.map(|m| m.folded));
    if !recs.is_empty()
        && let Some(f) = &folded
    {
        save_memo(conn, entity, entity_id, unit.text, f)?;
    }
    Ok(folded)
}

/// What the change-log rows of a record sorting at or before a clock look like: how many, and
/// their local sequences added up. A row stored, or rewritten in place (it gets a new local
/// sequence, also when its clock moves from one place before the point to another), changes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rows {
    n: i64,
    seqs: i64,
}

/// The [`Rows`] of the record that sort at or before `to`.
fn count_upto(
    conn: &Connection,
    entity: &str,
    entity_id: &str,
    to: &Folded,
) -> Result<Rows, SyncError> {
    Ok(conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(seq), 0) FROM change_log WHERE entity = ?1 AND entity_id = ?2
           AND (hlc_ms, hlc_counter, origin_device, origin_seq) <= (?3, ?4, ?5, ?6)",
        params![
            entity,
            entity_id,
            to.stamp.ms,
            to.stamp.counter,
            to.stamp.device,
            to.seq
        ],
        |r| {
            Ok(Rows {
                n: r.get(0)?,
                seqs: r.get(1)?,
            })
        },
    )?)
}

fn load_memo(
    conn: &Connection,
    entity: &str,
    entity_id: &str,
    unit: &str,
) -> Result<Option<Memo>, SyncError> {
    let row: Option<(i64, i64, String, i64, i64, i64, String)> = conn
        .query_row(
            "SELECT ms, counter, device, seq, n, seqs, state FROM sync_text_replay
             WHERE entity = ?1 AND entity_id = ?2 AND unit = ?3",
            params![entity, entity_id, unit],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .optional()?;
    let Some((ms, counter, device, seq, n, seqs, state)) = row else {
        return Ok(None);
    };
    // A memo that cannot be read is just not used.
    let Ok(State {
        values,
        op,
        at,
        clashes,
    }) = serde_json::from_str::<State>(&state)
    else {
        return Ok(None);
    };
    let Some(op) = ChangeOp::parse(&op) else {
        return Ok(None);
    };
    Ok(Some(Memo {
        rows: Rows { n, seqs },
        folded: Folded {
            values,
            op,
            at,
            clashes,
            stamp: Stamp {
                ms,
                counter,
                device,
            },
            seq,
        },
    }))
}

/// What of a [`Folded`] is kept in `sync_text_replay.state`.
#[derive(Serialize, Deserialize)]
struct State {
    values: Map<String, Value>,
    op: String,
    at: i64,
    clashes: Vec<Clashed>,
}

fn save_memo(
    conn: &Connection,
    entity: &str,
    entity_id: &str,
    unit: &str,
    f: &Folded,
) -> Result<(), SyncError> {
    let state = serde_json::to_string(&State {
        values: f.values.clone(),
        op: f.op.as_str().to_owned(),
        at: f.at,
        clashes: f.clashes.clone(),
    })
    // Cannot fail for these types; an empty state is simply not used when read back.
    .unwrap_or_default();
    let rows = count_upto(conn, entity, entity_id, f)?;
    conn.execute(
        "INSERT INTO sync_text_replay(entity, entity_id, unit, ms, counter, device, seq, n, seqs, state)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(entity, entity_id, unit) DO UPDATE SET ms = excluded.ms,
             counter = excluded.counter, device = excluded.device, seq = excluded.seq,
             n = excluded.n, seqs = excluded.seqs, state = excluded.state",
        params![
            entity,
            entity_id,
            unit,
            f.stamp.ms,
            f.stamp.counter,
            f.stamp.device,
            f.seq,
            rows.n,
            rows.seqs,
            state
        ],
    )?;
    Ok(())
}

/// Replays `recs` (in clock order) onto `start`, what an earlier replay came to.
fn replay(recs: &[Rec], unit: &TextUnit, start: Option<Folded>) -> Option<Folded> {
    replay_noting(recs, unit, start, None)
}

/// [`replay`], also noting into `seen` every text the replay held or met on the way.
fn replay_noting(
    recs: &[Rec],
    unit: &TextUnit,
    start: Option<Folded>,
    mut seen: Option<&mut BTreeSet<String>>,
) -> Option<Folded> {
    let mut held = start;
    for rec in recs {
        if let Some(seen) = seen.as_deref_mut() {
            seen.insert(rec.text.clone());
        }
        held = Some(match held.take() {
            None => Folded {
                values: rec.values.clone(),
                op: rec.op,
                at: rec.at,
                clashes: Vec::new(),
                stamp: rec.stamp.clone(),
                seq: rec.origin_seq,
            },
            Some(mut ours) => {
                let ours_text = ours
                    .values
                    .get(unit.text)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                match &rec.prev {
                    // It replaced what is held (or says the same): it takes over.
                    Some(base) if *base != ours_text && rec.text != ours_text => {
                        let merged = textmerge::merge(base, &ours_text, &rec.text);
                        if merged.clashes() > 0 {
                            ours.clashes.push(Clashed {
                                base: base.clone(),
                                ours: ours_text,
                                ours_device: ours.stamp.device.clone(),
                                ours_at: ours.at,
                                theirs: rec.text.clone(),
                                theirs_device: rec.stamp.device.clone(),
                                theirs_at: rec.at,
                            });
                        }
                        let text = merged.text();
                        if let Some(seen) = seen.as_deref_mut() {
                            seen.insert(text.clone());
                        }
                        Folded {
                            values: merged_values(unit, &text, &ours.values, rec),
                            op: rec.op,
                            at: rec.at,
                            clashes: ours.clashes,
                            stamp: rec.stamp.clone(),
                            seq: rec.origin_seq,
                        }
                    }
                    _ => Folded {
                        values: rec.values.clone(),
                        op: rec.op,
                        at: rec.at,
                        clashes: ours.clashes,
                        stamp: rec.stamp.clone(),
                        seq: rec.origin_seq,
                    },
                }
            }
        });
    }
    held
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

/// The stored records that touched the unit's text, after the clock `after` if given.
///
/// Only the unit's own values are read out of each row, by the JSON operators of `SQLite`, instead of
/// parsing every whole snapshot here. The values are what `wire_fields` would send for the row
/// (the record's own `fields`, or its snapshot for a row from before `fields`; a group's other
/// members completed from the snapshot for this device's own rows).
fn history(
    conn: &Connection,
    _schema: &SyncEntity,
    me: &str,
    entity: &str,
    entity_id: &str,
    unit: &TextUnit,
    after: Option<&Folded>,
) -> Result<Vec<Rec>, SyncError> {
    // The text itself comes from the record's own `fields` (the snapshot only for a row from
    // before `fields`); a member that travels with it is completed from the snapshot for this
    // device's own rows, as `wire_fields` does.
    let columns: String = std::iter::once((unit.text, false))
        .chain(unit.others.iter().map(|k| (*k, true)))
        .fold(String::new(), |mut sql, (k, member)| {
            let own = if member { " OR origin_device = ?3" } else { "" };
            let _ = write!(
                sql,
                ", COALESCE(fields -> '$.{k}', CASE WHEN fields IS NULL{own} THEN snapshot -> '$.{k}' END)"
            );
            sql
        });
    let (ms, counter, device, seq) = after.map_or((i64::MIN, 0, "", 0), |f| {
        (f.stamp.ms, f.stamp.counter, f.stamp.device.as_str(), f.seq)
    });
    let mut stmt = conn.prepare(&format!(
        "SELECT origin_device, hlc_ms, hlc_counter, origin_seq, op, at, prev -> '$.{}'{columns}
         FROM change_log WHERE entity = ?1 AND entity_id = ?2
           AND (hlc_ms, hlc_counter, origin_device, origin_seq) > (?4, ?5, ?6, ?7)",
        unit.text
    ))?;
    let mut rows = stmt.query(params![entity, entity_id, me, ms, counter, device, seq])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let parsed = |i: usize| -> Result<Option<Value>, SyncError> {
            let json: Option<String> = row.get(i)?;
            Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
        };
        let Some(Value::String(text)) = parsed(7)? else {
            continue;
        };
        let Some(op) = ChangeOp::parse(&row.get::<_, String>(4)?) else {
            continue;
        };
        let mut values = Map::new();
        values.insert(unit.text.to_owned(), Value::String(text.clone()));
        for (i, name) in unit.others.iter().enumerate() {
            if let Some(v) = parsed(8 + i)? {
                values.insert((*name).to_owned(), v);
            }
        }
        let prev = match parsed(6)? {
            Some(Value::String(p)) => Some(p),
            _ => None,
        };
        out.push(Rec {
            stamp: Stamp {
                ms: row.get(1)?,
                counter: row.get(2)?,
                device: row.get(0)?,
            },
            origin_seq: row.get(3)?,
            op,
            at: row.get(5)?,
            values,
            text,
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
        // A record made from a text no stored record holds (the session record that carried it
        // was saved again since, and the save replaced it) is merged like any other, but what
        // it "clashes" with is a deletion the replay only seems to have made: the author's real
        // clash is raised by the record that replaced it.
        if !text_was_held(conn, schema, me, entity, entity_id, &unit, &clash.base)? {
            continue;
        }
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

/// Whether the unit's text was ever `text` in the replay of every stored record, from the first:
/// a record's own text, or what the merges of the records came to. Worked out afresh, not from
/// the memo, so that it depends on the records alone and every device answers the same.
fn text_was_held(
    conn: &Connection,
    schema: &SyncEntity,
    me: &str,
    entity: &str,
    entity_id: &str,
    unit: &TextUnit,
    text: &str,
) -> Result<bool, SyncError> {
    let mut recs = history(conn, schema, me, entity, entity_id, unit, None)?;
    recs.sort_by(|a, b| (&a.stamp, a.origin_seq).cmp(&(&b.stamp, b.origin_seq)));
    let mut seen = BTreeSet::new();
    replay_noting(&recs, unit, None, Some(&mut seen));
    Ok(seen.contains(text))
}

/// Brings the text of this device's own records to the replay too. A device's own save is a
/// record like any other, but nothing "receives" it, so without this its stored text is whatever
/// its author last typed, while every other device replays the same records to something else
/// whenever the replay does not simply end on that save: an editing session saves again in
/// place, which gives its record a new clock and drops the text other records were made from.
/// Looks at the own records made since the last time; a unit an editor holds is left for the
/// next time (the mark stays before it).
pub(crate) fn settle_local(
    conn: &Connection,
    registry: &Registry,
    me: &str,
    now: i64,
) -> Result<Settled, SyncError> {
    let mut out = Settled::default();
    let mark: i64 = conn.query_row("SELECT texts FROM sync_local_settled", [], |r| r.get(0))?;
    let top: i64 = conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM change_log", [], |r| {
        r.get(0)
    })?;
    if top <= mark {
        return Ok(out);
    }
    let own: Vec<(i64, String, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT seq, entity, entity_id, fields FROM change_log
             WHERE seq > ?1 AND origin_device = ?2 AND fields IS NOT NULL
               AND entity NOT LIKE 'sync\\_%' ESCAPE '\\'
             ORDER BY seq",
        )?;
        stmt.query_map(params![mark, me], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?
    };
    let mut done: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut held_back: Option<i64> = None;
    for (seq, entity, entity_id, fields) in own {
        let Some((_, schema)) = registry.find(&entity) else {
            continue;
        };
        let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(&fields) else {
            continue;
        };
        let scopes = mnemora_core::editing::held(conn, &entity, &entity_id, now)?;
        for name in fields.keys() {
            let Some(field) = schema.field(name) else {
                continue;
            };
            let key = field.group.unwrap_or(field.name);
            if unit(schema, key).is_none() {
                continue;
            }
            if mnemora_core::editing::is_held(&scopes, key) {
                held_back.get_or_insert(seq);
            } else if done.insert((entity.clone(), entity_id.clone(), key.to_owned())) {
                let settled = settle_unit(conn, registry, me, &entity, &entity_id, key)?;
                out.changed.extend(settled.changed);
                out.events.extend(settled.events);
                out.recorded |= settled.recorded;
            }
        }
    }
    let next = held_back.map_or(top, |seq| seq - 1);
    conn.execute("UPDATE sync_local_settled SET texts = ?1", [next])?;
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
        let f = replay(&recs, &body(), None).unwrap();
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
        let f = replay(&recs, &body(), None).unwrap();
        assert_eq!(f.values["body"], "ALPHA beta GAMMA");
        assert!(f.clashes.is_empty());

        let recs = [
            rec(1, "a", Some(base), "alpha BETA gamma", false),
            rec(2, "b", Some(base), "alpha Beta gamma", false),
        ];
        let f = replay(&recs, &body(), None).unwrap();
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
        let want = replay(&sorted, &body(), None).unwrap().values["body"].clone();
        assert_eq!(want, "X Y Z!");
        // `fold` sorts, so any arrival order replays the same.
        let mut shuffled = all.to_vec();
        shuffled.reverse();
        shuffled.sort_by(|a, b| a.stamp.cmp(&b.stamp));
        assert_eq!(
            replay(&shuffled, &body(), None).unwrap().values["body"],
            want
        );
    }

    #[test]
    fn a_record_without_the_text_it_replaced_replaces() {
        let recs = [
            rec(1, "a", None, "old", false),
            rec(2, "b", None, "new", false),
        ];
        assert_eq!(replay(&recs, &body(), None).unwrap().values["body"], "new");
    }

    #[test]
    fn the_edited_flag_stays_set_when_either_side_set_it() {
        let recs = [
            rec(1, "a", Some("a b"), "A b", true),
            rec(2, "b", Some("a b"), "a B", false),
        ];
        let f = replay(&recs, &body(), None).unwrap();
        assert_eq!(
            (f.values["body"].as_str(), f.values["edited"].as_bool()),
            (Some("A B"), Some(true))
        );
    }
}
