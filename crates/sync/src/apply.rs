//! What this device does with a batch it receives: every record in one transaction that also
//! advances the version vector, so an interrupted exchange resumes exactly. Per record: the
//! clock is shown the sender's time, each unit of fields is compared with the clock held for
//! it, the Module writes the winners through its own repository, and the record is stored in
//! the change log as sent (origin device, origin sequence, clock) with a fresh local sequence.
//! Never log field values.

use std::collections::BTreeSet;

use mnemora_core::changelog::{self, ChangeOp, Remote};
use mnemora_core::db::physical_now;
use mnemora_core::editing;
use mnemora_core::hlc::{Stamp, Timestamp};
use mnemora_core::settings;
use mnemora_core::syncable::{FieldClass, RemoteChange, SyncEntity, SyncEvent};
use mnemora_core::util::now_ms;
use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde_json::{Map, Value};

use crate::activity;
use crate::clocks;
use crate::conflicts;
use crate::error::SyncError;
use crate::held::{self, HeldUnit};
use crate::merge::{Verdict, decide};
use crate::pause::MAX_AHEAD_MS;
use crate::registry::Registry;
use crate::resolutions::{self, Folded, Unit};
use crate::texts;
use crate::vector;
use crate::wire::WireRecord;

/// What a batch did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// Records stored (duplicates and own records are not counted).
    pub stored: usize,
    /// Modules whose data visibly changed (Preferences are reported as events instead).
    pub changed: BTreeSet<String>,
    /// What the Modules report about the records they took: the hook where the Activity log and
    /// Resolved automatically will record List merges, Tasks put in the Inbox and the like.
    pub events: Vec<SyncEvent>,
    /// Order keys and Preferences the batch took, for the session's "Order & preferences" line.
    pub folded: Vec<Folded>,
    /// The Module and entity of every record stored, for the session's Activity breakdown.
    pub moved: Vec<(String, String)>,
}

