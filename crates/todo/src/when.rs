//! Floating dates and times: what is acceptable, calendar maths, and the rule that turns a pick
//! into a Due date. Pure functions with no clock of their own: the UI says what "today" and "now"
//! are (floating local `YYYY-MM-DD` and `HH:MM`), so nothing here depends on a time zone.

use serde::Deserialize;
use specta::Type;

use crate::error::TodoError;

/// Milliseconds in a day.
pub const DAY_MS: i64 = 86_400_000;

const PAST: TodoError = TodoError::Validation("That date is in the past");
const BAD_DATE: TodoError = TodoError::Validation("Not a valid date");
const BAD_TIME: TodoError = TodoError::Validation("Not a valid time");

/// What the UI says "now" is on the device: every date-dependent command takes one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
pub struct LocalNow {
    /// Today's date, `YYYY-MM-DD`. "Today" ends at midnight, like the Journal's.
    pub today: String,
    /// The time of day, `HH:MM`.
    pub now: String,
    /// Minutes the device's clock is ahead of UTC. Only needed to tell which Tasks were closed
    /// today, because closing times are stored in UTC.
    pub offset_min: i32,
}

impl LocalNow {
    /// Both parts must be well-formed; a malformed clock is the caller's bug, not a user error.
    pub fn check(&self) -> Result<(), TodoError> {
        epoch_day(&self.today).ok_or(BAD_DATE)?;
        if !is_time(&self.now) {
            return Err(BAD_TIME);
        }
        Ok(())
    }

    /// The UTC millisecond range `[start, end)` of today on the device.
    pub fn today_range_ms(&self) -> Result<(i64, i64), TodoError> {
        let start =
            epoch_day(&self.today).ok_or(BAD_DATE)? * DAY_MS - i64::from(self.offset_min) * 60_000;
        Ok((start, start + DAY_MS))
    }
}

/// A date and time to give a new Task, with the clock it was picked against.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
pub struct NewWhen {
    /// Due date, `YYYY-MM-DD`.
    pub date: Option<String>,
    /// Due time, `HH:MM`; with no date it means today if still ahead, otherwise tomorrow.
    pub time: Option<String>,
    /// The device's clock.
    pub local: LocalNow,
}

