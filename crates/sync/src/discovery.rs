//! Finding each other on the local network with mDNS (service `_mnemora._tcp`, Sandbox
//! `_mnemora-sandbox._tcp`). What is advertised is opaque on purpose: a hash of the group id, the
//! device id, and a "pairing open" flag while a code is shown. No names, no platform, so public
//! Wi-Fi learns nothing about who this is.
//!
//! [`Discovery`] is the seam: [`Mdns`] is the real thing, [`Fixed`] a stand-in for tests.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use mnemora_core::env::Environment;
use ring::digest;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;

use crate::SyncError;
use crate::addresses;
use crate::pair::msg::hex;

/// The mDNS service type of an Environment: Production and Sandbox never see each other.
pub const fn service_type(environment: Environment) -> &'static str {
    match environment {
        Environment::Production => "_mnemora._tcp.local.",
        Environment::Sandbox => "_mnemora-sandbox._tcp.local.",
    }
}

/// What this device announces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advert {
    /// This device's id.
    pub device: String,
    /// The group id, if it has one; only a hash of it is announced.
    pub group: Option<String>,
    /// The port it listens on.
    pub port: u16,
    /// A code is shown right now.
    pub pairing_open: bool,
}

/// A device found on the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Its device id.
    pub device: String,
    /// Where to dial it (shareable addresses only).
    pub addrs: Vec<SocketAddr>,
    /// The port it listens on.
    pub port: u16,
    /// It is showing a pairing code.
    pub pairing_open: bool,
}

/// The first 16 hex digits of the group id's SHA-256: enough to tell groups apart, nothing more.
pub fn group_hash(group: &str) -> String {
    hex(&digest::digest(&digest::SHA256, group.as_bytes()).as_ref()[..8])
}

/// The TXT record: `d` device, `g` group hash (when there is a group), `p=1` while pairing is open.
pub fn txt(advert: &Advert) -> Vec<(String, String)> {
    let mut out = vec![("d".to_owned(), advert.device.clone())];
    if let Some(group) = &advert.group {
        out.push(("g".to_owned(), group_hash(group)));
    }
    if advert.pairing_open {
        out.push(("p".to_owned(), "1".to_owned()));
    }
    out
}

/// Reads a TXT record made by [`txt`]: the device id, the group hash and the pairing flag.
pub fn parse_txt<'a>(
    get: impl Fn(&str) -> Option<&'a str>,
) -> Option<(String, Option<String>, bool)> {
    let device = get("d").filter(|d| !d.is_empty())?;
    Some((
        device.to_owned(),
        get("g").map(str::to_owned),
        get("p") == Some("1"),
    ))
}

/// Devices found by a browse. Dropping it stops the browse.
pub struct Browsing {
    /// Devices as they are found (and found again when they change).
    pub found: UnboundedReceiver<Candidate>,
    _stop: Box<dyn Send>,
}

/// Announcing this device and finding others.
pub trait Discovery: Send + Sync {
    /// Announces this device, or stops announcing for `None` ("Visible on this network" is off).
    fn advertise(&self, advert: Option<&Advert>);
    /// Starts looking for other devices. Must be called inside the Tokio runtime.
    fn browse(&self) -> Result<Browsing, SyncError>;
    /// Live announcements plus live browses, for `RuntimeProbe`.
    fn active(&self) -> usize;
}

fn mdns_error(e: impl std::fmt::Display) -> SyncError {
    SyncError::Protocol(format!("local network discovery: {e}"))
}

/// mDNS through `mdns-sd`. Its daemon thread runs from [`Mdns::new`] until the value is dropped.
pub struct Mdns {
    daemon: ServiceDaemon,
    service_type: &'static str,
    registered: Mutex<Option<String>>,
    browses: Arc<AtomicUsize>,
}

impl Mdns {
    /// Starts the daemon for `environment`'s service type.
    pub fn new(environment: Environment) -> Result<Self, SyncError> {
        let daemon = ServiceDaemon::new().map_err(mdns_error)?;
        // "_mnemora-sandbox" is longer than RFC 6763's 15 bytes; the library only checks it.
        daemon.set_service_name_len_max(30).map_err(mdns_error)?;
        Ok(Self {
            daemon,
            service_type: service_type(environment),
            registered: Mutex::new(None),
            browses: Arc::new(AtomicUsize::new(0)),
        })
    }
}

