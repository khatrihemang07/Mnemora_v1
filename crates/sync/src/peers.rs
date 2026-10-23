//! The paired devices.

use mnemora_core::error::AppResult;
use mnemora_core::text::fold;
use mnemora_core::util::now_ms;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use specta::Type;

use crate::SyncError;
use crate::live::Progress;
use crate::name::MAX_NAME_CHARS;
use crate::net::PublicKey;
use crate::pair::msg::Member;
use crate::pair::view::Platform;
use crate::pair::{Paired, Stored};
use crate::peer_state::Route;

/// How many other devices this one is paired with.
pub fn paired_count(conn: &Connection) -> AppResult<u32> {
    Ok(conn.query_row("SELECT COUNT(*) FROM sync_peers", [], |r| r.get(0))?)
}

/// Forgets every paired device and leaves the group. Synced data stays; this device just stops
/// belonging to the group.
pub fn forget_all(conn: &Connection) -> AppResult<()> {
    let had_group = group(conn)?.is_some();
    if had_group {
        let me = mnemora_core::hlc::device_id(conn)?;
        crate::activity::log_left(
            conn,
            crate::activity::Local {
                device: &me,
                name: "",
            },
        )?;
    }
    conn.execute("DELETE FROM sync_peers", [])?;
    conn.execute("DELETE FROM sync_group", [])?;
    crate::peer_state::forget_all(conn)?;
    // The group is gone, and so are its decisions about who may join it.
    crate::revoke::clear(conn)?;
    crate::introduce::clear(conn)?;
    tracing::info!(target: "sync", "left the group, all paired devices forgotten");
    Ok(())
}

/// Where a paired device stands, in the words the Devices tab uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PeerState {
    /// Connected and nothing left to exchange.
    UpToDate,
    /// A session is running.
    Syncing,
    /// Not reachable right now.
    NotNearby,
    /// Sync with it is paused (its data version differs).
    Paused,
    /// It was forgotten group-wide (#69).
    Removed,
}

/// A paired device as the Devices tab lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct PeerRow {
    /// Its device id.
    pub id: String,
    /// Its name here.
    pub name: String,
    /// Its kind.
    pub platform: Platform,
    /// When it was paired: UTC milliseconds.
    #[specta(type = u32)]
    pub paired_at: i64,
    /// Where it stands now.
    pub state: PeerState,
    /// When it was last reached: UTC milliseconds.
    #[specta(type = Option<u32>)]
    pub last_seen_at: Option<i64>,
    /// When a session with it last finished.
    #[specta(type = Option<u32>)]
    pub last_synced_at: Option<i64>,
    /// Changes the last session that moved anything received from it.
    pub last_in: Option<u32>,
    /// Changes that session sent to it.
    pub last_out: Option<u32>,
    /// How it was reached the last time.
    pub route: Option<Route>,
    /// How far the running session has got.
    pub progress: Option<Progress>,
}

/// The paired devices, most recently paired first.
pub fn list(conn: &Connection) -> AppResult<Vec<PeerRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, platform, paired_at FROM sync_peers ORDER BY paired_at DESC, name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(PeerRow {
            id: r.get(0)?,
            name: r.get(1)?,
            platform: Platform::parse(&r.get::<_, String>(2)?),
            paired_at: r.get(3)?,
            state: PeerState::NotNearby,
            last_seen_at: None,
            last_synced_at: None,
            last_in: None,
            last_out: None,
            route: None,
            progress: None,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Whether `device` is one of the paired devices.
pub fn is_paired(conn: &Connection, device: &str) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_peers WHERE id = ?1)",
        [device],
        |r| r.get(0),
    )?)
}

/// The name of a paired device, if it is one.
pub fn name_of(conn: &Connection, device: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row("SELECT name FROM sync_peers WHERE id = ?1", [device], |r| {
            r.get(0)
        })
        .optional()?)
}

/// The keys to pin at startup, as `(key, device id)`. A peer with no stored key is skipped.
pub fn pinned(conn: &Connection) -> AppResult<Vec<(PublicKey, String)>> {
    let mut stmt =
        conn.prepare("SELECT id, public_key FROM sync_peers WHERE public_key IS NOT NULL")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows
        .filter_map(Result::ok)
        .filter_map(|(id, key)| Some((PublicKey::from_hex(&key)?, id)))
        .collect())
}

