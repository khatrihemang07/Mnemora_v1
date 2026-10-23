//! Sync's Device settings and how they change.

use mnemora_core::device::{DeviceSettings, DeviceStore};
use mnemora_core::error::AppResult;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::name::{default_device_name, validate_name};
use crate::peers::paired_count;
use crate::runtime::SyncRuntime;

/// What the Sync screens show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
// Mirrors the five switches of `SyncDevice`, one field each on the wire.
#[allow(clippy::struct_excessive_bools)]
pub struct SyncState {
    /// Sync is on for this device.
    pub enabled: bool,
    /// This device's name; before it is first chosen, the one that would be prefilled.
    pub name: String,
    /// Sync on discovery and after changes.
    pub automatic: bool,
    /// Phone: allow reaching devices away from Wi-Fi.
    pub mobile_data: bool,
    /// Advertise on the local network.
    pub visible: bool,
    /// Show a toast when a new conflict arrives (the other choice is Quietly).
    pub toast_conflicts: bool,
    /// Other devices this one is paired with.
    pub paired_devices: u32,
}

/// A yes/no choice in Settings › Sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SyncOption {
    /// Sync automatically.
    Automatic,
    /// Allow over mobile data.
    MobileData,
    /// Visible on this network.
    Visible,
    /// "New conflicts: Show a toast" (off is Quietly).
    ToastConflicts,
}

/// The current state, with this device's name filled in.
pub fn current(device: &DeviceSettings, conn: &Connection) -> AppResult<SyncState> {
    let sync = &device.sync;
    Ok(SyncState {
        enabled: sync.enabled,
        name: sync.name.clone().unwrap_or_else(default_device_name),
        automatic: sync.automatic,
        mobile_data: sync.mobile_data,
        visible: sync.visible,
        toast_conflicts: sync.toast_conflicts,
        paired_devices: paired_count(conn)?,
    })
}

/// Turns Sync on or off for this device and starts or stops its services to match. Turning it on
/// for the first time names the device after the computer or phone, so the name other devices
/// will see is fixed before anyone pairs.
pub fn set_enabled(device: &DeviceStore, runtime: &SyncRuntime, on: bool) -> AppResult<()> {
    let result = device.update(|d| {
        d.sync.enabled = on;
        if on && d.sync.name.is_none() {
            d.sync.name = Some(default_device_name());
        }
    });
    // The in-memory setting changes even if the file write failed (see `DeviceStore::update`),
    // so the services follow it either way.
    runtime.reconcile(device.get().sync.enabled);
    result
}

/// Renames this device. The name must be 1 to 40 characters.
pub fn rename(device: &DeviceStore, name: &str) -> AppResult<()> {
    let name = validate_name(name)?;
    device.update(|d| d.sync.name = Some(name))
}

/// Sets one yes/no choice.
pub fn set_option(device: &DeviceStore, option: SyncOption, on: bool) -> AppResult<()> {
    device.update(|d| match option {
        SyncOption::Automatic => d.sync.automatic = on,
        SyncOption::MobileData => d.sync.mobile_data = on,
        SyncOption::Visible => d.sync.visible = on,
        SyncOption::ToastConflicts => d.sync.toast_conflicts = on,
    })
}
