//! Activity: the log of what Sync did, kept forever and synced like any other record.
//!
//! A row is `sync_activity` and travels as the change-log entity `sync_activity`; it is written
//! once and never changes. Each event is logged by ONE device, or has an id both devices derive
//! alike, so it appears once on every device:
//!
//! * Events two devices both live through (a session, pairing, a pause, a lost connection) are
//!   logged by the one with the lower device id (`leads`). A session counts what moved, from the
//!   logger's side; the viewer turns "in" and "out" round when the other side is looking.
//! * Events of one device (renaming, forgetting) are logged by it.
//! * Events with a name of their own (a Conflict raised or resolved, a List merge, a member who
//!   joined) have an id derived from that name, so devices that notice the same thing agree.
//!
//! Records of this entity never count as "moved" in a session: a sync that only passed Activity
//! on is quiet, which also ends the chain (logging a session makes records to pass on).
//!
//! Never log content: a row holds device names, counts and Module/entity kinds. The one
//! exception is List names in a merge, which the log shows in plain words.

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::db::physical_now;
use mnemora_core::error::AppResult;
use mnemora_core::hlc;
use mnemora_core::syncable::{Applied, RemoteChange, SyncEntity, SyncEvent, SyncField};
use mnemora_core::util::new_id;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::error::SyncError;

/// The change-log entity name of an Activity row.
pub const ENTITY: &str = "sync_activity";

/// All fixed at creation: nothing about an event changes afterwards.
pub(crate) const ACTIVITY: SyncEntity = SyncEntity {
    name: ENTITY,
    fields: &[
        SyncField::value("kind"),
        SyncField::value("at"),
        SyncField::value("device"),
        SyncField::value("device_name"),
        SyncField::value("peer"),
        SyncField::value("peer_name"),
        SyncField::value("n_in"),
        SyncField::value("n_out"),
        SyncField::value("detail"),
        SyncField::value("extra"),
    ],
};

/// The namespace named ids are derived in (`UUIDv5`).
const NAMESPACE: Uuid = Uuid::from_u128(0x4d6e_656d_6f72_6120_6163_7469_7669_7479);

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A sync session that moved something.
    Session,
    /// Two devices paired.
    Paired,
    /// A device joined the group (this one learned of it).
    Joined,
    /// This device left the group.
    Left,
    /// A device was forgotten.
    Forgot,
    /// A device was renamed.
    Renamed,
    /// Sync with a device was paused.
    Paused,
    /// Sync with a device resumed.
    Resumed,
    /// A connection failed or was refused.
    Failed,
    /// A Conflict was raised.
    ConflictRaised,
    /// A Conflict was resolved by the person.
    ConflictResolved,
    /// Two Lists with one name became one.
    ListsMerged,
    /// The system started keeping this device off the local network.
    Blocked,
}

impl Kind {
    /// The value stored and synced.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Paired => "paired",
            Self::Joined => "joined",
            Self::Left => "left",
            Self::Forgot => "forgot",
            Self::Renamed => "renamed",
            Self::Paused => "paused",
            Self::Resumed => "resumed",
            Self::Failed => "failed",
            Self::ConflictRaised => "conflict_raised",
            Self::ConflictResolved => "conflict_resolved",
            Self::ListsMerged => "lists_merged",
            Self::Blocked => "blocked",
        }
    }

    /// The kind stored under `s`.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "session" => Self::Session,
            "paired" => Self::Paired,
            "joined" => Self::Joined,
            "left" => Self::Left,
            "forgot" => Self::Forgot,
            "renamed" => Self::Renamed,
            "paused" => Self::Paused,
            "resumed" => Self::Resumed,
            "failed" => Self::Failed,
            "conflict_raised" => Self::ConflictRaised,
            "conflict_resolved" => Self::ConflictResolved,
            "lists_merged" => Self::ListsMerged,
            "blocked" => Self::Blocked,
            _ => return None,
        })
    }
}

/// Why sync with a device stopped or a connection failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// The data versions differ: one of the devices needs an update.
    Update,
    /// The clocks are too far apart.
    Clock,
    /// The devices run in different Environments.
    Environment,
    /// The other device forgot this one.
    Revoked,
    /// The line was lost mid-sync.
    Lost,
}

