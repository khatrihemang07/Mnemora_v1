//! What this device sends: the records the other lacks, as changed-fields-only wire records in
//! batches, oldest clock first.
//!
//! A record's clock is later than every record its author had seen, so clock order puts a
//! record after the ones it depends on (a Task's creation before its edits) even when they come
//! from different devices. Within one origin, clock order is sequence order, which is what the
//! version vector counts on.

use std::collections::{BTreeMap, VecDeque};

use mnemora_core::changelog::ChangeOp;
use mnemora_core::syncable::{FieldClass, SyncEntity};
use rusqlite::Connection;
use serde_json::{Map, Value};

use crate::error::SyncError;
use crate::registry::{Registry, wire_fields};
use crate::vector;
use crate::wire::{Vector, WireRecord};

/// The most records in a batch.
pub const BATCH_RECORDS: usize = 500;
/// A batch is closed once its records add up to about this many bytes, so one batch stays far
/// below the frame limit even when notes are long.
pub const BATCH_BYTES: usize = 2 * 1024 * 1024;

/// How far a stream has got: per entity and origin device, the last sequence handed out.
#[derive(Debug, Default)]
pub struct Cursor(BTreeMap<(String, String), i64>);

/// The next batch for a peer that holds `peer` (its Hello vector), and whether more follows.
/// Only entities the peer declares are sent.
pub fn next_batch(
    conn: &Connection,
    registry: &Registry,
    me: &str,
    peer: &Vector,
    cursor: &mut Cursor,
) -> Result<(Vec<WireRecord>, bool), SyncError> {
    // One queue per (entity, origin), each in sequence order; a full queue may hide more.
    let mut queues: Vec<(String, String, VecDeque<WireRecord>)> = Vec::new();
    let mut more = false;
    for entity in registry.entities() {
        let Some(peer_entity) = peer.get(entity) else {
            continue;
        };
        for (device, newest) in vector::of_entity(conn, me, entity)? {
            let from = peer_entity.get(&device).copied().unwrap_or(0).max(
                cursor
                    .0
                    .get(&(entity.to_owned(), device.clone()))
                    .copied()
                    .unwrap_or(0),
            );
            if newest <= from {
                continue;
            }
            let rows = records_after(conn, registry, me, entity, &device, from)?;
            more |= rows.len() >= BATCH_RECORDS;
            queues.push((entity.to_owned(), device, rows.into()));
        }
    }

    let mut batch = Vec::new();
    let mut bytes = 0;
    while batch.len() < BATCH_RECORDS && bytes < BATCH_BYTES {
        // The queue whose next record has the oldest clock.
        let Some(queue) = queues
            .iter_mut()
            .filter(|q| !q.2.is_empty())
            .min_by_key(|q| q.2.front().map(order_key))
        else {
            break;
        };
        let Some(record) = queue.2.pop_front() else {
            break;
        };
        cursor
            .0
            .insert((queue.0.clone(), queue.1.clone()), record.origin_seq);
        bytes += serde_json::to_string(&record).map_or(0, |s| s.len());
        batch.push(record);
    }
    more |= queues.iter().any(|q| !q.2.is_empty());
    Ok((batch, more))
}

/// How many records `peer` lacks that this device holds: the size of the stream it is about to
/// receive, for progress.
pub fn pending(
    conn: &Connection,
    registry: &Registry,
    me: &str,
    peer: &Vector,
) -> Result<usize, SyncError> {
    let mut total: i64 = 0;
    for entity in registry.entities() {
        let Some(peer_entity) = peer.get(entity) else {
            continue;
        };
        for (device, newest) in vector::of_entity(conn, me, entity)? {
            let from = peer_entity.get(&device).copied().unwrap_or(0);
            if newest > from {
                let local_only = registry.local_only(entity);
                let mut args: Vec<&dyn rusqlite::ToSql> = vec![&entity, &device, &from];
                args.extend(local_only.iter().map(|id| id as &dyn rusqlite::ToSql));
                total += conn.query_row(
                    &format!(
                        "SELECT COUNT(*) FROM change_log
                         WHERE entity = ?1 AND origin_device = ?2 AND origin_seq > ?3
                           AND entity_id NOT IN ({})",
                        placeholders(local_only.len(), 4)
                    ),
                    args.as_slice(),
                    |r| r.get::<_, i64>(0),
                )?;
            }
        }
    }
    Ok(usize::try_from(total).unwrap_or(0))
}

