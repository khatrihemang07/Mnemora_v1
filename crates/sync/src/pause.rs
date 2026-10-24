//! Why sync with a paired device is paused, decided from the two handshakes. Both devices work
//! on their own meanwhile; nothing is exchanged either way until the cause is gone.

use std::collections::BTreeSet;

use serde::Serialize;
use specta::Type;

use crate::net::Handshake;

/// A device whose clock is further than this ahead of the other's is paused with: its records
/// would win every merge until real time caught up.
pub const MAX_AHEAD_MS: i64 = 5 * 60 * 1000;

/// What stops sync with a device. The words are made in the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Pause {
    /// The Modules' data versions differ. A Module absent on one side counts as older there.
    Update {
        /// Modules whose data is older on this device.
        older_here: Vec<String>,
        /// Modules whose data is older on the other device.
        older_there: Vec<String>,
    },
    /// The clocks agree now, but records were stamped by a clock that was ahead (the other
    /// device's, or one it relays): they are refused until real time is within five minutes of
    /// them. Fixing the clock does not shorten it.
    Stamped {
        /// How long until the newest of them is allowed, in milliseconds.
        #[specta(type = u32)]
        resumes_in_ms: i64,
    },
    /// One clock is more than five minutes ahead of the other.
    Clock {
        /// By how much, in milliseconds.
        #[specta(type = u32)]
        ahead_ms: i64,
        /// Whether it is this device's clock that is ahead.
        here: bool,
    },
}

impl Pause {
    /// The clock pause for a peer whose clock is `ahead_ms` ahead of this device's.
    pub const fn clock_ahead(ahead_ms: i64) -> Self {
        Self::Clock {
            ahead_ms,
            here: false,
        }
    }

    /// Whether this is the pause that outlives the handshake (see `Stamped`): a good handshake
    /// does not end it, only a sync that ran.
    pub const fn outlives_handshake(&self) -> bool {
        matches!(self, Self::Stamped { .. })
    }
}

/// Whether `local` may talk to `remote`: `None` when nothing stops it. A data-version difference
/// comes before a clock difference (the update must happen anyway).
pub fn pause_of(local: &Handshake, remote: &Handshake) -> Option<Pause> {
    let modules: BTreeSet<&String> = local
        .data_versions
        .keys()
        .chain(remote.data_versions.keys())
        .collect();
    let (mut mine, mut theirs) = (Vec::new(), Vec::new());
    for module in modules {
        let (here, there) = (
            local.data_versions.get(module),
            remote.data_versions.get(module),
        );
        if here < there {
            mine.push(module.clone());
        } else if there < here {
            theirs.push(module.clone());
        }
    }
    if !mine.is_empty() || !theirs.is_empty() {
        return Some(Pause::Update {
            older_here: mine,
            older_there: theirs,
        });
    }
    let ahead = remote.now_ms.saturating_sub(local.now_ms);
    if ahead > MAX_AHEAD_MS {
        return Some(Pause::clock_ahead(ahead));
    }
    if -ahead > MAX_AHEAD_MS {
        return Some(Pause::Clock {
            ahead_ms: -ahead,
            here: true,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mnemora_core::env::Environment;

    use super::*;
    use crate::net::PROTOCOL_VERSION;

    fn hs(todo: usize, now_ms: i64) -> Handshake {
        Handshake {
            protocol: PROTOCOL_VERSION,
            app_version: "1".into(),
            environment: Environment::Production,
            device: "d".into(),
            group: None,
            data_versions: BTreeMap::from([("core".into(), 3), ("todo".into(), todo)]),
            addresses: vec![],
            now_ms,
        }
    }

    #[test]
    fn matching_versions_and_clocks_do_not_pause() {
        assert_eq!(pause_of(&hs(5, 1_000), &hs(5, 1_000)), None);
        assert_eq!(
            pause_of(&hs(5, 0), &hs(5, MAX_AHEAD_MS)),
            None,
            "5 min is fine"
        );
        assert_eq!(pause_of(&hs(5, MAX_AHEAD_MS), &hs(5, 0)), None);
    }

    #[test]
    fn the_older_side_is_named_per_module() {
        assert_eq!(
            pause_of(&hs(5, 0), &hs(4, 0)),
            Some(Pause::Update {
                older_here: vec![],
                older_there: vec!["todo".into()]
            })
        );
        let mut mixed = hs(6, 0);
        mixed.data_versions.insert("core".into(), 2);
        assert_eq!(
            pause_of(&hs(5, 0), &mixed),
            Some(Pause::Update {
                older_here: vec!["todo".into()],
                older_there: vec!["core".into()]
            })
        );
        let mut lacks = hs(5, 0);
        lacks.data_versions.remove("todo");
        assert_eq!(
            pause_of(&hs(5, 0), &lacks),
            Some(Pause::Update {
                older_here: vec![],
                older_there: vec!["todo".into()]
            })
        );
    }

    #[test]
    fn a_clock_more_than_five_minutes_ahead_pauses_either_way_round() {
        let hours = 3 * 3_600_000;
        assert_eq!(
            pause_of(&hs(5, 0), &hs(5, hours)),
            Some(Pause::Clock {
                ahead_ms: hours,
                here: false
            })
        );
        assert_eq!(
            pause_of(&hs(5, hours), &hs(5, 0)),
            Some(Pause::Clock {
                ahead_ms: hours,
                here: true
            })
        );
        assert!(pause_of(&hs(5, 0), &hs(5, MAX_AHEAD_MS + 1)).is_some());
    }

    #[test]
    fn a_version_difference_is_told_before_a_clock_one() {
        assert!(matches!(
            pause_of(&hs(5, 0), &hs(4, 3_600_000)),
            Some(Pause::Update { .. })
        ));
    }
}