impl Why {
    /// The value stored in a row's `extra`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Update => "update",
            Self::Clock => "clock",
            Self::Environment => "environment",
            Self::Revoked => "revoked",
            Self::Lost => "lost",
        }
    }

    pub(crate) fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "update" => Self::Update,
            "clock" => Self::Clock,
            "environment" => Self::Environment,
            "revoked" => Self::Revoked,
            "lost" => Self::Lost,
            _ => return None,
        })
    }
}

/// How many records of one kind a session moved (never which).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Part {
    /// The owning Module's id.
    pub m: String,
    /// The change-log entity.
    pub e: String,
    /// Records moved, in either direction.
    pub n: i64,
}

/// One row as stored.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Row {
    pub(crate) id: String,
    pub(crate) kind: String,
    /// When it happened (clock milliseconds).
    pub(crate) at: i64,
    /// The device that logged it, and the name it had then.
    pub(crate) device: String,
    pub(crate) device_name: String,
    /// The other device it is about, if any, and the name it had then.
    pub(crate) peer: String,
    pub(crate) peer_name: String,
    /// Records moved, from the logging device's side.
    pub(crate) n_in: i64,
    pub(crate) n_out: i64,
    /// A session's breakdown.
    pub(crate) detail: Vec<Part>,
    /// Kind-specific JSON (a reason, names, the kind of record a Conflict is about).
    pub(crate) extra: Value,
}

pub(crate) const COLUMNS: &str =
    "id, kind, at, device, device_name, peer, peer_name, n_in, n_out, detail, extra";

pub(crate) fn read(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    Ok(Row {
        id: r.get(0)?,
        kind: r.get(1)?,
        at: r.get(2)?,
        device: r.get(3)?,
        device_name: r.get(4)?,
        peer: r.get(5)?,
        peer_name: r.get(6)?,
        n_in: r.get(7)?,
        n_out: r.get(8)?,
        detail: serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or_default(),
        extra: serde_json::from_str(&r.get::<_, String>(10)?).unwrap_or(Value::Null),
    })
}

fn extra_text(extra: &Value) -> String {
    if extra.is_null() {
        String::new()
    } else {
        extra.to_string()
    }
}

fn json_of(row: &Row) -> Value {
    let mut m = Map::new();
    m.insert("kind".into(), row.kind.clone().into());
    m.insert("at".into(), row.at.into());
    m.insert("device".into(), row.device.clone().into());
    m.insert("device_name".into(), row.device_name.clone().into());
    m.insert("peer".into(), row.peer.clone().into());
    m.insert("peer_name".into(), row.peer_name.clone().into());
    m.insert("n_in".into(), row.n_in.into());
    m.insert("n_out".into(), row.n_out.into());
    m.insert(
        "detail".into(),
        serde_json::to_string(&row.detail)
            .unwrap_or_else(|_| "[]".into())
            .into(),
    );
    m.insert("extra".into(), extra_text(&row.extra).into());
    Value::Object(m)
}

fn insert(conn: &Connection, row: &Row) -> Result<bool, SyncError> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO sync_activity(id, kind, at, device, device_name, peer, peer_name,
                                             n_in, n_out, detail, extra)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            row.id,
            row.kind,
            row.at,
            row.device,
            row.device_name,
            row.peer,
            row.peer_name,
            row.n_in,
            row.n_out,
            serde_json::to_string(&row.detail).unwrap_or_else(|_| "[]".into()),
            extra_text(&row.extra),
        ],
    )?;
    Ok(n > 0)
}

/// Adds a row unless this device already has it, and records a new one as a change so it
/// reaches the other devices. Returns whether it was new.
pub(crate) fn log(conn: &Connection, row: &Row) -> Result<bool, SyncError> {
    if !insert(conn, row)? {
        return Ok(false);
    }
    changelog::record(
        conn,
        &Change {
            module: crate::MODULE_ID.to_owned(),
            entity: ENTITY.to_owned(),
            entity_id: row.id.clone(),
            op: ChangeOp::Create,
            session_id: None,
            snapshot: json_of(row),
        },
    )?;
    tracing::debug!(target: "sync", kind = %row.kind, "activity logged");
    Ok(true)
}

