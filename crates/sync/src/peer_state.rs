//! What this device remembers about each paired device between runs: the addresses that worked
//! and what the last session moved. Local to this device; never synced.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

use mnemora_core::error::AppResult;
use mnemora_core::util::now_ms;
use rusqlite::{Connection, params};
use serde::Serialize;
use specta::Type;

use crate::addresses;

/// How a connection reached the other device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    /// Over the local network.
    Wifi,
    /// Over Tailscale.
    Tailscale,
}

impl Route {
    /// The route a connection to `ip` took.
    pub fn of(ip: IpAddr) -> Self {
        if addresses::is_tailscale(ip) {
            Self::Tailscale
        } else {
            Self::Wifi
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Wifi => "wifi",
            Self::Tailscale => "tailscale",
        }
    }

    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s {
            "wifi" => Some(Self::Wifi),
            "tailscale" => Some(Self::Tailscale),
            _ => None,
        }
    }
}

/// What is kept about one device's last sessions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Remembered {
    /// When it was last reached: UTC milliseconds.
    pub last_seen_at: Option<i64>,
    /// When a session with it last finished.
    pub last_synced_at: Option<i64>,
    /// Records the last session that moved anything received.
    pub last_in: Option<u32>,
    /// Records that session sent.
    pub last_out: Option<u32>,
    /// How it was reached the last time.
    pub route: Option<Route>,
}

/// Notes that the device was reached now over `route`.
pub fn saw(conn: &Connection, device: &str, route: Route) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sync_peer_status(device, last_seen_at, route) VALUES (?1, ?2, ?3)
         ON CONFLICT(device) DO UPDATE SET last_seen_at = ?2, route = ?3",
        params![device, now_ms(), route.as_str()],
    )?;
    Ok(())
}

/// Notes what the device said about itself when it was reached: its app version and the data
/// version of each Module.
pub fn met(
    conn: &Connection,
    device: &str,
    app_version: &str,
    data_versions: &std::collections::BTreeMap<String, usize>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sync_peer_status(device, app_version, data_versions) VALUES (?1, ?2, ?3)
         ON CONFLICT(device) DO UPDATE SET app_version = ?2, data_versions = ?3",
        params![
            device,
            app_version,
            serde_json::to_string(data_versions).unwrap_or_default()
        ],
    )?;
    Ok(())
}

/// Notes that a session with the device finished. The counts of an empty session (nothing moved
/// either way) are not kept: "12 in · 3 out" stays until something else moves.
pub fn finished(conn: &Connection, device: &str, received: usize, sent: usize) -> AppResult<()> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO sync_peer_status(device, last_seen_at, last_synced_at) VALUES (?1, ?2, ?2)
         ON CONFLICT(device) DO UPDATE SET last_seen_at = ?2, last_synced_at = ?2",
        params![device, now],
    )?;
    if received + sent > 0 {
        conn.execute(
            "UPDATE sync_peer_status SET last_in = ?2, last_out = ?3 WHERE device = ?1",
            params![
                device,
                i64::try_from(received).unwrap_or(i64::MAX),
                i64::try_from(sent).unwrap_or(i64::MAX)
            ],
        )?;
    }
    Ok(())
}

/// Whether the next records from `device` need a backup first: it was introduced by another
/// member (not paired with `me` directly, which backed up already) and no session with it has
/// finished yet, so this is the first time its records can reach this database.
pub fn needs_first_backup(conn: &Connection, me: &str, device: &str) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM sync_peers p
             WHERE p.id = ?2 AND COALESCE(p.paired_by, ?1) <> ?1
               AND NOT EXISTS(SELECT 1 FROM sync_peer_status s
                              WHERE s.device = p.id AND s.last_synced_at IS NOT NULL))",
        params![me, device],
        |r| r.get(0),
    )?)
}

/// What is remembered about every device.
pub fn all(conn: &Connection) -> AppResult<HashMap<String, Remembered>> {
    let mut stmt = conn.prepare(
        "SELECT device, last_seen_at, last_synced_at, last_in, last_out, route
         FROM sync_peer_status",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            Remembered {
                last_seen_at: r.get(1)?,
                last_synced_at: r.get(2)?,
                last_in: r.get(3)?,
                last_out: r.get(4)?,
                route: r
                    .get::<_, Option<String>>(5)?
                    .as_deref()
                    .and_then(Route::parse),
            },
        ))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// How many addresses are kept for one device.
const REMEMBERED_ADDRESSES: i64 = 8;

/// A remembered address with when it last worked.
pub fn known_addresses(conn: &Connection, device: &str) -> AppResult<Vec<(String, Option<i64>)>> {
    let mut stmt = conn.prepare(
        "SELECT address, last_ok_at FROM sync_peer_addresses WHERE device = ?1
         ORDER BY last_ok_at IS NULL, last_ok_at DESC, rowid DESC",
    )?;
    let rows = stmt.query_map([device], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Remembers addresses a device said it can be reached at (not yet known to work).
pub fn learn(conn: &Connection, device: &str, addrs: &[SocketAddr]) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO sync_peer_addresses(device, address, last_ok_at) VALUES (?1, ?2, NULL)",
    )?;
    for addr in addrs.iter().filter(|a| addresses::shareable(a.ip())) {
        stmt.execute(params![device, addr.to_string()])?;
    }
    // Every address is dialled at once, so a list that only ever grew (a new DHCP lease now and
    // then) would make each attempt a crowd. Keep the ones that worked most recently, then the
    // newest heard of.
    conn.execute(
        "DELETE FROM sync_peer_addresses WHERE device = ?1 AND rowid NOT IN (
             SELECT rowid FROM sync_peer_addresses WHERE device = ?1
             ORDER BY last_ok_at IS NULL, last_ok_at DESC, rowid DESC LIMIT ?2)",
        params![device, REMEMBERED_ADDRESSES],
    )?;
    Ok(())
}

