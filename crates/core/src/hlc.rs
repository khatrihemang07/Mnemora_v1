//! The hybrid logical clock (ADR 0017) that orders change-log records across devices.
//!
//! A [`Timestamp`] is wall-clock milliseconds plus a counter; it never goes backwards on one
//! device, and it moves past any later time it is shown ([`Hlc::receive`]), so "happened after"
//! survives small differences between device clocks. Records are totally ordered by
//! `(ms, counter, device id)`, which is [`Stamp`]'s `Ord`; every device sorts the same way.
//!
//! [`Hlc`] is the pure state machine; physical time is an argument, so tests drive it directly.
//! [`tick`] and [`observe`] keep one persisted [`Hlc`] per database in `device_clock`, so the
//! clock stays monotonic across restarts and changes in the same transaction as the record that
//! uses it. The physical time comes from the connection's time source (`db::set_time_source`).

use rusqlite::Connection;

use crate::db::physical_now;
use crate::error::AppResult;

/// A point on the hybrid logical clock. Orders by milliseconds, then counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Timestamp {
    /// Wall-clock milliseconds since the Unix epoch, or the latest time the clock was shown.
    pub ms: i64,
    /// Orders events inside one millisecond.
    pub counter: i64,
}

/// A [`Timestamp`] and the device that made it: the total order every device agrees on.
/// Field order is the sort order (derive), so ties on the clock fall to the device id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stamp {
    /// Clock milliseconds.
    pub ms: i64,
    /// Clock counter.
    pub counter: i64,
    /// Id of the device that made it.
    pub device: String,
}

/// The clock's state: the newest timestamp it has issued or seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Hlc {
    last: Timestamp,
}

impl Hlc {
    /// A clock that has already reached `last` (what was persisted).
    pub const fn at(last: Timestamp) -> Self {
        Self { last }
    }

    /// The newest timestamp issued or seen.
    pub const fn last(&self) -> Timestamp {
        self.last
    }

    /// A timestamp for a local event at wall-clock time `physical_ms`. Strictly greater than
    /// every earlier one, even if the wall clock stood still or went backwards.
    pub fn send(&mut self, physical_ms: i64) -> Timestamp {
        self.advance(physical_ms, None)
    }

    /// Merges a timestamp shown by another device and returns the next local one, which is
    /// strictly greater than both the remote and everything issued before.
    pub fn receive(&mut self, physical_ms: i64, remote: Timestamp) -> Timestamp {
        self.advance(physical_ms, Some(remote))
    }

    fn advance(&mut self, physical_ms: i64, remote: Option<Timestamp>) -> Timestamp {
        let remote = remote.unwrap_or_default();
        let ms = physical_ms.max(self.last.ms).max(remote.ms);
        // The counter continues from whichever input shares the winning millisecond.
        let mut counter = None;
        if ms == self.last.ms {
            counter = Some(self.last.counter);
        }
        if ms == remote.ms {
            counter = counter.max(Some(remote.counter));
        }
        self.last = Timestamp {
            ms,
            counter: counter.map_or(0, |c| c.saturating_add(1)),
        };
        self.last
    }
}

/// One local event: the clock timestamp and the new local sequence, both persisted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tick {
    /// The event's clock timestamp.
    pub clock: Timestamp,
    /// Local sequence: bumps on every write on this device, never reused.
    pub seq: i64,
    /// The wall-clock time the tick was taken at.
    pub physical_ms: i64,
}

/// Stores this device's id next to the clock. Call at startup, before any write.
pub fn set_device_id(conn: &Connection, device_id: &str) -> AppResult<()> {
    conn.execute("UPDATE device_clock SET device_id = ?1", [device_id])?;
    // Rows backfilled by the migration belong to this device.
    conn.execute(
        "UPDATE change_log SET origin_device = ?1 WHERE origin_device = ''",
        [device_id],
    )?;
    Ok(())
}

/// This device's id as stored with the clock (empty before [`set_device_id`] ran).
pub fn device_id(conn: &Connection) -> AppResult<String> {
    Ok(conn.query_row("SELECT device_id FROM device_clock", [], |r| r.get(0))?)
}

/// Takes the next local event: advances the clock and the local sequence. Pass the transaction
/// that writes the record so a rolled-back write leaves the clock untouched.
pub fn tick(conn: &Connection) -> AppResult<Tick> {
    step(conn, None)
}

/// Shows the clock a timestamp from another device (to be called when a record arrives). The
/// clock moves past it, so this device's next record sorts after everything it has seen.
pub fn observe(conn: &Connection, remote: Timestamp) -> AppResult<Timestamp> {
    Ok(step(conn, Some(remote))?.clock)
}

