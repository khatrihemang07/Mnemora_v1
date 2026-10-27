//! What the person can do to a Run while it answers: stop it, or end the Step in progress so the
//! notes they queued are sent now. One handle is shared by the Run's thread and the commands.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// The Run has not been asked to end.
const RUNNING: u8 = 0;
/// The person pressed Stop.
const STOPPED: u8 = 1;
/// The app left the foreground, which ends a Run on Android as interrupted.
const INTERRUPTED: u8 = 2;

/// A Run's stop and skip flags. Cloning shares the flags.
#[derive(Debug, Clone, Default)]
pub struct Control {
    ended: Arc<AtomicU8>,
    skip: Arc<AtomicBool>,
}

impl Control {
    /// A Run that has not been asked to stop or to skip.
    pub fn new() -> Self {
        Self::default()
    }

    /// The person pressed Stop: the Run ends after what is already written.
    pub fn stop(&self) {
        self.end(STOPPED);
    }

    /// The app left the foreground: the Run ends as interrupted.
    pub fn interrupt(&self) {
        self.end(INTERRUPTED);
    }

    /// Ends the current Step early so the queued notes are sent now. The Run goes on.
    pub fn skip_step(&self) {
        self.skip.store(true, Ordering::SeqCst);
    }

    /// Whether the Run must end (stopped or interrupted). Tools and Steps check this before they go on.
    pub fn ended(&self) -> bool {
        self.ended.load(Ordering::SeqCst) != RUNNING
    }

    /// Whether the Run ended because the app left the foreground.
    pub fn interrupted(&self) -> bool {
        self.ended.load(Ordering::SeqCst) == INTERRUPTED
    }

    /// Whether the Step in progress must stop reading: the Run ended, or the Step was skipped.
    pub fn halts_step(&self) -> bool {
        self.ended() || self.skip.load(Ordering::SeqCst)
    }

    /// Whether the current Step was skipped, so its Tools do not run.
    pub fn skipped(&self) -> bool {
        self.skip.load(Ordering::SeqCst)
    }

    /// A new Step begins: a skip applies to the Step it was asked during only.
    pub(crate) fn begin_step(&self) {
        self.skip.store(false, Ordering::SeqCst);
    }

    /// The first request to end the Run wins: a Stop pressed after an interruption changes nothing.
    fn end(&self, to: u8) {
        let _ = self
            .ended
            .compare_exchange(RUNNING, to, Ordering::SeqCst, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_end_request_wins_and_a_skip_is_for_one_step_only() {
        let control = Control::new();
        control.skip_step();
        assert!(control.halts_step());
        assert!(!control.ended());
        control.begin_step();
        assert!(!control.halts_step());

        control.interrupt();
        control.stop();
        assert!(control.ended());
        assert!(control.interrupted());
    }
}