/// Days since 1970-01-01 of a real calendar date written `YYYY-MM-DD`.
pub fn epoch_day(date: &str) -> Option<i64> {
    let b = date.as_bytes();
    let shape_ok = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if !shape_ok {
        return None;
    }
    let year: i64 = date[0..4].parse().ok()?;
    let month: i64 = date[5..7].parse().ok()?;
    let day: i64 = date[8..10].parse().ok()?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if year < 1 || !(1..=days_in_month).contains(&day) {
        return None;
    }
    // Howard Hinnant's `days_from_civil`.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// The date `days` after `date` (negative for before), or none when `date` is not a real date.
pub fn add_days(date: &str, days: i64) -> Option<String> {
    let z = epoch_day(date)? + days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// A time of day written `HH:MM`, 00:00 to 23:59.
pub fn is_time(time: &str) -> bool {
    let b = time.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 2 || c.is_ascii_digit())
        && time[0..2].parse::<u32>().is_ok_and(|h| h < 24)
        && time[3..5].parse::<u32>().is_ok_and(|m| m < 60)
}

/// A real calendar date, as given.
pub fn validate_date(date: &str) -> Result<&str, TodoError> {
    epoch_day(date).map(|_| date).ok_or(BAD_DATE)
}

/// A real time of day, as given.
pub fn validate_time(time: &str) -> Result<&str, TodoError> {
    is_time(time).then_some(time).ok_or(BAD_TIME)
}

/// A date that may be picked: real, and not before today.
pub fn validate_pickable(date: &str, today: &str) -> Result<(), TodoError> {
    validate_date(date)?;
    if date < today {
        return Err(PAST);
    }
    Ok(())
}

/// The Due date and time a pick means. A date is taken as given (a time on it may already be
/// behind us: the Task stays in Today with its time in red). A time without a date means today
/// if it is still ahead, otherwise tomorrow. Nothing at all clears both. Past dates are refused.
pub fn resolve_when(
    date: Option<&str>,
    time: Option<&str>,
    local: &LocalNow,
) -> Result<(Option<String>, Option<String>), TodoError> {
    local.check()?;
    let time = time.map(validate_time).transpose()?;
    let date = match (date, time) {
        (Some(date), _) => {
            validate_pickable(date, &local.today)?;
            Some(date.to_owned())
        }
        (None, Some(time)) if time >= local.now.as_str() => Some(local.today.clone()),
        (None, Some(_)) => add_days(&local.today, 1),
        (None, None) => None,
    };
    Ok((date, time.map(str::to_owned)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(today: &str, now: &str) -> LocalNow {
        LocalNow {
            today: today.into(),
            now: now.into(),
            offset_min: 0,
        }
    }

    #[test]
    fn dates_must_be_real_calendar_dates() {
        for ok in ["2026-10-21", "2024-02-29", "0001-01-01"] {
            assert!(epoch_day(ok).is_some(), "{ok}");
        }
        for bad in [
            "",
            "2026-13-01",
            "2026-02-29",
            "2026-1-01",
            "2026/10/21",
            "0000-01-01",
        ] {
            assert!(epoch_day(bad).is_none(), "{bad}");
        }
        assert_eq!(epoch_day("1970-01-01"), Some(0));
        assert_eq!(
            epoch_day("2026-10-22"),
            epoch_day("2026-10-21").map(|d| d + 1)
        );
    }

    #[test]
    fn adding_days_crosses_months_years_and_leap_days() {
        assert_eq!(add_days("2026-10-21", 1).as_deref(), Some("2026-10-22"));
        assert_eq!(add_days("2026-10-31", 1).as_deref(), Some("2026-11-01"));
        assert_eq!(add_days("2026-12-31", 1).as_deref(), Some("2027-01-01"));
        assert_eq!(add_days("2024-02-28", 1).as_deref(), Some("2024-02-29"));
        assert_eq!(add_days("2026-03-01", -1).as_deref(), Some("2026-02-28"));
        assert_eq!(add_days("2026-10-21", 0).as_deref(), Some("2026-10-21"));
        assert_eq!(add_days("nope", 1), None);
    }

    #[test]
    fn times_are_hh_mm_on_a_24_hour_clock() {
        for ok in ["00:00", "09:05", "23:59"] {
            assert!(is_time(ok), "{ok}");
        }
        for bad in ["", "24:00", "9:00", "09:60", "0900", "09:0a", "12:00:00"] {
            assert!(!is_time(bad), "{bad}");
        }
    }

    #[test]
    fn a_date_is_taken_as_given_even_with_a_time_already_behind_us() {
        let local = at("2026-10-21", "14:05");
        let got = resolve_when(Some("2026-10-21"), Some("09:00"), &local).unwrap();
        assert_eq!(got, (Some("2026-10-21".into()), Some("09:00".into())));
        let got = resolve_when(Some("2026-10-23"), None, &local).unwrap();
        assert_eq!(got, (Some("2026-10-23".into()), None));
    }

    #[test]
    fn a_time_without_a_date_means_today_if_ahead_else_tomorrow() {
        let local = at("2026-10-21", "14:05");
        let ahead = resolve_when(None, Some("15:00"), &local).unwrap();
        assert_eq!(ahead, (Some("2026-10-21".into()), Some("15:00".into())));
        let behind = resolve_when(None, Some("09:00"), &local).unwrap();
        assert_eq!(behind, (Some("2026-10-22".into()), Some("09:00".into())));
        // The minute we are in still counts as ahead.
        let same = resolve_when(None, Some("14:05"), &local).unwrap();
        assert_eq!(same.0.as_deref(), Some("2026-10-21"));
        // Tomorrow rolls over the end of a month.
        let eom = resolve_when(None, Some("09:00"), &at("2026-10-31", "23:00")).unwrap();
        assert_eq!(eom.0.as_deref(), Some("2026-11-01"));
    }

    #[test]
    fn nothing_clears_and_bad_input_is_refused() {
        let local = at("2026-10-21", "14:05");
        assert_eq!(resolve_when(None, None, &local).unwrap(), (None, None));
        assert!(resolve_when(Some("2026-10-20"), None, &local).is_err());
        assert!(resolve_when(Some("2026-02-30"), None, &local).is_err());
        assert!(resolve_when(None, Some("25:00"), &local).is_err());
        assert!(resolve_when(None, None, &at("garbage", "14:05")).is_err());
        assert!(resolve_when(None, None, &at("2026-10-21", "2pm")).is_err());
    }

    #[test]
    fn today_spans_a_day_in_utc_shifted_by_the_offset() {
        let mut local = at("1970-01-02", "00:00");
        assert_eq!(local.today_range_ms().unwrap(), (DAY_MS, 2 * DAY_MS));
        // Kolkata is 330 minutes ahead: its midnight is earlier in UTC.
        local.offset_min = 330;
        assert_eq!(
            local.today_range_ms().unwrap(),
            (DAY_MS - 330 * 60_000, 2 * DAY_MS - 330 * 60_000)
        );
    }
}