fn step(conn: &Connection, remote: Option<Timestamp>) -> AppResult<Tick> {
    // The first statement writes, so this takes the database's write lock before the clock is
    // read: a second connection (Sync gets its own) waits its turn instead of reading a stale
    // clock, and the clock and sequence stay unique and monotonic across connections. The
    // savepoint makes that hold outside a transaction too, and nests inside one.
    conn.execute_batch("SAVEPOINT hlc_step")?;
    match step_locked(conn, remote) {
        Ok(tick) => {
            conn.execute_batch("RELEASE hlc_step")?;
            Ok(tick)
        }
        Err(e) => {
            // Best effort: the original error is the one to report.
            let _ = conn.execute_batch("ROLLBACK TO hlc_step; RELEASE hlc_step");
            Err(e)
        }
    }
}

fn step_locked(conn: &Connection, remote: Option<Timestamp>) -> AppResult<Tick> {
    conn.execute("UPDATE device_clock SET seq = seq + 1", [])?;
    let (ms, counter, seq): (i64, i64, i64) =
        conn.query_row("SELECT ms, counter, seq FROM device_clock", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
    let physical_ms = physical_now(conn);
    let mut hlc = Hlc::at(Timestamp { ms, counter });
    let clock = match remote {
        Some(remote) => hlc.receive(physical_ms, remote),
        None => hlc.send(physical_ms),
    };
    conn.execute(
        "UPDATE device_clock SET ms = ?1, counter = ?2",
        rusqlite::params![clock.ms, clock.counter],
    )?;
    Ok(Tick {
        clock,
        seq,
        physical_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(ms: i64, counter: i64) -> Timestamp {
        Timestamp { ms, counter }
    }

    #[test]
    fn send_follows_the_wall_clock() {
        let mut hlc = Hlc::default();
        assert_eq!(hlc.send(1000), ts(1000, 0));
        assert_eq!(hlc.send(1005), ts(1005, 0));
    }

    #[test]
    fn send_is_monotonic_when_time_stands_still_or_goes_back() {
        let mut hlc = Hlc::default();
        let a = hlc.send(1000);
        let b = hlc.send(1000);
        let c = hlc.send(900);
        let d = hlc.send(1000);
        assert_eq!(
            [a, b, c, d],
            [ts(1000, 0), ts(1000, 1), ts(1000, 2), ts(1000, 3)]
        );
    }

    #[test]
    fn receive_advances_past_a_later_remote_time() {
        let mut hlc = Hlc::default();
        hlc.send(1000);
        let next = hlc.receive(1001, ts(5000, 7));
        assert_eq!(next, ts(5000, 8));
        assert!(
            hlc.send(1002) > ts(5000, 8),
            "later local events stay after the remote one"
        );
    }

    #[test]
    fn receive_of_an_older_remote_time_only_moves_forward() {
        let mut hlc = Hlc::default();
        hlc.send(2000);
        assert_eq!(hlc.receive(2000, ts(1000, 9)), ts(2000, 1));
        assert_eq!(hlc.receive(3000, ts(1000, 9)), ts(3000, 0));
    }

    #[test]
    fn receive_on_the_same_millisecond_takes_the_larger_counter() {
        let mut hlc = Hlc::at(ts(2000, 3));
        assert_eq!(hlc.receive(1500, ts(2000, 10)), ts(2000, 11));
        assert_eq!(hlc.last(), ts(2000, 11));
    }

    #[test]
    fn stamps_tie_break_by_device_id() {
        let stamp = |counter, device: &str| Stamp {
            ms: 10,
            counter,
            device: device.into(),
        };
        let mut stamps = [stamp(1, "b"), stamp(1, "a"), stamp(0, "z")];
        stamps.sort();
        let order: Vec<_> = stamps
            .iter()
            .map(|s| (s.counter, s.device.as_str()))
            .collect();
        assert_eq!(order, vec![(0, "z"), (1, "a"), (1, "b")]);
        let later = Stamp {
            ms: 11,
            counter: 0,
            device: "a".into(),
        };
        assert!(later > stamp(99, "z"));
    }

    #[test]
    fn two_connections_to_one_file_share_one_clock_and_sequence() {
        use crate::db;
        use crate::module::CoreModule;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.db");
        let mut a = db::open(&path).unwrap();
        db::migrate::run(&mut a, &[&CoreModule], dir.path()).unwrap();
        let b = db::open(&path).unwrap();
        let frozen: db::TimeSource = std::sync::Arc::new(|| 7_000);
        db::set_time_source(&a, frozen.clone()).unwrap();
        db::set_time_source(&b, frozen).unwrap();
        let mut seen = Vec::new();
        for _ in 0..3 {
            seen.push(tick(&a).unwrap());
            seen.push(tick(&b).unwrap());
        }
        let seqs: Vec<_> = seen.iter().map(|t| t.seq).collect();
        assert_eq!(seqs, vec![1, 2, 3, 4, 5, 6]);
        assert!(
            seen.windows(2).all(|w| w[0].clock < w[1].clock),
            "one clock across both connections"
        );
    }
}
