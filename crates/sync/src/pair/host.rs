//! What pairing needs from the app around it: this device's facts, a place to record a pairing,
//! and a way to tell the UI. The sync crate never touches the database or the window itself.

use mnemora_core::syncable::Holdings;

use super::msg::Who;
use super::view::{PairingView, Platform};
use crate::SyncError;
use crate::net::{Handshake, PublicKey};

/// A pairing that both ends agreed to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paired {
    /// The other device, as it described itself.
    pub who: Who,
    /// The group both now belong to.
    pub group: String,
}

/// What recording a [`Paired`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    /// The name the other device has here (unique among the group).
    pub name: String,
    /// Its kind.
    pub platform: Platform,
    /// That name is not the one it gave itself.
    pub renamed: bool,
    /// Devices of its group that were not known here before.
    pub also_joined: Vec<String>,
    /// Every key to pin now: the other device and the members it brought, as `(key, device id)`.
    pub pins: Vec<(PublicKey, String)>,
}

/// The app's side of pairing. Implemented in the app crate over its database and window.
pub trait PairingHost: Send + Sync + 'static {
    /// This device as it describes itself; `key` is filled in by the caller.
    fn me(&self) -> Result<Who, SyncError>;
    /// Our handshake, as sent to a paired device.
    fn handshake(&self) -> Result<Handshake, SyncError>;
    /// Whether this device may advertise itself on the local network.
    fn visible(&self) -> bool;
    /// The keys of the devices already paired, as `(key, device id)`.
    fn pinned(&self) -> Vec<(PublicKey, String)>;
    /// What this device holds (Entries, Tasks, Lists), for the first-merge preview. A device
    /// that reports nothing is treated as empty: no preview.
    fn holdings(&self) -> Result<Holdings, SyncError> {
        Ok(Holdings::default())
    }
    /// Backs up the database (the existing backup code and rotation) before the first merge
    /// with a new device. An error stops the pairing with nothing recorded.
    fn backup(&self) -> Result<(), SyncError> {
        Ok(())
    }
    /// Records the pairing: the group, the device and the members it brought.
    fn store(&self, paired: &Paired) -> Result<Stored, SyncError>;
    /// Tells the UI the pairing state changed.
    fn show(&self, view: &PairingView);
    /// A pairing by typed address worked: `address` (as stored, normalised) becomes the typed
    /// Address of `device`. Called after [`PairingHost::store`], before [`PairingHost::paired`].
    fn remember_address(&self, _device: &str, _address: &str) {}
    /// A pairing was recorded and its keys are pinned: the new devices can be connected to now.
    fn paired(&self) {}
}
