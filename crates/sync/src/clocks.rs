//! Per-field clocks (local, never synced): when each field of each record last changed, as the
//! clock of the change-log record that changed it. They are what a received value is compared
//! with. Local edits do not write here as they happen (the change log is core's); [`fold`]
//! reads them in from the log before a comparison, so the table is always the newest clock any
//! record of the entity has put on a field.

use std::collections::BTreeMap;

use mnemora_core::hlc::Stamp;
use mnemora_core::syncable::SyncEntity;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};

use crate::error::SyncError;
use crate::registry::wire_fields;

/// Brings the clocks of every record of `entity` up to date with the change log: each log row
/// since the last fold sets the clock of every field it changed (whole groups) to its own clock
/// if that is newer. Rows are read by local sequence, which also moves when an editing
/// session's record is rewritten, so a rewrite is read again.
pub fn fold(conn: &Connection, entity: &SyncEntity, me: &str) -> Result<(), SyncError> {
    let mark: i64 = conn
        .query_row(
            "SELECT seq FROM sync_clock_marks WHERE entity = ?1",
            [entity.name],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0);
    let mut stmt = conn.prepare(
        "SELECT entity_id, fields, snapshot, origin_device, hlc_ms, hlc_counter, seq
         FROM change_log WHERE entity = ?1 AND seq > ?2 ORDER BY seq",
    )?;
    let mut rows = stmt.query(params![entity.name, mark])?;
    let mut newest = mark;
    while let Some(row) = rows.next()? {
        let entity_id: String = row.get(0)?;
        let fields: Option<String> = row.get(1)?;
        let snapshot: String = row.get(2)?;
        let origin: String = row.get(3)?;
        let stamp = Stamp {
            ms: row.get(4)?,
            counter: row.get(5)?,
            device: origin.clone(),
        };
        newest = newest.max(row.get(6)?);
        let fields = fields
            .and_then(|f| serde_json::from_str::<Value>(&f).ok())
            .and_then(|v| match v {
                Value::Object(map) => Some(map),
                _ => None,
            });
        let snapshot: Value = serde_json::from_str(&snapshot).unwrap_or(Value::Null);
        let wire: Map<String, Value> = wire_fields(entity, fields, &snapshot, origin == me);
        for field in wire.keys() {
            raise(conn, entity.name, &entity_id, field, &stamp)?;
        }
    }
    if newest > mark {
        conn.execute(
            "INSERT INTO sync_clock_marks(entity, seq) VALUES (?1, ?2)
             ON CONFLICT(entity) DO UPDATE SET seq = excluded.seq",
            params![entity.name, newest],
        )?;
    }
    Ok(())
}

/// The clocks of one record's fields.
pub fn load(
    conn: &Connection,
    entity: &str,
    entity_id: &str,
) -> Result<BTreeMap<String, Stamp>, SyncError> {
    let mut stmt = conn.prepare(
        "SELECT field, ms, counter, device FROM sync_field_clocks
         WHERE entity = ?1 AND entity_id = ?2",
    )?;
    let rows = stmt.query_map([entity, entity_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            Stamp {
                ms: r.get(1)?,
                counter: r.get(2)?,
                device: r.get(3)?,
            },
        ))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Sets a field's clock to `stamp` if that is newer than the one held.
pub fn raise(
    conn: &Connection,
    entity: &str,
    entity_id: &str,
    field: &str,
    stamp: &Stamp,
) -> Result<(), SyncError> {
    conn.execute(
        "INSERT INTO sync_field_clocks(entity, entity_id, field, ms, counter, device)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(entity, entity_id, field) DO UPDATE
         SET ms = excluded.ms, counter = excluded.counter, device = excluded.device
         WHERE (excluded.ms, excluded.counter, excluded.device) > (ms, counter, device)",
        params![
            entity,
            entity_id,
            field,
            stamp.ms,
            stamp.counter,
            stamp.device
        ],
    )?;
    Ok(())
}
