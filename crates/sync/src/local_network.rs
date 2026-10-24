//! Telling that the system keeps this device off the local network (D12 to D15, ADR 0017).
//!
//! macOS 15 and later (Local Network privacy, Apple TN3179) refuses outgoing connections to LAN
//! addresses and every mDNS send until the person allows the app, and there is no API that reports
//! the state. The only sign is `EHOSTUNREACH` ("No route to host") from a connect or a send. That
//! error is also what a switched-off neighbour gives, so one failure proves nothing: the state is
//! "blocked" only after the same failure has come over and over for a while, and clears the moment
//! any LAN operation works. Android 17 says so outright: the app asks the system and reports a
//! refusal with [`LocalNetwork::set_os_denied`].
//!
//! Two sources feed [`Detector`]: dials to LAN addresses (the session manager's) and a probe, one
//! UDP datagram to the mDNS group, which depends on no other device being there. Where there is
//! no probe (not macOS, tests) the dials decide alone.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tokio::sync::Notify;

use crate::addresses;
use crate::net::NetError;

/// Failures needed before "blocked" is declared.
pub const MIN_FAILURES: u32 = 3;

/// How the detector waits and how often the probe runs; tests speed it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rhythm {
    /// How long the failures must have gone on, so a first prompt the person has not answered yet
    /// (which can refuse the first operations at once) is not taken for a refusal.
    pub grace: Duration,
    /// The pause between probes while the last one failed and the grace has not run out.
    pub fast: Duration,
    /// The pause between probes while they fail after the grace.
    pub slow: Duration,
    /// The pause between probes while they work: a permission switched off later is noticed.
    pub steady: Duration,
}

impl Default for Rhythm {
    fn default() -> Self {
        Self {
            grace: Duration::from_secs(30),
            fast: Duration::from_secs(3),
            slow: Duration::from_secs(10),
            steady: Duration::from_secs(60),
        }
    }
}

/// Whether `e` is what the system answers when it keeps the app off the local network:
/// `EHOSTUNREACH`, "No route to host" (65 on macOS).
pub fn is_denial(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::HostUnreachable
}

/// Whether `ip` is on the local network in the sense of the system's permission: a private or
/// link-local address, not loopback, not Tailscale (a VPN is outside it), not public.
pub fn is_lan(ip: IpAddr) -> bool {
    if addresses::is_tailscale(ip) {
        return false;
    }
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            // fe80::/10 link-local, fc00::/7 unique-local.
            first & 0xffc0 == 0xfe80 || first & 0xfe00 == 0xfc00
        }
    }
}

/// What one operation on the local network showed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Refused the way the permission refuses.
    Blocked,
    /// Got through, or got an answer that only a working network gives (a refused connection).
    Worked,
    /// Says nothing either way (a timeout, a network that is down).
    Unknown,
}

/// What a dial to `addr` that failed with `error` shows.
pub fn outcome_of_failure(addr: SocketAddr, error: &NetError) -> Outcome {
    // A link-local IPv6 address needs its interface named to be dialled; without it the system
    // answers "No route to host" with the permission wide open.
    let scopeless = matches!(addr.ip(), IpAddr::V6(v6) if v6.segments()[0] & 0xffc0 == 0xfe80);
    if !is_lan(addr.ip()) || scopeless {
        return Outcome::Unknown;
    }
    match error {
        NetError::Io(io) if is_denial(io) => Outcome::Blocked,
        NetError::Io(io) if io.kind() == io::ErrorKind::ConnectionRefused => Outcome::Worked,
        // Something answered at the other end: TLS, the handshake, a closed line.
        NetError::Io(_) | NetError::Timeout | NetError::Local(_) | NetError::Key(_) => {
            Outcome::Unknown
        }
        _ => Outcome::Worked,
    }
}

/// Where an observation came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A dial to a LAN address.
    Dial,
    /// The probe.
    Probe,
}

/// Decides, from a stream of outcomes, whether the local network is blocked.
#[derive(Debug, Default)]
pub struct Detector {
    grace: Duration,
    failures: u32,
    since: Option<Instant>,
    /// What the last probe showed, if there has been one: a working probe means a dial's
    /// `EHOSTUNREACH` is about that device, not about the permission.
    probe_ok: Option<bool>,
    blocked: bool,
}

