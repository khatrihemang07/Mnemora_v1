//! What this device sends: the records the other lacks, as changed-fields-only wire records in
//! batches, oldest clock first.
//!
//! A record's clock is later than every record its author had seen, so clock order puts a
//! record after the ones it depends on (a Task's creation before its edits) even when they come
//! from different devices. Within one origin, clock order is sequence order, which is what the
//! version vector counts on.

use std::collections::{BTreeMap, VecDeque};

use mnemora_core::changelog::ChangeOp;
use rusqlite::{Connection, params};
use serde_json::Value;

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
    let mut stmt = conn.prepare(
        "SELECT module, entity_id, op, session_id, at, origin_seq, hlc_ms, hlc_counter, fields,
                snapshot
         FROM change_log WHERE entity = ?1 AND origin_device = ?2 AND origin_seq > ?3
         ORDER BY origin_seq LIMIT ?4",
    )?;
    let mut rows = stmt.query(params![
        entity,
        device,
        from,
        i64::try_from(BATCH_RECORDS).unwrap_or(i64::MAX)
    ])?;
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
            fields: wire_fields(schema, fields, &snapshot, device == me),
        });
    }
    // The op is checked on receipt; sending an unknown one would only be refused there.
    debug_assert!(out.iter().all(|r| ChangeOp::parse(&r.op).is_some()));
    Ok(out)
}