/// The id of an event that has a name of its own.
fn named_id(parts: &[&str]) -> String {
    Uuid::new_v5(&NAMESPACE, parts.join("\0").as_bytes()).to_string()
}

/// Whether `me` is the device that logs what it shares with `peer`.
pub(crate) fn leads(me: &str, peer: &str) -> bool {
    me < peer
}

/// This device, as it signs what it logs.
#[derive(Debug, Clone, Copy)]
pub struct Local<'a> {
    /// Its id.
    pub device: &'a str,
    /// Its name; empty when not known to the caller.
    pub name: &'a str,
}

fn blank(conn: &Connection, id: String, kind: Kind, local: Local<'_>) -> Row {
    Row {
        id,
        kind: kind.as_str().to_owned(),
        at: physical_now(conn),
        device: local.device.to_owned(),
        device_name: local.name.to_owned(),
        peer: String::new(),
        peer_name: String::new(),
        n_in: 0,
        n_out: 0,
        detail: Vec::new(),
        extra: Value::Null,
    }
}

fn with_peer(conn: &Connection, mut row: Row, peer: &str, name: Option<&str>) -> Row {
    peer.clone_into(&mut row.peer);
    row.peer_name = match name {
        Some(n) if !n.is_empty() => n.to_owned(),
        _ => crate::peers::name_of(conn, peer)
            .ok()
            .flatten()
            .unwrap_or_default(),
    };
    row
}

/// A finished session with `peer`: `parts` is what moved, by Module and kind, `n_in` and `n_out`
/// its totals from this device's side. Only the leading device logs it, and only when something
/// moved. Returns whether a row was made.
pub(crate) fn log_session(
    conn: &Connection,
    local: Local<'_>,
    peer: &str,
    n_in: i64,
    n_out: i64,
    parts: Vec<Part>,
) -> Result<bool, SyncError> {
    if !leads(local.device, peer) || n_in + n_out == 0 {
        return Ok(false);
    }
    let mut row = with_peer(
        conn,
        blank(conn, new_id(), Kind::Session, local),
        peer,
        None,
    );
    row.n_in = n_in;
    row.n_out = n_out;
    row.detail = parts;
    log(conn, &row)
}

/// Two devices paired. Logged by the lower id of the two.
pub(crate) fn log_paired(
    conn: &Connection,
    local: Local<'_>,
    peer: &str,
    peer_name: &str,
) -> Result<bool, SyncError> {
    if !leads(local.device, peer) {
        return Ok(false);
    }
    let row = with_peer(
        conn,
        blank(conn, new_id(), Kind::Paired, local),
        peer,
        Some(peer_name),
    );
    log(conn, &row)
}

/// A device joined the group and this device learned of it (the same event on every device
/// that learns of it has one id).
pub(crate) fn log_joined(
    conn: &Connection,
    local: Local<'_>,
    member: &str,
    member_name: &str,
) -> Result<bool, SyncError> {
    let row = with_peer(
        conn,
        blank(conn, named_id(&["joined", member]), Kind::Joined, local),
        member,
        Some(member_name),
    );
    log(conn, &row)
}

/// This device left the group (forgot every device).
pub fn log_left(conn: &Connection, local: Local<'_>) -> Result<bool, SyncError> {
    log(conn, &blank(conn, new_id(), Kind::Left, local))
}

/// This device forgot `peer`. Called by "Forget" (#69).
pub fn log_forgot(
    conn: &Connection,
    local: Local<'_>,
    peer: &str,
    peer_name: &str,
) -> Result<bool, SyncError> {
    let row = with_peer(
        conn,
        blank(conn, new_id(), Kind::Forgot, local),
        peer,
        Some(peer_name),
    );
    log(conn, &row)
}

/// This device was renamed from `old` to `new`.
pub fn log_renamed(
    conn: &Connection,
    device: &str,
    old: &str,
    new: &str,
) -> Result<bool, SyncError> {
    let mut row = blank(conn, new_id(), Kind::Renamed, Local { device, name: new });
    row.extra = json!({ "old": old, "new": new });
    log(conn, &row)
}