/// Notes that connecting to `addr` worked now.
pub fn worked(conn: &Connection, device: &str, addr: SocketAddr) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sync_peer_addresses(device, address, last_ok_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(device, address) DO UPDATE SET last_ok_at = ?3",
        params![device, addr.to_string(), now_ms()],
    )?;
    Ok(())
}

/// The addresses remembered for a device, the one that worked most recently first.
pub fn addresses_of(conn: &Connection, device: &str) -> AppResult<Vec<SocketAddr>> {
    let mut stmt = conn.prepare(
        "SELECT address FROM sync_peer_addresses WHERE device = ?1
         ORDER BY last_ok_at IS NULL, last_ok_at DESC",
    )?;
    let rows = stmt.query_map([device], |r| r.get::<_, String>(0))?;
    Ok(rows
        .filter_map(Result::ok)
        .filter_map(|a| a.parse().ok())
        .collect())
}

/// Forgets everything remembered about every device.
pub fn forget_all(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM sync_peer_status", [])?;
    conn.execute("DELETE FROM sync_peer_addresses", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use mnemora_core::db;

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
        conn
    }

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn addresses_come_back_with_the_one_that_worked_last_first() {
        let conn = conn();
        learn(
            &conn,
            "p",
            &[addr("192.168.1.5:47311"), addr("100.64.0.9:47311")],
        )
        .unwrap();
        worked(&conn, "p", addr("100.64.0.9:47311")).unwrap();
        assert_eq!(
            addresses_of(&conn, "p").unwrap(),
            [addr("100.64.0.9:47311"), addr("192.168.1.5:47311")]
        );
    }

    #[test]
    fn only_the_best_few_addresses_are_kept_for_a_device() {
        let conn = conn();
        let lease = |n: u8| addr(&format!("192.168.1.{n}:47311"));
        learn(&conn, "p", &[lease(1), lease(2)]).unwrap();
        worked(&conn, "p", lease(1)).unwrap();
        let later: Vec<_> = (10..30).map(lease).collect();
        learn(&conn, "p", &later).unwrap();
        learn(&conn, "q", &[lease(99)]).unwrap();

        let kept = addresses_of(&conn, "p").unwrap();
        assert_eq!(kept.len(), 8);
        assert_eq!(kept[0], lease(1), "the one that worked stays, first");
        assert!(kept.contains(&lease(29)), "the newest heard of are kept");
        assert!(!kept.contains(&lease(2)), "the oldest unproven one goes");
        assert_eq!(addresses_of(&conn, "q").unwrap(), [lease(99)]);
    }

    #[test]
    fn addresses_that_may_not_be_shared_are_not_remembered() {
        let conn = conn();
        learn(
            &conn,
            "p",
            &[addr("8.8.8.8:47311"), addr("127.0.0.1:47311")],
        )
        .unwrap();
        assert!(addresses_of(&conn, "p").unwrap().is_empty());
    }

    #[test]
    fn an_empty_session_keeps_the_last_counts() {
        let conn = conn();
        finished(&conn, "p", 12, 3).unwrap();
        finished(&conn, "p", 0, 0).unwrap();
        let kept = &all(&conn).unwrap()["p"];
        assert_eq!((kept.last_in, kept.last_out), (Some(12), Some(3)));
        assert!(kept.last_synced_at.is_some());
    }

    #[test]
    fn only_an_introduced_device_never_synced_needs_a_first_backup() {
        let conn = conn();
        let add = |id: &str, by: &str| {
            conn.execute(
                "INSERT INTO sync_peers(id, name, paired_at, public_key, platform, paired_by)
                 VALUES (?1, ?1, 0, 'k', 'desktop', ?2)",
                [id, by],
            )
            .unwrap();
        };
        add("direct", "me");
        add("introduced", "direct");
        assert!(!needs_first_backup(&conn, "me", "direct").unwrap());
        assert!(needs_first_backup(&conn, "me", "introduced").unwrap());
        assert!(!needs_first_backup(&conn, "me", "stranger").unwrap());
        finished(&conn, "introduced", 0, 0).unwrap();
        assert!(!needs_first_backup(&conn, "me", "introduced").unwrap());
    }

    #[test]
    fn the_route_is_remembered() {
        let conn = conn();
        saw(&conn, "p", Route::Tailscale).unwrap();
        assert_eq!(all(&conn).unwrap()["p"].route, Some(Route::Tailscale));
        assert_eq!(Route::of("100.101.4.7".parse().unwrap()), Route::Tailscale);
        assert_eq!(Route::of("192.168.1.2".parse().unwrap()), Route::Wifi);
    }
}
