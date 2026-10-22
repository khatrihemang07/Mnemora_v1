//! The version vector: the highest sequence seen per entity and origin device. Records stream
//! per origin in sequence order, so "seen up to N" means "holds every record of that origin up
//! to N", whichever device they came through. Other devices' entries live in `sync_seen` and
//! advance in the transaction that stores what they count; this device's own entry is the
//! newest record it made.

use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::SyncError;
use crate::registry::Registry;
use crate::wire::Vector;

/// What this device holds of `entity`: `{origin device: highest sequence}`, itself included.
pub fn of_entity(
    conn: &Connection,
    me: &str,
    entity: &str,
) -> Result<BTreeMap<String, i64>, SyncError> {
    let mut stmt = conn.prepare("SELECT device, seq FROM sync_seen WHERE entity = ?1")?;
    let mut held: BTreeMap<String, i64> = stmt
        .query_map([entity], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let own: Option<i64> = conn
        .query_row(
            "SELECT MAX(origin_seq) FROM change_log WHERE entity = ?1 AND origin_device = ?2",
            params![entity, me],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    if let Some(own) = own {
        held.insert(me.to_owned(), own);
    }
    Ok(held)
}

/// The whole vector, for every entity that syncs.
pub fn all(conn: &Connection, registry: &Registry, me: &str) -> Result<Vector, SyncError> {
    registry
        .entities()
        .into_iter()
        .map(|entity| Ok((entity.to_owned(), of_entity(conn, me, entity)?)))
        .collect()
}

/// Highest sequence seen from `device` for `entity` (0 when none).
pub fn seen(conn: &Connection, entity: &str, device: &str) -> Result<i64, SyncError> {
    Ok(conn
        .query_row(
            "SELECT seq FROM sync_seen WHERE entity = ?1 AND device = ?2",
            [entity, device],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

/// Records that records of `entity` from `device` are held up to `seq`.
pub fn advance(conn: &Connection, entity: &str, device: &str, seq: i64) -> Result<(), SyncError> {
    conn.execute(
        "INSERT INTO sync_seen(entity, device, seq) VALUES (?1, ?2, ?3)
         ON CONFLICT(entity, device) DO UPDATE SET seq = MAX(seq, excluded.seq)",
        params![entity, device, seq],
    )?;
    Ok(())
}