impl Drop for Mdns {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}

impl Discovery for Mdns {
    fn advertise(&self, advert: Option<&Advert>) {
        let mut registered = self
            .registered
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(advert) = advert else {
            if let Some(name) = registered.take() {
                let _ = self.daemon.unregister(&name);
            }
            return;
        };
        let props = txt(advert);
        let info = ServiceInfo::new(
            self.service_type,
            &advert.device,
            &format!("{}.local.", advert.device),
            (),
            advert.port,
            &props[..],
        )
        .map(ServiceInfo::enable_addr_auto);
        match info {
            Ok(info) => {
                let name = info.get_fullname().to_owned();
                // Registering again re-announces with the new record.
                match self.daemon.register(info) {
                    Ok(()) => *registered = Some(name),
                    Err(e) => tracing::warn!(target: "sync", error = %e, "could not advertise"),
                }
            }
            Err(e) => tracing::warn!(target: "sync", error = %e, "could not describe the advert"),
        }
    }

    fn browse(&self) -> Result<Browsing, SyncError> {
        let events = self.daemon.browse(self.service_type).map_err(mdns_error)?;
        let (tx, found) = unbounded_channel();
        let task = tokio::spawn(async move {
            while let Ok(event) = events.recv_async().await {
                let ServiceEvent::ServiceResolved(info) = event else {
                    continue;
                };
                let Some((device, _group, pairing_open)) =
                    parse_txt(|k| info.get_property_val_str(k))
                else {
                    continue;
                };
                let port = info.get_port();
                let addrs: Vec<SocketAddr> = info
                    .get_addresses()
                    .iter()
                    .map(mdns_sd::ScopedIp::to_ip_addr)
                    .filter(|ip| addresses::shareable(*ip))
                    .map(|ip| SocketAddr::new(ip, port))
                    .collect();
                if tx
                    .send(Candidate {
                        device,
                        addrs,
                        port,
                        pairing_open,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        self.browses.fetch_add(1, Ordering::SeqCst);
        Ok(Browsing {
            found,
            _stop: Box::new(StopBrowse {
                daemon: self.daemon.clone(),
                service_type: self.service_type,
                task,
                browses: Arc::clone(&self.browses),
            }),
        })
    }

    fn active(&self) -> usize {
        usize::from(
            self.registered
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_some(),
        ) + self.browses.load(Ordering::SeqCst)
    }
}

struct StopBrowse {
    daemon: ServiceDaemon,
    service_type: &'static str,
    task: JoinHandle<()>,
    browses: Arc<AtomicUsize>,
}

impl Drop for StopBrowse {
    fn drop(&mut self) {
        self.task.abort();
        let _ = self.daemon.stop_browse(self.service_type);
        self.browses.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A stand-in for tests: it "finds" the addresses it was given, announcing nothing to the network.
#[derive(Default)]
pub struct Fixed {
    found: Mutex<Vec<SocketAddr>>,
    advert: Mutex<Option<Advert>>,
    browses: Arc<AtomicUsize>,
    /// Paired devices that "announced themselves": told to every browse, now and later.
    seen: Mutex<Vec<Candidate>>,
    watchers: Mutex<Vec<UnboundedSender<Candidate>>>,
}

impl Fixed {
    /// Finds nothing until [`Fixed::set`].
    pub fn new() -> Arc<Self> {
        Arc::default()
    }

    /// The devices every later browse finds, all showing a code.
    pub fn set(&self, addrs: Vec<SocketAddr>) {
        *self.found.lock().unwrap_or_else(PoisonError::into_inner) = addrs;
    }

    /// A device announces itself on the network, as a paired device would over mDNS: every
    /// browse, running or started later, is told.
    pub fn announce(&self, device: &str, addr: SocketAddr) {
        let candidate = Candidate {
            device: device.to_owned(),
            addrs: vec![addr],
            port: addr.port(),
            pairing_open: false,
        };
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(candidate.clone());
        self.watchers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|w| w.send(candidate.clone()).is_ok());
    }

    /// What was last announced, if anything.
    pub fn advertised(&self) -> Option<Advert> {
        self.advert
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

struct CountBrowse(Arc<AtomicUsize>);

impl Drop for CountBrowse {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Discovery for Fixed {
    fn advertise(&self, advert: Option<&Advert>) {
        *self.advert.lock().unwrap_or_else(PoisonError::into_inner) = advert.cloned();
    }

    fn browse(&self) -> Result<Browsing, SyncError> {
        let (tx, found) = unbounded_channel();
        for addr in self
            .found
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
        {
            let _ = tx.send(Candidate {
                device: format!("found-at-{addr}"),
                addrs: vec![*addr],
                port: addr.port(),
                pairing_open: true,
            });
        }
        for candidate in self
            .seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
        {
            let _ = tx.send(candidate.clone());
        }
        self.watchers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(tx);
        self.browses.fetch_add(1, Ordering::SeqCst);
        Ok(Browsing {
            found,
            _stop: Box::new(CountBrowse(Arc::clone(&self.browses))),
        })
    }

    fn active(&self) -> usize {
        usize::from(self.advertised().is_some()) + self.browses.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advert(group: Option<&str>, open: bool) -> Advert {
        Advert {
            device: "0192-dev".into(),
            group: group.map(str::to_owned),
            port: 47311,
            pairing_open: open,
        }
    }

    #[test]
    fn production_and_sandbox_use_different_services() {
        assert_eq!(
            service_type(Environment::Production),
            "_mnemora._tcp.local."
        );
        assert_eq!(
            service_type(Environment::Sandbox),
            "_mnemora-sandbox._tcp.local."
        );
    }

    #[test]
    fn the_record_holds_only_opaque_ids_and_the_pairing_flag() {
        let rec = txt(&advert(Some("my-group-id"), true));
        let keys: Vec<&str> = rec.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["d", "g", "p"]);
        assert!(
            !rec.iter().any(|(_, v)| v.contains("my-group-id")),
            "{rec:?}"
        );
        let no_group = txt(&advert(None, false));
        assert_eq!(no_group, vec![("d".to_owned(), "0192-dev".to_owned())]);
    }

    #[test]
    fn a_record_reads_back() {
        let rec = txt(&advert(Some("g"), true));
        let get = |k: &str| {
            rec.iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(
            parse_txt(get),
            Some(("0192-dev".to_owned(), Some(group_hash("g")), true))
        );
        assert_eq!(parse_txt(|_| None), None);
        assert_eq!(
            parse_txt(|k| (k == "d").then_some("x")),
            Some(("x".to_owned(), None, false))
        );
    }

    #[test]
    fn the_group_hash_is_stable_short_and_distinct() {
        assert_eq!(group_hash("a"), group_hash("a"));
        assert_ne!(group_hash("a"), group_hash("b"));
        assert_eq!(group_hash("a").len(), 16);
    }
}

#[cfg(test)]
mod network_tests {
    use std::time::Duration;

    use super::*;

    /// Needs working multicast on the machine, so it is not part of the normal run:
    /// `cargo test -p mnemora-sync mdns -- --ignored`.
    #[tokio::test]
    #[ignore = "needs multicast on the network"]
    async fn mdns_announces_and_finds_a_device_with_the_pairing_flag() {
        let (a, b) = (
            Mdns::new(Environment::Sandbox).unwrap(),
            Mdns::new(Environment::Sandbox).unwrap(),
        );
        let advert = Advert {
            device: "0192-test-device".into(),
            group: None,
            port: 47999,
            pairing_open: true,
        };
        a.advertise(Some(&advert));
        let mut browsing = b.browse().unwrap();
        let found = tokio::time::timeout(Duration::from_secs(8), async {
            while let Some(c) = browsing.found.recv().await {
                if c.device == advert.device {
                    return Some(c);
                }
            }
            None
        })
        .await
        .expect("not found in time")
        .unwrap();
        assert!(found.pairing_open);
        assert_eq!(found.port, 47999);
        assert_eq!(a.active(), 1);
        assert_eq!(b.active(), 1);
        a.advertise(None);
        assert_eq!(a.active(), 0);
    }
}
