//! Resolved automatically: what Sync decided without asking, kept as a log. A value two devices
//! changed beside each other (the newest took), a record deleted on one device and edited on
//! another, a Task put in the Inbox because its List is gone, a text clash on something deleted.
//!
//! A row is `sync_resolutions` and travels as the change-log entity `sync_resolution`. Its id is
//! derived from what was decided (the record, the field and the clocks of the two values).
//! Value clashes and a delete met by an edit are worked out on every device from the stored
//! records themselves, replayed in clock order (`settle_value`, `note_delete_edit`), so the log
//! is the same whatever order the records arrived in: a row made before a missing record turned
//! up is taken back, and a copy of such a row from another device is never taken as it is (its
//! outcome, "used", is). The one outcome ever added is "used": the person pressed "Use ‹other›
//! instead".
//!
//! Order keys and Preferences are not logged one by one: a session folds them into one local
//! line, "Order & preferences · N changes" (`fold_add`).
//!
//! Never log values.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::db::physical_now;
use mnemora_core::error::AppResult;
use mnemora_core::hlc::Stamp;
use mnemora_core::settings;
use mnemora_core::syncable::{Applied, FieldClass, RemoteChange, SyncEntity, SyncField};
use rusqlite::{Connection, OptionalExtension, Row as SqlRow, params};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::error::SyncError;
use crate::wire::WireRecord;

/// The change-log entity name of a resolution.
pub const ENTITY: &str = "sync_resolution";

/// What "Use ‹other› instead" sets once pressed.
const OUTCOME: &str = "outcome";

/// The resolution entity: all fixed at creation and identical on every device that makes it,
/// but for the outcome.
pub(crate) const RESOLUTION: SyncEntity = SyncEntity {
    name: ENTITY,
    fields: &[
        SyncField::value("kind"),
        SyncField::value("target"),
        SyncField::value("target_id"),
        SyncField::value("field"),
        SyncField::value("kept"),
        SyncField::value("kept_device"),
        SyncField::value("replaced"),
        SyncField::value("replaced_device"),
        SyncField::value("extra"),
        SyncField::value("at"),
        SyncField::value("used_at").in_group(OUTCOME),
        SyncField::value("used_by").in_group(OUTCOME),
    ],
};

/// The namespace resolution ids are derived in (`UUIDv5`).
const NAMESPACE: Uuid = Uuid::from_u128(0x4d6e_656d_6f72_6120_7265_736f_6c76_6564);

/// Fields that are bookkeeping, not something a person decided: never logged.
const BOOKKEEPING: &[&str] = &["created_at", "repeat_of", "skipped"];

/// How many names a folded line keeps for "Show" (the count stays exact).
const DETAIL_MAX: usize = 100;

/// What a row is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A value two devices changed beside each other: the newest took.
    Value,
    /// A record deleted on one device and edited on another.
    DeleteEdit,
    /// A Task put in the Inbox because the List it was moved to is gone.
    TaskInbox,
    /// A text clash on a record that is deleted.
    ConflictDeleted,
    /// Order and Preference changes of one sync session (this device's own line).
    OrderPrefs,
}

impl Kind {
    /// The value stored and synced.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::DeleteEdit => "delete_edit",
            Self::TaskInbox => "task_inbox",
            Self::ConflictDeleted => "conflict_deleted",
            Self::OrderPrefs => "order_prefs",
        }
    }

    /// The kind stored under `s`.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "value" => Self::Value,
            "delete_edit" => Self::DeleteEdit,
            "task_inbox" => Self::TaskInbox,
            "conflict_deleted" => Self::ConflictDeleted,
            "order_prefs" => Self::OrderPrefs,
            _ => return None,
        })
    }
}

/// One row as stored.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Row {
    pub(crate) id: String,
    pub(crate) kind: String,
    /// The change-log entity and id of the record it is about, and the unit (field or group).
    pub(crate) target: String,
    pub(crate) target_id: String,
    pub(crate) field: String,
    /// The values that stand, and the ones that gave way, by field.
    pub(crate) kept: Map<String, Value>,
    pub(crate) kept_device: String,
    pub(crate) replaced: Map<String, Value>,
    pub(crate) replaced_device: String,
    /// Kind-specific: the deleted List of a Task put in the Inbox; the Conflict of a clash.
    pub(crate) extra: String,
    /// When it was decided (clock milliseconds).
    pub(crate) at: i64,
    pub(crate) used_at: Option<i64>,
    pub(crate) used_by: Option<String>,
    /// When this device first had it.
    pub(crate) learned_at: i64,
    /// A folded line: how many changes, and what they were.
    pub(crate) count: i64,
    pub(crate) detail: Vec<Folded>,
}

const COLUMNS: &str = "id, kind, target, target_id, field, kept, kept_device, replaced, \
                       replaced_device, extra, at, used_at, used_by, learned_at, count, detail";