/// Applies `records` atomically: all of them, with the vector advanced, or none.
#[allow(clippy::too_many_lines)] // one pass over a batch: reads best in order
pub fn apply_batch(
    conn: &Connection,
    registry: &Registry,
    me: &str,
    mut records: Vec<WireRecord>,
) -> Result<Applied, SyncError> {
    // Stream order, whatever order they arrived in: a record must not be skipped as "already
    // seen" because a later one of its origin was applied first.
    records.sort_by(|a, b| {
        (a.hlc_ms, a.hlc_counter, &a.origin_device, a.origin_seq).cmp(&(
            b.hlc_ms,
            b.hlc_counter,
            &b.origin_device,
            b.origin_seq,
        ))
    });
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    // Nothing from further ahead than five minutes is stored or shown to the clock: it would win
    // every merge until real time caught up. The exchange stops (the transaction rolls back).
    let latest = physical_now(&tx).saturating_add(MAX_AHEAD_MS);
    let mut applied = Applied::default();
    let mut folded: BTreeSet<&str> = BTreeSet::new();
    let mut texts_to_settle: BTreeSet<(String, String, String)> = BTreeSet::new();
    // Editors that went away without letting go (their lease lapsed) hold nothing back now.
    let released = held::release_ready(&tx, registry, me, now_ms())?;
    applied.changed.extend(released.changed);
    applied.events.extend(released.events);
    for record in &records {
        let (module, schema) = registry
            .find(&record.entity)
            .ok_or(SyncError::UnknownEntity)?;
        // A Preference belongs to the Module that registered its key, whatever Module owns the
        // entity.
        if module.id() != record.module && record.entity != settings::ENTITY {
            return Err(SyncError::Protocol("record names the wrong Module".into()));
        }
        let op = ChangeOp::parse(&record.op)
            .ok_or_else(|| SyncError::Protocol("record has an unknown operation".into()))?;
        // Ours are never sent back, and what the vector covers is already held.
        if record.origin_device == me
            || record.origin_seq <= vector::seen(&tx, &record.entity, &record.origin_device)?
        {
            continue;
        }
        if record.hlc_ms > latest {
            // Sorted, so the last record is the newest: sync can resume once that one is allowed.
            let newest = records.last().map_or(record.hlc_ms, |r| r.hlc_ms);
            return Err(SyncError::ClockAhead {
                resumes_in_ms: newest - latest,
            });
        }
        if folded.insert(&record.entity) {
            clocks::fold(&tx, schema, me)?;
        }
        let decided = decide_fields(&tx, schema, record, op)?;
        let change = RemoteChange {
            entity: &record.entity,
            entity_id: &record.entity_id,
            op,
            at: record.at,
            fields: &decided.winners,
            settings: registry.settings(),
        };
        let outcome = module.apply_remote(&tx, &change)?;
        changelog::record_remote(
            &tx,
            &Remote {
                module: &record.module,
                entity: &record.entity,
                entity_id: &record.entity_id,
                op,
                session_id: record.session_id.as_deref(),
                at: record.at,
                origin_device: &record.origin_device,
                origin_seq: record.origin_seq,
                clock: Timestamp {
                    ms: record.hlc_ms,
                    counter: record.hlc_counter,
                },
                fields: &record.fields,
                prev: &record.prev,
            },
            &outcome.snapshot,
        )?;
        vector::advance(
            &tx,
            &record.entity,
            &record.origin_device,
            record.origin_seq,
        )?;
        held::stash(
            &tx,
            &record.entity,
            &record.entity_id,
            &Stamp {
                ms: record.hlc_ms,
                counter: record.hlc_counter,
                device: record.origin_device.clone(),
            },
            op,
            record.at,
            &decided.kept_back,
        )?;
        applied.stored += 1;
        applied
            .moved
            .push((record.module.clone(), record.entity.clone()));
        let first_event = applied.events.len();
        applied
            .events
            .extend(outcome.events.into_iter().map(|event| match event {
                // The Module cannot know who made the record.
                SyncEvent::TaskDeleted { task_id, title, .. } => SyncEvent::TaskDeleted {
                    task_id,
                    title,
                    device: Some(record.origin_device.clone()),
                },
                other => other,
            }));
        if outcome.changed && record.entity != settings::ENTITY {
            applied.changed.insert(record.module.clone());
        }
        // After the record is stored, so what the Module records for itself is stamped later
        // than the record and wins over it on every device.
        let settled = module.settle_remote(&tx, &change)?;
        if settled.recorded {
            // Its records raise field clocks too: read them in before the next comparison.
            folded.clear();
        }
        if settled.recorded || !settled.events.is_empty() {
            applied.changed.insert(module.id().to_owned());
        }
        applied.events.extend(settled.events);
        // Logged after the record is stored too: a clash is read off the stored records.
        for key in &decided.value_units {
            if let Some(members) = resolutions::value_members(schema, key) {
                resolutions::settle_value(
                    &tx,
                    me,
                    &record.entity,
                    &record.entity_id,
                    key,
                    &members,
                )?;
            }
        }
        applied.folded.extend(decided.folded);
        for event in &applied.events[first_event..] {
            if let SyncEvent::TaskToInbox { task_id, list_id } = event {
                resolutions::note_task_inbox(
                    &tx,
                    task_id,
                    list_id,
                    &record.origin_device,
                    record.hlc_ms,
                )?;
            }
        }
        // After the record is stored, so what is recorded now is stamped later than it.
        applied
            .events
            .extend(after_record(&tx, registry, me, record, &outcome.snapshot)?);
        for key in decided.text_units {
            texts_to_settle.insert((record.entity.clone(), record.entity_id.clone(), key));
        }
    }
    // Text last, once every record of the batch is stored: the text of a record is the replay of
    // all of them, so it is worked out once with all of them in hand (and a clash is only
    // raised for what is still a clash then).
    for (entity, entity_id, key) in &texts_to_settle {
        let settled = texts::settle_unit(&tx, registry, me, entity, entity_id, key)?;
        applied.changed.extend(settled.changed);
        applied.events.extend(settled.events);
    }
    // And what this device itself saved since: its text is the replay of the records too.
    let local = texts::settle_local(&tx, registry, me, now_ms())?;
    applied.changed.extend(local.changed);
    applied.events.extend(local.events);
    // A delete made here since may have replaced the one a line was made for.
    resolutions::settle_local(&tx, registry, me)?;
    resolutions::refresh_delete_edits(&tx, registry)?;
    activity::note_events(&tx, &applied.events)?;
    tx.commit()?;
    Ok(applied)
}

