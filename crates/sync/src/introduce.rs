//! Joining via one member joins the group: the member that paired a newcomer signs the newcomer's
//! key, and the members that were not there accept it.
//!
//! The introduction is a row of `sync_introductions` and travels as the change-log entity
//! `sync_introduction` (id: the newcomer), like any other synced record. It names the newcomer,
//! its key, name and kind, and is signed with the key of the introducing member (`by`) over all of
//! that. A receiver takes it only if the signature checks out against the key it holds for `by`
//! (a paired device that does not stand forgotten): a record signed by anybody else, or altered on
//! the way, is kept for relay but changes nothing. When it is taken the newcomer becomes a paired
//! device and its key is pinned, so the two can connect directly.
//!
//! Introductions and revocations (#69) race. A device that stands forgotten here is not taken in,
//! except when the introduction is newer than the Forget: that was a fresh pairing by a member
//! that had not heard of it, and the pairing is the newer word (as in `revoke::paired_since`).

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
use crate::revoke::{from_hex, hex};

/// The change-log entity name of an introduction.
pub const ENTITY: &str = "sync_introduction";

/// The members of one introduction never travel apart: a receiver takes the whole or none.
const INTRO: &str = "introduction";

pub(crate) const INTRODUCTION: SyncEntity = SyncEntity {
    name: ENTITY,
    fields: &[
        SyncField::value("public_key").in_group(INTRO),
        SyncField::value("name").in_group(INTRO),
        SyncField::value("platform").in_group(INTRO),
        SyncField::value("at").in_group(INTRO),
        SyncField::value("direct").in_group(INTRO),
        SyncField::value("by").in_group(INTRO),
        SyncField::value("signature").in_group(INTRO),
    ],
};

/// How far ahead of this device's clock a received introduction may be dated: one dated years
/// ahead would outrank every later Forget of the device.
const MAX_AHEAD_MS: i64 = 24 * 60 * 60 * 1000;

/// One introduction as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    device: String,
    public_key: String,
    name: String,
    platform: String,
    at: i64,
    /// The signer paired this device itself (a real pairing), rather than passing on a member of
    /// the group the paired device brought. Only the former may undo a Forget.
    direct: bool,
    by: String,
    signature: String,
}

impl Row {
    /// What the signature covers: every field, so none can be swapped.
    fn message(&self) -> Vec<u8> {
        [
            "mnemora-introduction-v2",
            &self.device,
            &self.public_key,
            &self.name,
            &self.platform,
            &self.at.to_string(),
            if self.direct { "direct" } else { "relayed" },
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

    /// Whether `key` made this introduction.
    fn is_signed_by(&self, key: &PublicKey) -> bool {
        from_hex(&self.signature).is_some_and(|sig| key.verify(&self.message(), &sig))
    }

    fn json(&self) -> Value {
        let mut m = Map::new();
        m.insert("public_key".into(), self.public_key.clone().into());
        m.insert("name".into(), self.name.clone().into());
        m.insert("platform".into(), self.platform.clone().into());
        m.insert("at".into(), self.at.into());
        m.insert("direct".into(), self.direct.into());
        m.insert("by".into(), self.by.clone().into());
        m.insert("signature".into(), self.signature.clone().into());
        Value::Object(m)
    }

    /// An introduction from the fields of a record, if they are all there and well formed.
    fn from_fields(device: &str, f: &Map<String, Value>) -> Option<Self> {
        let text = |k: &str| f.get(k).and_then(Value::as_str).map(str::to_owned);
        PublicKey::from_hex(&text("public_key")?)?;
        Some(Self {
            device: device.to_owned(),
            public_key: text("public_key")?,
            name: crate::name::validate_name(&text("name")?).ok()?,
            platform: text("platform")?,
            at: f.get("at").and_then(Value::as_i64)?,
            direct: f.get("direct").and_then(Value::as_bool)?,
            by: text("by")?,
            signature: text("signature")?,
        })
    }
}

fn read(conn: &Connection, device: &str) -> rusqlite::Result<Option<Row>> {
    conn.query_row(
        "SELECT device, public_key, name, platform, at, by_device, signature, direct
         FROM sync_introductions WHERE device = ?1",
        [device],
        |r| {
            Ok(Row {
                device: r.get(0)?,
                public_key: r.get(1)?,
                name: r.get(2)?,
                platform: r.get(3)?,
                at: r.get(4)?,
                by: r.get(5)?,
                signature: r.get(6)?,
                direct: r.get(7)?,
            })
        },
    )
    .optional()
}

fn store(conn: &Connection, row: &Row) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO sync_introductions(device, public_key, name, platform, at, by_device, signature, direct)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(device) DO UPDATE SET public_key = ?2, name = ?3, platform = ?4, at = ?5,
             by_device = ?6, signature = ?7, direct = ?8",
        params![
            row.device,
            row.public_key,
            row.name,
            row.platform,
            row.at,
            row.by,
            row.signature,
            row.direct
        ],
    )?;
    Ok(())
}

