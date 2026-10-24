//! What needs the person's attention, in one place: the Home card, the sidebar badge and the
//! "Open Sync conflicts" hint all read this. Sync is calm while all is fine: nothing here is
//! raised for a quiet sync, only for Conflicts that wait, a pause, or a paired device that has
//! not been seen for a long time.

use std::collections::HashMap;

use mnemora_core::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;
use specta::Type;

use crate::conflicts;
use crate::devices::peer_rows;
use crate::live::PeerLive;
use crate::peers::{PeerRow, PeerState};

/// A paired device is "unseen" after this long.
pub const UNSEEN_DAYS: i64 = 14;

const DAY_MS: i64 = 86_400_000;

/// A paired device that has not been around for a long time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UnseenDevice {
    /// Its name.
    pub name: String,
    /// Whole days since it was last seen (since pairing, if never).
    pub days: u32,
}

/// The last finished sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct LastSynced {
    /// The device it was with.
    pub device: String,
    /// When: UTC milliseconds.
    #[specta(type = u32)]
    pub at: i64,
}

/// What needs attention now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct Attention {
    /// Conflicts that wait for the person ("Needs you").
    pub conflicts: u32,
    /// Devices syncing is paused with (a version mismatch).
    pub paused: Vec<String>,
    /// The system keeps this device off the local network (nothing on the Wi-Fi can be reached
    /// or found): shown like a pause.
    pub local_network_blocked: bool,
    /// Paired devices not seen for 14 days.
    pub unseen: Vec<UnseenDevice>,
    /// The most recent finished sync, for the Home card's second half.
    pub last_synced: Option<LastSynced>,
    /// Whether the Home card shows: something above is not empty.
    pub needed: bool,
}

/// Works out what needs attention from the open Conflicts and the paired devices' state.
pub fn attention_of(
    conflicts: u32,
    peers: &[PeerRow],
    local_network_blocked: bool,
    now: i64,
) -> Attention {
    let paused: Vec<String> = peers
        .iter()
        .filter(|p| p.state == PeerState::Paused)
        .map(|p| p.name.clone())
        .collect();
    let unseen: Vec<UnseenDevice> = peers
        .iter()
        .filter(|p| p.state == PeerState::NotNearby)
        .filter_map(|p| {
            let since = p.last_seen_at.unwrap_or(p.paired_at);
            let days = (now - since).div_euclid(DAY_MS);
            (days >= UNSEEN_DAYS).then(|| UnseenDevice {
                name: p.name.clone(),
                days: u32::try_from(days).unwrap_or(u32::MAX),
            })
        })
        .collect();
    let last_synced = peers
        .iter()
        .filter_map(|p| {
            p.last_synced_at.map(|at| LastSynced {
                device: p.name.clone(),
                at,
            })
        })
        .max_by_key(|l| l.at);
    Attention {
        conflicts,
        needed: conflicts > 0 || !paused.is_empty() || local_network_blocked || !unseen.is_empty(),
        local_network_blocked,
        paused,
        unseen,
        last_synced,
    }
}

/// What needs attention, read from the database and the session manager's live state.
#[allow(
    clippy::implicit_hasher,
    reason = "the one caller passes the manager's plain map"
)]
pub fn attention(
    conn: &Connection,
    live: &HashMap<String, PeerLive>,
    local_network_blocked: bool,
    now: i64,
) -> AppResult<Attention> {
    Ok(attention_of(
        conflicts::open_count(conn)?,
        &peer_rows(conn, live)?,
        local_network_blocked,
        now,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pair::Platform;

    fn peer(name: &str, state: PeerState, seen: Option<i64>, synced: Option<i64>) -> PeerRow {
        PeerRow {
            id: name.into(),
            name: name.into(),
            platform: Platform::Phone,
            paired_at: 0,
            state,
            last_seen_at: seen,
            last_synced_at: synced,
            last_in: None,
            last_out: None,
            route: None,
            progress: None,
            pause: None,
        }
    }

    const NOW: i64 = 100 * DAY_MS;

    #[test]
    fn a_calm_group_needs_nothing() {
        let peers = [
            peer("Pixel 8", PeerState::UpToDate, Some(NOW), Some(NOW)),
            peer("Air", PeerState::NotNearby, Some(NOW - 3 * DAY_MS), None),
        ];
        let a = attention_of(0, &peers, false, NOW);
        assert!(!a.needed);
        assert_eq!(a.last_synced.map(|l| l.device), Some("Pixel 8".into()));
    }

    #[test]
    fn each_of_the_three_conditions_alone_shows_the_card() {
        let calm = [peer("Pixel 8", PeerState::UpToDate, Some(NOW), Some(NOW))];
        assert!(attention_of(2, &calm, false, NOW).needed, "conflicts wait");
        let paused = [peer("Pixel 8", PeerState::Paused, Some(NOW), None)];
        let a = attention_of(0, &paused, false, NOW);
        assert!(a.needed);
        assert_eq!(a.paused, ["Pixel 8"]);
        let away = [peer(
            "Pixel 8",
            PeerState::NotNearby,
            Some(NOW - 14 * DAY_MS),
            None,
        )];
        let a = attention_of(0, &away, false, NOW);
        assert!(a.needed, "exactly 14 days");
        assert_eq!(a.unseen[0].days, 14);
        let nearly = [peer(
            "Pixel 8",
            PeerState::NotNearby,
            Some(NOW - 14 * DAY_MS + 1),
            None,
        )];
        assert!(
            !attention_of(0, &nearly, false, NOW).needed,
            "13 days and a bit"
        );
    }

    #[test]
    fn a_device_here_now_is_not_unseen_whatever_its_history() {
        let here = [peer("Pixel 8", PeerState::UpToDate, Some(0), None)];
        assert!(!attention_of(0, &here, false, NOW).needed);
    }

    #[test]
    fn the_local_network_being_blocked_alone_shows_the_card() {
        let calm = [peer("Pixel 8", PeerState::UpToDate, Some(NOW), Some(NOW))];
        assert!(!attention_of(0, &calm, false, NOW).needed);
        let a = attention_of(0, &calm, true, NOW);
        assert!(a.needed);
        assert!(a.local_network_blocked);
    }

    #[test]
    fn a_device_never_seen_counts_from_pairing() {
        let mut p = peer("Pixel 8", PeerState::NotNearby, None, None);
        p.paired_at = NOW - 20 * DAY_MS;
        let a = attention_of(0, &[p], false, NOW);
        assert_eq!(a.unseen[0].days, 20);
    }
}