/// What `decide_fields` made of a record.
#[derive(Default)]
struct Decided {
    /// The values to write now, whole units at a time.
    winners: Map<String, Value>,
    /// Units an editor holds back until it lets go.
    kept_back: Vec<HeldUnit>,
    /// Text units the record touched that nothing holds back: settled once the batch is stored.
    text_units: Vec<String>,
    /// Value units the record touched: their clashes are read off the stored records.
    value_units: Vec<String>,
    /// Order keys and Preferences that moved, for the session's folded line.
    folded: Vec<Folded>,
}

/// The values of `record` that win against the clocks held, whole groups at a time, and the
/// winning units an editor holds back (kept apart until it lets go, see `held`). The clocks of
/// the winners move to the record's either way. A record that creates its entity is never
/// held: nothing can be editing what does not exist yet.
///
/// Text is not decided by the clock: the text of a record is the replay of every record that
/// touched it, merged three-way where two devices edited from different texts (`texts`). That
/// happens after the batch is stored, so a text unit contributes nothing here, except that the
/// record which creates its entity brings its own text (nothing can be made without it).
fn decide_fields(
    conn: &Connection,
    schema: &SyncEntity,
    record: &WireRecord,
    op: ChangeOp,
) -> Result<Decided, SyncError> {
    let scopes = if op == ChangeOp::Create {
        Vec::new()
    } else {
        editing::held(conn, &record.entity, &record.entity_id, now_ms())?
    };
    let stamp = Stamp {
        ms: record.hlc_ms,
        counter: record.hlc_counter,
        device: record.origin_device.clone(),
    };
    let held = clocks::load(conn, &record.entity, &record.entity_id)?;
    // What stands now, for telling a change made beside another from one made after it. Sync's
    // own records are no business of the log, and a record that creates has nothing to clash with.
    let current = if op == ChangeOp::Create || record.entity.starts_with("sync_") {
        None
    } else {
        // A Preference never set here has no state yet: what the other device sets is a change.
        texts::current(conn, &record.entity, &record.entity_id)?
            .or_else(|| (record.entity == settings::ENTITY).then(|| Value::Object(Map::new())))
    };
    // Units: each ungrouped field alone, each group as one.
    let mut units: Vec<(&str, Vec<&str>, FieldClass)> = Vec::new();
    for field in schema.fields {
        if field.class == FieldClass::Derived || !record.fields.contains_key(field.name) {
            continue;
        }
        let key = field.group.unwrap_or(field.name);
        match units.iter_mut().find(|u| u.0 == key) {
            Some(unit) => {
                unit.1.push(field.name);
                // Any text in a unit makes it a text unit.
                if field.class == FieldClass::Text {
                    unit.2 = FieldClass::Text;
                }
            }
            None => units.push((key, vec![field.name], field.class)),
        }
    }
    let mut out = Decided::default();
    for (key, members, class) in units {
        let is_text = class == FieldClass::Text;
        let newest_held = members.iter().filter_map(|m| held.get(*m)).max();
        if !is_text {
            let take = decide(class, newest_held, &stamp) == Verdict::Take;
            if class == FieldClass::Value {
                out.value_units.push(key.to_owned());
            }
            if let Some(current) = &current {
                let unit = Unit {
                    key,
                    members: &members,
                    class,
                };
                out.folded
                    .extend(resolutions::observe(record, &unit, take, current));
            }
            if !take {
                continue;
            }
        }
        let mut values = Map::new();
        for member in members {
            if let Some(value) = record.fields.get(member) {
                values.insert(member.to_owned(), value.clone());
                clocks::raise(conn, &record.entity, &record.entity_id, member, &stamp)?;
            }
        }
        if editing::is_held(&scopes, key) {
            out.kept_back.push(HeldUnit {
                unit: key.to_owned(),
                fields: values,
            });
            continue;
        }
        if is_text {
            out.text_units.push(key.to_owned());
            if op != ChangeOp::Create {
                continue;
            }
        }
        out.winners.extend(values);
    }
    Ok(out)
}

