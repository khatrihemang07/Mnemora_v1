//! What this device does with a batch it receives: every record in one transaction that also
//! advances the version vector, so an interrupted exchange resumes exactly. Per record: the
//! clock is shown the sender's time, each unit of fields is compared with the clock held for
//! it, the Module writes the winners through its own repository, and the record is stored in
//! the change log as sent (origin device, origin sequence, clock) with a fresh local sequence.
//! Never log field values.

use std::collections::BTreeSet;

use mnemora_core::changelog::{self, ChangeOp, Remote};
use mnemora_core::editing;
use mnemora_core::hlc::{Stamp, Timestamp};
use mnemora_core::settings;
use mnemora_core::syncable::{FieldClass, RemoteChange, SyncEntity, SyncEvent};
use mnemora_core::util::now_ms;
use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde_json::{Map, Value};

use crate::clocks;
use crate::error::SyncError;
use crate::held::{self, HeldUnit};
use crate::merge::{Verdict, decide};
use crate::registry::Registry;
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
}

/// Applies `records` atomically: all of them, with the vector advanced, or none.
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
    let mut applied = Applied::default();
    let mut folded: BTreeSet<&str> = BTreeSet::new();
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
        if folded.insert(&record.entity) {
            clocks::fold(&tx, schema, me)?;
        }
        let (winners, kept_back) = decide_fields(&tx, schema, record, op)?;
        let change = RemoteChange {
            entity: &record.entity,
            entity_id: &record.entity_id,
            op,
            at: record.at,
            fields: &winners,
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
            &kept_back,
        )?;
        applied.stored += 1;
        applied.events.extend(outcome.events);
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
    }
    tx.commit()?;
    Ok(applied)
}

/// The values of `record` that win against the clocks held, whole groups at a time, and the
/// winning units an editor holds back (kept apart until it lets go, see `held`). The clocks of
/// the winners move to the record's either way. A record that creates its entity is never
/// held: nothing can be editing what does not exist yet.
fn decide_fields(
    conn: &Connection,
    schema: &SyncEntity,
    record: &WireRecord,
    op: ChangeOp,
) -> Result<(Map<String, Value>, Vec<HeldUnit>), SyncError> {
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
    let mut winners = Map::new();
    let mut kept_back = Vec::new();
    for (key, members, class) in units {
        let newest_held = members.iter().filter_map(|m| held.get(*m)).max();
        if decide(class, newest_held, &stamp) == Verdict::Take {
            let waits = editing::is_held(&scopes, key);
            let mut values = Map::new();
            for member in members {
                if let Some(value) = record.fields.get(member) {
                    values.insert(member.to_owned(), value.clone());
                    clocks::raise(conn, &record.entity, &record.entity_id, member, &stamp)?;
                }
            }
            if waits {
                kept_back.push(HeldUnit {
                    unit: key.to_owned(),
                    fields: values,
                });
            } else {
                winners.extend(values);
            }
        }
    }
    Ok((winners, kept_back))
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
                snapshot: json!({"id": change.entity_id}),
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
        let modules: Modules = Arc::new(vec![Box::new(Fake)]);
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
}
