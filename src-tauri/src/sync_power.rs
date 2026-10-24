//! What lets Sync's network run (#74). Sync on is necessary, not sufficient:
//!
//! - Android: only while the app is in front, plus a short flush after it leaves (pending changes
//!   get out for up to 5 s, then the listener, discovery, dials, connections and timers all stop).
//!   Coming back starts them again with an immediate round.
//! - Android on mobile data: nothing runs unless "Allow over mobile data" is on (then Sync reaches
//!   the other devices over Tailscale). No dials, no sessions, no listener while it is off.
//! - Desktop: no lifecycle events arrive, so the state stays "in front" and Sync runs as long as
//!   the process does, window or not.
//!
//! Android's activity and connectivity callbacks reach [`foreground`] and [`link`] over JNI
//! (`android.rs`), not through the page: the `WebView` may be suspended in the background, and the
//! flush timer must be Rust's own.

// Off Android only the tests drive the lifecycle.
#![cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]

#[cfg(target_os = "android")]
pub mod android;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use mnemora_core::device::SyncDevice;
use tauri::{AppHandle, Manager, Runtime};

use crate::state::{AppState, lock};

/// How long pending changes get to leave after the app goes to the background.
pub const GRACE: Duration = Duration::from_secs(5);
/// How often the flush checks whether the sessions are done.
const POLL: Duration = Duration::from_millis(200);
/// The flush waits at least this long before it may end early, so the sessions it just asked for
/// have started.
const MIN_FLUSH: Duration = Duration::from_secs(1);

/// What kind of network the device is on, as the Android side reports it. Tailscale (a VPN) counts
/// as the network underneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    /// Not reported (desktop, or the Android side has not said yet): never blocks Sync.
    Unknown,
    /// No network.
    Offline,
    /// Wi-Fi, Ethernet or anything else that is not mobile data.
    Wifi,
    /// Mobile data, with no Wi-Fi or Ethernet beside it.
    Mobile,
}

impl Link {
    /// The code `SyncBridge.kt` sends: 0 offline, 1 Wi-Fi/other, 2 mobile data.
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Offline,
            1 => Self::Wifi,
            2 => Self::Mobile,
            _ => Self::Unknown,
        }
    }
}

/// Whether the app is in front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Front,
    /// Just left: the flush window.
    Leaving,
    /// In the background past the flush.
    Away,
}

/// Everything the decision reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conditions {
    pub enabled: bool,
    pub screen: Screen,
    pub link: Link,
    pub mobile_data: bool,
}

/// Whether Sync's network (listener, discovery, dials, connections, timers) should exist.
pub fn should_run(c: Conditions) -> bool {
    c.enabled && c.screen != Screen::Away && (c.link != Link::Mobile || c.mobile_data)
}

/// Whether Sync is waiting only because the phone is on mobile data with the setting off (the
/// Devices tab says so, instead of just "not listening").
pub fn paused_on_mobile_data(c: Conditions) -> bool {
    c.enabled && c.link == Link::Mobile && !c.mobile_data
}

/// Whether the flush after leaving is over: the grace is used up, or it is past the minimum and
/// no session is running.
pub fn flush_over(elapsed: Duration, grace: Duration, busy: bool) -> bool {
    elapsed >= grace || (elapsed >= MIN_FLUSH && !busy)
}

struct Inner {
    screen: Screen,
    link: Link,
    /// A start is in flight (binding the port): another must not begin.
    starting: bool,
    /// Counts screen changes, so a flush that was overtaken by a return knows to stand down.
    turn: u64,
}

/// The lifecycle inputs. Lives in [`AppState`].
pub struct Power {
    inner: Mutex<Inner>,
    grace: Duration,
}

impl Default for Power {
    fn default() -> Self {
        Self::new()
    }
}

impl Power {
    /// In front, link unknown, 5 s grace.
    pub fn new() -> Self {
        Self::with_grace(GRACE)
    }

