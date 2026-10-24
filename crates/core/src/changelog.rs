//! The app-wide change log (ADR 0008): full JSON snapshots of changed entities, one record per
//! editing session. A Module may read its own records to build a readable history (Todo's Task
//! History); the raw log itself is never sent over IPC.
//!
//! Snapshots hold user content, so nothing in this file may log them.

use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension};
use serde_json::{Map, Value};

use crate::error::AppResult;
use crate::hlc;
use crate::syncable::{FieldClass, SyncEntity};
use crate::util::new_id;

/// What happened to the entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeOp {
    /// The entity was created.
    Create,
    /// The entity was edited.
    Update,
    /// The entity was moved, e.g. a Journal Entry to another Page.
    Move,
    /// A checklist item inside the entity was ticked or unticked.
    Tick,
    /// The entity was deleted (soft or hard); the snapshot is its last state.
    Delete,
    /// A deleted entity was brought back.
    Restore,
}

impl ChangeOp {
    /// Reads a stored `op` value.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "create" => Self::Create,
            "update" => Self::Update,
            "move" => Self::Move,
            "tick" => Self::Tick,
            "delete" => Self::Delete,
            "restore" => Self::Restore,
            _ => return None,
        })
    }

    /// The value stored in the `op` column.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Move => "move",
            Self::Tick => "tick",
            Self::Delete => "delete",
            Self::Restore => "restore",
        }
    }
}

/// One change to record.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    /// Id of the owning Module.
    pub module: String,
    /// Kind of entity, e.g. `journal_entry`.
    pub entity: String,
    /// Id of the changed entity.
    pub entity_id: String,
    /// What happened.
    pub op: ChangeOp,
    /// Editing session. Changes sharing an entity and session collapse into one record.
    pub session_id: Option<String>,
    /// Full state of the entity after the change.
    pub snapshot: Value,
}

/// Records `change`. With a session id, the session's existing record for this entity is
/// replaced (snapshot, time, clock and sequence; a `create` stays a `create`); otherwise a new record is appended.
/// Pass a transaction to make it atomic with the entity write.
///
/// The record also keeps which fields the change touched, with their new values (`fields`): what
/// Sync sends. A session's rewrite keeps what the session changed before and adds what it
/// changed since.
pub fn record(conn: &Connection, change: &Change) -> AppResult<()> {
    // Every write ticks, a session's rewrite included: peers pick records up by sequence.
    let tick = hlc::tick(conn)?;
    write(conn, change, tick)
}

/// Writes `change` as a record stamped with `tick`.
fn write(conn: &Connection, change: &Change, tick: hlc::Tick) -> AppResult<()> {
    let device = hlc::device_id(conn)?;
    let touched = changed_fields(conn, change)?;
    let seen = seen_by_delete(conn, change, &touched, &device)?;
    conn.execute(
        "INSERT INTO change_log(id, module, entity, entity_id, op, session_id, snapshot, at,
                                origin_device, hlc_ms, hlc_counter, seq, origin_seq, fields, prev,
                                seen)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, ?13, ?14, ?15)
         ON CONFLICT(entity, entity_id, session_id) WHERE session_id IS NOT NULL
         DO UPDATE SET op = CASE WHEN op = 'create' THEN op ELSE excluded.op END,
             snapshot = excluded.snapshot, at = excluded.at,
             origin_device = excluded.origin_device, hlc_ms = excluded.hlc_ms,
             hlc_counter = excluded.hlc_counter, seq = excluded.seq,
             origin_seq = excluded.origin_seq, fields = excluded.fields,
             prev = excluded.prev, seen = excluded.seen",
        rusqlite::params![
            new_id(),
            change.module,
            change.entity,
            change.entity_id,
            change.op.as_str(),
            change.session_id,
            change.snapshot.to_string(),
            tick.physical_ms,
            device,
            tick.clock.ms,
            tick.clock.counter,
            tick.seq,
            touched.fields.map(|f| Value::Object(f).to_string()),
            Value::Object(touched.prev).to_string(),
            seen,
        ],
    )?;
    Ok(())
}

/// A row to bring into the log: see [`backfill`].
#[derive(Debug, Clone)]
pub struct Unlogged<'a> {
    /// The row as it stands: `snapshot` is its full state; `op` is chosen by [`backfill`].
    pub change: Change,
    /// When the row says it last changed (UTC milliseconds): where its record is stamped, at
    /// the earliest.
    pub ms: i64,
    /// What the row refers to (`(entity, id)`: a Task's List, a Comment's Task): the first record of
    /// each sorts before this one.
    pub after: Vec<(&'a str, &'a str)>,
}