impl Detector {
    /// A detector that waits `grace` of consistent failures before it blocks.
    pub fn new(grace: Duration) -> Self {
        Self {
            grace,
            ..Self::default()
        }
    }

    /// Takes one observation made at `now`; returns whether the network is blocked now.
    pub fn observe(&mut self, source: Source, outcome: Outcome, now: Instant) -> bool {
        match outcome {
            Outcome::Unknown => {}
            Outcome::Worked => {
                self.failures = 0;
                self.since = None;
                self.blocked = false;
                if source == Source::Probe {
                    self.probe_ok = Some(true);
                }
            }
            Outcome::Blocked => {
                if source == Source::Probe {
                    self.probe_ok = Some(false);
                } else if self.probe_ok == Some(true) {
                    // The probe says the network is open: this device is just not there.
                    return self.blocked;
                }
                self.failures = self.failures.saturating_add(1);
                let since = *self.since.get_or_insert(now);
                if self.failures >= MIN_FAILURES
                    && now.saturating_duration_since(since) >= self.grace
                {
                    self.blocked = true;
                }
            }
        }
        self.blocked
    }

    /// Whether the failures so far are inside the grace (the probe keeps trying fast).
    pub fn within_grace(&self, now: Instant) -> bool {
        self.since
            .is_some_and(|since| now.saturating_duration_since(since) < self.grace)
    }

    /// Whether the last probe failed.
    pub fn probe_failing(&self) -> bool {
        self.probe_ok == Some(false)
    }

    /// Whether the network is blocked.
    pub fn blocked(&self) -> bool {
        self.blocked
    }
}

type ProbeFn = dyn Fn() -> io::Result<()> + Send + Sync;

/// The local network's state for this device, shared by the session manager, the app and (on
/// Android) the permission the system reported. It outlives the network side of Sync: stopping
/// and starting that does not forget what is known.
pub struct LocalNetwork {
    detector: Mutex<Detector>,
    rhythm: Rhythm,
    os_denied: AtomicBool,
    probe: Option<Box<ProbeFn>>,
    /// Whether dials count. Where the system gives no such refusal (not macOS) an unreachable
    /// device would look like one, so only the system's own report decides there.
    dials_count: bool,
    /// What the window last heard, to raise `changed` only on a change.
    shown: AtomicBool,
    changed: Notify,
}

impl LocalNetwork {
    fn new(probe: Option<Box<ProbeFn>>, rhythm: Rhythm) -> Arc<Self> {
        Self::counting(probe, rhythm, true)
    }

    fn counting(probe: Option<Box<ProbeFn>>, rhythm: Rhythm, dials_count: bool) -> Arc<Self> {
        Arc::new(Self {
            dials_count,
            detector: Mutex::new(Detector::new(rhythm.grace)),
            rhythm,
            os_denied: AtomicBool::new(false),
            probe,
            shown: AtomicBool::new(false),
            changed: Notify::new(),
        })
    }

    /// For the app: on macOS the probe is a UDP datagram to the mDNS group; elsewhere the dials
    /// decide alone.
    pub fn system() -> Arc<Self> {
        if cfg!(target_os = "macos") {
            Self::new(Some(Box::new(probe_mdns_group)), Rhythm::default())
        } else {
            Self::counting(None, Rhythm::default(), false)
        }
    }

    /// Without a probe: only dials (and the system's report) decide.
    pub fn off() -> Arc<Self> {
        Self::new(None, Rhythm::default())
    }

    /// With `probe` as the probe (tests).
    pub fn with_probe(probe: impl Fn() -> io::Result<()> + Send + Sync + 'static) -> Arc<Self> {
        Self::new(Some(Box::new(probe)), Rhythm::default())
    }

    /// With `probe` as the probe and `rhythm` as the pace (tests).
    pub fn with_rhythm(
        probe: impl Fn() -> io::Result<()> + Send + Sync + 'static,
        rhythm: Rhythm,
    ) -> Arc<Self> {
        Self::new(Some(Box::new(probe)), rhythm)
    }