/// `?first, ?first+1, ...` for `n` bound ids. For none, an id no record has, so `NOT IN` keeps
/// every row (`NOT IN ()` is not valid SQL).
fn placeholders(n: usize, first: usize) -> String {
    if n == 0 {
        return "''".to_owned();
    }
    (first..first + n)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Clock order, then device, then sequence: the same on every device.
fn order_key(r: &WireRecord) -> (i64, i64, String, i64) {
    (
        r.hlc_ms,
        r.hlc_counter,
        r.origin_device.clone(),
        r.origin_seq,
    )
}

/// Up to one batch of an origin's records after `from`, as wire records.
fn records_after(
    conn: &Connection,
    registry: &Registry,
    me: &str,
    entity: &str,
    device: &str,
    from: i64,
) -> Result<Vec<WireRecord>, SyncError> {
    let Some((_, schema)) = registry.find(entity) else {
        return Err(SyncError::UnknownEntity);
    };
    let local_only = registry.local_only(entity);
    let mut stmt = conn.prepare(&format!(
        "SELECT module, entity_id, op, session_id, at, origin_seq, hlc_ms, hlc_counter, fields,
                snapshot, prev, seen
         FROM change_log WHERE entity = ?1 AND origin_device = ?2 AND origin_seq > ?3
           AND entity_id NOT IN ({})
         ORDER BY origin_seq LIMIT ?4",
        placeholders(local_only.len(), 5)
    ))?;
    let limit = i64::try_from(BATCH_RECORDS).unwrap_or(i64::MAX);
    let mut args: Vec<&dyn rusqlite::ToSql> = vec![&entity, &device, &from, &limit];
    args.extend(local_only.iter().map(|id| id as &dyn rusqlite::ToSql));
    let mut rows = stmt.query(args.as_slice())?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let fields: Option<String> = row.get(8)?;
        let snapshot: String = row.get(9)?;
        let fields = fields
            .and_then(|f| serde_json::from_str::<Value>(&f).ok())
            .and_then(|v| match v {
                Value::Object(map) => Some(map),
                _ => None,
            });
        let snapshot: Value = serde_json::from_str(&snapshot).unwrap_or(Value::Null);
        let prev: Option<String> = row.get(10)?;
        let seen: Option<String> = row.get(11)?;
        let fields = wire_fields(schema, fields, &snapshot, device == me);
        let prev = field_prev(schema, &fields, prev);
        out.push(WireRecord {
            module: row.get(0)?,
            entity: entity.to_owned(),
            entity_id: row.get(1)?,
            op: row.get(2)?,
            session_id: row.get(3)?,
            at: row.get(4)?,
            origin_device: device.to_owned(),
            origin_seq: row.get(5)?,
            hlc_ms: row.get(6)?,
            hlc_counter: row.get(7)?,
            fields,
            prev,
            seen: seen.and_then(|s| serde_json::from_str(&s).ok()),
        });
    }
    // The op is checked on receipt; sending an unknown one would only be refused there.
    debug_assert!(out.iter().all(|r| ChangeOp::parse(&r.op).is_some()));
    Ok(out)
}

/// What the record's fields replaced: the stored `prev` narrowed to the fields the record
/// sends. A text keeps only a string (the text a receiver merges against); any other field
/// keeps its earlier value, which is how a receiver tells two devices changing a value
/// apart from one device changing it after the other (Resolved automatically).
fn field_prev(
    schema: &SyncEntity,
    fields: &Map<String, Value>,
    prev: Option<String>,
) -> Map<String, Value> {
    let Some(Ok(Value::Object(prev))) = prev.map(|p| serde_json::from_str::<Value>(&p)) else {
        return Map::new();
    };
    prev.into_iter()
        .filter(|(k, v)| {
            fields.contains_key(k)
                && schema.field(k).is_some_and(|f| match f.class {
                    FieldClass::Text => v.is_string(),
                    FieldClass::Derived => false,
                    FieldClass::Value | FieldClass::Order => true,
                })
        })
        .collect()
}
