//! What the Devices tab lists: this device and the paired ones.

use std::net::SocketAddr;

use mnemora_core::device::DeviceSettings;
use mnemora_core::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;
use specta::Type;

use crate::addresses;
use crate::name::default_device_name;
use crate::pair::Platform;
use crate::peers::{self, PeerRow};

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
}

/// The Devices tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct DevicesView {
    /// This device.
    pub this_device: ThisDevice,
    /// The paired devices.
    pub peers: Vec<PeerRow>,
}

/// The Devices tab's data. `listening` is where the listener is bound, if it is.
pub fn view(
    device: &DeviceSettings,
    conn: &Connection,
    listening: Option<SocketAddr>,
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
        },
        peers: peers::list(conn)?,
    })
}