    /// Whether there is a probe to run.
    pub fn has_probe(&self) -> bool {
        self.probe.is_some()
    }

    fn detector(&self) -> std::sync::MutexGuard<'_, Detector> {
        self.detector.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether the system keeps this device off the local network, as far as is known.
    pub fn blocked(&self) -> bool {
        self.detector().blocked() || self.os_denied.load(Ordering::SeqCst)
    }

    /// Records what one operation on the local network showed.
    pub fn record(&self, source: Source, outcome: Outcome) {
        self.record_at(source, outcome, Instant::now());
    }

    /// [`record`](Self::record) at a given time.
    pub fn record_at(&self, source: Source, outcome: Outcome, now: Instant) {
        self.detector().observe(source, outcome, now);
        self.publish();
    }

    /// Records a dial to `addr` that failed with `error`.
    pub fn dial_failed(&self, addr: SocketAddr, error: &NetError) {
        if !self.dials_count {
            return;
        }
        self.record(Source::Dial, outcome_of_failure(addr, error));
    }

    /// Records a connection to `addr` that was made.
    pub fn dial_worked(&self, addr: SocketAddr) {
        if is_lan(addr.ip()) {
            self.record(Source::Dial, Outcome::Worked);
        }
    }

    /// Forgets what the dials and the probe showed (the network side starts afresh); what the
    /// system itself reported stays.
    pub fn reset(&self) {
        *self.detector() = Detector::new(self.rhythm.grace);
        self.publish();
    }

    /// The system said whether the app may use the local network (Android).
    pub fn set_os_denied(&self, denied: bool) {
        self.os_denied.store(denied, Ordering::SeqCst);
        self.publish();
    }

    fn publish(&self) {
        let now = self.blocked();
        if self.shown.swap(now, Ordering::SeqCst) != now {
            self.changed.notify_one();
        }
    }

    /// Resolves when `blocked` has changed since it was last awaited.
    pub async fn changed(&self) {
        self.changed.notified().await;
    }

    /// Runs the probe once and records it. Returns whether it worked; `true` without a probe.
    pub async fn run_probe(self: &Arc<Self>) -> bool {
        let Some(_) = &self.probe else {
            return true;
        };
        let this = Arc::clone(self);
        let result = tokio::task::spawn_blocking(move || this.probe.as_ref().map(|p| p())).await;
        let outcome = match result {
            Ok(Some(Ok(()))) => Outcome::Worked,
            Ok(Some(Err(e))) if is_denial(&e) => Outcome::Blocked,
            _ => Outcome::Unknown,
        };
        self.record(Source::Probe, outcome);
        outcome != Outcome::Blocked
    }

    /// How long to wait before the next probe, given how the last went.
    pub fn next_probe_in(&self, worked: bool) -> Duration {
        self.next_probe_at(worked, Instant::now())
    }

    fn next_probe_at(&self, worked: bool, now: Instant) -> Duration {
        if worked {
            self.rhythm.steady
        } else if self.detector().within_grace(now) {
            self.rhythm.fast
        } else {
            self.rhythm.slow
        }
    }
}

/// Sends a datagram with no questions to the mDNS group: nothing answers it, and macOS needs the
/// Local Network permission for the send (the first one also raises the prompt).
fn probe_mdns_group() -> io::Result<()> {
    let sent = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).and_then(|socket| {
        socket.connect((Ipv4Addr::new(224, 0, 0, 251), 5353))?;
        socket.send(&[0u8; 12]).map(drop)
    });
    match sent {
        // With no Wi-Fi or Ethernet up (or only a VPN) the same error comes from having no route
        // at all, which says nothing about the permission.
        Err(e) if is_denial(&e) && !on_a_lan() => {
            Err(io::Error::from(io::ErrorKind::NetworkUnreachable))
        }
        other => other,
    }
}