/// Forgets every introduction: the group was left.
pub fn clear(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM sync_introductions", [])?;
    Ok(())
}

/// Signs and records an introduction of each of `devices`, which this device has just paired
/// (`paired`, a direct pairing) or been told of by the device it paired, so the rest of the group
/// accepts them. A device that is
/// not paired here, has no usable key or stands forgotten is left out, and nothing is made while
/// `devices` are all the group has. Returns how many were made.
pub fn introduce(
    conn: &Connection,
    identity: &DeviceIdentity,
    paired: &str,
    devices: &[String],
) -> Result<usize, SyncError> {
    // Nobody else is in the group to tell: two devices pairing for the first time need no
    // introduction (it would only make the first session longer).
    let others: i64 = {
        let mut stmt = conn.prepare("SELECT id FROM sync_peers")?;
        let ids = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut n = 0;
        for id in ids {
            n += i64::from(!devices.contains(&id?));
        }
        n
    };
    if others == 0 {
        return Ok(0);
    }
    let me = hlc::device_id(conn)?;
    let mut made = 0;
    for device in devices {
        let peer: Option<(String, String, String)> = conn
            .query_row(
                "SELECT name, platform, public_key FROM sync_peers WHERE id = ?1 AND public_key IS NOT NULL",
                [device],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((name, platform, key)) = peer else {
            continue;
        };
        if *device == me || crate::revoke::is_revoked(conn, device)? {
            continue;
        }
        let previous = read(conn, device)?;
        let now = physical_now(conn);
        let row = Row {
            device: device.clone(),
            public_key: key,
            name,
            platform,
            at: previous
                .as_ref()
                .map_or(now, |p| now.max(p.at.saturating_add(1))),
            direct: device == paired,
            by: me.clone(),
            signature: String::new(),
        }
        .signed(identity)?;
        store(conn, &row)?;
        changelog::record(
            conn,
            &Change {
                module: crate::MODULE_ID.to_owned(),
                entity: ENTITY.to_owned(),
                entity_id: row.device.clone(),
                op: if previous.is_some() {
                    ChangeOp::Update
                } else {
                    ChangeOp::Create
                },
                session_id: None,
                snapshot: row.json(),
            },
        )?;
        made += 1;
    }
    if made > 0 {
        tracing::info!(target: "sync", made, "signed introductions for the rest of the group");
    }
    Ok(made)
}

/// Takes an introduction received from another device (Sync's side of the Module contract).
/// Taken only if a paired device signed it and it is newer than what is held; otherwise the
/// record is kept for relay and nothing changes.
pub(crate) fn apply(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let nothing = |why: &str| {
        tracing::debug!(target: "sync", why, "an introduction record was not taken");
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
    // What this device signed comes back only as an echo; one about this device means the group
    // knows it already.
    if row.by == me || device == me {
        return nothing("made here or about this device");
    }
    // Nobody vouches for itself.
    if row.by == *device {
        return nothing("a device introducing itself");
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
        tracing::warn!(target: "sync", "an introduction with a bad signature was ignored");
        return nothing("bad signature");
    }
    if row.at > physical_now(conn).saturating_add(MAX_AHEAD_MS) {
        return nothing("dated in the future");
    }
    if crate::revoke::is_revoked(conn, &row.by)? {
        return nothing("signer stands forgotten");
    }
    let previous = read(conn, device)?;
    if previous.as_ref().is_some_and(|p| p.at >= row.at) {
        return nothing("not newer");
    }
    if crate::peers::group(conn)?.is_none() {
        return nothing("no group here");
    }
    if key_held_by_other(conn, device, &row.public_key)? {
        return nothing("key belongs to another device");
    }
    // Forgotten here: only a pairing newer than the Forget brings it back, and then the Forget no
    // longer stands.
    if let Some(forgot_at) = revoked_at(conn, device)? {
        // A member that had not heard of the Forget may pass the device on from some other pairing
        // (a member the device paired brought it); only a pairing of the device itself is a
        // decision to have it back, or a Forget could be undone by anyone behind on news.
        if row.at <= forgot_at || !row.direct {
            store(conn, &row)?;
            return nothing("forgotten since");
        }
        conn.execute("DELETE FROM sync_revocations WHERE device = ?1", [device])?;
    }
    store(conn, &row)?;
    take_in(conn, &row)?;
    Ok(Applied {
        snapshot: row.json(),
        changed: false,
        events: Vec::new(),
    })
}

/// Whether a paired device other than `device` holds `key`.
fn key_held_by_other(conn: &Connection, device: &str, key: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_peers WHERE public_key = ?2 AND id <> ?1)",
        params![device, key],
        |r| r.get(0),
    )
}

