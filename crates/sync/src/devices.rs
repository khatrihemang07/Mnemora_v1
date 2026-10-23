//! What the Devices tab lists: this device and the paired ones.

use std::collections::HashMap;
use std::net::SocketAddr;

use mnemora_core::device::DeviceSettings;
use mnemora_core::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;
use specta::Type;

use crate::addresses;
use crate::live::PeerLive;
use crate::name::default_device_name;
use crate::pair::Platform;
use crate::peer_state::{self, Remembered, Route};
use crate::peers::{self, PeerRow, PeerState};

/// This device, as the This-device card shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ThisDevice {
    /// Its name.
    pub name: String,
    /// Its kind.
    pub platform: Platform,
    /// The port it listens on; `None` while it is not listening.
    pub port: Option<u16>,
    /// Its shareable addresses (Wi-Fi, Tailscale), without the port.
    pub addresses: Vec<String>,
    /// Which kinds of network those addresses are on, Wi-Fi first (told by range, so the card
    /// says what is really there).
    pub routes: Vec<Route>,
}

/// The Devices tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct DevicesView {
    /// This device.
    pub this_device: ThisDevice,
    /// The paired devices.
    pub peers: Vec<PeerRow>,
}

/// Where a device stands, from what the session manager says about it right now.
pub(crate) fn state_of(live: Option<&PeerLive>) -> PeerState {
    match live {
        Some(l) if l.paused => PeerState::Paused,
        Some(l) if l.syncing => PeerState::Syncing,
        Some(l) if l.reachable => PeerState::UpToDate,
        _ => PeerState::NotNearby,
    }
}

/// The paired devices with their live state and what was remembered of the last sessions:
/// devices that can be reached now first, then the most recently seen.
fn enrich(
    mut rows: Vec<PeerRow>,
    remembered: &HashMap<String, Remembered>,
    live: &HashMap<String, PeerLive>,
) -> Vec<PeerRow> {
    for row in &mut rows {
        let kept = remembered.get(&row.id).cloned().unwrap_or_default();
        let now = live.get(&row.id);
        row.state = state_of(now);
        row.last_seen_at = kept.last_seen_at;
        row.last_synced_at = kept.last_synced_at;
        row.last_in = kept.last_in;
        row.last_out = kept.last_out;
        row.route = kept.route;
        row.progress = now.filter(|l| l.syncing).and_then(|l| l.progress);
    }
    let reachable = |r: &PeerRow| r.state != PeerState::NotNearby;
    rows.sort_by(|a, b| {
        reachable(b)
            .cmp(&reachable(a))
            .then(b.last_seen_at.cmp(&a.last_seen_at))
            .then(b.paired_at.cmp(&a.paired_at))
            .then(a.name.cmp(&b.name))
    });
    rows
}

/// The paired devices as the Devices tab lists them.
#[allow(
    clippy::implicit_hasher,
    reason = "the callers pass the manager's plain map"
)]
pub(crate) fn peer_rows(
    conn: &Connection,
    live: &HashMap<String, PeerLive>,
) -> AppResult<Vec<PeerRow>> {
    rows_of(conn, live)
}

/// The rows as `enrich` makes them; all of them "Removed" on a device the group has forgotten.
fn rows_of(conn: &Connection, live: &HashMap<String, PeerLive>) -> AppResult<Vec<PeerRow>> {
    let mut rows = enrich(peers::list(conn)?, &peer_state::all(conn)?, live);
    if crate::revoke::removed_here(conn)? {
        for row in &mut rows {
            row.state = PeerState::Removed;
            row.progress = None;
        }
    }
    Ok(rows)
}

/// The kinds of network `ips` are on, Wi-Fi first, each once.
pub(crate) fn routes_of(ips: &[std::net::IpAddr]) -> Vec<Route> {
    let mut routes: Vec<Route> = Vec::new();
    for route in ips.iter().map(|ip| Route::of(*ip)) {
        if !routes.contains(&route) {
            routes.push(route);
        }
    }
    routes.sort_by_key(|r| matches!(r, Route::Tailscale));
    routes
}