/// Whether some interface has a local network address.
fn on_a_lan() -> bool {
    if_addrs::get_if_addrs().is_ok_and(|all| all.iter().any(|i| is_lan(i.ip())))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRACE: Duration = Duration::from_secs(30);

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    fn at(secs: u64, base: Instant) -> Instant {
        base + Duration::from_secs(secs)
    }

    #[test]
    fn no_route_to_host_is_the_denial() {
        assert!(is_denial(&io::Error::from(io::ErrorKind::HostUnreachable)));
        assert!(!is_denial(&io::Error::from(
            io::ErrorKind::ConnectionRefused
        )));
        #[cfg(target_os = "macos")]
        assert!(is_denial(&io::Error::from_raw_os_error(65)));
    }

    #[test]
    fn only_lan_addresses_count() {
        for lan in [
            "192.168.1.9",
            "10.0.0.2",
            "172.16.4.4",
            "169.254.3.3",
            "fe80::1",
            "fd00::5",
        ] {
            assert!(is_lan(ip(lan)), "{lan}");
        }
        for outside in [
            "100.101.4.7",
            "fd7a:115c:a1e0::7",
            "127.0.0.1",
            "::1",
            "8.8.8.8",
            "2001:db8::1",
        ] {
            assert!(!is_lan(ip(outside)), "{outside}");
        }
    }

    #[test]
    fn a_dial_failure_is_read_by_where_it_went_and_how_it_failed() {
        let lan: SocketAddr = "192.168.1.9:47311".parse().unwrap();
        let tailscale: SocketAddr = "100.101.4.7:47311".parse().unwrap();
        let denied = || NetError::Io(io::Error::from(io::ErrorKind::HostUnreachable));
        let refused = NetError::Io(io::Error::from(io::ErrorKind::ConnectionRefused));
        assert_eq!(outcome_of_failure(lan, &denied()), Outcome::Blocked);
        assert_eq!(outcome_of_failure(tailscale, &denied()), Outcome::Unknown);
        assert_eq!(outcome_of_failure(lan, &refused), Outcome::Worked);
        assert_eq!(
            outcome_of_failure(lan, &NetError::Timeout),
            Outcome::Unknown
        );
        assert_eq!(outcome_of_failure(lan, &NetError::Closed), Outcome::Worked);
    }

    #[test]
    fn one_failure_is_not_a_block_and_neither_is_a_burst() {
        let base = Instant::now();
        let mut d = Detector::new(GRACE);
        assert!(!d.observe(Source::Dial, Outcome::Blocked, base));
        for s in 1..10 {
            assert!(
                !d.observe(Source::Dial, Outcome::Blocked, at(s, base)),
                "many failures inside the grace are the prompt still being answered"
            );
        }
    }

    #[test]
    fn failures_that_go_on_past_the_grace_block_and_any_success_clears() {
        let base = Instant::now();
        let mut d = Detector::new(GRACE);
        d.observe(Source::Dial, Outcome::Blocked, base);
        d.observe(Source::Dial, Outcome::Blocked, at(5, base));
        assert!(d.observe(Source::Dial, Outcome::Blocked, at(31, base)));
        assert!(
            d.observe(Source::Dial, Outcome::Unknown, at(32, base)),
            "a timeout says nothing"
        );
        assert!(!d.observe(Source::Dial, Outcome::Worked, at(33, base)));
        // And it takes the whole run again.
        assert!(!d.observe(Source::Dial, Outcome::Blocked, at(34, base)));
    }

    #[test]
    fn a_success_in_between_breaks_the_run() {
        let base = Instant::now();
        let mut d = Detector::new(GRACE);
        d.observe(Source::Dial, Outcome::Blocked, base);
        d.observe(Source::Dial, Outcome::Blocked, at(10, base));
        d.observe(Source::Dial, Outcome::Worked, at(20, base));
        d.observe(Source::Dial, Outcome::Blocked, at(30, base));
        assert!(!d.observe(Source::Dial, Outcome::Blocked, at(45, base)));
    }

    #[test]
    fn a_working_probe_means_a_dial_failure_is_about_the_device() {
        let base = Instant::now();
        let mut d = Detector::new(GRACE);
        d.observe(Source::Probe, Outcome::Worked, base);
        for s in 1..100 {
            assert!(!d.observe(Source::Dial, Outcome::Blocked, at(s, base)));
        }
    }

    #[test]
    fn a_failing_probe_blocks_by_itself_after_the_grace() {
        let base = Instant::now();
        let mut d = Detector::new(GRACE);
        for s in (0..30).step_by(3) {
            assert!(!d.observe(Source::Probe, Outcome::Blocked, at(s, base)));
        }
        assert!(d.observe(Source::Probe, Outcome::Blocked, at(30, base)));
        assert!(!d.observe(Source::Probe, Outcome::Worked, at(33, base)));
    }

    #[tokio::test]
    async fn the_change_is_announced_once_each_way() {
        let local = LocalNetwork::off();
        let base = Instant::now();
        local.record_at(Source::Dial, Outcome::Blocked, base);
        local.record_at(Source::Dial, Outcome::Blocked, at(10, base));
        assert!(!local.blocked());
        local.record_at(Source::Dial, Outcome::Blocked, at(40, base));
        assert!(local.blocked());
        tokio::time::timeout(Duration::from_secs(1), local.changed())
            .await
            .expect("announced");
        local.record_at(Source::Dial, Outcome::Blocked, at(41, base));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), local.changed())
                .await
                .is_err(),
            "not announced again"
        );
        local.record_at(Source::Dial, Outcome::Worked, at(42, base));
        assert!(!local.blocked());
        tokio::time::timeout(Duration::from_secs(1), local.changed())
            .await
            .expect("announced the clearing");
    }

    #[test]
    fn the_system_saying_no_blocks_at_once_and_yes_clears() {
        let local = LocalNetwork::off();
        local.set_os_denied(true);
        assert!(local.blocked());
        local.set_os_denied(false);
        assert!(!local.blocked());
    }

    #[tokio::test]
    async fn the_probe_is_read_by_how_it_fails() {
        let local =
            LocalNetwork::with_probe(|| Err(io::Error::from(io::ErrorKind::HostUnreachable)));
        assert!(!local.run_probe().await);
        assert!(local.detector().probe_failing());
        let other =
            LocalNetwork::with_probe(|| Err(io::Error::from(io::ErrorKind::PermissionDenied)));
        assert!(other.run_probe().await, "another error says nothing");
        assert!(
            LocalNetwork::off().run_probe().await,
            "no probe, nothing wrong"
        );
        assert!(LocalNetwork::with_probe(|| Ok(())).run_probe().await);
    }

    #[test]
    fn the_probe_hurries_only_while_it_may_still_be_a_prompt() {
        let local = LocalNetwork::with_probe(|| Ok(()));
        let base = Instant::now();
        assert_eq!(local.next_probe_at(true, base), Rhythm::default().steady);
        local.record_at(Source::Probe, Outcome::Blocked, base);
        assert_eq!(
            local.next_probe_at(false, at(1, base)),
            Rhythm::default().fast
        );
        assert_eq!(
            local.next_probe_at(false, at(40, base)),
            Rhythm::default().slow
        );
    }

    #[test]
    fn a_link_local_ipv6_dial_says_nothing() {
        let addr: SocketAddr = "[fe80::1]:47311".parse().unwrap();
        let denied = NetError::Io(io::Error::from(io::ErrorKind::HostUnreachable));
        assert_eq!(outcome_of_failure(addr, &denied), Outcome::Unknown);
    }

    #[test]
    fn where_the_system_gives_no_refusal_dials_never_block() {
        let local = LocalNetwork::counting(None, Rhythm::default(), false);
        let addr: SocketAddr = "192.168.1.9:47311".parse().unwrap();
        let denied = NetError::Io(io::Error::from(io::ErrorKind::HostUnreachable));
        for _ in 0..10 {
            local.dial_failed(addr, &denied);
        }
        assert_eq!(local.detector().failures, 0, "not even counted");
    }

    #[test]
    fn a_fresh_start_forgets_old_failures_but_not_what_the_system_said() {
        let local = LocalNetwork::off();
        let base = Instant::now();
        local.record_at(Source::Dial, Outcome::Blocked, base);
        local.record_at(Source::Dial, Outcome::Blocked, at(1, base));
        local.record_at(Source::Dial, Outcome::Blocked, at(40, base));
        assert!(local.blocked());
        local.reset();
        assert!(!local.blocked());
        local.set_os_denied(true);
        local.reset();
        assert!(local.blocked());
    }
}