/// Brings the log up to date with the rows of one entity that it never (fully) described: state
/// that predates the log recording every entity and field, comes from an older version or from
/// an import. Sync sends the log, not the tables (ADR 0017: pairing combines everything both
/// devices hold), so it calls this before it first reads the log. Returns how many records it
/// wrote.
///
/// Each row's snapshot is compared with the entity's latest snapshot on the non-derived fields of
/// `schema` that the row carries (a key the log never held counts as null). A row with no record
/// gets a `create`; one that differs gets an `update` of just the differing fields; one that
/// matches gets nothing, so running this again, or on any device, writes nothing more.
///
/// Each record is stamped on the clock at the row's own `ms` (counter 0, never later than now),
/// so unlogged history never outranks a real edit made after it, on any device. Only what keeps
/// this device's own story straight raises it: the entity's newest record of the same id (the
/// row is what this device holds now, so its record sorts after what the log said of it) and the
/// first record of everything it is `after` (a Task's List must sort before the Task). The
/// device clock is shown the stamp, so changes made later sort after. A stamp may sort before
/// records written earlier; the receiving side applies a batch by clock but skips by sequence
/// per origin, so that cannot drop a record.
pub fn backfill(
    conn: &Connection,
    schema: &SyncEntity,
    rows: Vec<Unlogged<'_>>,
) -> AppResult<usize> {
    let now = crate::db::physical_now(conn);
    let mut written = 0;
    for row in rows {
        let Unlogged {
            mut change,
            ms,
            after,
        } = row;
        let latest: Option<String> = conn
            .query_row(
                "SELECT snapshot FROM change_log WHERE entity = ?1 AND entity_id = ?2
                 ORDER BY seq DESC LIMIT 1",
                [&change.entity, &change.entity_id],
                |r| r.get(0),
            )
            .optional()?;
        let before: Option<Value> = latest.and_then(|s| serde_json::from_str(&s).ok());
        match &before {
            None => change.op = ChangeOp::Create,
            Some(before) => {
                let Some(now) = change.snapshot.as_object() else {
                    continue;
                };
                let differs = schema.fields.iter().any(|f| {
                    f.class != FieldClass::Derived
                        && now
                            .get(f.name)
                            .is_some_and(|v| v != before.get(f.name).unwrap_or(&Value::Null))
                });
                if !differs {
                    continue;
                }
                change.op = ChangeOp::Update;
            }
        }
        let ms = ms.min(now);
        let mut clock = hlc::Timestamp { ms, counter: 0 };
        let mut raise = |floor: hlc::Timestamp| {
            if clock <= floor {
                clock = hlc::Timestamp {
                    ms: floor.ms,
                    counter: floor.counter.saturating_add(1),
                };
            }
        };
        if let Some(newest) = clock_of(conn, &change.entity, &change.entity_id, true)? {
            raise(newest);
        }
        for (entity, id) in after {
            if let Some(first) = clock_of(conn, entity, id, false)? {
                raise(first);
            }
        }
        let tick = hlc::tick_at(conn, clock, ms)?;
        write(conn, &change, tick)?;
        written += 1;
    }
    Ok(written)
}

/// The newest (or first) clock of any record of one entity row.
fn clock_of(
    conn: &Connection,
    entity: &str,
    id: &str,
    newest: bool,
) -> AppResult<Option<hlc::Timestamp>> {
    let order = if newest { "DESC" } else { "ASC" };
    Ok(conn
        .query_row(
            &format!(
                "SELECT hlc_ms, hlc_counter FROM change_log WHERE entity = ?1 AND entity_id = ?2
                 ORDER BY hlc_ms {order}, hlc_counter {order} LIMIT 1"
            ),
            [entity, id],
            |r| {
                Ok(hlc::Timestamp {
                    ms: r.get(0)?,
                    counter: r.get(1)?,
                })
            },
        )
        .optional()?)
}

