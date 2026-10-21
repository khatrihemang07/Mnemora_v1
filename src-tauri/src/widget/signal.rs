//! Tells the Todo widget that Todo changed, so it redraws (ADR 0014). One signal is held in app
//! state and every Todo write command goes through it; reads, Comments and Settings never do.

use std::sync::atomic::{AtomicU32, Ordering};

/// Notifies every Todo widget of a change. On Android that is a package-scoped broadcast; on
/// desktop it does nothing; tests read how many times it fired.
pub struct WidgetSignal {
    sent: AtomicU32,
    notify: fn(),
}

impl WidgetSignal {
    /// The signal for this platform.
    pub fn platform() -> Self {
        Self::with(platform_notify)
    }

    /// A signal that only counts, for tests.
    #[cfg(test)]
    pub fn silent() -> Self {
        Self::with(|| {})
    }

    fn with(notify: fn()) -> Self {
        Self {
            sent: AtomicU32::new(0),
            notify,
        }
    }

    /// Todo data changed: the widgets should redraw.
    pub fn todo_changed(&self) {
        self.sent.fetch_add(1, Ordering::Relaxed);
        (self.notify)();
    }

    /// How many times the signal fired.
    #[cfg(test)]
    pub fn sent(&self) -> u32 {
        self.sent.load(Ordering::Relaxed)
    }
}

#[cfg(target_os = "android")]
fn platform_notify() {
    super::android::notify_todo_changed();
}

#[cfg(not(target_os = "android"))]
fn platform_notify() {}