/// Whether the newest pause or resume this device logged about `peer` is a pause. Read from the
/// log rather than kept in memory, so a restart does not log the same pause again.
fn paused_now(conn: &Connection, me: &str, peer: &str) -> Result<bool, SyncError> {
    let newest: Option<String> = conn
        .query_row(
            "SELECT kind FROM sync_activity WHERE kind IN ('paused', 'resumed')
               AND device = ?1 AND peer = ?2 ORDER BY rowid DESC LIMIT 1",
            params![me, peer],
            |r| r.get(0),
        )
        .optional()?;
    Ok(newest.as_deref() == Some("paused"))
}

/// Sync with `peer` was paused for `why`. Logged by the lower id of the two, and once per pause
/// however often it is met again. Called by the session manager and by the pause banners (#72).
pub fn log_paused(
    conn: &Connection,
    local: Local<'_>,
    peer: &str,
    why: Why,
) -> Result<bool, SyncError> {
    if !leads(local.device, peer) || paused_now(conn, local.device, peer)? {
        return Ok(false);
    }
    let mut row = with_peer(conn, blank(conn, new_id(), Kind::Paused, local), peer, None);
    row.extra = json!({ "why": why.as_str() });
    log(conn, &row)
}

/// Sync with `peer` resumed after a pause (nothing is logged when it was not paused).
pub fn log_resumed(conn: &Connection, local: Local<'_>, peer: &str) -> Result<bool, SyncError> {
    if !leads(local.device, peer) || !paused_now(conn, local.device, peer)? {
        return Ok(false);
    }
    let row = with_peer(
        conn,
        blank(conn, new_id(), Kind::Resumed, local),
        peer,
        None,
    );
    log(conn, &row)
}

/// A connection with `peer` failed for `why`. Logged by the lower id of the two, and not again
/// within ten minutes of the last such line for the same device (a flaky link is one line).
pub fn log_failed(
    conn: &Connection,
    local: Local<'_>,
    peer: &str,
    why: Why,
) -> Result<bool, SyncError> {
    const QUIET_MS: i64 = 600_000;
    if !leads(local.device, peer) {
        return Ok(false);
    }
    let now = physical_now(conn);
    let recent: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_activity WHERE kind = 'failed' AND peer = ?1
                        AND device = ?2 AND at > ?3)",
        params![peer, local.device, now - QUIET_MS],
        |r| r.get(0),
    )?;
    if recent {
        return Ok(false);
    }
    let mut row = with_peer(conn, blank(conn, new_id(), Kind::Failed, local), peer, None);
    row.extra = json!({ "why": why.as_str() });
    log(conn, &row)
}

/// The system started keeping this device off the local network (macOS Local Network privacy,
/// Android's local network permission). One line when it starts: not again within ten minutes
/// of the last, so a permission that flickers is one line.
pub fn log_local_network_blocked(conn: &Connection, local: Local<'_>) -> Result<bool, SyncError> {
    const QUIET_MS: i64 = 600_000;
    let now = physical_now(conn);
    let recent: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_activity WHERE kind = 'blocked' AND device = ?1
                        AND at > ?2)",
        params![local.device, now - QUIET_MS],
        |r| r.get(0),
    )?;
    if recent {
        return Ok(false);
    }
    log(conn, &blank(conn, new_id(), Kind::Blocked, local))
}

/// A Conflict was raised here about `field` of an `entity` record. One row per Conflict, whichever
/// device notices it first.
pub(crate) fn log_conflict_raised(
    conn: &Connection,
    conflict: &str,
    entity: &str,
    field: &str,
    raised_at: i64,
    other_device: &str,
) -> Result<bool, SyncError> {
    let me = hlc::device_id(conn)?;
    let mut row = blank(
        conn,
        named_id(&["conflict_raised", conflict]),
        Kind::ConflictRaised,
        Local {
            device: &me,
            name: "",
        },
    );
    row.at = raised_at;
    if other_device != me {
        row = with_peer(conn, row, other_device, None);
    }
    row.extra = json!({ "entity": entity, "field": field, "conflict": conflict });
    log(conn, &row)
}