/// For a record that deletes, what this device had of the other devices' records of the same
/// kind of entity: `{device: highest origin sequence held}`, as JSON. Sync tells an edit made
/// without knowing of the delete from one the delete's author had already seen. `None` for any
/// other record.
fn seen_by_delete(
    conn: &Connection,
    change: &Change,
    touched: &Touched,
    device: &str,
) -> AppResult<Option<String>> {
    let deletes = change.op == ChangeOp::Delete
        || touched
            .fields
            .as_ref()
            .and_then(|f| f.get("deleted_at"))
            .is_some_and(|d| !d.is_null());
    if !deletes {
        return Ok(None);
    }
    let mut stmt = conn.prepare(
        "SELECT origin_device, MAX(origin_seq) FROM change_log
         WHERE entity = ?1 AND origin_device != ?2 GROUP BY origin_device",
    )?;
    let seen = stmt
        .query_map([change.entity.as_str(), device], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(Some(serde_json::to_string(&seen).unwrap_or_default()))
}

/// What a change touched, and what it replaced.
struct Touched {
    /// `{field: new value}`; `None` stands for "every field of the snapshot".
    fields: Option<Map<String, Value>>,
    /// `{field: value before}` for the touched fields whose earlier value is known.
    prev: Map<String, Value>,
}

/// What `change` touched, as `{field: new value}`: the keys whose value differs from the
/// entity's latest snapshot (all of them for a first record), plus, for a session's rewrite,
/// what the session's record already held. `fields` is `None` when the session's record
/// predates this column: it keeps meaning "every field of the snapshot".
///
/// `prev` says what each touched key held before: in the latest snapshot, or, for a key the
/// session claimed earlier, what the session replaced when it first claimed it (the text the
/// editor started from, however often it saved since).
fn changed_fields(conn: &Connection, change: &Change) -> AppResult<Touched> {
    let latest: Option<String> = conn
        .query_row(
            "SELECT snapshot FROM change_log WHERE entity = ?1 AND entity_id = ?2
             ORDER BY seq DESC LIMIT 1",
            [&change.entity, &change.entity_id],
            |r| r.get(0),
        )
        .optional()?;
    let before: Option<Value> = latest.and_then(|s| serde_json::from_str(&s).ok());
    let mut keys: Vec<String> = diff_keys(&change.snapshot, before.as_ref());
    let diff_now: Vec<String> = keys.clone();
    // What the session's record held before this write.
    let mut earlier_prev = Map::new();
    let mut earlier_values = Map::new();
    let mut earlier_keys: Vec<String> = Vec::new();
    if let Some(session) = &change.session_id {
        let earlier: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT fields, prev FROM change_log
                 WHERE entity = ?1 AND entity_id = ?2 AND session_id = ?3",
                rusqlite::params![change.entity, change.entity_id, session],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match earlier {
            // A record from before `fields` existed stays "everything".
            Some((None, _)) => {
                return Ok(Touched {
                    fields: None,
                    prev: Map::new(),
                });
            }
            Some((Some(text), prev)) => {
                if let Ok(Value::Object(map)) = serde_json::from_str(&text) {
                    earlier_keys = map.keys().cloned().collect();
                    earlier_values.clone_from(&map);
                    // Still the session's own claim only while the value is still the one the
                    // session left: if another device's change landed since, and this write did
                    // not touch the key, the session must not re-stamp that value as its own.
                    let held = before.as_ref().and_then(Value::as_object);
                    keys.extend(map.into_iter().filter_map(|(k, v)| {
                        let untouched =
                            held.is_none_or(|h| h.get(&k).unwrap_or(&Value::Null) == &v);
                        (untouched || diff_now.contains(&k)).then_some(k)
                    }));
                }
                if let Some(Ok(Value::Object(map))) =
                    prev.map(|p| serde_json::from_str::<Value>(&p))
                {
                    earlier_prev = map;
                }
            }
            None => {}
        }
    }
    let now = change.snapshot.as_object();
    let was = before.as_ref().and_then(Value::as_object);
    let mut prev = Map::new();
    let fields = keys
        .into_iter()
        .map(|k| {
            // What the session started from, unless something else has changed the value since
            // the session left it: then this write replaces that newer value, and says so.
            let moved_on = was.is_some_and(|w| {
                earlier_values
                    .get(&k)
                    .is_some_and(|v| w.get(&k).unwrap_or(&Value::Null) != v)
            });
            if let Some(old) = earlier_prev.get(&k).filter(|_| !moved_on) {
                prev.insert(k.clone(), old.clone());
            } else if moved_on {
                prev.insert(
                    k.clone(),
                    was.and_then(|w| w.get(&k)).cloned().unwrap_or(Value::Null),
                );
            } else if earlier_keys.contains(&k) {
                // The session claimed it without knowing what it replaced (it made the record).
            } else if let Some(was) = was {
                // A key that did not exist before held nothing: null, like the record's own.
                prev.insert(k.clone(), was.get(&k).cloned().unwrap_or(Value::Null));
            }
            let v = now.and_then(|n| n.get(&k)).cloned().unwrap_or(Value::Null);
            (k, v)
        })
        .collect();
    Ok(Touched {
        fields: Some(fields),
        prev,
    })
}

/// The keys of `now` whose value differs from `before` (a key that vanished counts, and is
/// null in the record).
fn diff_keys(now: &Value, before: Option<&Value>) -> Vec<String> {
    let (Some(now), Some(Value::Object(before))) = (now.as_object(), before) else {
        return now
            .as_object()
            .map(|n| n.keys().cloned().collect())
            .unwrap_or_default();
    };
    let mut keys: Vec<String> = now
        .iter()
        .filter(|(k, v)| before.get(*k) != Some(*v))
        .map(|(k, _)| k.clone())
        .collect();
    keys.extend(before.keys().filter(|k| !now.contains_key(*k)).cloned());
    keys
}

/// A record received from another device, to be stored as it was sent.
#[derive(Debug, Clone)]
pub struct Remote<'a> {
    /// Id of the owning Module.
    pub module: &'a str,
    /// Kind of entity.
    pub entity: &'a str,
    /// Id of the changed entity.
    pub entity_id: &'a str,
    /// What happened.
    pub op: ChangeOp,
    /// The origin's editing session, if any: a record of the same session replaces the one held.
    pub session_id: Option<&'a str>,
    /// When the origin made it: its wall-clock milliseconds.
    pub at: i64,
    /// The device that made it.
    pub origin_device: &'a str,
    /// Its sequence on that device.
    pub origin_seq: i64,
    /// Its hybrid logical clock.
    pub clock: hlc::Timestamp,
    /// What it changed, with the origin's values, exactly as sent (so this device can relay it).
    pub fields: &'a Map<String, Value>,
    /// What the origin's change replaced, for the fields that travel with it (text), exactly as
    /// sent.
    pub prev: &'a Map<String, Value>,
    /// For a delete: what its origin held of the other devices' records of the entity kind,
    /// `{device: origin sequence}`, exactly as sent.
    pub seen: Option<&'a BTreeMap<String, i64>>,
}

