//! Changes held back while someone is typing (spec "Runtime"; `core::editing` has the holds).
//!
//! A record for a held unit (a field, or a group of fields) is stored and counted as seen like
//! any other, and the unit's field clocks move to the record's clock as usual, so every device
//! keeps judging it the same way. Only the **values** of the held units are kept apart in
//! `sync_held` instead of being written to the Module's tables, so the editor's text never
//! jumps and the editor's own save can never overwrite text it has not seen without it being
//! decided by the clocks.
//!
//! When the hold ends (the editor closes, its lease lapses, the app restarts) the held values
//! are applied if they are still the newest the clocks know for that unit. If the person kept
//! typing after the record arrived their save carries a later clock and wins (newest wins for
//! now). This is the one place the three-way text merge slots in (#66): where a held Text unit
//! is found not to be the newest, the base, the local text and the held text are all at hand.
//! Either way the other side is never lost without a trace: every record stays in the change
//! log, and the loser's text is in it.
//!
//! Never log field values.

use std::collections::BTreeSet;

use mnemora_core::changelog::ChangeOp;
use mnemora_core::editing;
use mnemora_core::hlc::Stamp;
use mnemora_core::syncable::{RemoteChange, SyncEntity, SyncEvent};
use rusqlite::{Connection, params};
use serde_json::{Map, Value};

use crate::clocks;
use crate::conflicts;
use crate::error::SyncError;
use crate::registry::Registry;
use crate::resolutions;
use crate::texts;

/// The values of one unit of a received record, kept until the hold on it ends.
#[derive(Debug)]
pub(crate) struct HeldUnit {
    /// The unit: a group name, or the field name of an ungrouped field.
    pub(crate) unit: String,
    /// The unit's `{field: value}`.
    pub(crate) fields: Map<String, Value>,
}

/// What releasing held changes did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Released {
    /// Modules whose data visibly changed.
    pub changed: BTreeSet<String>,
    /// What the Modules report about it.
    pub events: Vec<SyncEvent>,
}

/// Keeps `units` of the record with clock `stamp` apart.
pub(crate) fn stash(
    conn: &Connection,
    entity: &str,
    entity_id: &str,
    stamp: &Stamp,
    op: ChangeOp,
    at: i64,
    units: &[HeldUnit],
) -> Result<(), SyncError> {
    for held in units {
        conn.execute(
            "INSERT OR REPLACE INTO sync_held(entity, entity_id, unit, ms, counter, device, op, at, fields)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entity,
                entity_id,
                held.unit,
                stamp.ms,
                stamp.counter,
                stamp.device,
                op.as_str(),
                at,
                Value::Object(held.fields.clone()).to_string(),
            ],
        )?;
    }
    Ok(())
}

/// The unit a field belongs to: its group, else itself.
pub(crate) fn unit_of<'a>(entity: &'a SyncEntity, field: &'a str) -> Option<&'a str> {
    entity.field(field).map(|f| f.group.unwrap_or(f.name))
}

struct Row {
    unit: String,
    stamp: Stamp,
    op: ChangeOp,
    at: i64,
    fields: Map<String, Value>,
}

