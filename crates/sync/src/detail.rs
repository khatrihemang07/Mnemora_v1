//! The device sheet: everything known about one paired device, and the Address a person typed for
//! it.

use std::collections::{BTreeMap, HashMap};

use mnemora_core::error::AppResult;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use specta::Type;

use crate::address::Address;
use crate::devices::state_of;
use crate::error::SyncError;
use crate::live::PeerLive;
use crate::pair::Platform;
use crate::peer_state::{self, Route};
use crate::peers::PeerState;

/// An address that was remembered for the device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct KnownAddress {
    /// `ip:port`.
    pub address: String,
    /// When connecting to it last worked: UTC milliseconds; `None` if only heard of.
    #[specta(type = Option<u32>)]
    pub last_ok_at: Option<i64>,
}

/// One Module's data version on the device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ModuleVersion {
    /// The Module's id, e.g. `todo`.
    pub module: String,
    /// How many migrations its data has had.
    pub version: u32,
}

/// What the device sheet shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct DeviceDetail {
    /// Its device id.
    pub id: String,
    /// Its name here.
    pub name: String,
    /// Its kind.
    pub platform: Platform,
    /// Where it stands now.
    pub state: PeerState,
    /// When it was paired: UTC milliseconds.
    #[specta(type = u32)]
    pub paired_at: i64,
    /// The name of the device that paired it; this device's own name when this one did.
    pub paired_by: Option<String>,
    /// When it was last reached.
    #[specta(type = Option<u32>)]
    pub last_seen_at: Option<i64>,
    /// When a session with it last finished.
    #[specta(type = Option<u32>)]
    pub last_synced_at: Option<i64>,
    /// How it was reached the last time.
    pub route: Option<Route>,
    /// Addresses remembered for it, the one that worked most recently first.
    pub addresses: Vec<KnownAddress>,
    /// The Address typed for it, as stored.
    pub typed_address: Option<String>,
    /// Its app version, once it has been reached.
    pub app_version: Option<String>,
    /// Its Modules' data versions, once it has been reached.
    pub data_versions: Vec<ModuleVersion>,
}

#[allow(
    clippy::implicit_hasher,
    reason = "the one caller passes the manager's plain map"
)]
/// The sheet of `device`, or `None` if it is not paired. `me` and `my_name` are this device's id
/// and name (a device paired by this one says so).
pub fn detail(
    conn: &Connection,
    device: &str,
    me: &str,
    my_name: &str,
    live: &HashMap<String, PeerLive>,
) -> AppResult<Option<DeviceDetail>> {
    let row = conn
        .query_row(
            "SELECT p.name, p.platform, p.paired_at, p.paired_by, p.typed_address,
                    s.last_seen_at, s.last_synced_at, s.route, s.app_version, s.data_versions
             FROM sync_peers p LEFT JOIN sync_peer_status s ON s.device = p.id WHERE p.id = ?1",
            [device],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, Option<String>>(9)?,
                ))
            },
        )
        .optional()?;
    let Some((name, platform, paired_at, by, typed, seen, synced, route, app_version, versions)) =
        row
    else {
        return Ok(None);
    };
    let paired_by = match by {
        Some(id) if id == me => Some(my_name.to_owned()),
        Some(id) => crate::peers::name_of(conn, &id)?,
        None => None,
    };
    let versions: BTreeMap<String, u32> = versions
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default();
    Ok(Some(DeviceDetail {
        id: device.to_owned(),
        name,
        platform: Platform::parse(&platform),
        state: state_of(live.get(device)),
        paired_at,
        paired_by,
        last_seen_at: seen,
        last_synced_at: synced,
        route: route.as_deref().and_then(Route::parse),
        addresses: peer_state::known_addresses(conn, device)?
            .into_iter()
            .map(|(address, last_ok_at)| KnownAddress {
                address,
                last_ok_at,
            })
            .collect(),
        typed_address: typed,
        app_version,
        data_versions: versions
            .into_iter()
            .map(|(module, version)| ModuleVersion { module, version })
            .collect(),
    }))
}

/// The Address typed for `device`, as stored (`None`: none).
pub fn typed_address(conn: &Connection, device: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT typed_address FROM sync_peers WHERE id = ?1",
            [device],
            |r| r.get(0),
        )
        .optional()?
        .flatten())
}

/// Saves the Address typed for `device` ("" clears it) and returns what is stored. A text that
/// breaks the rules is refused with the reason.
pub fn set_typed_address(
    conn: &Connection,
    device: &str,
    text: &str,
) -> Result<Option<String>, SyncError> {
    let stored = if text.trim().is_empty() {
        None
    } else {
        Some(
            Address::parse(text)
                .map_err(|e| SyncError::Validation(e.message()))?
                .normalized(),
        )
    };
    conn.execute(
        "UPDATE sync_peers SET typed_address = ?2 WHERE id = ?1",
        params![device, stored],
    )?;
    Ok(stored)
}

/// Where the Address `device` was typed with resolves; used as the third list of dial candidates.
pub fn typed_for_dial(conn: &Connection, device: &str) -> Result<Option<Address>, SyncError> {
    Ok(typed_address(conn, device)?.and_then(|t| Address::parse(&t).ok()))
}
