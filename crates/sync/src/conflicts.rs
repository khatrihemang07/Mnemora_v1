//! Conflicts: two devices rewrote the same words of a text. The merge (see `texts`) has already
//! put the newest version in the record, so every device shows the same text; what waits here is
//! the other version, for the person to compare.
//!
//! A Conflict is a row of `sync_conflicts` and travels as the change-log entity `sync_conflict`,
//! so the one decision made on one device reaches them all. Its id is derived from what clashed
//! (the record and field, the text both started from and the two versions), so every device
//! that meets the same clash makes the same row, and a device that learns of it from another
//! finds it already there. Only the outcome (`status`, `resolution`, ...) is ever changed, and
//! only from open to resolved.
//!
//! Never log text.

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::error::AppResult;
use mnemora_core::syncable::{Applied, RemoteChange, SyncEntity, SyncEvent, SyncField};
use rusqlite::{Connection, OptionalExtension, Row as SqlRow, params};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::error::SyncError;
use crate::texts::Clashed;

/// The change-log entity name of a Conflict.
pub const ENTITY: &str = "sync_conflict";

/// What a resolved Conflict's fields say changed it.
const OUTCOME: &str = "outcome";

/// The Conflict entity: everything but the outcome is fixed at creation and identical on every
/// device that makes it; the outcome is one atomic group.
pub(crate) const CONFLICT: SyncEntity = SyncEntity {
    name: ENTITY,
    fields: &[
        SyncField::value("target"),
        SyncField::value("target_id"),
        SyncField::value("field"),
        SyncField::value("base"),
        SyncField::value("a_text"),
        SyncField::value("a_device"),
        SyncField::value("a_at"),
        SyncField::value("b_text"),
        SyncField::value("b_device"),
        SyncField::value("b_at"),
        SyncField::value("raised_at"),
        SyncField::value("status").in_group(OUTCOME),
        SyncField::value("resolution").in_group(OUTCOME),
        SyncField::value("resolved_at").in_group(OUTCOME),
        SyncField::value("resolved_by").in_group(OUTCOME),
    ],
};

/// The namespace Conflict ids are derived in (`UUIDv5`).
const NAMESPACE: Uuid = Uuid::from_u128(0x4d6e_656d_6f72_6120_636f_6e66_6c69_6374);

/// How a Conflict ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// The version already showing was kept.
    KeptThis,
    /// The other version was written.
    KeptOther,
    /// A text chosen clash by clash and polished was written.
    Merged,
    /// The record is deleted: nothing was asked.
    Deleted,
    /// A record that arrived late showed the clash was never one: every device replays the same
    /// records, so it is closed on all of them.
    Superseded,
}

impl Resolution {
    /// The value stored and synced.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KeptThis => "keep_this",
            Self::KeptOther => "keep_other",
            Self::Merged => "merged",
            Self::Deleted => "deleted",
            Self::Superseded => "superseded",
        }
    }
}

/// One Conflict as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stored {
    pub(crate) id: String,
    /// The change-log entity and id of the record it is about, and the text field.
    pub(crate) target: String,
    pub(crate) target_id: String,
    pub(crate) field: String,
    /// What both started from.
    pub(crate) base: String,
    /// The newest version: what was applied.
    pub(crate) a_text: String,
    pub(crate) a_device: String,
    pub(crate) a_at: i64,
    /// The other version.
    pub(crate) b_text: String,
    pub(crate) b_device: String,
    pub(crate) b_at: i64,
    /// When the newest version was written.
    pub(crate) raised_at: i64,
    pub(crate) open: bool,
    pub(crate) resolution: Option<String>,
    pub(crate) resolved_at: Option<i64>,
    pub(crate) resolved_by: Option<String>,
}

const COLUMNS: &str = "id, target, target_id, field, base, a_text, a_device, a_at, b_text, \
                       b_device, b_at, raised_at, status, resolution, resolved_at, resolved_by";

fn read(r: &SqlRow<'_>) -> rusqlite::Result<Stored> {
    Ok(Stored {
        id: r.get(0)?,
        target: r.get(1)?,
        target_id: r.get(2)?,
        field: r.get(3)?,
        base: r.get(4)?,
        a_text: r.get(5)?,
        a_device: r.get(6)?,
        a_at: r.get(7)?,
        b_text: r.get(8)?,
        b_device: r.get(9)?,
        b_at: r.get(10)?,
        raised_at: r.get(11)?,
        open: r.get::<_, String>(12)? == "open",
        resolution: r.get(13)?,
        resolved_at: r.get(14)?,
        resolved_by: r.get(15)?,
    })
}