/// The Devices tab's data. `listening` is where the listener is bound, if it is; `live` is what
/// the session manager says about each paired device.
#[allow(
    clippy::implicit_hasher,
    reason = "the one caller passes the manager's plain map"
)]
pub fn view(
    device: &DeviceSettings,
    conn: &Connection,
    listening: Option<SocketAddr>,
    live: &HashMap<String, PeerLive>,
) -> AppResult<DevicesView> {
    Ok(DevicesView {
        this_device: ThisDevice {
            name: device.sync.name.clone().unwrap_or_else(default_device_name),
            platform: Platform::this(),
            port: listening.map(|a| a.port()),
            addresses: if listening.is_some() {
                addresses::local().iter().map(ToString::to_string).collect()
            } else {
                Vec::new()
            },
            routes: if listening.is_some() {
                routes_of(&addresses::local())
            } else {
                Vec::new()
            },
        },
        peers: rows_of(conn, live)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::Progress;

    fn row(id: &str, paired_at: i64) -> PeerRow {
        PeerRow {
            id: id.into(),
            name: id.into(),
            platform: Platform::Phone,
            paired_at,
            state: PeerState::NotNearby,
            last_seen_at: None,
            last_synced_at: None,
            last_in: None,
            last_out: None,
            route: None,
            progress: None,
        }
    }

    fn seen(at: i64) -> Remembered {
        Remembered {
            last_seen_at: Some(at),
            ..Remembered::default()
        }
    }

    const REACHED: PeerLive = PeerLive {
        reachable: true,
        syncing: false,
        progress: None,
        paused: false,
    };

    #[test]
    fn the_card_names_the_networks_that_are_really_there() {
        let ips = |l: &[&str]| l.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>();
        assert_eq!(
            routes_of(&ips(&["100.101.4.7", "192.168.1.20", "fd7a:115c:a1e0::1"])),
            [Route::Wifi, Route::Tailscale]
        );
        assert_eq!(routes_of(&ips(&["100.101.4.7"])), [Route::Tailscale]);
        assert_eq!(routes_of(&ips(&["10.0.0.4"])), [Route::Wifi]);
        assert!(routes_of(&[]).is_empty());
    }

    #[test]
    fn the_state_follows_what_the_manager_says() {
        assert_eq!(state_of(None), PeerState::NotNearby);
        assert_eq!(state_of(Some(&REACHED)), PeerState::UpToDate);
        let syncing = PeerLive {
            syncing: true,
            ..REACHED
        };
        assert_eq!(state_of(Some(&syncing)), PeerState::Syncing);
        let paused = PeerLive {
            paused: true,
            ..syncing
        };
        assert_eq!(state_of(Some(&paused)), PeerState::Paused);
        assert_eq!(state_of(Some(&PeerLive::default())), PeerState::NotNearby);
    }

    #[test]
    fn devices_that_can_be_reached_come_first_then_the_most_recently_seen() {
        let rows = vec![
            row("old", 1),
            row("recent", 2),
            row("here", 3),
            row("never", 4),
        ];
        let remembered = HashMap::from([
            ("old".to_owned(), seen(100)),
            ("recent".to_owned(), seen(900)),
            ("here".to_owned(), seen(50)),
        ]);
        let live = HashMap::from([("here".to_owned(), REACHED)]);
        let ids: Vec<String> = enrich(rows, &remembered, &live)
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, ["here", "recent", "old", "never"]);
    }

    #[test]
    fn progress_is_shown_only_while_syncing() {
        let progress = Some(Progress {
            done: 120,
            total: 340,
        });
        let busy = PeerLive {
            syncing: true,
            progress,
            ..REACHED
        };
        let idle = PeerLive {
            progress,
            ..REACHED
        };
        let live = HashMap::from([("a".to_owned(), busy), ("b".to_owned(), idle)]);
        let rows = enrich(vec![row("a", 1), row("b", 2)], &HashMap::new(), &live);
        let by_id = |id: &str| rows.iter().find(|r| r.id == id).unwrap().progress;
        assert_eq!(by_id("a"), progress);
        assert_eq!(by_id("b"), None);
    }

    #[test]
    fn every_device_shows_as_removed_on_one_the_group_forgot() {
        use mnemora_core::{db, hlc};
        let mut conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        db::migrate::run(
            &mut conn,
            &[&mnemora_core::module::CoreModule, &crate::SyncModule],
            dir.path(),
        )
        .unwrap();
        hlc::set_device_id(&conn, "me").unwrap();
        conn.execute("INSERT INTO sync_group(one, id) VALUES (1, 'g')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO sync_peers(id, name, paired_at, platform) VALUES ('a', 'Alice', 1, 'phone')",
            [],
        )
        .unwrap();
        let live = HashMap::new();
        assert_eq!(
            rows_of(&conn, &live).unwrap()[0].state,
            PeerState::NotNearby
        );
        conn.execute(
            "INSERT INTO sync_revocations(device, public_key, name, platform, state, at, by_device, signature)
             VALUES ('me', 'k', 'Me', 'phone', 'revoked', 5, 'a', 's')",
            [],
        )
        .unwrap();
        assert_eq!(rows_of(&conn, &live).unwrap()[0].state, PeerState::Removed);
    }
}