fn object(text: &str) -> Map<String, Value> {
    match serde_json::from_str(text) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

fn read(r: &SqlRow<'_>) -> rusqlite::Result<Row> {
    Ok(Row {
        id: r.get(0)?,
        kind: r.get(1)?,
        target: r.get(2)?,
        target_id: r.get(3)?,
        field: r.get(4)?,
        kept: object(&r.get::<_, String>(5)?),
        kept_device: r.get(6)?,
        replaced: object(&r.get::<_, String>(7)?),
        replaced_device: r.get(8)?,
        extra: r.get(9)?,
        at: r.get(10)?,
        used_at: r.get(11)?,
        used_by: r.get(12)?,
        learned_at: r.get(13)?,
        count: r.get(14)?,
        detail: serde_json::from_str(&r.get::<_, String>(15)?).unwrap_or_default(),
    })
}

/// One row.
pub(crate) fn get(conn: &Connection, id: &str) -> Result<Option<Row>, SyncError> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM sync_resolutions WHERE id = ?1"),
            [id],
            read,
        )
        .optional()?)
}

/// Every row, newest first.
pub(crate) fn all(conn: &Connection) -> Result<Vec<Row>, SyncError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM sync_resolutions ORDER BY at DESC, id"
    ))?;
    Ok(stmt.query_map([], read)?.collect::<Result<_, _>>()?)
}

/// The moment of the person's last look at the log; nothing before the first.
pub(crate) fn visited_at(conn: &Connection) -> Result<i64, SyncError> {
    Ok(conn
        .query_row("SELECT at FROM sync_resolution_visit", [], |r| r.get(0))
        .optional()?
        .unwrap_or(0))
}

/// Notes that the person has looked at the log now: the rows it holds stop being new.
pub fn mark_visited(conn: &Connection) -> Result<(), SyncError> {
    conn.execute(
        "INSERT INTO sync_resolution_visit(one, at) VALUES (1, ?1)
         ON CONFLICT(one) DO UPDATE SET at = excluded.at",
        [physical_now(conn)],
    )?;
    Ok(())
}

fn stamp_text(s: &Stamp) -> String {
    format!("{}.{}.{}", s.ms, s.counter, s.device)
}

fn make_id(parts: &[&str]) -> String {
    Uuid::new_v5(&NAMESPACE, parts.join("\0").as_bytes()).to_string()
}

fn json(row: &Row) -> Value {
    let mut m = Map::new();
    let mut put = |k: &str, v: Value| {
        m.insert(k.to_owned(), v);
    };
    put("kind", row.kind.clone().into());
    put("target", row.target.clone().into());
    put("target_id", row.target_id.clone().into());
    put("field", row.field.clone().into());
    put("kept", Value::Object(row.kept.clone()).to_string().into());
    put("kept_device", row.kept_device.clone().into());
    put(
        "replaced",
        Value::Object(row.replaced.clone()).to_string().into(),
    );
    put("replaced_device", row.replaced_device.clone().into());
    put("extra", row.extra.clone().into());
    put("at", row.at.into());
    if let Some(at) = row.used_at {
        put("used_at", at.into());
        put("used_by", row.used_by.clone().into());
    }
    Value::Object(m)
}

fn insert(conn: &Connection, row: &Row) -> Result<bool, SyncError> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO sync_resolutions(id, kind, target, target_id, field, kept,
                kept_device, replaced, replaced_device, extra, at, used_at, used_by, learned_at,
                count, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            row.id,
            row.kind,
            row.target,
            row.target_id,
            row.field,
            Value::Object(row.kept.clone()).to_string(),
            row.kept_device,
            Value::Object(row.replaced.clone()).to_string(),
            row.replaced_device,
            row.extra,
            row.at,
            row.used_at,
            row.used_by,
            row.learned_at,
            row.count,
            serde_json::to_string(&row.detail).unwrap_or_else(|_| "[]".into()),
        ],
    )?;
    Ok(n > 0)
}

fn record(conn: &Connection, row: &Row, op: ChangeOp) -> Result<(), SyncError> {
    changelog::record(
        conn,
        &Change {
            module: crate::MODULE_ID.to_owned(),
            entity: ENTITY.to_owned(),
            entity_id: row.id.clone(),
            op,
            session_id: None,
            snapshot: json(row),
        },
    )?;
    Ok(())
}

/// Adds a row unless this device already has it; a new one is recorded as a change so it
/// reaches the other devices (which make the same row on their own when they meet the same
/// decision).
fn note(conn: &Connection, row: &Row) -> Result<bool, SyncError> {
    if !insert(conn, row)? {
        return Ok(false);
    }
    take_used_before_made(conn, &row.id)?;
    record(conn, row, ChangeOp::Create)?;
    tracing::debug!(target: "sync", kind = %row.kind, "an automatic decision was logged");
    Ok(true)
}