/// One Conflict.
pub(crate) fn get(conn: &Connection, id: &str) -> Result<Option<Stored>, SyncError> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM sync_conflicts WHERE id = ?1"),
            [id],
            read,
        )
        .optional()?)
}

/// Every Conflict that waits, newest first.
pub(crate) fn open(conn: &Connection) -> Result<Vec<Stored>, SyncError> {
    settle_all_deleted(conn)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM sync_conflicts WHERE status = 'open' ORDER BY raised_at DESC, id"
    ))?;
    Ok(stmt.query_map([], read)?.collect::<Result<_, _>>()?)
}

/// How many Conflicts wait.
pub fn open_count(conn: &Connection) -> Result<u32, SyncError> {
    settle_all_deleted(conn)?;
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM sync_conflicts WHERE status = 'open'",
        [],
        |r| r.get(0),
    )?)
}

/// The id of a clash: the same on every device for the same record, field, base and pair of
/// versions, whichever side each device calls its own.
pub fn conflict_id(
    entity: &str,
    entity_id: &str,
    field: &str,
    base: &str,
    a: &str,
    b: &str,
) -> String {
    let hash = |t: &str| {
        ring::digest::digest(&ring::digest::SHA256, t.as_bytes())
            .as_ref()
            .iter()
            .fold(String::new(), |mut acc, b| {
                use std::fmt::Write;
                let _ = write!(acc, "{b:02x}");
                acc
            })
    };
    let mut versions = [hash(a), hash(b)];
    versions.sort();
    let name = format!(
        "{entity}\0{entity_id}\0{field}\0{}\0{}\0{}",
        hash(base),
        versions[0],
        versions[1]
    );
    Uuid::new_v5(&NAMESPACE, name.as_bytes()).to_string()
}

fn json(row: &Stored) -> Value {
    let mut m = Map::new();
    let mut put = |k: &str, v: Value| {
        m.insert(k.to_owned(), v);
    };
    put("target", row.target.clone().into());
    put("target_id", row.target_id.clone().into());
    put("field", row.field.clone().into());
    put("base", row.base.clone().into());
    put("a_text", row.a_text.clone().into());
    put("a_device", row.a_device.clone().into());
    put("a_at", row.a_at.into());
    put("b_text", row.b_text.clone().into());
    put("b_device", row.b_device.clone().into());
    put("b_at", row.b_at.into());
    put("raised_at", row.raised_at.into());
    if !row.open {
        put("status", "resolved".into());
        put("resolution", row.resolution.clone().into());
        put("resolved_at", row.resolved_at.into());
        put("resolved_by", row.resolved_by.clone().into());
    }
    Value::Object(m)
}

