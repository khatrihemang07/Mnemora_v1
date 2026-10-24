//! Forget: a member of the group decides that a device is out, and says so with a signature.
//!
//! The decision is a row of `sync_revocations` and travels as the change-log entity
//! `sync_revocation` (id: the forgotten device), like any other synced record. It names the
//! device, its key, its name and kind, and is signed with the key of the member that made it
//! (`by`), over all of that. A receiver takes it only if the signature checks out against the key
//! it holds for `by` (a paired device): a record from anybody else, or altered on the way, is
//! kept for relay but changes nothing. When it is taken the device is dropped from the paired
//! devices and its key unpinned, so it can neither connect nor be dialled; whoever holds an open
//! connection to it closes it.
//!
//! Pairing again undoes it: the member that pairs the device writes a newer decision with state
//! `lifted`, signed in the same way, and the others take the device back. Decisions are ordered by
//! `at` (the newest wins), which the signature covers, so an old "revoked" replayed later does not
//! undo the "lifted" that followed it. There is no remote wipe: the device keeps its data.

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::db::physical_now;
use mnemora_core::error::AppResult;
use mnemora_core::hlc;
use mnemora_core::syncable::{Applied, RemoteChange, SyncEntity, SyncField};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};

use crate::error::SyncError;
use crate::net::{DeviceIdentity, PublicKey};
use crate::pair::Platform;

/// The change-log entity name of a decision.
pub const ENTITY: &str = "sync_revocation";

/// The members of one decision never travel apart: a receiver takes the whole decision or none.
const DECISION: &str = "decision";

pub(crate) const REVOCATION: SyncEntity = SyncEntity {
    name: ENTITY,
    fields: &[
        SyncField::value("public_key").in_group(DECISION),
        SyncField::value("name").in_group(DECISION),
        SyncField::value("platform").in_group(DECISION),
        SyncField::value("state").in_group(DECISION),
        SyncField::value("at").in_group(DECISION),
        SyncField::value("by").in_group(DECISION),
        SyncField::value("signature").in_group(DECISION),
    ],
};

const REVOKED: &str = "revoked";
const LIFTED: &str = "lifted";

/// One decision as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    device: String,
    public_key: String,
    name: String,
    platform: String,
    state: String,
    at: i64,
    by: String,
    signature: String,
}

impl Row {
    /// What the signature covers: every field, so none can be swapped.
    fn message(&self) -> Vec<u8> {
        [
            "mnemora-revocation-v1",
            &self.device,
            &self.public_key,
            &self.name,
            &self.platform,
            &self.state,
            &self.at.to_string(),
            &self.by,
        ]
        .join("\0")
        .into_bytes()
    }

    fn signed(mut self, identity: &DeviceIdentity) -> Result<Self, SyncError> {
        let signature = identity.sign(&self.message())?;
        self.signature = hex(&signature);
        Ok(self)
    }

    /// Whether `key` made this decision.
    fn is_signed_by(&self, key: &PublicKey) -> bool {
        from_hex(&self.signature).is_some_and(|sig| key.verify(&self.message(), &sig))
    }

    fn json(&self) -> Value {
        let mut m = Map::new();
        m.insert("public_key".into(), self.public_key.clone().into());
        m.insert("name".into(), self.name.clone().into());
        m.insert("platform".into(), self.platform.clone().into());
        m.insert("state".into(), self.state.clone().into());
        m.insert("at".into(), self.at.into());
        m.insert("by".into(), self.by.clone().into());
        m.insert("signature".into(), self.signature.clone().into());
        Value::Object(m)
    }

