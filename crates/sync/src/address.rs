//! The Address a person may type for a device (a Tailscale `MagicDNS` name, a host, an IP): what is
//! accepted, how it is stored, and how it is turned into socket addresses to dial.
//!
//! Accepted: `pixel.tail1234.ts.net`, `host`, `host:47311`, `100.88.12.4`, `100.88.12.4:47311`,
//! `[fd7a:115c:a1e0::1]` and `[fd7a:115c:a1e0::1]:47311`. The port defaults to
//! [`DEFAULT_PORT`]. The browser mock runs the same rules (`src/modules/sync/address.ts`); both
//! read `src/lib/ipc/fixtures/sync-rules.json`.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use crate::net::DEFAULT_PORT;

/// Longest host name (DNS limit), and a label's.
const MAX_HOST: usize = 253;
const MAX_LABEL: usize = 63;

/// Why a typed address was refused. The message says what to fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressError {
    /// Nothing typed.
    Empty,
    /// Not a host name or IP address.
    Host,
    /// The port is not a number from 1 to 65535.
    Port,
    /// An IPv6 address without square brackets.
    Brackets,
}

impl AddressError {
    /// What to tell the person.
    pub const fn message(self) -> &'static str {
        match self {
            Self::Empty => "Type an address",
            Self::Host => "That isn’t a host name or an IP address",
            Self::Port => "The port is a number from 1 to 65535",
            Self::Brackets => "Put an IPv6 address in square brackets, like [fd7a::1]",
        }
    }
}

impl fmt::Display for AddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for AddressError {}

/// The host part of an [`Address`].
#[derive(Debug, Clone, PartialEq, Eq)]
enum Host {
    Ip(IpAddr),
    Name(String),
}

/// A typed address that passed the rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    host: Host,
    port: Option<u16>,
}

impl Address {
    /// Reads what was typed.
    pub fn parse(text: &str) -> Result<Self, AddressError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(AddressError::Empty);
        }
        if let Some(rest) = text.strip_prefix('[') {
            let (inner, after) = rest.split_once(']').ok_or(AddressError::Host)?;
            // No embedded IPv4 (`::ffff:1.2.3.4`): the browser mock does not read it either.
            let ip: Ipv6Addr = inner
                .parse()
                .ok()
                .filter(|_| !inner.contains('.'))
                .ok_or(AddressError::Host)?;
            let port = match after {
                "" => None,
                _ => Some(port_of(after.strip_prefix(':').ok_or(AddressError::Host)?)?),
            };
            return Ok(Self {
                host: Host::Ip(IpAddr::V6(ip)),
                port,
            });
        }
        if text.parse::<Ipv6Addr>().is_ok() {
            return Err(AddressError::Brackets);
        }
        let (host, port) = match text.split_once(':') {
            // Two colons, or a scheme (`http://…`): not a host with a port.
            Some((_, rest)) if rest.contains(':') || rest.starts_with("//") => {
                return Err(AddressError::Host);
            }
            Some((host, port)) => (host, Some(port_of(port)?)),
            None => (text, None),
        };
        let host = match host.parse::<Ipv4Addr>() {
            Ok(ip) => Host::Ip(IpAddr::V4(ip)),
            Err(_) => Host::Name(name_of(host)?),
        };
        Ok(Self { host, port })
    }

    /// The text that is stored and shown back: lower case, IPv6 in brackets, the port only if one
    /// was typed.
    pub fn normalized(&self) -> String {
        let host = match &self.host {
            Host::Ip(IpAddr::V6(ip)) => format!("[{ip}]"),
            Host::Ip(ip) => ip.to_string(),
            Host::Name(name) => name.clone(),
        };
        match self.port {
            Some(port) => format!("{host}:{port}"),
            None => host,
        }
    }

    /// The port to dial.
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(DEFAULT_PORT)
    }

    /// The socket addresses this stands for. A name is looked up (it may answer with several); an
    /// IP is itself. Fails when the name does not resolve or takes longer than `within`.
    pub async fn resolve(&self, within: Duration) -> Result<Vec<SocketAddr>, ResolveError> {
        let port = self.port();
        match &self.host {
            Host::Ip(ip) => Ok(vec![SocketAddr::new(*ip, port)]),
            Host::Name(name) => {
                let found =
                    tokio::time::timeout(within, tokio::net::lookup_host((name.as_str(), port)))
                        .await
                        .map_err(|_| ResolveError::TimedOut)?
                        .map_err(|_| ResolveError::NotFound)?;
                let found: Vec<SocketAddr> = found.collect();
                if found.is_empty() {
                    Err(ResolveError::NotFound)
                } else {
                    Ok(found)
                }
            }
        }
    }
}

/// Why a name could not be turned into addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveError {
    /// No such name.
    NotFound,
    /// The lookup took too long.
    TimedOut,
}

fn port_of(text: &str) -> Result<u16, AddressError> {
    match text.parse::<u16>() {
        Ok(port) if port > 0 && text.bytes().all(|b| b.is_ascii_digit()) => Ok(port),
        _ => Err(AddressError::Port),
    }
}

/// A host name in lower case, without a trailing dot.
fn name_of(text: &str) -> Result<String, AddressError> {
    let name = text.strip_suffix('.').unwrap_or(text).to_ascii_lowercase();
    let label_ok = |l: &str| {
        !l.is_empty()
            && l.len() <= MAX_LABEL
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    };
    let numeric = name.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    if name.is_empty() || name.len() > MAX_HOST || numeric || !name.split('.').all(label_ok) {
        return Err(AddressError::Host);
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rules the browser mock also runs (`address.test.ts`).
    #[test]
    fn addresses_match_the_shared_fixture() {
        const RULES: &str = include_str!("../../../src/lib/ipc/fixtures/sync-rules.json");
        let rules: serde_json::Value = serde_json::from_str(RULES).unwrap();
        for case in rules["address_cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let got = Address::parse(case["input"].as_str().unwrap());
            match (case.get("stored"), case.get("error")) {
                (Some(stored), _) => {
                    let got = got.unwrap_or_else(|e| panic!("{name}: refused: {e}"));
                    assert_eq!(got.normalized(), stored.as_str().unwrap(), "{name}");
                    assert_eq!(
                        u64::from(got.port()),
                        case["port"].as_u64().unwrap(),
                        "{name}"
                    );
                }
                (None, Some(error)) => {
                    let e = got.expect_err(name);
                    assert_eq!(e.to_string(), error.as_str().unwrap(), "{name}");
                }
                _ => panic!("{name}: needs `stored` or `error`"),
            }
        }
    }

    #[test]
    fn an_ip_resolves_to_itself_with_the_default_port() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let a = Address::parse("100.88.12.4").unwrap();
        let got = rt.block_on(a.resolve(Duration::from_secs(1))).unwrap();
        assert_eq!(got, ["100.88.12.4:47311".parse::<SocketAddr>().unwrap()]);
        let a = Address::parse("[fd7a::1]:9").unwrap();
        let got = rt.block_on(a.resolve(Duration::from_secs(1))).unwrap();
        assert_eq!(got, ["[fd7a::1]:9".parse::<SocketAddr>().unwrap()]);
    }

    #[test]
    fn localhost_resolves() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let a = Address::parse("localhost:5000").unwrap();
        let got = rt.block_on(a.resolve(Duration::from_secs(3))).unwrap();
        assert!(got.iter().all(|s| s.port() == 5000 && s.ip().is_loopback()));
    }
}