fn record(conn: &Connection, row: &Stored, op: ChangeOp) -> Result<(), SyncError> {
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

/// Whether the record a Conflict is about is deleted, by the newest state this device holds.
pub(crate) fn target_deleted(
    conn: &Connection,
    target: &str,
    target_id: &str,
) -> Result<bool, SyncError> {
    let snapshot = crate::texts::current(conn, target, target_id)?;
    Ok(snapshot
        .and_then(|s| s.get("deleted_at").cloned())
        .is_some_and(|d| !d.is_null()))
}

/// Raises the Conflict for a clash found while merging `field` of a record, unless this device
/// already has it. The row is recorded as a change so it reaches the other devices. A clash on
/// a record that is deleted is resolved at once (and says so).
pub(crate) fn raise(
    conn: &Connection,
    target: &str,
    target_id: &str,
    field: &str,
    clash: &Clashed,
) -> Result<Option<SyncEvent>, SyncError> {
    let id = conflict_id(
        target,
        target_id,
        field,
        &clash.base,
        &clash.ours,
        &clash.theirs,
    );
    if get(conn, &id)?.is_some() {
        return Ok(None);
    }
    let deleted = target_deleted(conn, target, target_id)?;
    let row = Stored {
        id: id.clone(),
        target: target.to_owned(),
        target_id: target_id.to_owned(),
        field: field.to_owned(),
        base: clash.base.clone(),
        a_text: clash.theirs.clone(),
        a_device: clash.theirs_device.clone(),
        a_at: clash.theirs_at,
        b_text: clash.ours.clone(),
        b_device: clash.ours_device.clone(),
        b_at: clash.ours_at,
        raised_at: clash.theirs_at,
        open: !deleted,
        resolution: deleted.then(|| Resolution::Deleted.as_str().to_owned()),
        resolved_at: None,
        resolved_by: None,
    };
    insert(conn, &row)?;
    record(conn, &row, ChangeOp::Create)?;
    if deleted {
        crate::resolutions::note_conflict_deleted(conn, &id)?;
    } else {
        // The device across from the one that detected it: the author of the other version.
        let me = mnemora_core::hlc::device_id(conn)?;
        let other = if clash.theirs_device == me {
            &clash.ours_device
        } else {
            &clash.theirs_device
        };
        crate::activity::log_conflict_raised(conn, &id, target, field, row.raised_at, other)?;
    }
    tracing::debug!(target: "sync", auto = deleted, "a text clash was recorded");
    let (entity, entity_id, field) = (target.to_owned(), target_id.to_owned(), field.to_owned());
    Ok(Some(if deleted {
        SyncEvent::ConflictAutoResolved {
            id,
            entity,
            entity_id,
            field,
        }
    } else {
        SyncEvent::ConflictRaised {
            id,
            entity,
            entity_id,
            field,
        }
    }))
}

fn insert(conn: &Connection, row: &Stored) -> Result<(), SyncError> {
    conn.execute(
        "INSERT OR IGNORE INTO sync_conflicts(id, target, target_id, field, base, a_text, a_device,
                                              a_at, b_text, b_device, b_at, raised_at, status,
                                              resolution, resolved_at, resolved_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            row.id,
            row.target,
            row.target_id,
            row.field,
            row.base,
            row.a_text,
            row.a_device,
            row.a_at,
            row.b_text,
            row.b_device,
            row.b_at,
            row.raised_at,
            if row.open { "open" } else { "resolved" },
            row.resolution,
            row.resolved_at,
            row.resolved_by,
        ],
    )?;
    Ok(())
}

/// Ends a Conflict, as a change that reaches the other devices. Nothing happens to one that is
/// already resolved.
pub(crate) fn resolve(
    conn: &Connection,
    id: &str,
    resolution: Resolution,
    by: &str,
    now: i64,
) -> Result<bool, SyncError> {
    let changed = conn.execute(
        "UPDATE sync_conflicts SET status = 'resolved', resolution = ?2, resolved_at = ?3,
                                   resolved_by = ?4
         WHERE id = ?1 AND status = 'open'",
        params![id, resolution.as_str(), now, by],
    )?;
    if changed == 0 {
        return Ok(false);
    }
    if let Some(row) = get(conn, id)? {
        record(conn, &row, ChangeOp::Update)?;
        // A person's choice is Activity; a closing the engine did on its own is not.
        if matches!(
            resolution,
            Resolution::KeptThis | Resolution::KeptOther | Resolution::Merged
        ) {
            crate::activity::log_conflict_resolved(conn, id, &row.target, &row.field, by, now)?;
        }
    }
    Ok(true)
}

/// Settles every waiting Conflict whose record is deleted. A delete made on this device never
/// passes through the receiving side (`settle_deleted` after a record), so what lists or counts
/// the Conflicts that wait looks for these first: a record deleted after its clash was raised
/// leaves "Needs you" on every path, not only when the delete arrives from another device.
pub(crate) fn settle_all_deleted(conn: &Connection) -> Result<(), SyncError> {
    let targets: Vec<(String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT target, target_id FROM sync_conflicts WHERE status = 'open'",
        )?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    for (target, target_id) in targets {
        settle_deleted(conn, &target, &target_id)?;
    }
    Ok(())
}

