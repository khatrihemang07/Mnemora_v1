//! Local days and times in the device's IANA time zone. Instants are stored as epoch milliseconds
//! (UTC); a Tool shows them, and reads a day the person names, in the zone the client sends.

use jiff::{Timestamp, civil::Date, tz::TimeZone};

/// A local date and time as a Tool shows it: `2026-10-27 23:50`.
const MINUTE_FORMAT: &str = "%Y-%m-%d %H:%M";

/// The zone an IANA name refers to, if the bundled tz data knows it.
fn zone(time_zone: &str) -> Option<TimeZone> {
    TimeZone::get(time_zone).ok()
}

/// The instant as `YYYY-MM-DD HH:MM` in `time_zone`, or `None` when either is unknown.
pub fn minute(ms: i64, time_zone: &str) -> Option<String> {
    let at = Timestamp::from_millisecond(ms).ok()?;
    Some(
        at.to_zoned(zone(time_zone)?)
            .strftime(MINUTE_FORMAT)
            .to_string(),
    )
}

/// The instant `day` (`YYYY-MM-DD`) starts in `time_zone`, in epoch milliseconds. Midnight, or the
/// first moment of the day where the zone skips midnight.
pub fn day_start(day: &str, time_zone: &str) -> Option<i64> {
    let date: Date = day.parse().ok()?;
    let start = date.to_zoned(zone(time_zone)?).ok()?;
    Some(start.timestamp().as_millisecond())
}

/// The instant the day after `day` starts in `time_zone`: the end, not included, of `day`.
pub fn day_end(day: &str, time_zone: &str) -> Option<i64> {
    let next = day.parse::<Date>().ok()?.tomorrow().ok()?;
    day_start(&next.to_string(), time_zone)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-27 18:20 UTC, which is 23:50 in Kolkata and 14:20 in New York.
    const KOLKATA_2350: i64 = 1_793_125_200_000;
    /// 2026-10-28 03:50 UTC, which is 23:50 on the 27th in New York (EDT, UTC-4).
    const NEW_YORK_2350: i64 = 1_793_159_400_000;

    #[test]
    fn an_instant_reads_in_the_named_zone_not_in_utc() {
        assert_eq!(
            minute(KOLKATA_2350, "Asia/Kolkata").unwrap(),
            "2026-10-27 23:50"
        );
        assert_eq!(
            minute(NEW_YORK_2350, "America/New_York").unwrap(),
            "2026-10-27 23:50"
        );
        assert_eq!(minute(NEW_YORK_2350, "UTC").unwrap(), "2026-10-28 03:50");
    }

    #[test]
    fn an_unknown_zone_or_instant_has_no_local_reading() {
        assert_eq!(minute(NEW_YORK_2350, "Mars/Olympus"), None);
        assert_eq!(minute(i64::MAX, "UTC"), None);
    }

    #[test]
    fn a_day_starts_and_ends_at_its_local_midnight() {
        // New York is EDT (UTC-4) on the 27th: midnight is 04:00 UTC.
        assert_eq!(
            day_start("2026-10-27", "America/New_York"),
            Some(1_793_073_600_000)
        );
        assert_eq!(
            day_end("2026-10-27", "America/New_York"),
            Some(1_793_160_000_000)
        );
        assert_eq!(day_start("2026-10-27", "Mars/Olympus"), None);
        assert_eq!(day_start("2026-13-01", "UTC"), None);
    }
}