/// Applies every held change whose hold has ended (as of `now`), in one go for each record:
/// the values that are still the newest are written through the Module, the rest dropped.
/// Safe to call any time; with nothing held it is one empty query.
pub fn release_ready(
    conn: &Connection,
    registry: &Registry,
    me: &str,
    now: i64,
) -> Result<Released, SyncError> {
    let mut out = Released::default();
    let records: Vec<(String, String)> = {
        let mut stmt = conn.prepare("SELECT DISTINCT entity, entity_id FROM sync_held")?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    let mut folded: BTreeSet<String> = BTreeSet::new();
    for (entity, entity_id) in records {
        let scopes = editing::held(conn, &entity, &entity_id, now)?;
        let rows = ready_rows(conn, &entity, &entity_id, &scopes)?;
        if rows.is_empty() {
            continue;
        }
        let Some((module, schema)) = registry.find(&entity) else {
            continue;
        };
        if folded.insert(entity.clone()) {
            clocks::fold(conn, schema, me)?;
        }
        let mut folded_clocks_stale = false;
        let held = clocks::load(conn, &entity, &entity_id)?;
        let mut winning: Vec<&Row> = Vec::new();
        for row in rows
            .iter()
            .filter(|r| texts::unit(schema, &r.unit).is_none())
        {
            // Still the newest the clocks know for the unit: nothing newer landed since.
            let newest_held = schema
                .fields
                .iter()
                .filter(|f| unit_of(schema, f.name) == Some(row.unit.as_str()))
                .filter_map(|f| held.get(f.name))
                .max();
            if newest_held == Some(&row.stamp) {
                winning.push(row);
            }
        }
        conn.execute(
            "DELETE FROM sync_held WHERE entity = ?1 AND entity_id = ?2 AND unit IN (SELECT value FROM json_each(?3))",
            params![
                entity,
                entity_id,
                Value::from(rows.iter().map(|r| r.unit.clone()).collect::<Vec<_>>()).to_string()
            ],
        )?;
        // One record at a time, oldest clock first, each with its own operation and moment, as
        // a device that held nothing applied them: what derives from them (the edited time)
        // comes out the same on every device.
        let mut index = 0;
        while index < winning.len() {
            let stamp = &winning[index].stamp;
            let mut fields = Map::new();
            let mut end = index;
            while end < winning.len() && &winning[end].stamp == stamp {
                fields.extend(winning[end].fields.clone());
                end += 1;
            }
            let row = winning[index];
            index = end;
            let change = RemoteChange {
                entity: &entity,
                entity_id: &entity_id,
                op: row.op,
                at: row.at,
                fields: &fields,
                settings: registry.settings(),
            };
            let outcome = module.apply_remote(conn, &change)?;
            // The newest log row says what the record looks like now (the next local change is
            // compared with it to see what it touched).
            conn.execute(
                "UPDATE change_log SET snapshot = ?3
                 WHERE entity = ?1 AND entity_id = ?2
                   AND seq = (SELECT MAX(seq) FROM change_log WHERE entity = ?1 AND entity_id = ?2)",
                params![entity, entity_id, outcome.snapshot.to_string()],
            )?;
            out.events.extend(outcome.events);
            if outcome.changed {
                out.changed.insert(module.id().to_owned());
            }
            let settled = module.settle_remote(conn, &change)?;
            if settled.recorded {
                folded.clear();
            }
            if settled.recorded || !settled.events.is_empty() {
                out.changed.insert(module.id().to_owned());
            }
            out.events.extend(settled.events);
        }
        // Text is not "still the newest or dropped": the person's own saves since the record
        // arrived are records too, and the text is the replay of all of them, merged.
        let mut replayed: BTreeSet<&str> = BTreeSet::new();
        for row in &rows {
            if texts::unit(schema, &row.unit).is_none() || !replayed.insert(&row.unit) {
                continue;
            }
            let settled = texts::settle_unit(conn, registry, me, &entity, &entity_id, &row.unit)?;
            folded_clocks_stale |= settled.recorded;
            out.changed.extend(settled.changed);
            out.events.extend(settled.events);
        }
        if folded_clocks_stale {
            folded.clear();
        }
        after_release(conn, schema, &entity, &entity_id, &mut out)?;
    }
    Ok(out)
}

/// What follows a record whose held units were just applied: an edit made beside a delete, and
/// Conflicts on a deleted record, are judged now, as on a device that held nothing back (the
/// delete itself may have been held).
fn after_release(
    conn: &Connection,
    schema: &SyncEntity,
    entity: &str,
    entity_id: &str,
    out: &mut Released,
) -> Result<(), SyncError> {
    resolutions::note_delete_edit(conn, schema, entity_id)?;
    let deleted: bool = conn.query_row(
        "SELECT COALESCE((SELECT json_extract(snapshot, '$.deleted_at') IS NOT NULL
                          FROM change_log WHERE entity = ?1 AND entity_id = ?2
                          ORDER BY seq DESC LIMIT 1), 0)",
        params![entity, entity_id],
        |r| r.get(0),
    )?;
    if deleted {
        out.events
            .extend(conflicts::settle_deleted(conn, entity, entity_id)?);
    }
    Ok(())
}

/// The held rows of a record whose unit is no longer held, oldest clock first.
fn ready_rows(
    conn: &Connection,
    entity: &str,
    entity_id: &str,
    scopes: &[String],
) -> Result<Vec<Row>, SyncError> {
    let mut stmt = conn.prepare(
        "SELECT unit, ms, counter, device, op, at, fields FROM sync_held
         WHERE entity = ?1 AND entity_id = ?2 ORDER BY ms, counter, device",
    )?;
    let mut rows = Vec::new();
    let mut query = stmt.query(params![entity, entity_id])?;
    while let Some(r) = query.next()? {
        let unit: String = r.get(0)?;
        if editing::is_held(scopes, &unit) {
            continue;
        }
        let op: String = r.get(4)?;
        let fields: String = r.get(6)?;
        let (Some(op), Ok(Value::Object(fields))) =
            (ChangeOp::parse(&op), serde_json::from_str::<Value>(&fields))
        else {
            continue;
        };
        rows.push(Row {
            unit,
            stamp: Stamp {
                ms: r.get(1)?,
                counter: r.get(2)?,
                device: r.get(3)?,
            },
            op,
            at: r.get(5)?,
            fields,
        });
    }
    Ok(rows)
}