/// Stores a record received from another device: shows the clock its timestamp, appends it
/// with a fresh local sequence and keeps its origin device, origin sequence and clock. A record
/// of an editing session this device already holds replaces it (the origin rewrote it).
/// `snapshot` is the entity's full state after the change was applied here. Pass a transaction.
pub fn record_remote(conn: &Connection, remote: &Remote<'_>, snapshot: &Value) -> AppResult<()> {
    let tick = hlc::observe_tick(conn, remote.clock)?;
    conn.execute(
        "INSERT INTO change_log(id, module, entity, entity_id, op, session_id, snapshot, at,
                                origin_device, hlc_ms, hlc_counter, seq, origin_seq, fields, prev,
                                seen)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
         ON CONFLICT(entity, entity_id, session_id) WHERE session_id IS NOT NULL
         DO UPDATE SET op = CASE WHEN op = 'create' THEN op ELSE excluded.op END,
             snapshot = excluded.snapshot, at = excluded.at,
             origin_device = excluded.origin_device, hlc_ms = excluded.hlc_ms,
             hlc_counter = excluded.hlc_counter, seq = excluded.seq,
             origin_seq = excluded.origin_seq, fields = excluded.fields,
             prev = excluded.prev, seen = excluded.seen",
        rusqlite::params![
            new_id(),
            remote.module,
            remote.entity,
            remote.entity_id,
            remote.op.as_str(),
            remote.session_id,
            snapshot.to_string(),
            remote.at,
            remote.origin_device,
            remote.clock.ms,
            remote.clock.counter,
            tick.seq,
            remote.origin_seq,
            Value::Object(remote.fields.clone()).to_string(),
            Value::Object(remote.prev.clone()).to_string(),
            remote
                .seen
                .map(|s| serde_json::to_string(s).unwrap_or_default()),
        ],
    )?;
    Ok(())
}

/// The device that made an entity: the origin of its earliest `create` record by clock, if this
/// device holds one.
pub fn creator(conn: &Connection, entity: &str, entity_id: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT origin_device FROM change_log
             WHERE entity = ?1 AND entity_id = ?2 AND op = 'create'
             ORDER BY hlc_ms, hlc_counter, origin_device, origin_seq LIMIT 1",
            [entity, entity_id],
            |r| r.get(0),
        )
        .optional()?)
}

/// One stored record, as read back.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// What happened.
    pub op: String,
    /// The editing session, if the change had one.
    pub session_id: Option<String>,
    /// Full state of the entity after the change.
    pub snapshot: Value,
    /// When it was recorded: UTC milliseconds. A session's record carries its latest edit.
    pub at: i64,
    /// What the author changed, with the author's values: the same on every device. `None` for
    /// a record from before `fields` existed, which stands for every field of its snapshot.
    pub fields: Option<Map<String, Value>>,
    /// Made on this device. Only an own record has to be completed (a group whole) the way it is
    /// sent; a received one already carries whole groups.
    pub local: bool,
}

