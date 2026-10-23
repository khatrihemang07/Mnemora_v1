//! What the pairing sheet shows. One [`PairingView`] says the whole state, so the UI renders it and
//! sends back only what the person does; it never works out a phase itself.

use mnemora_core::env::Environment;
use serde::{Deserialize, Serialize};
use specta::Type;

/// What kind of device: decides the icon, and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    /// A computer.
    Desktop,
    /// A phone.
    Phone,
}

impl Platform {
    /// The platform this build runs on.
    pub const fn this() -> Self {
        if cfg!(target_os = "android") {
            Self::Phone
        } else {
            Self::Desktop
        }
    }

    /// The word stored in `sync_peers.platform`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Phone => "phone",
        }
    }

    /// Reads [`Platform::as_str`]; anything unknown is a computer.
    pub fn parse(text: &str) -> Self {
        if text == "phone" {
            Self::Phone
        } else {
            Self::Desktop
        }
    }
}

/// Which device has the older data, when two cannot pair because their data versions differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Older {
    /// This device: it must be updated.
    Me,
    /// The other device: it must be updated.
    Them,
    /// Each is older in some Module.
    Both,
}

/// Why pairing did not happen. Each is shown inline with what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PairingError {
    /// The code typed was wrong. `tries_left` 0 means the code is dead. `name` is the device
    /// showing it.
    WrongCode {
        /// Tries left on that code.
        tries_left: u8,
        /// The device showing the code.
        name: String,
    },
    /// The code ran out of time. On the typing side `name` says where to get a new one.
    CodeExpired {
        /// The device that showed the code, when known.
        name: Option<String>,
    },
    /// The code was typed wrong three times (shown on the device showing it).
    TriesUsed,
    /// The person on the other device pressed Deny.
    Declined {
        /// The device that said no.
        name: String,
    },
    /// The other device cancelled.
    Cancelled {
        /// The device that left, when known.
        name: Option<String>,
    },
    /// The other device runs the other Environment (Sandbox with Production).
    DifferentEnvironment {
        /// What the other device is.
        theirs: Environment,
    },
    /// The two devices run different data versions.
    VersionMismatch {
        /// The other device.
        name: String,
        /// Which of the two must be updated.
        older: Older,
    },
    /// Both devices already belong to a group of devices, not the same one.
    DifferentGroups,
    /// No device showing a code was found on the network.
    NothingNearby,
    /// The other device went away or could not be reached.
    ConnectionLost,
    /// The typed address is a name that does not resolve (Pair by address).
    AddressNotFound,
    /// Nothing answered at the typed address in time, or it is not Mnemora (Pair by address).
    AddressUnreachable,
    /// The other device is already pairing with someone.
    Busy,
    /// Sync is not ready (still starting, or off).
    NotReady,
}

/// The pairing sheet's state, in the order a pairing goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PairingView {
    /// Nothing is happening.
    Idle,
    /// This device shows a code and waits for the other to type it.
    Showing {
        /// The 8 digits, `"4821 0937"`.
        code: String,
        /// How long a code lives, in milliseconds (for the ring).
        life_ms: u32,
        /// How long this one has left.
        left_ms: u32,
        /// Wrong tries left before the code dies.
        tries_left: u8,
        /// Other devices can find this one (Visible on this network is on).
        discoverable: bool,
        /// The last attempt that failed for another reason than a wrong code.
        notice: Option<PairingError>,
    },
    /// The right code was typed: Allow or Deny.
    Asking {
        /// The device that wants to pair.
        name: String,
        /// Its kind.
        platform: Platform,
        /// Length of the whole window, in milliseconds.
        life_ms: u32,
        /// What is left of it.
        left_ms: u32,
    },
    /// This device typed the code and is looking for the device that shows it.
    Searching,
    /// The code was right; waiting for the person on the other device to Allow.
    Waiting {
        /// The device being waited for.
        name: String,
    },
    /// Allowed here; recording the pairing and telling the other device.
    Finishing {
        /// The device being paired.
        name: String,
    },
    /// Done.
    Paired {
        /// The other device's name here (may differ from its own if names clashed).
        name: String,
        /// Its kind.
        platform: Platform,
        /// The name was changed to keep names unique.
        renamed: bool,
        /// Devices of its group that came along.
        also_joined: Vec<String>,
    },
    /// It did not work.
    Failed {
        /// Why.
        error: PairingError,
    },
}