/// When `device` was forgotten, if it stands forgotten here.
fn revoked_at(conn: &Connection, device: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT at FROM sync_revocations WHERE device = ?1 AND state = 'revoked'",
        [device],
        |r| r.get(0),
    )
    .optional()
}

/// Makes the introduced device a paired device with its key, or updates the key of one this
/// device paired earlier than the introduction was made.
fn take_in(conn: &Connection, row: &Row) -> AppResult<()> {
    let held: Option<(Option<String>, i64)> = conn
        .query_row(
            "SELECT public_key, paired_at FROM sync_peers WHERE id = ?1",
            [&row.device],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match held {
        None => {
            // Paired "when the introducer paired it", so a Forget that was decided earlier than
            // that cannot take it out again (`revoke::paired_since`).
            conn.execute(
                "INSERT INTO sync_peers(id, name, paired_at, public_key, platform, paired_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    row.device,
                    row.name,
                    row.at,
                    row.public_key,
                    Platform::parse(&row.platform).as_str(),
                    row.by
                ],
            )?;
            // Names settle from the wishes alone, so every device ends with the same ones.
            crate::rename::settle(conn, &row.device, &row.name)?;
            tracing::info!(target: "sync", "a device paired by another member was accepted");
        }
        // A new key for a device already paired: only from the member that paired that key.
        Some((key, paired_at))
            if row.direct && key.as_deref() != Some(&row.public_key) && paired_at < row.at =>
        {
            conn.execute(
                "UPDATE sync_peers SET public_key = ?2, paired_at = ?3, paired_by = ?4 WHERE id = ?1",
                params![row.device, row.public_key, row.at, row.by],
            )?;
            tracing::info!(target: "sync", "a paired device came back with a new key, accepted");
        }
        // Known under that key already, or paired here more recently than this was signed.
        Some(_) => {}
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

    fn intro(
        device: &str,
        newcomer: &DeviceIdentity,
        by: &str,
        signer: &DeviceIdentity,
        at: i64,
    ) -> Row {
        intro_as(device, newcomer, by, signer, at, true)
    }

    fn intro_as(
        device: &str,
        newcomer: &DeviceIdentity,
        by: &str,
        signer: &DeviceIdentity,
        at: i64,
        direct: bool,
    ) -> Row {
        Row {
            device: device.into(),
            public_key: newcomer.public_key().to_hex(),
            name: "Pixel 8".into(),
            platform: "phone".into(),
            at,
            direct,
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

    fn forgot(conn: &Connection, device: &str, key: &DeviceIdentity, at: i64) {
        conn.execute(
            "INSERT INTO sync_revocations(device, public_key, name, platform, state, at, by_device, signature)
             VALUES (?1, ?2, 'x', 'phone', 'revoked', ?3, 'alice', '')",
            params![device, key.public_key().to_hex(), at],
        )
        .unwrap();
    }

    #[test]
    fn an_introduction_signed_by_a_member_pairs_the_newcomer_and_pins_its_key() {
        let conn = conn();
        let (bob, dana) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "bob", "Bob", &bob);
        receive(&conn, &intro("dana", &dana, "bob", &bob, 10));
        assert!(paired(&conn, "dana"));
        assert!(
            crate::peers::pinned(&conn)
                .unwrap()
                .contains(&(dana.public_key(), "dana".to_owned()))
        );
    }

    #[test]
    fn an_introduction_signed_by_a_non_member_is_ignored() {
        let conn = conn();
        let (bob, mallory, dana) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "bob", "Bob", &bob);
        // Signed by a stranger, claiming to be Bob, and by a stranger under its own name.
        receive(&conn, &intro("dana", &dana, "bob", &mallory, 10));
        receive(&conn, &intro("dana", &dana, "mallory", &mallory, 11));
        assert!(!paired(&conn, "dana"));
    }

    #[test]
    fn an_introduction_altered_after_signing_is_ignored() {
        let conn = conn();
        let (bob, dana, mallory) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "bob", "Bob", &bob);
        let mut row = intro("dana", &dana, "bob", &bob, 10);
        row.public_key = mallory.public_key().to_hex();
        receive(&conn, &row);
        assert!(!paired(&conn, "dana"));
    }

    #[test]
    fn an_introduction_by_a_forgotten_member_is_ignored() {
        let conn = conn();
        let (bob, dana) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "bob", "Bob", &bob);
        conn.execute(
            "INSERT INTO sync_revocations(device, public_key, name, platform, state, at, by_device, signature)
             VALUES ('bob', ?1, 'Bob', 'phone', 'revoked', 5, 'alice', '')",
            [bob.public_key().to_hex()],
        )
        .unwrap();
        receive(&conn, &intro("dana", &dana, "bob", &bob, 10));
        assert!(!paired(&conn, "dana"));
    }

    #[test]
    fn a_device_forgotten_later_than_its_introduction_stays_out() {
        let conn = conn();
        let (bob, dana) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "bob", "Bob", &bob);
        forgot(&conn, "dana", &dana, 20);
        receive(&conn, &intro("dana", &dana, "bob", &bob, 10));
        assert!(!paired(&conn, "dana"));
        assert!(crate::revoke::is_revoked(&conn, "dana").unwrap());
    }

    #[test]
    fn a_pairing_newer_than_the_forget_brings_the_device_back() {
        let conn = conn();
        let (bob, dana) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "bob", "Bob", &bob);
        forgot(&conn, "dana", &dana, 5);
        receive(&conn, &intro("dana", &dana, "bob", &bob, 10));
        assert!(paired(&conn, "dana"));
        assert!(!crate::revoke::is_revoked(&conn, "dana").unwrap());
        // Paired as of the introduction, so a Forget decided before that cannot take it out again.
        let paired_at: i64 = conn
            .query_row(
                "SELECT paired_at FROM sync_peers WHERE id = 'dana'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(paired_at, 10);
    }

    #[test]
    fn an_introduction_of_a_device_the_signer_did_not_pair_cannot_undo_a_forget() {
        let conn = conn();
        let (bob, dana) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "bob", "Bob", &bob);
        forgot(&conn, "dana", &dana, 5);
        // Bob had not heard of the Forget and passed Dana on from another pairing, later than it.
        receive(&conn, &intro_as("dana", &dana, "bob", &bob, 10, false));
        assert!(!paired(&conn, "dana"));
        assert!(crate::revoke::is_revoked(&conn, "dana").unwrap());
    }

    #[test]
    fn the_direct_flag_is_signed() {
        let conn = conn();
        let (bob, dana) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "bob", "Bob", &bob);
        forgot(&conn, "dana", &dana, 5);
        let mut row = intro_as("dana", &dana, "bob", &bob, 10, false);
        row.direct = true;
        receive(&conn, &row);
        assert!(!paired(&conn, "dana"));
    }

    #[test]
    fn a_member_cannot_introduce_itself_or_take_over_another_members_id() {
        let conn = conn();
        let (bob, carol, mallory) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "bob", "Bob", &bob);
        pair(&conn, "carol", "Carol", &carol);
        // Bob signs "bob" with a new key, and "carol" with Mallory's key, later than both pairings.
        receive(&conn, &intro("bob", &mallory, "bob", &bob, 10));
        receive(&conn, &intro_as("carol", &mallory, "bob", &bob, 11, false));
        let keys: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT public_key FROM sync_peers ORDER BY id")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        assert_eq!(
            keys,
            [bob.public_key().to_hex(), carol.public_key().to_hex()]
        );
    }

    #[test]
    fn a_key_already_held_by_another_device_is_not_taken_again() {
        let conn = conn();
        let (bob, carol) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "bob", "Bob", &bob);
        pair(&conn, "carol", "Carol", &carol);
        // Under a new id, Carol's key: two ids for one key would let one device count twice.
        receive(&conn, &intro("carol2", &carol, "bob", &bob, 10));
        assert!(!paired(&conn, "carol2"));
    }

    #[test]
    fn a_pairing_that_is_not_direct_does_not_replace_the_key_of_a_paired_device() {
        let conn = conn();
        let (bob, carol, other) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "bob", "Bob", &bob);
        pair(&conn, "carol", "Carol", &carol);
        receive(&conn, &intro_as("carol", &other, "bob", &bob, 10, false));
        let key: String = conn
            .query_row(
                "SELECT public_key FROM sync_peers WHERE id = 'carol'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(key, carol.public_key().to_hex());
        // The member that really paired the new key may.
        receive(&conn, &intro("carol", &other, "bob", &bob, 11));
        let key: String = conn
            .query_row(
                "SELECT public_key FROM sync_peers WHERE id = 'carol'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(key, other.public_key().to_hex());
    }

    #[test]
    fn a_name_that_clashes_is_numbered_the_same_way_on_every_device() {
        let conn = conn();
        let (bob, dana) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        pair(&conn, "bob", "Pixel 8", &bob);
        receive(&conn, &intro("dana", &dana, "bob", &bob, 10));
        let names: Vec<(String, String)> = {
            let mut stmt = conn
                .prepare("SELECT id, name FROM sync_peers ORDER BY id")
                .unwrap();
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        // "bob" sorts before "dana": Bob keeps the plain name.
        assert_eq!(
            names,
            [
                ("bob".to_owned(), "Pixel 8".to_owned()),
                ("dana".to_owned(), "Pixel 8 (2)".to_owned())
            ]
        );
    }

    #[test]
    fn introducing_signs_with_this_devices_key_and_a_receiver_accepts_it() {
        let sender = conn();
        let me = DeviceIdentity::generate().unwrap();
        let dana = DeviceIdentity::generate().unwrap();
        pair(&sender, "dana", "Dana", &dana);
        pair(
            &sender,
            "erin",
            "Erin",
            &DeviceIdentity::generate().unwrap(),
        );
        let made = introduce(
            &sender,
            &me,
            "dana",
            &["dana".to_owned(), "ghost".to_owned()],
        )
        .unwrap();
        assert_eq!(made, 1);
        let alone = conn();
        pair(&alone, "dana", "Dana", &dana);
        assert_eq!(
            introduce(&alone, &me, "dana", &["dana".to_owned()]).unwrap(),
            0
        );
        let row = read(&sender, "dana").unwrap().unwrap();
        assert_eq!(row.by, "me");
        assert!(row.direct);

        let receiver = conn();
        hlc::set_device_id(&receiver, "receiver").unwrap();
        pair(&receiver, "me", "Sender", &me);
        receive(&receiver, &row);
        assert!(paired(&receiver, "dana"));
    }
}