    pub fn with_grace(grace: Duration) -> Self {
        let power = Self {
            inner: Mutex::new(Inner {
                screen: Screen::Front,
                link: Link::Unknown,
                starting: false,
                turn: 0,
            }),
            grace,
        };
        #[cfg(target_os = "android")]
        {
            // What the Android side said before the app finished starting.
            let early = android::early();
            lock(&power.inner).link = early.link;
            if !early.front {
                lock(&power.inner).screen = Screen::Away;
            }
        }
        power
    }

    /// What decides right now, with the device's Sync settings.
    pub fn conditions(&self, sync: &SyncDevice) -> Conditions {
        let inner = lock(&self.inner);
        Conditions {
            enabled: sync.enabled,
            screen: inner.screen,
            link: inner.link,
            mobile_data: sync.mobile_data,
        }
    }

    /// Marks a start as begun; false when one is already in flight.
    pub fn begin_start(&self) -> bool {
        let mut inner = lock(&self.inner);
        !std::mem::replace(&mut inner.starting, true)
    }

    pub fn end_start(&self) {
        lock(&self.inner).starting = false;
    }

    #[cfg(test)]
    fn screen(&self) -> Screen {
        lock(&self.inner).screen
    }
}

fn state<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::State<'_, AppState>> {
    app.try_state::<AppState>()
}

/// Makes the network match the conditions: starts it when it should run and does not, drops it
/// when it runs and should not. Cheap when nothing needs to change.
pub fn settle<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = state(app) else { return };
    let wanted = should_run(state.power.conditions(&state.device.get().sync));
    let running = state.sync.network_running();
    if wanted && !running {
        crate::sync::launch_network(app);
    } else if !wanted && let Some(network) = state.sync.take_network() {
        tracing::info!(target: "sync", "network stopped");
        // Dropped on the runtime, where its tasks live.
        tauri::async_runtime::spawn(async move { drop(network) });
        // The Devices tab reads "not nearby" for everyone now.
        crate::sync::announce_peers(app);
    }
}

/// The app came to the front (`true`) or left it (`false`).
pub fn foreground<R: Runtime>(app: &AppHandle<R>, front: bool) {
    let Some(state) = state(app) else { return };
    if front {
        {
            let mut inner = lock(&state.power.inner);
            inner.screen = Screen::Front;
            inner.turn += 1;
        }
        settle(app);
        // Back inside the flush window the network never stopped: try everyone again now.
        state.sync.resume();
        state.sync.flush();
        return;
    }
    let turn = {
        let mut inner = lock(&state.power.inner);
        if inner.screen != Screen::Front {
            return;
        }
        inner.turn += 1;
        inner.screen = if state.sync.network_running() {
            Screen::Leaving
        } else {
            Screen::Away
        };
        inner.turn
    };
    if lock(&state.power.inner).screen == Screen::Away {
        return;
    }
    state.sync.flush();
    // The sessions the flush asked for run on Tauri's runtime; this only watches the clock.
    let app = app.clone();
    std::thread::spawn(move || flush_then_stop(&app, turn));
}

/// Waits for the flush to finish (at most the grace), then stops the network unless the app
/// came back meanwhile.
fn flush_then_stop<R: Runtime>(app: &AppHandle<R>, turn: u64) {
    let started = Instant::now();
    loop {
        std::thread::sleep(POLL);
        let Some(state) = state(app) else { return };
        if lock(&state.power.inner).turn != turn {
            return;
        }
        if flush_over(started.elapsed(), state.power.grace, state.sync.busy()) {
            break;
        }
    }
    let Some(state) = state(app) else { return };
    {
        let mut inner = lock(&state.power.inner);
        if inner.turn != turn {
            return;
        }
        inner.screen = Screen::Away;
    }
    tracing::info!(target: "sync", "app in the background: stopping");
    settle(app);
}

/// The network type changed (or the same kind of network was joined again).
pub fn link<R: Runtime>(app: &AppHandle<R>, now: Link) {
    let Some(state) = state(app) else { return };
    lock(&state.power.inner).link = now;
    settle(app);
    // The Devices tab says why Sync waits (or no longer does) on mobile data.
    crate::sync::announce_peers(app);
    // Devices that could not be reached are worth another try on the new network.
    state.sync.resume();
}