/// What follows a stored record: Conflicts on it are settled if it is deleted; a Conflict
/// received about a record is checked against the replay of that record's text, and ended if the
/// record is deleted.
fn after_record(
    conn: &Connection,
    registry: &Registry,
    me: &str,
    record: &WireRecord,
    snapshot: &Value,
) -> Result<Vec<SyncEvent>, SyncError> {
    if record.entity == conflicts::ENTITY {
        let Some(row) = conflicts::get(conn, &record.entity_id)? else {
            return Ok(Vec::new());
        };
        if !row.open {
            return Ok(Vec::new());
        }
        let mut events = conflicts::settle_deleted(conn, &row.target, &row.target_id)?;
        // A Conflict made from fewer records than this device holds may not be one any more.
        if let Some((_, schema)) = registry.find(&row.target)
            && let Some(unit) = schema.field(&row.field).map(|f| f.group.unwrap_or(f.name))
        {
            let settled =
                texts::settle_unit(conn, registry, me, &row.target, &row.target_id, unit)?;
            events.extend(settled.events);
        }
        return Ok(events);
    }
    // A delete met by an edit stays on record when the entity is restored later: judged from the
    // records, whatever the entity looks like now (a device that held the delete back, or got
    // the restore with the edit, must make the same line).
    if let Some((_, schema)) = registry.find(&record.entity) {
        resolutions::note_delete_edit(conn, schema, &record.entity_id)?;
    }
    if snapshot.get("deleted_at").is_some_and(|d| !d.is_null()) {
        return conflicts::settle_deleted(conn, &record.entity, &record.entity_id);
    }
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mnemora_core::db::{self, migrate};
    use mnemora_core::error::{AppError, AppResult, ErrorCode};
    use mnemora_core::hlc;
    use mnemora_core::module::{CoreModule, Module};
    use mnemora_core::settings::SettingsRegistry;
    use mnemora_core::syncable::{Applied as ModuleApplied, SyncField};
    use rusqlite_migration::{M, Migrations};
    use serde_json::json;

    use super::*;
    use crate::SyncModule;
    use crate::registry::Modules;

    /// A Module with one syncable entity whose text "boom" its repository refuses.
    struct Fake;

    const ENTITIES: &[SyncEntity] = &[SyncEntity {
        name: "thing",
        fields: &[SyncField::text("name"), SyncField::value("n")],
    }];

    impl Module for Fake {
        fn id(&self) -> &'static str {
            "fake"
        }
        fn migrations(&self) -> Migrations<'static> {
            Migrations::new(vec![M::up(
                "CREATE TABLE fake_things(id TEXT PRIMARY KEY NOT NULL, name TEXT, n INTEGER) STRICT",
            )])
        }
        fn register_settings(&self, _: &mut SettingsRegistry) {}
        fn sync_entities(&self) -> &'static [SyncEntity] {
            ENTITIES
        }
        fn apply_remote(
            &self,
            conn: &Connection,
            change: &RemoteChange<'_>,
        ) -> AppResult<ModuleApplied> {
            if change.fields.get("name") == Some(&json!("boom")) {
                return Err(AppError::new(ErrorCode::Validation, "refused"));
            }
            conn.execute(
                "INSERT INTO fake_things(id, name, n) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET name = COALESCE(?2, name), n = COALESCE(?3, n)",
                rusqlite::params![
                    change.entity_id,
                    change.fields.get("name").and_then(Value::as_str),
                    change.fields.get("n").and_then(Value::as_i64),
                ],
            )?;
            Ok(ModuleApplied {
                snapshot: json!({"id": change.entity_id, "name": change.fields.get("name")}),
                changed: true,
                events: Vec::new(),
            })
        }
    }

    fn device(id: &str) -> (Connection, Registry) {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        migrate::run(&mut conn, &[&CoreModule, &SyncModule, &Fake], dir.path()).unwrap();
        hlc::set_device_id(&conn, id).unwrap();
        let modules: Modules = Arc::new(vec![Box::new(Fake), Box::new(SyncModule)]);
        (conn, Registry::new(modules))
    }

    fn record(device: &str, seq: i64, id: &str, fields: Value) -> WireRecord {
        WireRecord {
            module: "fake".into(),
            entity: "thing".into(),
            entity_id: id.into(),
            op: "update".into(),
            session_id: None,
            at: 1,
            origin_device: device.into(),
            origin_seq: seq,
            hlc_ms: 1_000 + seq,
            hlc_counter: 0,
            fields: fields.as_object().unwrap().clone(),
            prev: Map::new(),
        }
    }

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn a_batch_is_applied_whole_or_not_at_all_and_the_vector_moves_with_it() {
        let (conn, registry) = device("me");
        let batch = vec![
            record("other", 1, "a", json!({"name": "one"})),
            record("other", 2, "b", json!({"name": "two"})),
            record("other", 3, "c", json!({"name": "boom"})),
        ];
        assert!(apply_batch(&conn, &registry, "me", batch.clone()).is_err());
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM fake_things"), 0);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM change_log"), 0);
        assert_eq!(
            count(&conn, "SELECT COUNT(*) FROM sync_seen"),
            0,
            "the vector did not move"
        );
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM sync_field_clocks"), 0);

        // Without the bad record the same batch goes in, and the vector says how far.
        let applied = apply_batch(&conn, &registry, "me", batch[..2].to_vec()).unwrap();
        assert_eq!(applied.stored, 2);
        assert_eq!(vector::seen(&conn, "thing", "other").unwrap(), 2);
    }

    #[test]
    fn delivering_a_batch_twice_changes_nothing_the_second_time() {
        let (conn, registry) = device("me");
        let batch = vec![
            record("other", 1, "a", json!({"name": "one", "n": 1})),
            record("other", 2, "a", json!({"n": 2})),
        ];
        assert_eq!(
            apply_batch(&conn, &registry, "me", batch.clone())
                .unwrap()
                .stored,
            2
        );
        assert_eq!(
            apply_batch(&conn, &registry, "me", batch).unwrap().stored,
            0
        );
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM change_log"), 2);
        assert_eq!(count(&conn, "SELECT n FROM fake_things"), 2);
    }

    #[test]
    fn records_delivered_out_of_order_are_applied_in_stream_order_and_none_is_skipped() {
        let (conn, registry) = device("me");
        let batch = vec![
            record("other", 3, "a", json!({"n": 3})),
            record("other", 1, "a", json!({"name": "one", "n": 1})),
            record("other", 2, "a", json!({"n": 2})),
        ];
        assert_eq!(
            apply_batch(&conn, &registry, "me", batch).unwrap().stored,
            3
        );
        assert_eq!(count(&conn, "SELECT n FROM fake_things"), 3);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM change_log"), 3);
    }

    #[test]
    fn a_stale_record_arriving_late_does_not_override_a_newer_value() {
        let (conn, registry) = device("me");
        apply_batch(
            &conn,
            &registry,
            "me",
            vec![record("b", 9, "a", json!({"n": 9}))],
        )
        .unwrap();
        // A's record is older on the clock, but this device had not seen it.
        apply_batch(
            &conn,
            &registry,
            "me",
            vec![record("a", 1, "a", json!({"n": 1}))],
        )
        .unwrap();
        assert_eq!(count(&conn, "SELECT n FROM fake_things"), 9);
        assert_eq!(
            count(&conn, "SELECT COUNT(*) FROM change_log"),
            2,
            "still held, for relaying"
        );
    }

    #[test]
    fn a_record_of_an_unknown_kind_or_from_this_device_is_refused_or_skipped() {
        let (conn, registry) = device("me");
        let mut odd = record("other", 1, "a", json!({}));
        odd.entity = "mystery".into();
        assert!(matches!(
            apply_batch(&conn, &registry, "me", vec![odd]),
            Err(SyncError::UnknownEntity)
        ));
        let own = record("me", 1, "a", json!({"n": 1}));
        assert_eq!(
            apply_batch(&conn, &registry, "me", vec![own])
                .unwrap()
                .stored,
            0
        );
    }

    #[test]
    fn settling_a_text_with_a_long_history_stays_fast() {
        let (conn, registry) = device("me");
        // A body of about 4 KB, rewritten once per editing session, 2,000 times over.
        let line = "a line of the journal that is long enough to look like prose, ".repeat(2);
        let text = |n: usize| format!("{}\nrevision {n}", line.repeat(32));
        let history: Vec<WireRecord> = (1..=2000usize)
            .map(|n| {
                let seq = i64::try_from(n).unwrap();
                let mut r = record("other", seq, "a", json!({ "name": text(n) }));
                if n > 1 {
                    r.prev = json!({ "name": text(n - 1) }).as_object().unwrap().clone();
                }
                r
            })
            .collect();
        // Arriving in batches, as a session sends them.
        for batch in history.chunks(100) {
            apply_batch(&conn, &registry, "me", batch.to_vec()).unwrap();
        }

        let mut next = record("other", 2001, "a", json!({ "name": text(2001) }));
        next.prev = json!({ "name": text(2000) }).as_object().unwrap().clone();
        let started = std::time::Instant::now();
        apply_batch(&conn, &registry, "me", vec![next]).unwrap();
        let took = started.elapsed();
        assert!(
            took < std::time::Duration::from_millis(50),
            "settling one more record took {took:?}"
        );
    }

    /// A record of `name` from `device`, clocked at `ms`, made from `prev`.
    fn text_record(
        device: &str,
        seq: i64,
        ms: i64,
        session: Option<&str>,
        prev: Option<&str>,
        name: &str,
    ) -> WireRecord {
        let mut r = record(device, seq, "a", json!({ "name": name }));
        r.hlc_ms = ms;
        r.session_id = session.map(str::to_owned);
        r.op = if prev.is_none() { "create" } else { "update" }.into();
        if let Some(p) = prev {
            r.prev = json!({ "name": p }).as_object().unwrap().clone();
        }
        r
    }

    fn name_and_conflicts(conn: &Connection) -> (String, Vec<String>) {
        let name = conn
            .query_row("SELECT name FROM fake_things WHERE id = 'a'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let mut stmt = conn
            .prepare("SELECT id FROM sync_conflicts ORDER BY id")
            .unwrap();
        let ids = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        (name, ids)
    }

    #[test]
    fn the_text_is_the_same_whatever_batches_and_order_the_records_arrive_in() {
        let base = "one two three\nfour five six";
        let r_base = text_record("d1", 1, 1000, None, None, base);
        let r_upper = text_record(
            "d2",
            1,
            1001,
            None,
            Some(base),
            "ONE two three\nfour five six",
        );
        let r_then = text_record(
            "d2",
            2,
            1004,
            None,
            Some("ONE two three\nfour five six"),
            "ONE two three\nfour five SIX",
        );
        let r_clash = text_record(
            "d3",
            1,
            1002,
            None,
            Some(base),
            "Uno two three\nfour five six",
        );
        let session_v1 = text_record(
            "d1",
            2,
            1003,
            Some("s"),
            Some(base),
            "one TWO three\nfour five six",
        );
        let session_v2 = text_record(
            "d1",
            3,
            1005,
            Some("s"),
            Some(base),
            "one TWO three\nfour five six more",
        );

        // One device takes them as they come (each device's records in order, but devices
        // late and interleaved), the session's record twice.
        let (a, ra) = device("a");
        for batch in [
            vec![r_base.clone()],
            vec![r_clash.clone()],
            vec![session_v1.clone()],
            vec![r_upper.clone(), r_then.clone()],
            vec![session_v2.clone()],
        ] {
            apply_batch(&a, &ra, "a", batch).unwrap();
        }
        // Another gets the final records in clock order in one go.
        let (b, rb) = device("b");
        apply_batch(
            &b,
            &rb,
            "b",
            vec![
                r_base.clone(),
                r_upper.clone(),
                r_clash.clone(),
                r_then.clone(),
                session_v2.clone(),
            ],
        )
        .unwrap();
        let (name, conflicts) = name_and_conflicts(&a);
        assert_eq!(name_and_conflicts(&b), (name.clone(), conflicts));
        assert!(name.contains("TWO") && name.contains("SIX") && name.contains("more"));

        // Resuming from where the last replay got to is the same as replaying it all.
        let (_, schema) = ra.find("thing").unwrap();
        let unit = texts::unit(schema, "name").unwrap();
        let resumed = texts::fold(&a, schema, "a", "thing", "a", &unit)
            .unwrap()
            .unwrap();
        a.execute("DELETE FROM sync_text_replay", []).unwrap();
        let whole = texts::fold(&a, schema, "a", "thing", "a", &unit)
            .unwrap()
            .unwrap();
        assert_eq!(resumed.values, whole.values);
        assert_eq!(resumed.clashes, whole.clashes);
    }

    /// A change of the Value field `n` of thing `a`, clocked at `ms`, made from `prev`.
    fn n_record(device: &str, seq: i64, ms: i64, n: i64, prev: Option<i64>) -> WireRecord {
        let mut r = record(device, seq, "a", json!({ "n": n }));
        r.hlc_ms = ms;
        r.op = if prev.is_none() { "create" } else { "update" }.into();
        if let Some(p) = prev {
            r.prev = json!({ "n": p }).as_object().unwrap().clone();
        }
        r
    }

    /// The Resolved-automatically rows of kind value, as (id, kept, replaced, devices).
    fn value_rows(conn: &Connection) -> Vec<(String, Value, Value, String, String)> {
        let mut rows: Vec<_> = resolutions::all(conn)
            .unwrap()
            .into_iter()
            .filter(|r| r.kind == "value")
            .map(|r| {
                (
                    r.id,
                    Value::Object(r.kept),
                    Value::Object(r.replaced),
                    r.kept_device,
                    r.replaced_device,
                )
            })
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows
    }

    #[test]
    fn a_change_made_after_another_is_never_logged_as_a_clash_whatever_order_they_arrive_in() {
        let make = n_record("a", 1, 1000, 1, None);
        let by_a = n_record("a", 2, 1500, 2, Some(1));
        // B had seen A's change when it made its own.
        let by_b = n_record("b", 1, 2000, 3, Some(2));
        for order in [
            vec![vec![make.clone(), by_a.clone(), by_b.clone()]],
            vec![vec![make.clone(), by_b.clone()], vec![by_a.clone()]],
            vec![vec![by_b.clone()], vec![make.clone(), by_a.clone()]],
        ] {
            let (conn, registry) = device("c");
            for batch in order {
                apply_batch(&conn, &registry, "c", batch).unwrap();
            }
            assert!(value_rows(&conn).is_empty());
            assert_eq!(count(&conn, "SELECT n FROM fake_things"), 3);
        }
    }

    #[test]
    fn a_clash_is_the_same_row_whatever_order_the_records_arrive_in() {
        let make = n_record("a", 1, 1000, 1, None);
        let by_a = n_record("a", 2, 1500, 2, Some(1));
        let by_b = n_record("b", 1, 2000, 3, Some(1));
        let later = n_record("a", 3, 3000, 4, Some(2));
        let mut seen = Vec::new();
        for order in [
            vec![vec![
                make.clone(),
                by_a.clone(),
                by_b.clone(),
                later.clone(),
            ]],
            vec![vec![by_b.clone()], vec![make.clone(), by_a.clone()]],
            vec![vec![make.clone(), by_b.clone()], vec![by_a.clone()]],
        ] {
            let (conn, registry) = device("c");
            for batch in order {
                apply_batch(&conn, &registry, "c", batch).unwrap();
            }
            seen.push(value_rows(&conn));
        }
        assert_eq!(seen[1].len(), 1, "{:?}", seen[1]);
        assert_eq!(seen[1], seen[2]);
        assert_eq!(seen[1][0].1, json!({"n": 3}));
        assert_eq!(seen[1][0].2, json!({"n": 2}));
        // `later` was made on A after A's own change; the newest value now beats B's, which is
        // a second clash (A had not seen B's), kept 4 over 3.
        assert_eq!(seen[0].len(), 2, "{:?}", seen[0]);
    }

    #[test]
    fn a_row_logged_before_a_missing_record_arrived_is_taken_back() {
        let (conn, registry) = device("c");
        apply_batch(
            &conn,
            &registry,
            "c",
            vec![
                n_record("a", 1, 1000, 1, None),
                n_record("b", 1, 2000, 3, Some(2)),
            ],
        )
        .unwrap();
        // Without A's change in between, B's looks like a clash.
        assert_eq!(value_rows(&conn).len(), 1);
        apply_batch(
            &conn,
            &registry,
            "c",
            vec![n_record("a", 2, 1500, 2, Some(1))],
        )
        .unwrap();
        assert!(value_rows(&conn).is_empty());
    }

    #[test]
    fn records_from_before_the_replaced_value_travelled_are_never_logged() {
        let (conn, registry) = device("c");
        let mut old = n_record("b", 1, 2000, 3, Some(9));
        old.prev.clear();
        apply_batch(
            &conn,
            &registry,
            "c",
            vec![
                n_record("a", 1, 1000, 1, None),
                n_record("a", 2, 1500, 2, Some(1)),
                old,
            ],
        )
        .unwrap();
        assert!(value_rows(&conn).is_empty());
        assert_eq!(count(&conn, "SELECT n FROM fake_things"), 3);
    }

    #[test]
    fn a_used_outcome_that_arrives_before_the_row_is_taken_when_the_row_is_made() {
        let (conn, registry) = device("c");
        let make = n_record("a", 1, 1000, 1, None);
        let by_a = n_record("a", 2, 1500, 2, Some(1));
        let by_b = n_record("b", 1, 2000, 3, Some(1));
        apply_batch(&conn, &registry, "c", vec![make, by_b]).unwrap();
        // Another device made the row (from the records it had) and pressed "Use".
        let id = value_rows(&conn);
        assert!(id.is_empty(), "no clash yet: A's change is missing here");
        let row_id = {
            let (other, other_registry) = device("d");
            apply_batch(
                &other,
                &other_registry,
                "d",
                vec![
                    n_record("a", 1, 1000, 1, None),
                    by_a.clone(),
                    n_record("b", 1, 2000, 3, Some(1)),
                ],
            )
            .unwrap();
            value_rows(&other)[0].0.clone()
        };
        let mut used = record("d", 1, &row_id, json!({"used_at": 5000, "used_by": "d"}));
        used.entity = resolutions::ENTITY.into();
        used.module = crate::MODULE_ID.into();
        used.hlc_ms = 3000;
        apply_batch(&conn, &registry, "c", vec![used]).unwrap();
        apply_batch(&conn, &registry, "c", vec![by_a]).unwrap();
        let row = resolutions::get(&conn, &row_id).unwrap().unwrap();
        assert_eq!(row.used_at, Some(5000));
    }
}