/// "Used" can reach a device before it has made the row itself (rows are worked out from the
/// records): the update is in the change log, and is taken now that the row is there.
fn take_used_before_made(conn: &Connection, id: &str) -> Result<(), SyncError> {
    let used: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT fields ->> '$.used_at', fields ->> '$.used_by' FROM change_log
             WHERE entity = ?1 AND entity_id = ?2 AND fields ->> '$.used_at' IS NOT NULL
             ORDER BY hlc_ms, hlc_counter, origin_device LIMIT 1",
            params![ENTITY, id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((at, by)) = used {
        conn.execute(
            "UPDATE sync_resolutions SET used_at = ?2, used_by = ?3 WHERE id = ?1 AND used_at IS NULL",
            params![id, at, by],
        )?;
    }
    Ok(())
}

fn blank(kind: Kind, id: String, at: i64, now: i64) -> Row {
    Row {
        id,
        kind: kind.as_str().to_owned(),
        target: String::new(),
        target_id: String::new(),
        field: String::new(),
        kept: Map::new(),
        kept_device: String::new(),
        replaced: Map::new(),
        replaced_device: String::new(),
        extra: String::new(),
        at,
        used_at: None,
        used_by: None,
        learned_at: now,
        count: 0,
        detail: Vec::new(),
    }
}

// ---------------------------------------------------------------- value clashes

/// A value decided newest-wins against a different one that was changed beside it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Clash {
    /// The field, or the group it belongs to.
    pub(crate) unit: String,
    pub(crate) kept: Map<String, Value>,
    pub(crate) kept_stamp: Stamp,
    pub(crate) replaced: Map<String, Value>,
    pub(crate) replaced_stamp: Stamp,
}

/// An order key or Preference a sync took, for the session's folded line.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Folded {
    /// `order` or `preference`.
    pub kind: String,
    /// The change-log entity.
    pub entity: String,
    /// Its id.
    pub entity_id: String,
    /// The field, or for a Preference its `module.key`.
    pub key: String,
}

/// One unit of a record: a field, or a group of fields that is decided as one.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Unit<'a> {
    pub(crate) key: &'a str,
    pub(crate) members: &'a [&'a str],
    pub(crate) class: FieldClass,
}

/// What deciding one order key or Preference of `record` means for the folded line: counted
/// whenever the newest one takes and moves the value. `take` says the record's values won;
/// `current` is the record's state before it.
///
/// Value fields are not looked at here: whether one was changed beside another is worked out
/// from all the stored records (`settle_value`), so it cannot depend on delivery order.
pub(crate) fn observe(
    record: &WireRecord,
    which: &Unit<'_>,
    take: bool,
    current: &Value,
) -> Option<Folded> {
    let Unit {
        key: unit,
        members,
        class,
    } = *which;
    if !(record.entity == settings::ENTITY || class == FieldClass::Order) || !take {
        return None;
    }
    let stands = |m: &str| current.get(m).unwrap_or(&Value::Null);
    if members
        .iter()
        .filter_map(|m| record.fields.get(*m).map(|v| (m, v)))
        .all(|(m, v)| stands(m) == v)
    {
        return None;
    }
    let preference = record.entity == settings::ENTITY;
    Some(Folded {
        kind: if preference { "preference" } else { "order" }.to_owned(),
        entity: record.entity.clone(),
        entity_id: record.entity_id.clone(),
        key: if preference {
            record.entity_id.clone()
        } else {
            unit.to_owned()
        },
    })
}

/// The members of the Value unit `key` of `schema`, when the unit is one that is logged: a
/// Value field (or group of them) of a record that is not Sync's own, none of it bookkeeping.
pub(crate) fn value_members(schema: &SyncEntity, key: &str) -> Option<Vec<&'static str>> {
    if schema.name.starts_with("sync_") || schema.name == settings::ENTITY {
        return None;
    }
    let members: Vec<_> = schema
        .fields
        .iter()
        .filter(|f| f.class != FieldClass::Derived && f.group.unwrap_or(f.name) == key)
        .collect();
    if members.is_empty()
        || members
            .iter()
            .any(|f| f.class != FieldClass::Value || BOOKKEEPING.contains(&f.name))
    {
        return None;
    }
    Some(members.iter().map(|f| f.name).collect())
}

/// What of a value a person chose, for telling whether two devices chose the same: that a
/// Task is deleted, not the moment it was; that it is Done, not when.
fn essence(unit: &str, member: &str, value: Value) -> Value {
    match (unit, member) {
        ("deleted_at", _) => Value::Bool(!value.is_null()),
        ("state", "closed_at") => Value::Null,
        _ => value,
    }
}

