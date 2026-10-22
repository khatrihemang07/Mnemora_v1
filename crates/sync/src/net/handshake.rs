//! The first frame each way after TLS: who this device is and what it holds, and the rules that
//! decide whether the two should talk at all. Both sides send theirs first, then read the
//! other's, and each runs [`check`] on what it received, so a refusal is reached on both ends.
//! Nothing here depends on the sockets.

use std::collections::BTreeMap;
use std::fmt;

use mnemora_core::db::migrate;
use mnemora_core::env::Environment;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::identity::PublicKey;
use super::tls::Trust;
use crate::error::SyncError;

/// The wire protocol this build speaks. A different number is refused: there is no negotiation.
pub const PROTOCOL_VERSION: u32 = 1;

/// What a device says about itself when a connection opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handshake {
    /// [`PROTOCOL_VERSION`].
    pub protocol: u32,
    /// The app's version, for diagnostics.
    pub app_version: String,
    /// Production and Sandbox refuse each other.
    pub environment: Environment,
    /// This device's id.
    pub device: String,
    /// The group of paired devices this one belongs to; `None` before the first pairing.
    pub group: Option<String>,
    /// For each Module, how many migrations its schema has had. Different versions mean
    /// different data shapes: the session pauses (#72); this is only carried and compared here.
    pub data_versions: BTreeMap<String, usize>,
    /// Where this device can be reached, as `ip:port` (gossiped to other peers).
    pub addresses: Vec<String>,
}

impl Handshake {
    /// This device's handshake, reading the Module schema versions from `conn`.
    pub fn local(
        conn: &Connection,
        environment: Environment,
        device: &str,
        group: Option<String>,
        addresses: Vec<String>,
    ) -> Result<Self, SyncError> {
        Ok(Self {
            protocol: PROTOCOL_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            environment,
            device: device.to_owned(),
            group,
            data_versions: migrate::versions(conn)?,
            addresses,
        })
    }

    /// Modules whose data version differs from `other`'s, or that only one side has.
    pub fn differing_modules(&self, other: &Self) -> Vec<String> {
        let mut out: Vec<String> = self
            .data_versions
            .iter()
            .filter(|(m, v)| other.data_versions.get(*m) != Some(*v))
            .map(|(m, _)| m.clone())
            .chain(
                other
                    .data_versions
                    .keys()
                    .filter(|m| !self.data_versions.contains_key(*m))
                    .cloned(),
            )
            .collect();
        out.sort();
        out
    }
}

/// Why a peer was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The peer speaks another protocol version.
    Protocol {
        /// The peer's version.
        theirs: u32,
    },
    /// The peer runs in the other Environment.
    Environment,
    /// The peer's key is not pinned.
    Unpinned,
    /// The peer's key is pinned, but as another device than the one it claims to be.
    WrongDevice,
    /// The peer belongs to a different group.
    Group,
    /// The peer is this device.
    SelfDial,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol { theirs } => write!(f, "protocol version {theirs} is not supported"),
            Self::Environment => f.write_str("it runs in another Environment"),
            Self::Unpinned => f.write_str("its key is not pinned"),
            Self::WrongDevice => f.write_str("its key belongs to another device"),
            Self::Group => f.write_str("it belongs to another group"),
            Self::SelfDial => f.write_str("it is this device"),
        }
    }
}

/// Decides whether `local` talks to `remote`, who proved it holds `key`. Returns the Modules
/// whose data versions differ (empty when the two match); that is not a refusal here.
pub fn check(
    local: &Handshake,
    remote: &Handshake,
    key: &PublicKey,
    trust: &Trust,
) -> Result<Vec<String>, Refusal> {
    if remote.protocol != local.protocol {
        return Err(Refusal::Protocol {
            theirs: remote.protocol,
        });
    }
    if remote.environment != local.environment {
        return Err(Refusal::Environment);
    }
    if remote.device == local.device {
        return Err(Refusal::SelfDial);
    }
    match trust.device_of(key) {
        None => return Err(Refusal::Unpinned),
        Some(device) if device != remote.device => return Err(Refusal::WrongDevice),
        Some(_) => {}
    }
    if let (Some(ours), Some(theirs)) = (&local.group, &remote.group)
        && ours != theirs
    {
        return Err(Refusal::Group);
    }
    Ok(local.differing_modules(remote))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hs(device: &str) -> Handshake {
        Handshake {
            protocol: PROTOCOL_VERSION,
            app_version: "1".into(),
            environment: Environment::Production,
            device: device.into(),
            group: None,
            data_versions: BTreeMap::from([("core".into(), 3), ("todo".into(), 5)]),
            addresses: vec![],
        }
    }

    fn trust_with(key: PublicKey, device: &str) -> Trust {
        let t = Trust::new();
        t.pin(key, device);
        t
    }

    const KEY: PublicKey = PublicKey::from_bytes([1; 32]);

    #[test]
    fn a_pinned_peer_in_the_same_environment_is_accepted() {
        let trust = trust_with(KEY, "b");
        assert_eq!(check(&hs("a"), &hs("b"), &KEY, &trust), Ok(vec![]));
    }

    #[test]
    fn each_rule_has_its_own_refusal() {
        let trust = trust_with(KEY, "b");
        let mut other_env = hs("b");
        other_env.environment = Environment::Sandbox;
        assert_eq!(
            check(&hs("a"), &other_env, &KEY, &trust),
            Err(Refusal::Environment)
        );
        let mut other_protocol = hs("b");
        other_protocol.protocol = 2;
        assert_eq!(
            check(&hs("a"), &other_protocol, &KEY, &trust),
            Err(Refusal::Protocol { theirs: 2 })
        );
        assert_eq!(
            check(&hs("a"), &hs("b"), &KEY, &Trust::new()),
            Err(Refusal::Unpinned)
        );
        assert_eq!(
            check(&hs("a"), &hs("c"), &KEY, &trust),
            Err(Refusal::WrongDevice)
        );
        assert_eq!(
            check(&hs("a"), &hs("a"), &KEY, &trust_with(KEY, "a")),
            Err(Refusal::SelfDial)
        );
        let (mut mine, mut theirs) = (hs("a"), hs("b"));
        mine.group = Some("g1".into());
        theirs.group = Some("g2".into());
        assert_eq!(check(&mine, &theirs, &KEY, &trust), Err(Refusal::Group));
        theirs.group = None;
        assert!(check(&mine, &theirs, &KEY, &trust).is_ok(), "no group yet");
    }

    #[test]
    fn differing_data_versions_are_reported_not_refused() {
        let trust = trust_with(KEY, "b");
        let mut newer = hs("b");
        newer.data_versions.insert("todo".into(), 6);
        newer.data_versions.insert("sync".into(), 1);
        assert_eq!(
            check(&hs("a"), &newer, &KEY, &trust),
            Ok(vec!["sync".to_owned(), "todo".to_owned()])
        );
    }
}