    /// A decision from the fields of a record, if they are all there and well formed.
    fn from_fields(device: &str, f: &Map<String, Value>) -> Option<Self> {
        let text = |k: &str| f.get(k).and_then(Value::as_str).map(str::to_owned);
        let state = text("state").filter(|s| s == REVOKED || s == LIFTED)?;
        PublicKey::from_hex(&text("public_key")?)?;
        Some(Self {
            device: device.to_owned(),
            public_key: text("public_key")?,
            name: text("name")?,
            platform: text("platform")?,
            state,
            at: f.get("at").and_then(Value::as_i64)?,
            by: text("by")?,
            signature: text("signature")?,
        })
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

pub(crate) fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

fn read(conn: &Connection, device: &str) -> rusqlite::Result<Option<Row>> {
    conn.query_row(
        "SELECT device, public_key, name, platform, state, at, by_device, signature
         FROM sync_revocations WHERE device = ?1",
        [device],
        |r| {
            Ok(Row {
                device: r.get(0)?,
                public_key: r.get(1)?,
                name: r.get(2)?,
                platform: r.get(3)?,
                state: r.get(4)?,
                at: r.get(5)?,
                by: r.get(6)?,
                signature: r.get(7)?,
            })
        },
    )
    .optional()
}

fn store(conn: &Connection, row: &Row) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO sync_revocations(device, public_key, name, platform, state, at, by_device, signature)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(device) DO UPDATE SET public_key = ?2, name = ?3, platform = ?4, state = ?5,
             at = ?6, by_device = ?7, signature = ?8",
        params![
            row.device,
            row.public_key,
            row.name,
            row.platform,
            row.state,
            row.at,
            row.by,
            row.signature
        ],
    )?;
    Ok(())
}

/// Records a decision of this device so it reaches the others.
fn record(conn: &Connection, row: &Row, existed: bool) -> AppResult<()> {
    changelog::record(
        conn,
        &Change {
            module: crate::MODULE_ID.to_owned(),
            entity: ENTITY.to_owned(),
            entity_id: row.device.clone(),
            op: if existed {
                ChangeOp::Update
            } else {
                ChangeOp::Create
            },
            session_id: None,
            snapshot: row.json(),
        },
    )
}

/// The time of a new decision: now, but always after the one it replaces.
fn next_at(conn: &Connection, previous: Option<&Row>) -> i64 {
    let now = physical_now(conn);
    previous.map_or(now, |p| now.max(p.at.saturating_add(1)))
}

/// How far ahead of this device's clock a received decision may be dated. A member that dated one
/// years ahead would make it impossible to take the device back (nothing could be newer).
const MAX_AHEAD_MS: i64 = 24 * 60 * 60 * 1000;

/// Whether this device has been told it stands forgotten by the group: it syncs with nobody until
/// it is paired again.
pub fn removed_here(conn: &Connection) -> AppResult<bool> {
    let me = hlc::device_id(conn)?;
    Ok(is_revoked(conn, &me)?)
}

/// Whether `device` stands revoked here.
pub fn is_revoked(conn: &Connection, device: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_revocations WHERE device = ?1 AND state = 'revoked')",
        [device],
        |r| r.get(0),
    )
}

/// The keys that stand revoked, as `(key, device id)`: never to be trusted.
pub fn revoked_keys(conn: &Connection) -> AppResult<Vec<(PublicKey, String)>> {
    let mut stmt =
        conn.prepare("SELECT device, public_key FROM sync_revocations WHERE state = 'revoked'")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows
        .filter_map(Result::ok)
        .filter_map(|(device, key)| Some((PublicKey::from_hex(&key)?, device)))
        .collect())
}

/// Forgets nothing any more: the group was left, and so were its decisions.
pub fn clear(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM sync_revocations", [])?;
    Ok(())
}

/// What Forget did, for the caller to act on the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forgotten {
    /// The name the device had.
    pub name: String,
    /// Its key, now revoked; `None` for a device that was never given one.
    pub key: Option<PublicKey>,
}