/// Looks at the Value units of this device's own records made since the last time, for clashes
/// with records it had received before. A clash is read off the records when one is stored, and
/// an own record is not "stored" by Sync: made from a value other than the one the records say
/// stands (a Task sitting in the Inbox because its List merged away, then moved), it clashes with
/// the received record before it, which the other devices see and this one would otherwise
/// never log.
pub(crate) fn settle_local(
    conn: &Connection,
    registry: &crate::registry::Registry,
    me: &str,
) -> Result<(), SyncError> {
    let mark: i64 = conn.query_row("SELECT seq FROM sync_local_settled", [], |r| r.get(0))?;
    let top: i64 = conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM change_log", [], |r| {
        r.get(0)
    })?;
    if top <= mark {
        return Ok(());
    }
    let own: Vec<(String, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT entity, entity_id, fields FROM change_log
             WHERE seq > ?1 AND origin_device = ?2 AND op <> 'create' AND fields IS NOT NULL
               AND entity NOT LIKE 'sync\\_%' ESCAPE '\\'
             ORDER BY seq",
        )?;
        stmt.query_map(params![mark, me], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?
    };
    let mut done: BTreeSet<(String, String, String)> = BTreeSet::new();
    for (entity, entity_id, fields) in own {
        let Some((_, schema)) = registry.find(&entity) else {
            continue;
        };
        for name in object(&fields).keys() {
            let Some(field) = schema.field(name) else {
                continue;
            };
            let key = field.group.unwrap_or(field.name);
            if let Some(members) = value_members(schema, key)
                && done.insert((entity.clone(), entity_id.clone(), key.to_owned()))
            {
                settle_value(conn, me, &entity, &entity_id, key, &members)?;
            }
        }
    }
    conn.execute("UPDATE sync_local_settled SET seq = ?1", [top])?;
    Ok(())
}