/// Every record of one entity, oldest first by clock `(ms, counter, origin device)`: the same
/// order on every device. On one device that is the order the changes were made. A record whose snapshot cannot be read back is
/// skipped: the log must never stop a screen from loading.
pub fn read(conn: &Connection, entity: &str, entity_id: &str) -> AppResult<Vec<Record>> {
    let mut stmt = conn.prepare(
        "SELECT op, session_id, snapshot, at, fields,
                origin_device = (SELECT device_id FROM device_clock) FROM change_log
         WHERE entity = ?1 AND entity_id = ?2
         ORDER BY hlc_ms, hlc_counter, origin_device, origin_seq",
    )?;
    let rows = stmt.query_map([entity, entity_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<bool>>(5)?.unwrap_or(false),
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (op, session_id, snapshot, at, fields, local) = row?;
        if let Ok(snapshot) = serde_json::from_str(&snapshot) {
            out.push(Record {
                op,
                session_id,
                snapshot,
                at,
                fields: fields.and_then(|f| match serde_json::from_str(&f) {
                    Ok(Value::Object(map)) => Some(map),
                    _ => None,
                }),
                local,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::{self, migrate};
    use crate::module::{CoreModule, Module};

    fn migrated() -> Connection {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        migrate::run(&mut conn, &[&CoreModule], dir.path()).unwrap();
        conn
    }

    fn change(entity_id: &str, session: Option<&str>, snapshot: Value) -> Change {
        Change {
            module: "journal".into(),
            entity: "entry".into(),
            entity_id: entity_id.into(),
            op: ChangeOp::Update,
            session_id: session.map(str::to_owned),
            snapshot,
        }
    }

    fn snapshots(conn: &Connection, entity_id: &str) -> Vec<Value> {
        let mut stmt = conn
            .prepare("SELECT snapshot FROM change_log WHERE entity_id = ?1 ORDER BY id")
            .unwrap();
        stmt.query_map([entity_id], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|s| serde_json::from_str(&s.unwrap()).unwrap())
            .collect()
    }

    #[test]
    fn changes_without_a_session_append() {
        let conn = migrated();
        record(&conn, &change("e1", None, json!({"v": 1}))).unwrap();
        record(&conn, &change("e1", None, json!({"v": 2}))).unwrap();
        assert_eq!(
            snapshots(&conn, "e1"),
            vec![json!({"v": 1}), json!({"v": 2})]
        );
    }

    #[test]
    fn same_session_collapses_to_the_latest_snapshot_and_keeps_create() {
        let conn = migrated();
        let mut first = change("e1", Some("s1"), json!({"v": 1}));
        first.op = ChangeOp::Create;
        record(&conn, &first).unwrap();
        record(&conn, &change("e1", Some("s1"), json!({"v": 2}))).unwrap();
        record(&conn, &change("e1", Some("s1"), json!({"v": 3}))).unwrap();
        assert_eq!(snapshots(&conn, "e1"), vec![json!({"v": 3})]);
        let op: String = conn
            .query_row("SELECT op FROM change_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            op, "create",
            "an Entry created and edited in one session was created"
        );
    }

    #[test]
    fn different_sessions_and_entities_append() {
        let conn = migrated();
        record(&conn, &change("e1", Some("s1"), json!({"v": 1}))).unwrap();
        record(&conn, &change("e1", Some("s2"), json!({"v": 2}))).unwrap();
        record(&conn, &change("e2", Some("s1"), json!({"v": 3}))).unwrap();
        assert_eq!(snapshots(&conn, "e1").len(), 2);
        assert_eq!(snapshots(&conn, "e2").len(), 1);
    }

    #[test]
    fn read_returns_one_entity_oldest_first() {
        let conn = migrated();
        record(&conn, &change("e1", None, json!({"v": 1}))).unwrap();
        record(&conn, &change("e2", None, json!({"v": 9}))).unwrap();
        record(&conn, &change("e1", Some("s1"), json!({"v": 2}))).unwrap();
        let read = read(&conn, "entry", "e1").unwrap();
        let seen: Vec<_> = read.iter().map(|r| r.snapshot.clone()).collect();
        assert_eq!(seen, vec![json!({"v": 1}), json!({"v": 2})]);
        assert_eq!(read[1].session_id.as_deref(), Some("s1"));
        assert!(super::read(&conn, "entry", "nope").unwrap().is_empty());
    }

    fn seqs(conn: &Connection) -> Vec<i64> {
        let mut stmt = conn
            .prepare("SELECT seq FROM change_log ORDER BY seq")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn every_record_carries_this_device_a_clock_and_a_rising_sequence() {
        let conn = migrated();
        hlc::set_device_id(&conn, "dev-a").unwrap();
        let source: db::TimeSource = std::sync::Arc::new(|| 5_000);
        db::set_time_source(&conn, source).unwrap();
        record(&conn, &change("e1", None, json!({}))).unwrap();
        record(&conn, &change("e2", None, json!({}))).unwrap();
        let rows: Vec<(String, i64, i64, i64, i64)> = conn
            .prepare("SELECT origin_device, hlc_ms, hlc_counter, seq, origin_seq FROM change_log ORDER BY seq")
            .unwrap()
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            rows,
            vec![
                ("dev-a".to_owned(), 5_000, 0, 1, 1),
                ("dev-a".to_owned(), 5_000, 1, 2, 2)
            ],
            "a standing wall clock is told apart by the counter"
        );
    }

    #[test]
    fn rewriting_a_sessions_record_gives_it_a_new_higher_sequence_and_clock() {
        let conn = migrated();
        record(&conn, &change("e1", Some("s1"), json!({"v": 1}))).unwrap();
        record(&conn, &change("e2", None, json!({"v": 9}))).unwrap();
        let before = seqs(&conn);
        record(&conn, &change("e1", Some("s1"), json!({"v": 2}))).unwrap();
        let after = seqs(&conn);
        assert_eq!(before, vec![1, 2]);
        assert_eq!(
            after,
            vec![2, 3],
            "one row rewritten: its old sequence is gone"
        );
        let rewritten: i64 = conn
            .query_row(
                "SELECT seq FROM change_log WHERE entity_id = 'e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rewritten, 3);
    }

    #[test]
    fn read_orders_by_clock_then_device_not_by_arrival() {
        let conn = migrated();
        hlc::set_device_id(&conn, "dev-a").unwrap();
        record(&conn, &change("e1", None, json!({"v": "local"}))).unwrap();
        // As if received: an earlier clock arriving later, and a tie decided by device id.
        let received = |id: &str, device: &str, ms: i64, counter: i64, seq: i64, v: &str| {
            conn.execute(
                "INSERT INTO change_log(id, module, entity, entity_id, op, snapshot, at,
                     origin_device, hlc_ms, hlc_counter, seq, origin_seq)
                 VALUES (?1, 'journal', 'entry', 'e1', 'update', ?2, 1, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    id,
                    json!({"v": v}).to_string(),
                    device,
                    ms,
                    counter,
                    seq + 100,
                    seq
                ],
            )
            .unwrap();
        };
        let local_ms: i64 = conn
            .query_row("SELECT hlc_ms FROM change_log", [], |r| r.get(0))
            .unwrap();
        received("r1", "dev-b", local_ms - 10, 0, 1, "earlier");
        received("r2", "dev-c", local_ms, 0, 2, "tie-c");
        received("r3", "dev-0", local_ms, 0, 3, "tie-0");
        let seen: Vec<Value> = read(&conn, "entry", "e1")
            .unwrap()
            .into_iter()
            .map(|r| r.snapshot["v"].clone())
            .collect();
        assert_eq!(
            seen,
            vec![
                json!("earlier"),
                json!("tie-0"),
                json!("local"),
                json!("tie-c")
            ],
            "the local record has counter >= 0 on the same ms, so ties on (ms, counter 0) go by device id"
        );
    }

    #[test]
    fn upgrade_backfills_existing_rows_in_the_order_history_always_showed() {
        struct Old;
        impl Module for Old {
            fn id(&self) -> &'static str {
                "core"
            }
            fn migrations(&self) -> rusqlite_migration::Migrations<'static> {
                rusqlite_migration::Migrations::new(vec![
                    rusqlite_migration::M::up(
                        "CREATE TABLE settings(
                         module TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL,
                         updated_at INTEGER NOT NULL, PRIMARY KEY(module, key)
                     ) STRICT",
                    ),
                    rusqlite_migration::M::up(
                        "CREATE TABLE change_log(
                         id TEXT PRIMARY KEY NOT NULL, module TEXT NOT NULL, entity TEXT NOT NULL,
                         entity_id TEXT NOT NULL, op TEXT NOT NULL, session_id TEXT,
                         snapshot TEXT NOT NULL, at INTEGER NOT NULL
                     ) STRICT;
                     CREATE UNIQUE INDEX change_log_session
                         ON change_log(entity, entity_id, session_id) WHERE session_id IS NOT NULL;",
                    ),
                ])
            }
            fn register_settings(&self, _: &mut crate::settings::SettingsRegistry) {}
        }
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        migrate::run(&mut conn, &[&Old], dir.path()).unwrap();
        // Inserted out of time order; ids are not in time order either.
        for (id, at, v) in [
            ("z", 300, "third"),
            ("y", 100, "first"),
            ("x", 200, "second"),
        ] {
            conn.execute(
                "INSERT INTO change_log(id, module, entity, entity_id, op, snapshot, at)
                 VALUES (?1, 'journal', 'entry', 'e1', 'update', ?2, ?3)",
                rusqlite::params![id, json!({"v": v}).to_string(), at],
            )
            .unwrap();
        }
        migrate::run(&mut conn, &[&CoreModule], dir.path()).unwrap();
        hlc::set_device_id(&conn, "dev-a").unwrap();
        let seen: Vec<_> = read(&conn, "entry", "e1")
            .unwrap()
            .into_iter()
            .map(|r| r.snapshot["v"].clone())
            .collect();
        assert_eq!(seen, vec![json!("first"), json!("second"), json!("third")]);
        let (device, ms, counter, seq): (String, i64, i64, i64) = conn
            .query_row(
                "SELECT origin_device, hlc_ms, hlc_counter, seq FROM change_log WHERE id = 'x'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!((device.as_str(), ms, counter, seq), ("dev-a", 200, 0, 2));
        // New records continue after the backfill, on a clock past the old times.
        record(&conn, &change("e1", None, json!({"v": "fourth"}))).unwrap();
        assert_eq!(seqs(&conn), vec![1, 2, 3, 4]);
        let last = read(&conn, "entry", "e1").unwrap().pop().unwrap();
        assert_eq!(last.snapshot["v"], json!("fourth"));
    }

    #[test]
    fn a_clock_shown_a_later_remote_time_stamps_the_next_record_after_it() {
        let conn = migrated();
        let source: db::TimeSource = std::sync::Arc::new(|| 1_000);
        db::set_time_source(&conn, source).unwrap();
        hlc::observe(
            &conn,
            hlc::Timestamp {
                ms: 9_000,
                counter: 4,
            },
        )
        .unwrap();
        record(&conn, &change("e1", None, json!({}))).unwrap();
        let (ms, counter): (i64, i64) = conn
            .query_row("SELECT hlc_ms, hlc_counter FROM change_log", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert!((ms, counter) > (9_000, 4));
    }

    fn stored_fields(conn: &Connection) -> Vec<Value> {
        let mut stmt = conn
            .prepare("SELECT fields FROM change_log ORDER BY seq")
            .unwrap();
        stmt.query_map([], |r| r.get::<_, Option<String>>(0))
            .unwrap()
            .map(|f| {
                f.unwrap()
                    .map_or(Value::Null, |s| serde_json::from_str(&s).unwrap())
            })
            .collect()
    }

    #[test]
    fn a_record_keeps_only_the_fields_it_changed() {
        let conn = migrated();
        record(&conn, &change("e1", None, json!({"a": 1, "b": 2}))).unwrap();
        record(&conn, &change("e1", None, json!({"a": 1, "b": 3}))).unwrap();
        record(&conn, &change("e1", None, json!({"a": 1, "b": 3, "c": 4}))).unwrap();
        record(&conn, &change("e2", None, json!({"a": 9}))).unwrap();
        assert_eq!(
            stored_fields(&conn),
            vec![
                json!({"a": 1, "b": 2}),
                json!({"b": 3}),
                json!({"c": 4}),
                json!({"a": 9})
            ]
        );
    }

    #[test]
    fn a_sessions_rewrite_keeps_what_the_session_changed_before() {
        let conn = migrated();
        record(&conn, &change("e1", None, json!({"a": 1, "b": 1, "c": 1}))).unwrap();
        record(
            &conn,
            &change("e1", Some("s"), json!({"a": 2, "b": 1, "c": 1})),
        )
        .unwrap();
        // Another change lands between, then the session goes on.
        record(&conn, &change("e1", None, json!({"a": 2, "b": 5, "c": 1}))).unwrap();
        record(
            &conn,
            &change("e1", Some("s"), json!({"a": 2, "b": 5, "c": 7})),
        )
        .unwrap();
        assert_eq!(
            stored_fields(&conn),
            vec![
                json!({"a": 1, "b": 1, "c": 1}),
                json!({"b": 5}),
                json!({"a": 2, "c": 7}),
            ],
            "the session still says it changed a, and not b"
        );
    }

    #[test]
    fn a_session_does_not_keep_claiming_a_value_that_changed_since_and_it_did_not_touch() {
        let conn = migrated();
        record(&conn, &change("e1", None, json!({"a": 1, "b": 1}))).unwrap();
        record(&conn, &change("e1", Some("s"), json!({"a": 2, "b": 1}))).unwrap();
        // Another change takes `a` on, then the session saves again without touching it.
        record(&conn, &change("e1", None, json!({"a": 9, "b": 1}))).unwrap();
        record(&conn, &change("e1", Some("s"), json!({"a": 9, "b": 2}))).unwrap();
        assert_eq!(
            stored_fields(&conn),
            vec![json!({"a": 1, "b": 1}), json!({"a": 9}), json!({"b": 2})],
            "the session now claims b only"
        );
    }

    fn stored_prev(conn: &Connection) -> Vec<Value> {
        let mut stmt = conn
            .prepare("SELECT prev FROM change_log ORDER BY seq")
            .unwrap();
        stmt.query_map([], |r| r.get::<_, Option<String>>(0))
            .unwrap()
            .map(|f| {
                f.unwrap()
                    .map_or(Value::Null, |s| serde_json::from_str(&s).unwrap())
            })
            .collect()
    }

    #[test]
    fn a_record_keeps_what_each_changed_field_held_before() {
        let conn = migrated();
        record(&conn, &change("e1", None, json!({"a": "one", "b": 1}))).unwrap();
        record(&conn, &change("e1", None, json!({"a": "two", "b": 1}))).unwrap();
        record(
            &conn,
            &change("e1", None, json!({"a": "two", "b": 2, "c": "new"})),
        )
        .unwrap();
        assert_eq!(
            stored_prev(&conn),
            vec![json!({}), json!({"a": "one"}), json!({"b": 1, "c": null})],
            "a first record replaced nothing; a key that did not exist held null"
        );
    }

    #[test]
    fn a_sessions_rewrite_keeps_the_text_the_session_started_from() {
        let conn = migrated();
        record(&conn, &change("e1", None, json!({"a": "base", "b": 1}))).unwrap();
        record(
            &conn,
            &change("e1", Some("s"), json!({"a": "typed", "b": 1})),
        )
        .unwrap();
        record(
            &conn,
            &change("e1", Some("s"), json!({"a": "typed more", "b": 2})),
        )
        .unwrap();
        assert_eq!(
            stored_prev(&conn),
            vec![json!({}), json!({"a": "base", "b": 1})],
            "a is still measured from the text the session began with"
        );
        // A session that made the record never knew what it replaced.
        record(&conn, &change("e2", Some("n"), json!({"a": "first"}))).unwrap();
        record(&conn, &change("e2", Some("n"), json!({"a": "first, more"}))).unwrap();
        assert_eq!(stored_prev(&conn)[2], json!({}));
    }

    #[test]
    fn a_session_that_overwrites_a_value_changed_since_says_it_replaced_that_value() {
        let conn = migrated();
        record(&conn, &change("e1", None, json!({"a": "base"}))).unwrap();
        record(&conn, &change("e1", Some("s"), json!({"a": "typed"}))).unwrap();
        // Another device's text lands while the session is open and nothing holds it back.
        record(&conn, &change("e1", None, json!({"a": "theirs"}))).unwrap();
        record(&conn, &change("e1", Some("s"), json!({"a": "typed more"}))).unwrap();
        assert_eq!(
            stored_prev(&conn),
            vec![json!({}), json!({"a": "typed"}), json!({"a": "theirs"})],
            "the session's rewrite replaced \"theirs\", what its author held at that moment"
        );
    }

    #[test]
    fn a_session_record_from_before_fields_stays_everything() {
        let conn = migrated();
        record(&conn, &change("e1", Some("s"), json!({"a": 1}))).unwrap();
        conn.execute("UPDATE change_log SET fields = NULL", [])
            .unwrap();
        record(&conn, &change("e1", Some("s"), json!({"a": 2}))).unwrap();
        assert_eq!(stored_fields(&conn), vec![Value::Null]);
    }

    fn remote<'a>(
        fields: &'a Map<String, Value>,
        seq: i64,
        session: Option<&'a str>,
    ) -> Remote<'a> {
        Remote {
            module: "journal",
            entity: "entry",
            entity_id: "e1",
            op: ChangeOp::Update,
            session_id: session,
            at: 7,
            origin_device: "dev-b",
            origin_seq: seq,
            clock: hlc::Timestamp {
                ms: 9_000,
                counter: 2,
            },
            fields,
            prev: fields,
            seen: None,
        }
    }

    #[test]
    fn a_received_record_keeps_its_origin_and_gets_a_fresh_local_sequence() {
        let conn = migrated();
        hlc::set_device_id(&conn, "dev-a").unwrap();
        record(&conn, &change("e0", None, json!({}))).unwrap();
        let fields = json!({"a": 1}).as_object().unwrap().clone();
        record_remote(&conn, &remote(&fields, 40, None), &json!({"a": 1, "z": 0})).unwrap();
        let row: (String, i64, i64, i64, i64, i64, String) = conn
            .query_row(
                "SELECT origin_device, origin_seq, hlc_ms, hlc_counter, seq, at, fields
                 FROM change_log WHERE entity_id = 'e1'",
                [],
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
            .unwrap();
        assert_eq!(
            row,
            (
                "dev-b".into(),
                40,
                9_000,
                2,
                2,
                7,
                json!({"a": 1}).to_string()
            ),
            "seq 1 was the local record"
        );
        // The clock moved past it: the next local record sorts after.
        record(&conn, &change("e1", None, json!({"a": 2}))).unwrap();
        let (ms, counter): (i64, i64) = conn
            .query_row(
                "SELECT hlc_ms, hlc_counter FROM change_log WHERE origin_device = 'dev-a' ORDER BY seq DESC",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!((ms, counter) > (9_000, 2));
    }

    #[test]
    fn a_received_record_of_a_held_session_replaces_it() {
        let conn = migrated();
        let first = json!({"a": 1}).as_object().unwrap().clone();
        let second = json!({"a": 2, "b": 3}).as_object().unwrap().clone();
        record_remote(&conn, &remote(&first, 5, Some("s")), &json!({"a": 1})).unwrap();
        record_remote(
            &conn,
            &remote(&second, 9, Some("s")),
            &json!({"a": 2, "b": 3}),
        )
        .unwrap();
        let rows: Vec<(i64, String)> = conn
            .prepare("SELECT origin_seq, fields FROM change_log")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(rows, vec![(9, json!({"a": 2, "b": 3}).to_string())]);
    }

    #[test]
    fn upgrading_an_existing_database_keeps_settings() {
        struct CoreV1;
        impl Module for CoreV1 {
            fn id(&self) -> &'static str {
                "core"
            }
            fn migrations(&self) -> rusqlite_migration::Migrations<'static> {
                rusqlite_migration::Migrations::new(vec![rusqlite_migration::M::up(
                    "CREATE TABLE settings(
                         module TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL,
                         updated_at INTEGER NOT NULL, PRIMARY KEY(module, key)
                     ) STRICT",
                )])
            }
            fn register_settings(&self, _: &mut crate::settings::SettingsRegistry) {}
        }

        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        migrate::run(&mut conn, &[&CoreV1], dir.path()).unwrap();
        conn.execute(
            "INSERT INTO settings(module, key, value, updated_at) VALUES ('core', 'theme', '\"dark\"', 1)",
            [],
        )
        .unwrap();

        let report = migrate::run(&mut conn, &[&CoreModule], dir.path()).unwrap();
        assert_eq!(report.applied, vec![("core".into(), 1, 7)]);
        assert!(report.backup.is_some(), "existing data is backed up first");
        let theme: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'theme'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(theme, "\"dark\"");
        record(&conn, &change("e1", None, json!({}))).unwrap();
    }
}