/// The person resolved a Conflict.
pub(crate) fn log_conflict_resolved(
    conn: &Connection,
    conflict: &str,
    entity: &str,
    field: &str,
    by: &str,
    at: i64,
) -> Result<bool, SyncError> {
    let mut row = blank(
        conn,
        named_id(&["conflict_resolved", conflict]),
        Kind::ConflictResolved,
        Local {
            device: by,
            name: "",
        },
    );
    row.at = at;
    row.extra = json!({ "entity": entity, "field": field });
    log(conn, &row)
}

/// Two Lists with one name became one. `other_device` made the List that went away (or the one
/// that stayed), when it was not this one.
pub(crate) fn log_lists_merged(
    conn: &Connection,
    from: (&str, &str),
    into: (&str, &str),
    other_device: Option<&str>,
) -> Result<bool, SyncError> {
    let me = hlc::device_id(conn)?;
    let mut row = blank(
        conn,
        named_id(&["lists_merged", from.0, into.0]),
        Kind::ListsMerged,
        Local {
            device: &me,
            name: "",
        },
    );
    if let Some(other) = other_device.filter(|d| *d != me) {
        row = with_peer(conn, row, other, None);
    }
    row.extra = json!({ "from": from.1, "into": into.1 });
    log(conn, &row)
}

/// What the Modules reported while a batch was applied, turned into Activity (same transaction).
pub(crate) fn note_events(conn: &Connection, events: &[SyncEvent]) -> Result<(), SyncError> {
    for event in events {
        if let SyncEvent::ListsMerged {
            from_id,
            from_name,
            from_device,
            into_id,
            into_name,
            into_device,
        } = event
        {
            let other = [into_device, from_device]
                .into_iter()
                .flatten()
                .map(String::as_str)
                .next();
            log_lists_merged(conn, (from_id, from_name), (into_id, into_name), other)?;
        }
    }
    Ok(())
}

/// Takes an Activity record received from another device (Sync's side of the Module contract).
/// Rows never change, so only a record that has all of a row is taken, and only once.
pub(crate) fn apply(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let f = change.fields;
    let text = |k: &str| f.get(k).and_then(Value::as_str).map(str::to_owned);
    let int = |k: &str| f.get(k).and_then(Value::as_i64);
    let made = (|| {
        let kind = text("kind").filter(|k| Kind::parse(k).is_some())?;
        Some(Row {
            id: change.entity_id.to_owned(),
            kind,
            at: int("at")?,
            device: text("device")?,
            device_name: text("device_name")?,
            peer: text("peer")?,
            peer_name: text("peer_name")?,
            n_in: int("n_in")?,
            n_out: int("n_out")?,
            detail: serde_json::from_str(&text("detail")?).ok()?,
            extra: serde_json::from_str(&text("extra")?).unwrap_or(Value::Null),
        })
    })();
    let Some(row) = made else {
        tracing::debug!(target: "sync", "an activity record without its creation, kept for relay only");
        return Ok(Applied {
            snapshot: Value::Object(f.clone()),
            changed: false,
            events: Vec::new(),
        });
    };
    let new = insert(conn, &row).map_err(mnemora_core::error::AppError::from)?;
    Ok(Applied {
        snapshot: json_of(&row),
        changed: new,
        events: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_one_of_two_devices_leads() {
        assert_ne!(leads("a", "b"), leads("b", "a"));
    }

    #[test]
    fn named_ids_agree_and_differ_by_name() {
        assert_eq!(named_id(&["x", "1"]), named_id(&["x", "1"]));
        assert_ne!(named_id(&["x", "1"]), named_id(&["x", "2"]));
    }

    #[test]
    fn every_kind_and_reason_round_trips() {
        for k in [
            Kind::Session,
            Kind::Paired,
            Kind::Joined,
            Kind::Left,
            Kind::Forgot,
            Kind::Renamed,
            Kind::Paused,
            Kind::Resumed,
            Kind::Failed,
            Kind::ConflictRaised,
            Kind::ConflictResolved,
            Kind::ListsMerged,
            Kind::Blocked,
        ] {
            assert_eq!(Kind::parse(k.as_str()), Some(k));
        }
        for w in [
            Why::Update,
            Why::Clock,
            Why::Environment,
            Why::Revoked,
            Why::Lost,
        ] {
            assert_eq!(Why::parse(w.as_str()), Some(w));
        }
    }
}