/// The screen state, for tests.
#[cfg(test)]
pub fn test_screen<R: Runtime>(app: &AppHandle<R>) -> Screen {
    app.state::<AppState>().power.screen()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(screen: Screen, link: Link, mobile_data: bool) -> Conditions {
        Conditions {
            enabled: true,
            screen,
            link,
            mobile_data,
        }
    }

    #[test]
    fn nothing_runs_while_sync_is_off() {
        for screen in [Screen::Front, Screen::Leaving, Screen::Away] {
            for link in [Link::Unknown, Link::Offline, Link::Wifi, Link::Mobile] {
                let mut c = on(screen, link, true);
                c.enabled = false;
                assert!(!should_run(c), "{screen:?} {link:?}");
            }
        }
    }

    #[test]
    fn in_front_on_wifi_it_runs_and_so_when_the_link_is_unknown_or_down() {
        for link in [Link::Unknown, Link::Offline, Link::Wifi] {
            assert!(should_run(on(Screen::Front, link, false)));
        }
    }

    #[test]
    fn in_the_flush_window_it_still_runs_and_after_it_it_does_not() {
        assert!(should_run(on(Screen::Leaving, Link::Wifi, false)));
        assert!(!should_run(on(Screen::Away, Link::Wifi, false)));
        assert!(!should_run(on(Screen::Away, Link::Wifi, true)));
        assert!(!should_run(on(Screen::Away, Link::Unknown, false)));
    }

    #[test]
    fn mobile_data_needs_the_setting() {
        assert!(!should_run(on(Screen::Front, Link::Mobile, false)));
        assert!(should_run(on(Screen::Front, Link::Mobile, true)));
        // The flush window follows the same rule: no flush over mobile data that is not allowed.
        assert!(!should_run(on(Screen::Leaving, Link::Mobile, false)));
        assert!(should_run(on(Screen::Leaving, Link::Mobile, true)));
        assert!(!should_run(on(Screen::Away, Link::Mobile, true)));
    }

    #[test]
    fn it_is_paused_on_mobile_data_only_while_sync_is_on_and_the_setting_is_off() {
        assert!(paused_on_mobile_data(on(
            Screen::Front,
            Link::Mobile,
            false
        )));
        assert!(!paused_on_mobile_data(on(
            Screen::Front,
            Link::Mobile,
            true
        )));
        for link in [Link::Unknown, Link::Offline, Link::Wifi] {
            assert!(!paused_on_mobile_data(on(Screen::Front, link, false)));
        }
        let mut off = on(Screen::Front, Link::Mobile, false);
        off.enabled = false;
        assert!(!paused_on_mobile_data(off));
    }

    #[test]
    fn the_flush_ends_at_the_grace_or_when_idle_after_the_minimum() {
        let grace = Duration::from_secs(5);
        let ms = Duration::from_millis;
        assert!(
            !flush_over(ms(200), grace, false),
            "too early to call it done"
        );
        assert!(
            !flush_over(ms(2000), grace, true),
            "a session is still running"
        );
        assert!(flush_over(ms(1000), grace, false));
        assert!(!flush_over(ms(4800), grace, true));
        assert!(
            flush_over(ms(5000), grace, true),
            "never longer than the grace"
        );
    }

    #[test]
    fn only_one_start_can_be_in_flight() {
        let power = Power::with_grace(GRACE);
        assert!(power.begin_start());
        assert!(!power.begin_start(), "a second start while one binds");
        power.end_start();
        assert!(power.begin_start());
    }

    #[test]
    fn link_codes_match_the_android_side() {
        assert_eq!(Link::from_code(0), Link::Offline);
        assert_eq!(Link::from_code(1), Link::Wifi);
        assert_eq!(Link::from_code(2), Link::Mobile);
        assert_eq!(Link::from_code(-1), Link::Unknown);
        assert_eq!(Link::from_code(7), Link::Unknown);
    }
}
