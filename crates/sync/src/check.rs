//! "Check" in the device sheet: dials a typed Address and says what answered, in words.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use serde::Serialize;
use specta::Type;

use crate::address::{Address, ResolveError};
use crate::net::{NetConfig, NetError, Probed, Refusal, probe};

/// How long a Check may take: a name to look up, each address to answer, and all of it. A Check
/// never hangs, whatever is (or is not) at the other end.
#[derive(Debug, Clone, Copy)]
struct Limits {
    lookup: Duration,
    each: Duration,
    total: Duration,
}

const LIMITS: Limits = Limits {
    lookup: Duration::from_secs(3),
    each: Duration::from_secs(4),
    total: Duration::from_secs(10),
};

/// What a Check found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    /// The device answered.
    Reached,
    /// The text is not an address.
    Invalid,
    /// The name does not resolve.
    NotFound,
    /// Nothing answered there.
    Unreachable,
    /// Nothing answered in time.
    TimedOut,
    /// A different device answered.
    WrongDevice,
    /// Something answered that is not a device paired with this one, or no longer trusts it.
    NotPaired,
    /// Something answered that is not a Mnemora.
    Other,
}

/// The result of a Check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct CheckResult {
    /// The device was reached.
    pub reached: bool,
    /// What happened.
    pub kind: CheckKind,
    /// "Reached Pixel 8 in 84 ms", or why not.
    pub message: String,
    /// How long the device took to answer, when it did.
    pub ms: Option<u32>,
}

impl CheckResult {
    fn failed(kind: CheckKind, message: impl Into<String>) -> Self {
        Self {
            reached: false,
            kind,
            message: message.into(),
            ms: None,
        }
    }
}

/// The device a Check expects to find.
pub struct Expect<'a> {
    /// Its id.
    pub device: &'a str,
    /// Its name here.
    pub name: &'a str,
}

/// Why a probe failed, in words for `expect`.
fn failure(error: &NetError, expect: &Expect<'_>) -> CheckResult {
    match error {
        NetError::Timeout => CheckResult::failed(
            CheckKind::TimedOut,
            format!(
                "Timed out. {} may be asleep, off or on another network.",
                expect.name
            ),
        ),
        NetError::Tls(_) | NetError::Refused(Refusal::Unpinned) | NetError::Closed => {
            CheckResult::failed(
                CheckKind::NotPaired,
                "That is not a device paired with this one, or it has forgotten this device.",
            )
        }
        NetError::Malformed(_) | NetError::TooLarge(_) => CheckResult::failed(
            CheckKind::Other,
            "Something answered, but it is not Mnemora.",
        ),
        _ => CheckResult::failed(
            CheckKind::Unreachable,
            "Nothing answered there. Is Mnemora open with Sync on?",
        ),
    }
}

/// Dials `text` and says whether `expect` is there. `name_of` turns another paired device's id
/// into its name. The second value is the address that answered, for the caller to remember.
pub async fn check(
    config: &NetConfig,
    expect: &Expect<'_>,
    text: &str,
    name_of: impl Fn(&str) -> Option<String>,
) -> (CheckResult, Option<SocketAddr>) {
    check_within(LIMITS, config, expect, text, name_of).await
}

async fn check_within(
    limits: Limits,
    config: &NetConfig,
    expect: &Expect<'_>,
    text: &str,
    name_of: impl Fn(&str) -> Option<String>,
) -> (CheckResult, Option<SocketAddr>) {
    let address = match Address::parse(text) {
        Ok(a) => a,
        Err(e) => return (CheckResult::failed(CheckKind::Invalid, e.message()), None),
    };
    let addrs = match address.resolve(limits.lookup).await {
        Ok(a) => a,
        Err(ResolveError::NotFound) => {
            return (
                CheckResult::failed(
                    CheckKind::NotFound,
                    "Could not find that name. Check the spelling and that Tailscale is connected.",
                ),
                None,
            );
        }
        Err(ResolveError::TimedOut) => {
            return (
                CheckResult::failed(CheckKind::TimedOut, "Looking up that name took too long."),
                None,
            );
        }
    };
    let started = Instant::now();
    let mut last = None;
    for addr in addrs {
        let left = limits.total.saturating_sub(started.elapsed());
        if left.is_zero() {
            last = Some(NetError::Timeout);
            break;
        }
        match tokio::time::timeout(limits.each.min(left), probe(config, addr)).await {
            Ok(Ok(found)) => {
                let ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
                return (verdict(&found, expect, ms, &name_of), Some(addr));
            }
            Ok(Err(e)) => last = Some(e),
            Err(_) => last = Some(NetError::Timeout),
        }
    }
    let result = last.map_or_else(
        || CheckResult::failed(CheckKind::Unreachable, "Nothing answered there."),
        |e| failure(&e, expect),
    );
    (result, None)
}

/// What a device that answered means for the one that was expected.
fn verdict(
    found: &Probed,
    expect: &Expect<'_>,
    ms: u32,
    name_of: &impl Fn(&str) -> Option<String>,
) -> CheckResult {
    if found.handshake.device != expect.device {
        let who = name_of(&found.handshake.device);
        let message = match who {
            Some(who) => format!("That is {who}, not {}.", expect.name),
            None => format!("That is another device, not {}.", expect.name),
        };
        return CheckResult::failed(CheckKind::WrongDevice, message);
    }
    CheckResult {
        reached: true,
        kind: CheckKind::Reached,
        message: format!("Reached {} in {ms} ms", expect.name),
        ms: Some(ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect() -> Expect<'static> {
        Expect {
            device: "pixel",
            name: "Pixel 8",
        }
    }

    #[tokio::test]
    async fn a_listener_that_never_answers_times_out() {
        use crate::net::{DeviceIdentity, LocalHandshake, Trust};
        use std::sync::Arc;
        // Accepts and says nothing, as a dead host behind a firewall might.
        let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = silent.local_addr().unwrap().port();
        let config = NetConfig {
            identity: Arc::new(DeviceIdentity::generate().unwrap()),
            trust: Trust::new(),
            local: LocalHandshake::new(|| Err(crate::error::SyncError::NotReady)),
        };
        let limits = Limits {
            lookup: Duration::from_millis(200),
            each: Duration::from_millis(300),
            total: Duration::from_millis(500),
        };
        let started = Instant::now();
        let (result, worked) = check_within(
            limits,
            &config,
            &expect(),
            &format!("127.0.0.1:{port}"),
            |_| None,
        )
        .await;
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(result.kind, CheckKind::TimedOut, "{result:?}");
        assert!(worked.is_none());
        drop(silent);
    }

    #[test]
    fn each_failure_has_its_own_words() {
        let kind = |e: NetError| failure(&e, &expect()).kind;
        assert_eq!(kind(NetError::Timeout), CheckKind::TimedOut);
        assert_eq!(kind(NetError::Tls("x".into())), CheckKind::NotPaired);
        assert_eq!(kind(NetError::Closed), CheckKind::NotPaired);
        assert_eq!(
            kind(NetError::Refused(Refusal::Unpinned)),
            CheckKind::NotPaired
        );
        assert_eq!(kind(NetError::Malformed("x".into())), CheckKind::Other);
        let refused = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        assert_eq!(kind(NetError::Io(refused)), CheckKind::Unreachable);
    }
}