/// Forgets a paired device for the whole group: signs a revocation with this device's key, records
/// it (so it syncs), drops the device from this device's list and logs it in Activity. One
/// transaction.
pub fn forget(
    conn: &mut Connection,
    identity: &DeviceIdentity,
    my_name: &str,
    device: &str,
) -> Result<Forgotten, SyncError> {
    let me = hlc::device_id(conn)?;
    let tx = conn.transaction()?;
    let peer: Option<(String, String, Option<String>)> = tx
        .query_row(
            "SELECT name, platform, public_key FROM sync_peers WHERE id = ?1",
            [device],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((name, platform, key)) = peer else {
        return Err(SyncError::Validation("That device is not paired"));
    };
    let parsed = key.as_deref().and_then(PublicKey::from_hex);
    if let (Some(key), true) = (&key, parsed.is_some()) {
        let previous = read(&tx, device)?;
        let row = Row {
            device: device.to_owned(),
            public_key: key.clone(),
            name: name.clone(),
            platform,
            state: REVOKED.to_owned(),
            at: next_at(&tx, previous.as_ref()),
            by: me.clone(),
            signature: String::new(),
        }
        .signed(identity)?;
        store(&tx, &row)?;
        record(&tx, &row, previous.is_some())?;
    }
    crate::activity::log_forgot(
        &tx,
        crate::activity::Local {
            device: &me,
            name: my_name,
        },
        device,
        &name,
    )?;
    remove_peer(&tx, device)?;
    tx.commit()?;
    tracing::info!(target: "sync", "forgot a paired device, revocation signed");
    Ok(Forgotten { name, key: parsed })
}

/// Leaving: this device tells the group it is out. It signs a revocation of itself (the others
/// trust its key, so they take it like any member's Forget), records it so the next exchange
/// carries it, and writes the "left" line the others then show. Unlike Forget nothing is kept
/// here: this device does not stand revoked to itself (that would stop its supervisor before the
/// notice left), and `forget_all` removes the group right after. Call it only when a member is
/// connected to hear it; the caller pushes, then forgets.
pub fn leave(
    conn: &mut Connection,
    identity: &DeviceIdentity,
    my_name: &str,
) -> Result<(), SyncError> {
    let me = hlc::device_id(conn)?;
    let tx = conn.transaction()?;
    let previous = read(&tx, &me)?;
    let row = Row {
        device: me.clone(),
        public_key: identity.public_key().to_hex(),
        name: my_name.to_owned(),
        platform: Platform::this().as_str().to_owned(),
        state: REVOKED.to_owned(),
        at: next_at(&tx, previous.as_ref()),
        by: me.clone(),
        signature: String::new(),
    }
    .signed(identity)?;
    record(&tx, &row, previous.is_some())?;
    crate::activity::log_left(
        &tx,
        crate::activity::Local {
            device: &me,
            name: my_name,
        },
    )?;
    tx.commit()?;
    tracing::info!(target: "sync", "left the group, the members are told");
    Ok(())
}

/// Pairing took `device` back: if it stood revoked, a newer decision says so, signed by this
/// device, and the others take it back too. Returns whether anything was lifted.
pub fn lift(conn: &Connection, identity: &DeviceIdentity, device: &str) -> Result<bool, SyncError> {
    let Some(previous) = read(conn, device)?.filter(|r| r.state == REVOKED) else {
        return Ok(false);
    };
    let me = hlc::device_id(conn)?;
    // The device as paired now: its key and name may be new.
    let now: Option<(String, String, String)> = conn
        .query_row(
            "SELECT name, platform, public_key FROM sync_peers WHERE id = ?1 AND public_key IS NOT NULL",
            [device],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (name, platform, key) = now.unwrap_or((
        previous.name.clone(),
        previous.platform.clone(),
        previous.public_key.clone(),
    ));
    let row = Row {
        device: device.to_owned(),
        public_key: key,
        name,
        platform,
        state: LIFTED.to_owned(),
        at: next_at(conn, Some(&previous)),
        by: me,
        signature: String::new(),
    }
    .signed(identity)?;
    store(conn, &row)?;
    record(conn, &row, true)?;
    tracing::info!(target: "sync", "a forgotten device was paired again, revocation lifted");
    Ok(true)
}

/// Drops a device from the paired devices and everything remembered about it.
fn remove_peer(conn: &Connection, device: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM sync_peers WHERE id = ?1", [device])?;
    conn.execute("DELETE FROM sync_peer_status WHERE device = ?1", [device])?;
    conn.execute(
        "DELETE FROM sync_peer_addresses WHERE device = ?1",
        [device],
    )?;
    Ok(())
}

/// Takes a decision received from another device (Sync's side of the Module contract). Taken only
/// if a paired device signed it and it is newer than what is held; otherwise the record is kept
/// for relay and nothing changes.
pub(crate) fn apply(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let nothing = |why: &str| {
        tracing::debug!(target: "sync", why, "a revocation record was not taken");
        Ok(Applied {
            snapshot: Value::Object(change.fields.clone()),
            changed: false,
            events: Vec::new(),
        })
    };
    let device = change.entity_id;
    let Some(row) = Row::from_fields(device, change.fields) else {
        return nothing("incomplete");
    };
    let me = hlc::device_id(conn)?;
    // A decision this device made comes back only as an echo of what it holds already.
    if row.by == me {
        return nothing("made here");
    }
    let signer: Option<String> = conn
        .query_row(
            "SELECT public_key FROM sync_peers WHERE id = ?1",
            [&row.by],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let Some(signer) = signer.as_deref().and_then(PublicKey::from_hex) else {
        return nothing("signer is not a paired device");
    };
    if !row.is_signed_by(&signer) {
        tracing::warn!(target: "sync", "a revocation with a bad signature was ignored");
        return nothing("bad signature");
    }
    if row.at > physical_now(conn).saturating_add(MAX_AHEAD_MS) {
        return nothing("dated in the future");
    }
    // A forgotten device has no say: what it signed before it was forgotten, and what reaches
    // here after, is not taken.
    if is_revoked(conn, &row.by)? {
        return nothing("signer stands forgotten");
    }
    let previous = read(conn, device)?;
    if previous.as_ref().is_some_and(|p| p.at >= row.at) {
        return nothing("not newer");
    }
    // Paired again since it was decided: the pairing is the newer word.
    if row.state == REVOKED && paired_since(conn, device, &row)? {
        return nothing("paired again since");
    }
    store(conn, &row)?;
    if device != me {
        take_effect(conn, &row)?;
    }
    Ok(Applied {
        snapshot: row.json(),
        changed: false,
        events: Vec::new(),
    })
}

/// Whether `row.device` was paired here (under the key the decision names) after the decision was
/// made, as when another member paired it again while this device had not heard of the Forget.
fn paired_since(conn: &Connection, device: &str, row: &Row) -> AppResult<bool> {
    let paired_at: Option<i64> = conn
        .query_row(
            "SELECT paired_at FROM sync_peers WHERE id = ?1 AND public_key = ?2",
            params![device, row.public_key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(paired_at.is_some_and(|at| at > row.at))
}

/// What a taken decision does to the paired devices.
fn take_effect(conn: &Connection, row: &Row) -> AppResult<()> {
    let device = &row.device;
    let held: Option<Option<String>> = conn
        .query_row(
            "SELECT public_key FROM sync_peers WHERE id = ?1",
            [device],
            |r| r.get(0),
        )
        .optional()?;
    match (row.state.as_str(), held) {
        (REVOKED, Some(key)) => {
            // Only the key that was revoked: a device paired again under another key stays.
            if key.is_none_or(|k| k == row.public_key) {
                remove_peer(conn, device)?;
                tracing::info!(target: "sync", "a device was forgotten by another member");
            }
        }
        (LIFTED, None) if crate::peers::group(conn)?.is_some() => {
            // This device's own name lives in the device file, out of reach here: a clash with it
            // is settled by the next rename.
            let taken: Vec<String> = {
                let mut stmt = conn.prepare("SELECT name FROM sync_peers")?;
                stmt.query_map([], |r| r.get::<_, String>(0))?
                    .collect::<Result<_, _>>()?
            };
            let name = crate::peers::unique_name(&row.name, &taken);
            conn.execute(
                "INSERT INTO sync_peers(id, name, paired_at, public_key, platform, paired_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    device,
                    name,
                    physical_now(conn),
                    row.public_key,
                    Platform::parse(&row.platform).as_str(),
                    row.by
                ],
            )?;
            tracing::info!(target: "sync", "a forgotten device was taken back");
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use mnemora_core::db;
    use mnemora_core::settings::SettingsRegistry;

    use super::*;
    use crate::SyncModule;

    fn conn() -> Connection {
        let mut conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        db::migrate::run(
            &mut conn,
            &[&mnemora_core::module::CoreModule, &SyncModule],
            dir.path(),
        )
        .unwrap();
        hlc::set_device_id(&conn, "me").unwrap();
        conn
    }

    fn pair(conn: &Connection, id: &str, name: &str, key: &DeviceIdentity) {
        conn.execute(
            "INSERT OR IGNORE INTO sync_group(one, id) VALUES (1, 'g')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sync_peers(id, name, paired_at, public_key, platform) VALUES (?1, ?2, 1, ?3, 'phone')",
            params![id, name, key.public_key().to_hex()],
        )
        .unwrap();
    }

    fn decision(device: &str, by: &str, signer: &DeviceIdentity, state: &str, at: i64) -> Row {
        Row {
            device: device.into(),
            public_key: DeviceIdentity::generate().unwrap().public_key().to_hex(),
            name: "Pixel 8".into(),
            platform: "phone".into(),
            state: state.into(),
            at,
            by: by.into(),
            signature: String::new(),
        }
        .signed(signer)
        .unwrap()
    }

    fn receive(conn: &Connection, row: &Row) {
        let fields = row.json();
        let registry = SettingsRegistry::default();
        let change = RemoteChange {
            entity: ENTITY,
            entity_id: &row.device,
            op: ChangeOp::Create,
            at: row.at,
            fields: fields.as_object().unwrap(),
            settings: &registry,
        };
        apply(conn, &change).unwrap();
    }

    fn paired(conn: &Connection, id: &str) -> bool {
        crate::peers::is_paired(conn, id).unwrap()
    }

    #[test]
    fn a_decision_signed_by_a_member_forgets_the_device_and_revokes_its_key() {
        let conn = conn();
        let (alice, carol) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "alice", "Alice", &alice);
        pair(&conn, "carol", "Carol", &carol);
        let mut row = decision("carol", "alice", &alice, "revoked", 10);
        row.public_key = carol.public_key().to_hex();
        let row = row.signed(&alice).unwrap();
        receive(&conn, &row);
        assert!(!paired(&conn, "carol"));
        assert!(paired(&conn, "alice"));
        assert!(is_revoked(&conn, "carol").unwrap());
        assert_eq!(
            revoked_keys(&conn).unwrap(),
            [(carol.public_key(), "carol".to_owned())]
        );
    }

    #[test]
    fn a_decision_from_anyone_who_is_not_a_member_changes_nothing() {
        let conn = conn();
        let (alice, mallory) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "alice", "Alice", &alice);
        // Mallory claims to be alice: the signature is checked against alice's key.
        receive(&conn, &decision("alice", "alice", &mallory, "revoked", 10));
        // Mallory signs as an unknown device.
        receive(
            &conn,
            &decision("alice", "mallory", &mallory, "revoked", 11),
        );
        assert!(paired(&conn, "alice"));
        assert!(revoked_keys(&conn).unwrap().is_empty());
    }

    #[test]
    fn a_decision_altered_after_signing_is_not_taken() {
        let conn = conn();
        let (alice, bob) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "alice", "Alice", &alice);
        pair(&conn, "bob", "Bob", &bob);
        let mut row = decision("carol", "alice", &alice, "revoked", 10);
        row.device = "bob".into();
        receive(&conn, &row);
        assert!(
            paired(&conn, "bob"),
            "signed for carol, replayed against bob"
        );
        let mut row = decision("bob", "alice", &alice, "revoked", 10);
        row.state = LIFTED.into();
        receive(&conn, &row);
        assert!(revoked_keys(&conn).unwrap().is_empty());
    }

    #[test]
    fn a_later_lift_is_not_undone_by_an_old_revocation_replayed() {
        let conn = conn();
        let alice = DeviceIdentity::generate().unwrap();
        pair(&conn, "alice", "Alice", &alice);
        let revoked = decision("carol", "alice", &alice, "revoked", 10);
        receive(&conn, &revoked);
        assert!(is_revoked(&conn, "carol").unwrap());
        let lifted = decision("carol", "alice", &alice, "lifted", 20);
        receive(&conn, &lifted);
        assert!(!is_revoked(&conn, "carol").unwrap());
        assert!(paired(&conn, "carol"), "taken back into the group");
        receive(&conn, &revoked);
        assert!(
            !is_revoked(&conn, "carol").unwrap(),
            "the old decision is stale"
        );
        assert!(paired(&conn, "carol"));
    }

    #[test]
    fn forgetting_signs_records_and_drops_the_device() {
        let mut conn = conn();
        let (me, carol) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "carol", "Carol", &carol);
        let done = forget(&mut conn, &me, "Mac", "carol").unwrap();
        assert_eq!(done.name, "Carol");
        assert_eq!(done.key, Some(carol.public_key()));
        assert!(!paired(&conn, "carol"));
        let row = read(&conn, "carol").unwrap().unwrap();
        assert!(row.is_signed_by(&me.public_key()));
        let recorded: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM change_log WHERE entity = ?1 AND entity_id = 'carol'",
                [ENTITY],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(recorded, 1);
        assert!(
            forget(&mut conn, &me, "Mac", "carol").is_err(),
            "not paired any more"
        );
    }

    #[test]
    fn pairing_again_lifts_a_revocation_with_a_newer_decision() {
        let mut conn = conn();
        let (me, carol) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "carol", "Carol", &carol);
        forget(&mut conn, &me, "Mac", "carol").unwrap();
        let before = read(&conn, "carol").unwrap().unwrap();
        assert!(!lift(&conn, &me, "dave").unwrap(), "nothing to lift");
        pair(&conn, "carol", "Carol", &carol);
        assert!(lift(&conn, &me, "carol").unwrap());
        let after = read(&conn, "carol").unwrap().unwrap();
        assert_eq!(after.state, LIFTED);
        assert!(after.at > before.at);
        assert!(after.is_signed_by(&me.public_key()));
        assert!(!is_revoked(&conn, "carol").unwrap());
        assert!(
            !lift(&conn, &me, "carol").unwrap(),
            "lifting twice does nothing"
        );
    }

    #[test]
    fn a_decision_dated_far_ahead_is_not_taken_so_it_cannot_block_pairing_again() {
        let conn = conn();
        let alice = DeviceIdentity::generate().unwrap();
        pair(&conn, "alice", "Alice", &alice);
        receive(
            &conn,
            &decision("carol", "alice", &alice, "revoked", i64::MAX),
        );
        assert!(!is_revoked(&conn, "carol").unwrap());
        let next = next_at(
            &conn,
            Some(&decision("carol", "alice", &alice, "revoked", i64::MAX)),
        );
        assert_eq!(next, i64::MAX, "no overflow");
    }

    #[test]
    fn a_decision_a_forgotten_device_signed_is_not_taken_even_when_it_arrives_late() {
        let conn = conn();
        let (alice, mallory) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "alice", "Alice", &alice);
        pair(&conn, "bob", "Bob", &DeviceIdentity::generate().unwrap());
        pair(&conn, "mallory", "Mallory", &mallory);
        let mut forgotten = decision("mallory", "alice", &alice, "revoked", 10);
        forgotten.public_key = mallory.public_key().to_hex();
        receive(&conn, &forgotten.signed(&alice).unwrap());
        assert!(!paired(&conn, "mallory"));
        // Mallory signed this before it was forgotten; it reaches this device afterwards.
        receive(&conn, &decision("bob", "mallory", &mallory, "revoked", 5));
        assert!(paired(&conn, "bob"));
        assert!(!is_revoked(&conn, "bob").unwrap());
        // Even if its row were back, a stood-forgotten signer counts for nothing.
        pair(&conn, "mallory", "Mallory", &mallory);
        receive(&conn, &decision("bob", "mallory", &mallory, "revoked", 6));
        assert!(paired(&conn, "bob"));
    }

    #[test]
    fn a_device_paired_again_after_a_decision_stays_when_the_old_decision_arrives() {
        let conn = conn();
        let (alice, carol) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "alice", "Alice", &alice);
        pair(&conn, "carol", "Carol", &carol);
        conn.execute(
            "UPDATE sync_peers SET paired_at = 500 WHERE id = 'carol'",
            [],
        )
        .unwrap();
        let mut old = decision("carol", "alice", &alice, "revoked", 100);
        old.public_key = carol.public_key().to_hex();
        receive(&conn, &old.signed(&alice).unwrap());
        assert!(paired(&conn, "carol"));
        assert!(revoked_keys(&conn).unwrap().is_empty());
    }

    #[test]
    fn a_device_told_it_was_forgotten_is_removed_until_it_is_paired_again() {
        let conn = conn();
        let (alice, me) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "alice", "Alice", &alice);
        assert!(!removed_here(&conn).unwrap());
        let mut row = decision("me", "alice", &alice, "revoked", 10);
        row.public_key = me.public_key().to_hex();
        receive(&conn, &row.signed(&alice).unwrap());
        assert!(removed_here(&conn).unwrap());
        assert!(paired(&conn, "alice"), "nothing is wiped");
        // Pairing again lifts the decision about this device too.
        assert!(lift(&conn, &me, "me").unwrap());
        assert!(!removed_here(&conn).unwrap());
    }
}
