//! Tells the Todo widget that Todo changed, so it redraws (ADR 0014). One signal is held in app
//! state and every Todo write command goes through it; reads and Comments never do. Settings do
//! only when they change what the Todo widget draws ([`follow_settings`]).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use mnemora_core::events::{EventBus, SettingsChanged};

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

/// Makes `signal` fire when a saved Setting changes what the Todo widget draws (the app's theme,
/// the time format, the week start, how Todo views group and sort), and for no other Setting.
pub fn follow_settings(bus: &EventBus, signal: &Arc<WidgetSignal>) {
    let signal = Arc::clone(signal);
    bus.subscribe(move |event: &SettingsChanged| {
        if mnemora_todo::settings_affect_widget(&event.module, &event.keys) {
            signal.todo_changed();
        }
    });
}

#[cfg(target_os = "android")]
fn platform_notify() {
    super::android::notify_todo_changed();
}

#[cfg(not(target_os = "android"))]
fn platform_notify() {}
