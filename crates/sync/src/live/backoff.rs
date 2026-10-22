//! How long to wait before dialling a device again: 5 s, 15 s, 60 s, then every 5 minutes while
//! the app is open. A success, a network change or the app resuming starts the schedule over.

use std::time::Duration;

/// The waits between attempts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    /// The first waits, in order.
    pub steps: Vec<Duration>,
    /// The wait once the steps are used up, repeated for as long as the app is open.
    pub steady: Duration,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            steps: [5, 15, 60].map(Duration::from_secs).to_vec(),
            steady: Duration::from_secs(300),
        }
    }
}

/// Where one device stands in the [`Schedule`].
#[derive(Debug, Clone)]
pub struct Backoff {
    schedule: Schedule,
    failures: usize,
}

impl Backoff {
    /// A backoff at the start of `schedule`.
    pub fn new(schedule: Schedule) -> Self {
        Self {
            schedule,
            failures: 0,
        }
    }

    /// An attempt failed: how long to wait before the next one.
    pub fn next_delay(&mut self) -> Duration {
        let wait = self
            .schedule
            .steps
            .get(self.failures)
            .copied()
            .unwrap_or(self.schedule.steady);
        self.failures = self.failures.saturating_add(1);
        wait
    }

    /// Starts over: the next failure waits the first step again.
    pub fn reset(&mut self) {
        self.failures = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(b: &mut Backoff, n: usize) -> Vec<u64> {
        (0..n).map(|_| b.next_delay().as_secs()).collect()
    }

    #[test]
    fn waits_5_then_15_then_60_seconds_and_then_5_minutes_for_good() {
        let mut b = Backoff::new(Schedule::default());
        assert_eq!(secs(&mut b, 7), [5, 15, 60, 300, 300, 300, 300]);
    }

    #[test]
    fn a_reset_starts_the_schedule_over() {
        let mut b = Backoff::new(Schedule::default());
        secs(&mut b, 5);
        b.reset();
        assert_eq!(secs(&mut b, 3), [5, 15, 60]);
    }

    #[test]
    fn a_reset_with_nothing_failed_changes_nothing() {
        let mut b = Backoff::new(Schedule::default());
        b.reset();
        assert_eq!(b.next_delay(), Duration::from_secs(5));
    }
}