/// The devices of the group other than this one, as they are introduced to a newcomer.
pub fn members(conn: &Connection) -> AppResult<Vec<Member>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, platform, public_key FROM sync_peers WHERE public_key IS NOT NULL ORDER BY paired_at, id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Member {
            device: r.get(0)?,
            name: r.get(1)?,
            platform: Platform::parse(&r.get::<_, String>(2)?),
            key: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The group this device belongs to, once it has paired.
pub fn group(conn: &Connection) -> AppResult<Option<String>> {
    Ok(conn
        .query_row("SELECT id FROM sync_group", [], |r| r.get(0))
        .optional()?)
}

/// `wanted`, or `wanted (2)`, `wanted (3)`… when `taken` already has that name, ignoring case and
/// accents. The result is at most [`MAX_NAME_CHARS`] long.
pub fn unique_name(wanted: &str, taken: &[String]) -> String {
    let key = |s: &str| fold(&s.split_whitespace().collect::<Vec<_>>().join(" "));
    let used: Vec<String> = taken.iter().map(|t| key(t)).collect();
    let free = |name: &str| !used.contains(&key(name));
    if free(wanted) {
        return wanted.to_owned();
    }
    // A group never has anywhere near this many devices of one name.
    (2..1000)
        .map(|n| {
            let suffix = format!(" ({n})");
            let room = MAX_NAME_CHARS.saturating_sub(suffix.chars().count());
            let base: String = wanted.chars().take(room).collect();
            format!("{}{suffix}", base.trim_end())
        })
        .find(|candidate| free(candidate))
        .unwrap_or_else(|| wanted.to_owned())
}

/// Records a pairing in one transaction: the group (made now if neither device had one), the
/// other device, and the members of its group this device did not know. `my_name` and
/// `my_device` are this device's; names stay unique across them and every peer.
pub fn store_paired(
    conn: &mut Connection,
    my_device: &str,
    my_name: &str,
    paired: &Paired,
) -> Result<Stored, SyncError> {
    let Paired {
        who,
        group: group_id,
    } = paired;
    let key = PublicKey::from_hex(&who.key)
        .ok_or_else(|| SyncError::Protocol("the other device sent no usable key".into()))?;
    let tx = conn.transaction()?;
    match group(&tx)? {
        Some(ours) if ours != *group_id => {
            return Err(SyncError::Protocol(
                "the devices are in different groups".into(),
            ));
        }
        Some(_) => {}
        None => {
            tx.execute("INSERT INTO sync_group(one, id) VALUES (1, ?1)", [group_id])?;
        }
    }
    let mut taken: Vec<String> = vec![my_name.to_owned()];
    {
        let mut stmt = tx.prepare("SELECT name FROM sync_peers WHERE id <> ?1")?;
        taken.extend(
            stmt.query_map([&who.device], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    let name = unique_name(&who.name, &taken);
    let now = now_ms();
    tx.execute(
        "INSERT INTO sync_peers(id, name, paired_at, public_key, platform, paired_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET name = ?2, paired_at = ?3, public_key = ?4, platform = ?5, paired_by = ?6",
        params![who.device, name, now, who.key, who.platform.as_str(), my_device],
    )?;
    taken.push(name.clone());
    let local = crate::activity::Local {
        device: my_device,
        name: my_name,
    };
    crate::activity::log_paired(&tx, local, &who.device, &name)?;
    let mut also_joined = Vec::new();
    let mut pins = vec![(key, who.device.clone())];
    for member in &who.members {
        let known: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sync_peers WHERE id = ?1)",
            [&member.device],
            |r| r.get(0),
        )?;
        let Some(member_key) = PublicKey::from_hex(&member.key) else {
            continue;
        };
        // A member that stands revoked here is not taken in: whoever introduced it has not yet
        // heard that it was forgotten.
        if known
            || member.device == my_device
            || member.device == who.device
            || crate::revoke::is_revoked(&tx, &member.device)?
        {
            continue;
        }
        let member_name = unique_name(&member.name, &taken);
        tx.execute(
            "INSERT INTO sync_peers(id, name, paired_at, public_key, platform, paired_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                member.device,
                member_name,
                now,
                member.key,
                member.platform.as_str(),
                who.device
            ],
        )?;
        taken.push(member_name.clone());
        crate::activity::log_joined(&tx, local, &member.device, &member_name)?;
        also_joined.push(member_name);
        pins.push((member_key, member.device.clone()));
    }
    tx.commit()?;
    Ok(Stored {
        renamed: name != who.name,
        name,
        platform: who.platform,
        also_joined,
        pins,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn taken(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_free_name_is_kept() {
        assert_eq!(unique_name("Pixel 8", &taken(&["MacBook"])), "Pixel 8");
    }

    #[test]
    fn a_clash_ignores_case_accents_and_spacing() {
        assert_eq!(unique_name("Pixel 8", &taken(&["Pixel 8"])), "Pixel 8 (2)");
        assert_eq!(
            unique_name("PIXEL  8", &taken(&["pixel 8"])),
            "PIXEL  8 (2)"
        );
        assert_eq!(unique_name("Café", &taken(&["cafe"])), "Café (2)");
        assert_eq!(
            unique_name("Pixel 8", &taken(&["Pixel 8", "Pixel 8 (2)"])),
            "Pixel 8 (3)"
        );
    }

    #[test]
    fn a_long_name_still_fits_with_its_suffix() {
        let long = "x".repeat(MAX_NAME_CHARS);
        let out = unique_name(&long, &taken(&[&long]));
        assert!(out.chars().count() <= MAX_NAME_CHARS, "{out}");
        assert!(out.ends_with(" (2)"));
    }

    /// The naming rule the browser mock also runs (`mock-sync.test.ts`).
    #[test]
    fn unique_names_match_the_shared_fixture() {
        const RULES: &str = include_str!("../../../src/lib/ipc/fixtures/sync-rules.json");
        let rules: serde_json::Value = serde_json::from_str(RULES).unwrap();
        for case in rules["unique_name_cases"].as_array().unwrap() {
            let taken: Vec<String> = case["taken"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t.as_str().unwrap().to_owned())
                .collect();
            assert_eq!(
                unique_name(case["wanted"].as_str().unwrap(), &taken),
                case["stored"].as_str().unwrap(),
                "{}",
                case["name"]
            );
        }
    }
}