/// Every clash the stored records of one Value unit make: replayed in clock order, a record
/// made from a value other than the one the replay holds was made BESIDE the one before it
/// (its author had not seen it), and the later of the two (this record) is kept. A change made
/// after another replaced exactly what stands, and is just a change. Pure function of the
/// records, so delivery order cannot matter.
fn replay_clashes(
    conn: &Connection,
    me: &str,
    (entity, entity_id, unit): (&str, &str, &str),
    members: &[&str],
) -> Result<Vec<Clash>, SyncError> {
    // The values come from the record's own `fields`; a member the record does not carry is
    // completed from the snapshot for this device's own rows, as `wire_fields` does for the wire.
    let touched = members
        .iter()
        .map(|m| format!("json_type(fields, '$.{m}') IS NOT NULL"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let columns = members.iter().fold(String::new(), |mut sql, m| {
        let _ = write!(
            sql,
            ", COALESCE(fields -> '$.{m}', CASE WHEN origin_device = ?3 THEN snapshot -> '$.{m}' END), prev -> '$.{m}'"
        );
        sql
    });
    let mut stmt = conn.prepare(&format!(
        "SELECT origin_device, hlc_ms, hlc_counter{columns} FROM change_log
         WHERE entity = ?1 AND entity_id = ?2 AND fields IS NOT NULL AND ({touched})
         ORDER BY hlc_ms, hlc_counter, origin_device, origin_seq"
    ))?;
    let mut rows = stmt.query(params![entity, entity_id, me])?;
    let parse = |text: Option<String>| text.and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let mut out = Vec::new();
    let mut held: Option<(Map<String, Value>, Stamp)> = None;
    while let Some(row) = rows.next()? {
        let stamp = Stamp {
            device: row.get(0)?,
            ms: row.get(1)?,
            counter: row.get(2)?,
        };
        let mut values = held.as_ref().map(|h| h.0.clone()).unwrap_or_default();
        let mut beside = false;
        let stands =
            |before: &Map<String, Value>, m: &str| before.get(m).cloned().unwrap_or(Value::Null);
        for (i, m) in members.iter().enumerate() {
            if let Some(v) = parse(row.get(3 + 2 * i)?) {
                if let Some(before) = parse(row.get(4 + 2 * i)?)
                    && held.is_some()
                    && before != stands(&values, m)
                {
                    beside = true;
                }
                values.insert((*m).to_owned(), v);
            }
        }
        if let Some((replaced, replaced_stamp)) = held.take().filter(|_| beside) {
            // Both chose the same value: nothing was lost.
            if members.iter().any(|m| {
                essence(unit, m, stands(&replaced, m)) != essence(unit, m, stands(&values, m))
            }) {
                out.push(Clash {
                    unit: unit.to_owned(),
                    kept: values.clone(),
                    kept_stamp: stamp.clone(),
                    replaced,
                    replaced_stamp,
                });
            }
        }
        held = Some((values, stamp));
    }
    Ok(out)
}

/// Brings the log to what the stored records of one Value unit say: a row for every clash they
/// make (once, from the same id on every device), and no row for one that a record turning up
/// late showed was none. Called after each record is stored. A row someone already acted on
/// ("Use ‹other› instead") stays.
pub(crate) fn settle_value(
    conn: &Connection,
    me: &str,
    entity: &str,
    entity_id: &str,
    unit: &str,
    members: &[&str],
) -> Result<(), SyncError> {
    let now = physical_now(conn);
    let mut live = Vec::new();
    for c in replay_clashes(conn, me, (entity, entity_id, unit), members)? {
        let id = make_id(&[
            "value",
            entity,
            entity_id,
            &c.unit,
            &stamp_text(&c.kept_stamp),
            &stamp_text(&c.replaced_stamp),
        ]);
        let mut row = blank(
            Kind::Value,
            id.clone(),
            c.kept_stamp.ms.max(c.replaced_stamp.ms),
            now,
        );
        entity.clone_into(&mut row.target);
        entity_id.clone_into(&mut row.target_id);
        row.field.clone_from(&c.unit);
        row.kept = members
            .iter()
            .map(|m| {
                (
                    (*m).to_owned(),
                    c.kept.get(*m).cloned().unwrap_or(Value::Null),
                )
            })
            .collect();
        row.kept_device.clone_from(&c.kept_stamp.device);
        row.replaced = members
            .iter()
            .map(|m| {
                (
                    (*m).to_owned(),
                    c.replaced.get(*m).cloned().unwrap_or(Value::Null),
                )
            })
            .collect();
        row.replaced_device.clone_from(&c.replaced_stamp.device);
        note(conn, &row)?;
        live.push(id);
    }
    let held: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM sync_resolutions
             WHERE kind = 'value' AND target = ?1 AND target_id = ?2 AND field = ?3
               AND used_at IS NULL",
        )?;
        stmt.query_map(params![entity, entity_id, unit], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    for id in held.into_iter().filter(|id| !live.contains(id)) {
        conn.execute("DELETE FROM sync_resolutions WHERE id = ?1", [&id])?;
    }
    Ok(())
}

// ---------------------------------------------------------------- delete and edit

/// The first edit, in clock order, of a record by a device other than the one that deleted it
/// (`delete` is its clock and device) from before it was restored, that the delete's author
/// did not know of: later by the clock, or above what the delete says its device had `seen`.
/// Returns the editing device, the edit's clock milliseconds and its fields as JSON.
fn first_unaware_edit(
    conn: &Connection,
    entity: &str,
    entity_id: &str,
    delete: (i64, i64, &str),
    restored: (i64, i64),
    seen: Option<&BTreeMap<String, i64>>,
) -> Result<Option<(String, i64, String)>, SyncError> {
    let (ms, counter, device) = delete;
    let mut stmt = conn.prepare(
        "SELECT origin_device, hlc_ms, fields, origin_seq, hlc_counter FROM change_log
         WHERE entity = ?1 AND entity_id = ?2 AND origin_device != ?3
           AND (hlc_ms, hlc_counter) < (?4, ?5)
           AND fields IS NOT NULL
           AND json_type(fields, '$.deleted_at') IS NULL AND op != 'delete'
         ORDER BY hlc_ms, hlc_counter, origin_device",
    )?;
    let mut rows = stmt.query(params![entity, entity_id, device, restored.0, restored.1])?;
    while let Some(row) = rows.next()? {
        let (edit_device, edit_ms, fields): (String, i64, String) =
            (row.get(0)?, row.get(1)?, row.get(2)?);
        let (edit_seq, edit_counter): (i64, i64) = (row.get(3)?, row.get(4)?);
        let later = (edit_ms, edit_counter) > (ms, counter);
        let unseen = seen.is_some_and(|s| edit_seq > s.get(&edit_device).copied().unwrap_or(0));
        if later || unseen {
            return Ok(Some((edit_device, edit_ms, fields)));
        }
    }
    Ok(None)
}

/// Logs a record that is deleted here although another device edited it without having seen
/// the delete: both took effect (it is deleted, with the edit inside, back if restored).
/// Called after a record of a deleted entity is stored. An edit is concurrent with the delete
/// when its clock is later than the delete's (whoever wrote it had not seen the delete, or the
/// delete's own author would have had a later clock), or when the delete's author had not yet
/// received it (the delete says what its device held, `seen`). Whichever clock is later, the
/// line is the same.
pub(crate) fn note_delete_edit(
    conn: &Connection,
    schema: &SyncEntity,
    entity_id: &str,
) -> Result<(), SyncError> {
    let entity = schema.name;
    let delete: Option<(i64, i64, String, Option<String>)> = conn
        .query_row(
            "SELECT hlc_ms, hlc_counter, origin_device, seen FROM change_log
             WHERE entity = ?1 AND entity_id = ?2 AND json_extract(fields, '$.deleted_at') IS NOT NULL
             ORDER BY hlc_ms DESC, hlc_counter DESC, origin_device DESC LIMIT 1",
            [entity, entity_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((ms, counter, device, seen)) = delete else {
        return Ok(());
    };
    // `None`: a delete from before records said what they had seen; only a later clock counts.
    let seen: Option<BTreeMap<String, i64>> = seen.and_then(|s| serde_json::from_str(&s).ok());
    // Whoever restored it had seen the delete, and so has everything written after that.
    let restored: (i64, i64) = conn
        .query_row(
            "SELECT hlc_ms, hlc_counter FROM change_log
             WHERE entity = ?1 AND entity_id = ?2 AND (hlc_ms, hlc_counter) > (?3, ?4)
               AND json_type(fields, '$.deleted_at') = 'null'
             ORDER BY hlc_ms, hlc_counter LIMIT 1",
            params![entity, entity_id, ms, counter],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .unwrap_or((i64::MAX, 0));
    let edit = first_unaware_edit(
        conn,
        entity,
        entity_id,
        (ms, counter, &device),
        restored,
        seen.as_ref(),
    )?;
    let Some((edit_device, edit_ms, fields)) = edit else {
        retract_delete_edits(conn, entity, entity_id, None)?;
        return Ok(());
    };
    let id = make_id(&[
        "delete_edit",
        entity,
        entity_id,
        &stamp_text(&Stamp {
            ms,
            counter,
            device: device.clone(),
        }),
    ]);
    let mut row = blank(Kind::DeleteEdit, id, ms.max(edit_ms), physical_now(conn));
    entity.clone_into(&mut row.target);
    entity_id.clone_into(&mut row.target_id);
    row.kept_device = device;
    // Which fields the edit touched, not what it wrote.
    row.replaced = object(&fields)
        .keys()
        .filter(|k| {
            schema
                .field(k)
                .is_some_and(|f| f.class != FieldClass::Derived)
        })
        .map(|k| (k.clone(), Value::Null))
        .collect();
    row.replaced_device = edit_device;
    retract_delete_edits(conn, entity, entity_id, Some(&row.id))?;
    if !note(conn, &row)? {
        // The edit that is first in clock order may have turned up after a later one made the
        // row: it says who edited, whichever device saw which record first.
        conn.execute(
            "UPDATE sync_resolutions SET at = ?2, replaced = ?3, replaced_device = ?4 WHERE id = ?1",
            params![
                row.id,
                row.at,
                Value::Object(row.replaced.clone()).to_string(),
                row.replaced_device
            ],
        )?;
    }
    Ok(())
}

/// Judges again every record that has a delete-and-edit line: a delete made on this device
/// since (it is not a received record, so nothing else looks) may be newer than the one the line
/// was made for, which then no longer stands. Cheap: there are few such lines.
pub(crate) fn refresh_delete_edits(
    conn: &Connection,
    registry: &crate::registry::Registry,
) -> Result<(), SyncError> {
    let records: Vec<(String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT target, target_id FROM sync_resolutions WHERE kind = 'delete_edit'",
        )?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    for (entity, entity_id) in records {
        if let Some((_, schema)) = registry.find(&entity) {
            note_delete_edit(conn, schema, &entity_id)?;
        }
    }
    Ok(())
}

/// Takes back the delete-and-edit rows of a record that are not `keep`: they were made from a
/// delete that a later one replaced, or before an edit's absence was known.
fn retract_delete_edits(
    conn: &Connection,
    entity: &str,
    entity_id: &str,
    keep: Option<&str>,
) -> Result<(), SyncError> {
    conn.execute(
        "DELETE FROM sync_resolutions
         WHERE kind = 'delete_edit' AND target = ?1 AND target_id = ?2 AND id IS NOT ?3",
        params![entity, entity_id, keep],
    )?;
    Ok(())
}

// ---------------------------------------------------------------- Inbox and deleted clashes

/// Logs a Task put in the Inbox because the List it was moved to is gone here. `by` is the
/// device that moved it, `at` the clock time of that record.
pub(crate) fn note_task_inbox(
    conn: &Connection,
    task_id: &str,
    list_id: &str,
    by: &str,
    at: i64,
) -> Result<(), SyncError> {
    let mut row = blank(
        Kind::TaskInbox,
        make_id(&["task_inbox", task_id, list_id]),
        at,
        physical_now(conn),
    );
    row.target = "todo_task".into();
    task_id.clone_into(&mut row.target_id);
    row.field = "list_id".into();
    row.kept.insert("list_id".into(), Value::Null);
    row.replaced
        .insert("list_id".into(), Value::String(list_id.to_owned()));
    by.clone_into(&mut row.replaced_device);
    list_id.clone_into(&mut row.extra);
    note(conn, &row)?;
    Ok(())
}

/// Logs a text clash that was settled without asking because its record is deleted.
pub(crate) fn note_conflict_deleted(conn: &Connection, conflict: &str) -> Result<(), SyncError> {
    let Some(c) = crate::conflicts::get(conn, conflict)? else {
        return Ok(());
    };
    let mut row = blank(
        Kind::ConflictDeleted,
        make_id(&["conflict_deleted", conflict]),
        c.raised_at,
        physical_now(conn),
    );
    row.target = c.target;
    row.target_id = c.target_id;
    row.field = c.field;
    row.kept_device = c.a_device;
    row.replaced_device = c.b_device;
    row.extra = c.id;
    note(conn, &row)?;
    Ok(())
}

// ---------------------------------------------------------------- Order and Preferences

/// Adds what a batch of one session took to that session's folded line, making it with the
/// first. The line is this device's own account of the session: it is not recorded as a change.
pub(crate) fn fold_add(
    conn: &Connection,
    session: &str,
    peer: &str,
    changes: &[Folded],
) -> Result<(), SyncError> {
    if changes.is_empty() {
        return Ok(());
    }
    let now = physical_now(conn);
    let mut row = get(conn, session)?.unwrap_or_else(|| {
        let mut row = blank(Kind::OrderPrefs, session.to_owned(), now, now);
        peer.clone_into(&mut row.replaced_device);
        row
    });
    row.count += i64::try_from(changes.len()).unwrap_or(i64::MAX);
    for c in changes {
        if row.detail.len() < DETAIL_MAX && !row.detail.contains(c) {
            row.detail.push(c.clone());
        }
    }
    conn.execute(
        "INSERT INTO sync_resolutions(id, kind, replaced_device, at, learned_at, count, detail)
         VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET count = excluded.count, detail = excluded.detail",
        params![
            row.id,
            Kind::OrderPrefs.as_str(),
            row.replaced_device,
            row.at,
            row.count,
            serde_json::to_string(&row.detail).unwrap_or_else(|_| "[]".into()),
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------- remote rows

/// The fields of the record that created the line `id`, as stored in the change log.
fn creation_fields(conn: &Connection, id: &str) -> AppResult<Option<Map<String, Value>>> {
    let fields: Option<String> = conn
        .query_row(
            "SELECT fields FROM change_log WHERE entity = ?1 AND entity_id = ?2 AND op = 'create'
             ORDER BY hlc_ms, hlc_counter, origin_device LIMIT 1",
            params![ENTITY, id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(fields.and_then(|f| match serde_json::from_str(&f) {
        Ok(Value::Object(map)) => Some(map),
        _ => None,
    }))
}

/// Takes the values of a resolution received from another device (Sync's side of the Module
/// contract). The row is made from a record that has all of it; the outcome only ever moves
/// from unused to used.
pub(crate) fn apply(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let id = change.entity_id;
    let f = change.fields;
    let before = get(conn, id)?;
    // A value clash and a delete met by an edit are worked out on every device from the records
    // themselves (a pure function of them): a copy made elsewhere may rest on records this
    // device lacks, or not yet on a later one, so it is never taken as it is. The exception is a
    // line somebody used (changed back): it stays however the records read later, so a device
    // that took it back before hearing of that makes it again from its creation record.
    // The kind is in the record that creates a line, not in the update that uses it.
    let from_creation;
    let g = if f.contains_key("kind") {
        Some(f)
    } else if before.is_none() {
        from_creation = creation_fields(conn, id)?;
        from_creation.as_ref()
    } else {
        None
    };
    let derived = g.is_some_and(|g| {
        matches!(
            g.get("kind").and_then(Value::as_str).and_then(Kind::parse),
            Some(Kind::Value | Kind::DeleteEdit)
        )
    });
    let text = |k: &str| f.get(k).and_then(Value::as_str).map(str::to_owned);
    let int = |k: &str| f.get(k).and_then(Value::as_i64);
    if before.is_none() && (!derived || f.contains_key("used_at")) {
        let made = g.and_then(|g| {
            let text = |k: &str| g.get(k).and_then(Value::as_str).map(str::to_owned);
            let int = |k: &str| g.get(k).and_then(Value::as_i64);
            let kind = text("kind").filter(|k| Kind::parse(k).is_some())?;
            Some(Row {
                id: id.to_owned(),
                kind,
                target: text("target")?,
                target_id: text("target_id")?,
                field: text("field")?,
                kept: object(&text("kept")?),
                kept_device: text("kept_device")?,
                replaced: object(&text("replaced")?),
                replaced_device: text("replaced_device")?,
                extra: text("extra")?,
                at: int("at")?,
                used_at: None,
                used_by: None,
                learned_at: physical_now(conn),
                count: 0,
                detail: Vec::new(),
            })
        });
        let Some(row) = made else {
            tracing::debug!(target: "sync", "a resolution record without its creation, kept for relay only");
            return Ok(Applied {
                snapshot: Value::Object(f.clone()),
                changed: false,
                events: Vec::new(),
            });
        };
        insert(conn, &row)?;
    }
    if let Some(at) = int("used_at") {
        conn.execute(
            "UPDATE sync_resolutions SET used_at = ?2, used_by = ?3
             WHERE id = ?1 AND used_at IS NULL",
            params![id, at, text("used_by")],
        )?;
    }
    let after = get(conn, id)?;
    Ok(Applied {
        snapshot: after.as_ref().map_or(Value::Null, json),
        changed: before != after,
        events: Vec::new(),
    })
}

// ---------------------------------------------------------------- Use ‹other› instead

/// What the app must write to put the value that gave way back, through the owning Module.
#[derive(Debug, Clone, PartialEq)]
pub struct UsePlan {
    /// The change-log entity of the record.
    pub entity: String,
    /// Its id.
    pub entity_id: String,
    /// The field, or the group it belongs to.
    pub unit: String,
    /// The value(s) to write, by field.
    pub values: Map<String, Value>,
}

/// What to write for "Use ‹other› instead". Errors if the row is gone, was already used, or is
/// not a value that can be written back.
pub fn plan_use(conn: &Connection, id: &str) -> Result<UsePlan, SyncError> {
    let Some(row) = get(conn, id)? else {
        return Err(SyncError::Validation("That item is gone"));
    };
    if row.used_at.is_some() {
        return Err(SyncError::Validation("That was already changed"));
    }
    if Kind::parse(&row.kind) != Some(Kind::Value) {
        return Err(SyncError::Validation("That cannot be changed back"));
    }
    Ok(UsePlan {
        entity: row.target,
        entity_id: row.target_id,
        unit: row.field,
        values: row.replaced,
    })
}

/// Marks a row used, as a change that reaches the other devices.
pub fn finish_use(conn: &Connection, id: &str, by: &str) -> Result<(), SyncError> {
    let now = physical_now(conn);
    let n = conn.execute(
        "UPDATE sync_resolutions SET used_at = ?2, used_by = ?3 WHERE id = ?1 AND used_at IS NULL",
        params![id, now, by],
    )?;
    if n > 0
        && let Some(row) = get(conn, id)?
    {
        record(conn, &row, ChangeOp::Update)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn stamp(ms: i64, device: &str) -> Stamp {
        Stamp {
            ms,
            counter: 0,
            device: device.into(),
        }
    }

    fn record(entity: &str, fields: Value, prev: Value) -> WireRecord {
        WireRecord {
            module: "todo".into(),
            entity: entity.into(),
            entity_id: "t1".into(),
            op: "update".into(),
            session_id: None,
            at: 1,
            origin_device: "b".into(),
            origin_seq: 1,
            hlc_ms: 10,
            hlc_counter: 0,
            fields: fields.as_object().unwrap().clone(),
            prev: prev.as_object().unwrap().clone(),
            seen: None,
        }
    }

    fn unit(key: &str, class: FieldClass) -> Unit<'_> {
        Unit {
            key,
            members: match key {
                "sort_key" => &["sort_key"],
                _ => &["value"],
            },
            class,
        }
    }

    #[test]
    fn order_keys_and_preferences_are_folded_not_logged() {
        let r = record(
            "todo_task",
            json!({"sort_key": "b"}),
            json!({"sort_key": "a"}),
        );
        let seen = observe(
            &r,
            &unit("sort_key", FieldClass::Order),
            true,
            &json!({"sort_key": "c"}),
        );
        assert!(matches!(seen, Some(f) if f.kind == "order"));
        let lost = observe(
            &r,
            &unit("sort_key", FieldClass::Order),
            false,
            &json!({"sort_key": "c"}),
        );
        assert!(lost.is_none(), "an order that gave way changed nothing");

        let pref = record("setting", json!({"value": "sun"}), json!({"value": "mon"}));
        let seen = observe(
            &pref,
            &unit("value", FieldClass::Value),
            true,
            &json!({"value": "tue"}),
        );
        assert!(matches!(seen, Some(f) if f.kind == "preference"));
    }

    #[test]
    fn only_value_fields_that_a_person_decided_are_logged() {
        const SCHEMA: SyncEntity = SyncEntity {
            name: "todo_task",
            fields: &[
                SyncField::value("priority"),
                SyncField::value("skipped"),
                SyncField::order("sort_key"),
                SyncField::text("title"),
                SyncField::value("due_date").in_group("due"),
                SyncField::value("due_time").in_group("due"),
            ],
        };
        assert_eq!(value_members(&SCHEMA, "priority"), Some(vec!["priority"]));
        assert_eq!(
            value_members(&SCHEMA, "due"),
            Some(vec!["due_date", "due_time"])
        );
        for none in ["skipped", "sort_key", "title", "nothing"] {
            assert_eq!(value_members(&SCHEMA, none), None, "{none}");
        }
    }

    #[test]
    fn the_id_is_the_same_whichever_device_decided() {
        let kept = stamp(10, "b");
        let replaced = stamp(5, "a");
        let a = make_id(&[
            "value",
            "e",
            "1",
            "priority",
            &stamp_text(&kept),
            &stamp_text(&replaced),
        ]);
        let b = make_id(&[
            "value",
            "e",
            "1",
            "priority",
            &stamp_text(&kept),
            &stamp_text(&replaced),
        ]);
        assert_eq!(a, b);
        assert!(Uuid::parse_str(&a).is_ok());
    }
}
