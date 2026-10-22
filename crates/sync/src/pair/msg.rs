//! The pairing messages, framed like everything else on the wire. The typing device speaks first,
//! and its first message (`Start`) must never parse as a [`Handshake`](crate::net::Handshake):
//! the listener tells a pinned peer's handshake from a pairing attempt by that.

use std::collections::BTreeMap;

use mnemora_core::env::Environment;
use serde::{Deserialize, Serialize};

use super::view::Platform;

/// A device of the group, as one member introduces it to a newcomer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// Its device id.
    pub device: String,
    /// Its name in the group.
    pub name: String,
    /// Its kind.
    pub platform: Platform,
    /// Its public key, hex.
    pub key: String,
}

/// What a device says about itself once the code has proved the other end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Who {
    /// Device id.
    pub device: String,
    /// The name it gives itself.
    pub name: String,
    /// Its kind.
    pub platform: Platform,
    /// Its public key, hex. Must equal the key its TLS certificate proved.
    pub key: String,
    /// Production or Sandbox.
    pub environment: Environment,
    /// The group it belongs to, if it has paired before.
    pub group: Option<String>,
    /// Per-Module data versions (pairing is refused when they differ).
    pub data_versions: BTreeMap<String, usize>,
    /// The other devices of its group.
    pub members: Vec<Member>,
}

/// Why one side stops the pairing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "why", rename_all = "snake_case")]
pub enum Reason {
    /// The sender runs `sender`, which is not the receiver's Environment.
    Environment {
        /// The sender's Environment.
        sender: Environment,
    },
    /// Data versions differ.
    Version,
    /// Both belong to groups, not the same one.
    Groups,
    /// The person pressed Deny.
    Declined,
    /// The code (or the window to allow) ran out.
    Expired,
    /// The sender gave up.
    Cancelled,
    /// The sender is pairing with someone else.
    Busy,
}

/// One pairing message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "pair", rename_all = "snake_case")]
pub enum Msg {
    /// Typer to shower: the typer's Environment and its SPAKE2 message.
    Start {
        /// The typer's Environment.
        environment: Environment,
        /// SPAKE2 message, hex.
        spake: String,
    },
    /// Shower to typer: its SPAKE2 message.
    Reply {
        /// SPAKE2 message, hex.
        spake: String,
    },
    /// Typer to shower: proof it knows the code and sees the same two keys.
    Confirm {
        /// HMAC tag, hex.
        tag: String,
    },
    /// Shower to typer: its own proof, and who it is.
    Confirmed {
        /// HMAC tag, hex.
        tag: String,
        /// The shower.
        who: Box<Who>,
    },
    /// Shower to typer: the proof did not match.
    Wrong {
        /// Tries left on the code.
        tries_left: u8,
        /// The shower's name, so the typer can say where to look.
        name: String,
    },
    /// Typer to shower: who it is.
    Identity {
        /// The typer.
        who: Box<Who>,
    },
    /// Shower to typer: allowed. `group` is the group both now belong to.
    Accepted {
        /// Group id.
        group: String,
    },
    /// Typer to shower: stored on its side.
    Done,
    /// Either way: no.
    Refuse {
        /// Why.
        reason: Reason,
    },
}

/// Bytes as lowercase hex.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Reads [`hex`].
pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::to_value;

    use super::*;
    use crate::net::Handshake;

    #[test]
    fn no_pairing_message_reads_as_a_handshake() {
        let who = Who {
            device: "a".into(),
            name: "A".into(),
            platform: Platform::Desktop,
            key: "00".into(),
            environment: Environment::Production,
            group: None,
            data_versions: BTreeMap::new(),
            members: vec![],
        };
        for msg in [
            Msg::Start {
                environment: Environment::Production,
                spake: "00".into(),
            },
            Msg::Reply { spake: "00".into() },
            Msg::Confirm { tag: "00".into() },
            Msg::Confirmed {
                tag: "00".into(),
                who: Box::new(who.clone()),
            },
            Msg::Identity { who: Box::new(who) },
            Msg::Accepted { group: "g".into() },
            Msg::Done,
            Msg::Refuse {
                reason: Reason::Version,
            },
        ] {
            let json = to_value(&msg).unwrap();
            assert!(
                serde_json::from_value::<Handshake>(json.clone()).is_err(),
                "{json}"
            );
            assert_eq!(serde_json::from_value::<Msg>(json).unwrap(), msg);
        }
    }

    #[test]
    fn hex_round_trips_and_rejects_junk() {
        assert_eq!(hex(&[0, 15, 255]), "000fff");
        assert_eq!(unhex("000fff"), Some(vec![0, 15, 255]));
        assert_eq!(unhex("0"), None);
        assert_eq!(unhex("zz"), None);
        assert_eq!(unhex("é"), None);
    }
}