/// Resolves, without asking, every Conflict that waits on a record that has since been deleted.
pub(crate) fn settle_deleted(
    conn: &Connection,
    target: &str,
    target_id: &str,
) -> Result<Vec<SyncEvent>, SyncError> {
    let waiting: Vec<(String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, field FROM sync_conflicts
             WHERE target = ?1 AND target_id = ?2 AND status = 'open'",
        )?;
        stmt.query_map([target, target_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    if waiting.is_empty() || !target_deleted(conn, target, target_id)? {
        return Ok(Vec::new());
    }
    let mut events = Vec::new();
    for (id, field) in waiting {
        if resolve(conn, &id, Resolution::Deleted, "", 0)? {
            crate::resolutions::note_conflict_deleted(conn, &id)?;
            crate::activity::log_conflict_deleted(conn, &id, target, &field)?;
            events.push(SyncEvent::ConflictAutoResolved {
                id,
                entity: target.to_owned(),
                entity_id: target_id.to_owned(),
                field,
            });
        }
    }
    Ok(events)
}

/// Closes the Conflicts about a text that the replay of all its records no longer produces
/// (`keep` are the ones it does). The text was merged from fewer records when they were raised.
pub(crate) fn supersede(
    conn: &Connection,
    target: &str,
    target_id: &str,
    field: &str,
    keep: &[String],
) -> Result<(), SyncError> {
    let waiting: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM sync_conflicts
             WHERE target = ?1 AND target_id = ?2 AND field = ?3 AND status = 'open'",
        )?;
        stmt.query_map([target, target_id, field], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    for id in waiting.into_iter().filter(|id| !keep.contains(id)) {
        resolve(conn, &id, Resolution::Superseded, "", 0)?;
    }
    Ok(())
}

/// Takes the values of a Conflict record received from another device (Sync's side of the
/// Module contract). The row is created from a record that has all of it; the outcome only ever
/// moves from open to resolved.
pub(crate) fn apply(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let id = change.entity_id;
    let f = change.fields;
    let text = |k: &str| f.get(k).and_then(Value::as_str).map(str::to_owned);
    let int = |k: &str| f.get(k).and_then(Value::as_i64);
    let before = get(conn, id).map_err(into_app)?;
    let mut events = Vec::new();
    if before.is_none() {
        let made = (|| {
            Some(Stored {
                id: id.to_owned(),
                target: text("target")?,
                target_id: text("target_id")?,
                field: text("field")?,
                base: text("base")?,
                a_text: text("a_text")?,
                a_device: text("a_device")?,
                a_at: int("a_at")?,
                b_text: text("b_text")?,
                b_device: text("b_device")?,
                b_at: int("b_at")?,
                raised_at: int("raised_at")?,
                open: true,
                resolution: None,
                resolved_at: None,
                resolved_by: None,
            })
        })();
        let Some(row) = made else {
            tracing::debug!(target: "sync", "a conflict record without its creation, kept for relay only");
            return Ok(Applied {
                snapshot: Value::Object(f.clone()),
                changed: false,
                events: Vec::new(),
            });
        };
        insert(conn, &row).map_err(into_app)?;
        // Learned from the device that raised it: it waits here too.
        if f.get("status").and_then(Value::as_str) != Some("resolved") {
            events.push(SyncEvent::ConflictRaised {
                id: id.to_owned(),
                entity: row.target.clone(),
                entity_id: row.target_id.clone(),
                field: row.field.clone(),
            });
        }
    }
    if f.get("status").and_then(Value::as_str) == Some("resolved") {
        conn.execute(
            "UPDATE sync_conflicts SET status = 'resolved', resolution = ?2, resolved_at = ?3,
                                       resolved_by = ?4
             WHERE id = ?1",
            params![
                id,
                text("resolution"),
                int("resolved_at"),
                text("resolved_by")
            ],
        )?;
    }
    let after = get(conn, id).map_err(into_app)?;
    Ok(Applied {
        snapshot: after.as_ref().map_or(Value::Null, json),
        changed: before != after,
        events,
    })
}

fn into_app(e: SyncError) -> mnemora_core::error::AppError {
    e.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_id_is_the_same_whichever_side_a_device_calls_its_own() {
        let a = conflict_id("e", "1", "body", "base", "mine", "yours");
        assert_eq!(a, conflict_id("e", "1", "body", "base", "yours", "mine"));
        assert_ne!(a, conflict_id("e", "1", "body", "base", "mine", "other"));
        assert_ne!(a, conflict_id("e", "2", "body", "base", "mine", "yours"));
        assert_ne!(a, conflict_id("e", "1", "notes", "base", "mine", "yours"));
        assert_ne!(a, conflict_id("e", "1", "body", "other", "mine", "yours"));
        assert!(Uuid::parse_str(&a).is_ok());
    }
}
