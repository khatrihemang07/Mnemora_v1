//! Repeat rules: what is acceptable and the date maths. Pure functions with no clock of their own
//! (the caller says what today is), so the same rules run in the browser mock from the shared
//! fixtures (`src/lib/ipc/fixtures/recurrence-rules.json`, ADR 0009).
//!
//! A rule counts either from the Due date (`fixed`) or from the day the Task is done (`after`).
//! Weeks are Monday to Sunday here: only a rule that picks weekdays "every N weeks" can tell.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::TodoError;
use crate::when::{add_days, epoch_day};

/// Largest interval: "every 99 days".
pub const MAX_EVERY: u32 = 99;
/// Weekday bits, Sunday = bit 0 … Saturday = bit 6.
const WEEKDAYS: u32 = 0b111_1111;
const MAX_YEAR: i64 = 9999;

const BAD_UNIT: TodoError = TodoError::Validation("Repeat by days, weeks, months or years");
const BAD_EVERY: TodoError = TodoError::Validation("Repeat every 1 to 99");
const BAD_MODE: TodoError =
    TodoError::Validation("Count from the due date or from the day it's done");
const BAD_DAYS: TodoError = TodoError::Validation("Not a valid repeat day");
const AFTER_NEEDS_INTERVAL: TodoError =
    TodoError::Validation("From the day it's done needs an interval, not weekdays");
const TOO_FAR: TodoError = TodoError::Validation("That repeat runs past the year 9999");

/// A Repeat as it crosses IPC and is stored in the Task's columns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RepeatRule {
    /// `day`, `week`, `month` or `year`.
    pub freq: String,
    /// The interval, 1 to 99.
    pub every: u32,
    /// Weeks: a bitmask of the weekdays (Sunday is bit 0), none meaning the Due date's own
    /// weekday. Months and years: the day of the month the Repeat keeps to (31 clamps to the
    /// last day of a short month and comes back), set from the Due date.
    pub days: Option<u32>,
    /// `fixed` (from the Due date) or `after` (from the day it was done).
    pub mode: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Day,
    Week,
    Month,
    Year,
}

/// A validated Repeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rule {
    unit: Unit,
    every: u32,
    /// Weekday bits for weeks, the day of the month for months and years; 0 is "the date's own".
    days: u32,
    after: bool,
}

impl RepeatRule {
    /// Checks the rule and settles its parts: days mean nothing for a day interval, and an
    /// "after done" rule keeps no weekdays or day of month.
    pub(crate) fn check(&self) -> Result<Rule, TodoError> {
        let unit = match self.freq.as_str() {
            "day" => Unit::Day,
            "week" => Unit::Week,
            "month" => Unit::Month,
            "year" => Unit::Year,
            _ => return Err(BAD_UNIT),
        };
        if !(1..=MAX_EVERY).contains(&self.every) {
            return Err(BAD_EVERY);
        }
        let after = match self.mode.as_str() {
            "fixed" => false,
            "after" => true,
            _ => return Err(BAD_MODE),
        };
        let given = self.days.unwrap_or(0);
        let days = match unit {
            Unit::Day => 0,
            Unit::Week if given > WEEKDAYS => return Err(BAD_DAYS),
            Unit::Week if after && given != 0 => return Err(AFTER_NEEDS_INTERVAL),
            Unit::Month | Unit::Year if given > 31 => return Err(BAD_DAYS),
            Unit::Month | Unit::Year if after => 0,
            _ => given,
        };
        Ok(Rule {
            unit,
            every: self.every,
            days,
            after,
        })
    }
}

impl Rule {
    /// The rule as stored and sent.
    pub(crate) fn to_wire(self) -> RepeatRule {
        RepeatRule {
            freq: match self.unit {
                Unit::Day => "day",
                Unit::Week => "week",
                Unit::Month => "month",
                Unit::Year => "year",
            }
            .into(),
            every: self.every,
            days: (self.days != 0).then_some(self.days),
            mode: if self.after { "after" } else { "fixed" }.into(),
        }
    }

    /// A fixed month or year rule keeps to the day of the month of its Due date, so a 31st that
    /// was clamped to the 28th comes back as the 31st.
    pub(crate) fn anchored_on(mut self, due: &str) -> Self {
        if !self.after && matches!(self.unit, Unit::Month | Unit::Year) {
            self.days = Ymd::parse(due).map_or(0, |d| d.day);
        }
        self
    }

    /// Whether the rule is a month or year one that already keeps to a day of the month.
    pub(crate) fn has_day(self) -> bool {
        matches!(self.unit, Unit::Month | Unit::Year) && self.days != 0
    }

    /// The first day on or after `today` the rule lands on: a Repeat on an undated Task gives it
    /// this date. Only chosen weekdays can make it later than today.
    pub(crate) fn first_match(self, today: &str) -> Option<String> {
        if self.unit == Unit::Week && self.days != 0 {
            (0..7)
                .filter_map(|i| add_days(today, i))
                .find(|d| weekday(d).is_some_and(|w| self.days & (1 << w) != 0))
        } else {
            epoch_day(today).map(|_| today.to_owned())
        }
    }

    /// The date one step after `date`, strictly later; none past the year 9999.
    pub(crate) fn step(self, date: &str) -> Option<String> {
        self.raw_step(date).filter(|d| d.len() == 10)
    }

    fn raw_step(self, date: &str) -> Option<String> {
        let every = i64::from(self.every);
        match self.unit {
            Unit::Day => add_days(date, every),
            Unit::Week if self.days == 0 => add_days(date, 7 * every),
            Unit::Week => {
                // The week runs Monday (0) to Sunday (6); chosen days later in this week come
                // first, then the week `every` weeks on.
                let from = i64::from((weekday(date)? + 6) % 7);
                let hit = |offset: i64| self.days & (1 << ((offset + 1) % 7)) != 0;
                if let Some(offset) = (from + 1..7).find(|&o| hit(o)) {
                    return add_days(date, offset - from);
                }
                let offset = (0..7).find(|&o| hit(o))?;
                add_days(date, 7 * every - from + offset)
            }
            Unit::Month => {
                let d = Ymd::parse(date)?;
                let months = i64::from(d.year) * 12 + i64::from(d.month) - 1 + every;
                let (year, month) = (months / 12, months % 12 + 1);
                Ymd::clamped(year, month, if self.days > 0 { self.days } else { d.day })
            }
            Unit::Year => {
                let d = Ymd::parse(date)?;
                let day = if self.days > 0 { self.days } else { d.day };
                Ymd::clamped(i64::from(d.year) + every, i64::from(d.month), day)
            }
        }
    }

    /// Where a Task due `due` goes when it is done on `today`: from the day it is done in "after"
    /// mode; otherwise the first date on the schedule after today, which is one step after the
    /// Due date when that is still ahead (done early) and skips the missed ones when late.
    pub(crate) fn next_due(self, due: &str, today: &str) -> Result<String, TodoError> {
        if self.after {
            return self.step(today).ok_or(TOO_FAR);
        }
        let mut date = self.step(due).ok_or(TOO_FAR)?;
        while date.as_str() <= today {
            date = self.step(&date).ok_or(TOO_FAR)?;
        }
        Ok(date)
    }

    /// The next `count` dates after `from` (the Due date, or for "after done" the day it is
    /// done), each a step on the last: the "Next:" line of the picker. The picker draws it from the
    /// TypeScript copy of this engine; this one pins the maths in tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn preview(self, from: &str, count: usize) -> Vec<String> {
        let mut out = Vec::with_capacity(count);
        let mut date = from.to_owned();
        while out.len() < count {
            let Some(next) = self.step(&date) else { break };
            out.push(next.clone());
            date = next;
        }
        out
    }
}

/// A calendar date taken apart.
struct Ymd {
    year: u32,
    month: u32,
    day: u32,
}

impl Ymd {
    fn parse(date: &str) -> Option<Self> {
        epoch_day(date)?;
        Some(Self {
            year: date[0..4].parse().ok()?,
            month: date[5..7].parse().ok()?,
            day: date[8..10].parse().ok()?,
        })
    }

    /// The date in `year`-`month` on `day`, or that month's last day when it is shorter.
    fn clamped(year: i64, month: i64, day: u32) -> Option<String> {
        if !(1..=MAX_YEAR).contains(&year) {
            return None;
        }
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let last = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            _ => 28,
        };
        Some(format!("{year:04}-{month:02}-{:02}", day.min(last)))
    }
}

/// 0 (Sunday) to 6 (Saturday).
fn weekday(date: &str) -> Option<u32> {
    u32::try_from((epoch_day(date)? + 4).rem_euclid(7)).ok()
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn rule(freq: &str, every: u32, days: Option<u32>, mode: &str) -> Result<Rule, TodoError> {
        RepeatRule {
            freq: freq.into(),
            every,
            days,
            mode: mode.into(),
        }
        .check()
    }

    fn fixed(freq: &str, every: u32, days: u32) -> Rule {
        rule(freq, every, Some(days), "fixed").unwrap()
    }

    const MON_THU: u32 = 0b001_0010;

    #[test]
    fn a_set_on_wednesday_with_every_monday_and_thursday_lands_on_thursday() {
        // 21 October 2026 is a Wednesday.
        let r = fixed("week", 1, MON_THU);
        assert_eq!(r.first_match("2026-10-21").as_deref(), Some("2026-10-22"));
        assert_eq!(r.first_match("2026-10-22").as_deref(), Some("2026-10-22"));
        assert_eq!(r.first_match("2026-10-23").as_deref(), Some("2026-10-26"));
    }

    #[test]
    fn every_n_days_steps_n_days() {
        let r = fixed("day", 3, 0);
        assert_eq!(r.step("2026-10-21").as_deref(), Some("2026-10-24"));
        assert_eq!(r.step("2026-12-30").as_deref(), Some("2027-01-02"));
        assert_eq!(
            fixed("day", 99, 0).step("2026-10-21").as_deref(),
            Some("2027-01-28")
        );
    }

    #[test]
    fn weekday_sets_walk_the_week_then_skip_to_the_next_block() {
        let r = fixed("week", 1, MON_THU);
        assert_eq!(r.step("2026-10-19").as_deref(), Some("2026-10-22"));
        assert_eq!(r.step("2026-10-22").as_deref(), Some("2026-10-26"));
        // A date that is not one of the chosen days still finds the next one.
        assert_eq!(r.step("2026-10-21").as_deref(), Some("2026-10-22"));
        assert_eq!(r.step("2026-10-24").as_deref(), Some("2026-10-26"));
    }

    #[test]
    fn every_two_weeks_on_monday_and_thursday_skips_the_off_week() {
        let r = fixed("week", 2, MON_THU);
        assert_eq!(
            r.preview("2026-10-22", 3),
            ["2026-11-02", "2026-11-05", "2026-11-16"]
        );
        assert_eq!(r.preview("2026-10-19", 2), ["2026-10-22", "2026-11-02"]);
    }

    #[test]
    fn sunday_closes_the_week() {
        let sunday = fixed("week", 1, 1);
        assert_eq!(sunday.step("2026-10-25").as_deref(), Some("2026-11-01"));
        let every_weekday = fixed("week", 1, 0b011_1110);
        assert_eq!(
            every_weekday.preview("2026-10-23", 3),
            ["2026-10-26", "2026-10-27", "2026-10-28"]
        );
    }

    #[test]
    fn weeks_with_no_chosen_days_keep_the_dates_own_weekday() {
        assert_eq!(
            fixed("week", 1, 0).step("2026-10-21").as_deref(),
            Some("2026-10-28")
        );
        assert_eq!(
            fixed("week", 3, 0).step("2026-10-21").as_deref(),
            Some("2026-11-11")
        );
        assert_eq!(
            fixed("week", 1, 0).first_match("2026-10-21").as_deref(),
            Some("2026-10-21")
        );
    }

    #[test]
    fn the_31st_clamps_to_short_months_and_comes_back() {
        let r = fixed("month", 1, 31);
        assert_eq!(
            r.preview("2026-01-31", 4),
            ["2026-02-28", "2026-03-31", "2026-04-30", "2026-05-31"]
        );
        // Without the remembered day the clamp would stick.
        assert_eq!(
            fixed("month", 1, 0).preview("2026-01-31", 2),
            ["2026-02-28", "2026-03-28"]
        );
        assert_eq!(
            fixed("month", 2, 31).preview("2026-10-31", 3),
            ["2026-12-31", "2027-02-28", "2027-04-30"]
        );
    }

    #[test]
    fn leap_years_keep_the_29th() {
        assert_eq!(
            fixed("month", 1, 29).preview("2024-01-29", 2),
            ["2024-02-29", "2024-03-29"]
        );
        assert_eq!(
            fixed("month", 1, 29).preview("2026-01-29", 1),
            ["2026-02-28"]
        );
        assert_eq!(
            fixed("month", 1, 31).preview("2024-01-31", 1),
            ["2024-02-29"]
        );
        // 1900 was not a leap year, 2000 was.
        assert_eq!(
            fixed("month", 1, 29).step("1900-01-29").as_deref(),
            Some("1900-02-28")
        );
        assert_eq!(
            fixed("month", 1, 29).step("2000-01-29").as_deref(),
            Some("2000-02-29")
        );
    }

    #[test]
    fn months_roll_over_the_year() {
        assert_eq!(
            fixed("month", 1, 15).step("2026-12-15").as_deref(),
            Some("2027-01-15")
        );
        assert_eq!(
            fixed("month", 13, 0).step("2026-10-21").as_deref(),
            Some("2027-11-21")
        );
    }

    #[test]
    fn years_clamp_a_leap_day_and_return_to_it() {
        assert_eq!(
            fixed("year", 1, 29).preview("2024-02-29", 4),
            ["2025-02-28", "2026-02-28", "2027-02-28", "2028-02-29"]
        );
        assert_eq!(
            fixed("year", 4, 0).preview("2024-02-29", 3),
            ["2028-02-29", "2032-02-29", "2036-02-29"]
        );
        assert_eq!(
            fixed("year", 1, 0).preview("2026-10-21", 2),
            ["2027-10-21", "2028-10-21"]
        );
    }

    #[test]
    fn anchoring_on_the_due_date_remembers_its_day_for_months_and_years_only() {
        assert_eq!(
            fixed("month", 1, 0)
                .anchored_on("2026-01-31")
                .to_wire()
                .days,
            Some(31)
        );
        assert_eq!(
            fixed("year", 1, 0).anchored_on("2024-02-29").to_wire().days,
            Some(29)
        );
        assert_eq!(
            fixed("week", 1, MON_THU)
                .anchored_on("2026-01-31")
                .to_wire()
                .days,
            Some(MON_THU)
        );
        let after = rule("month", 1, Some(31), "after").unwrap();
        assert_eq!(after.anchored_on("2026-01-31").to_wire().days, None);
    }

    #[test]
    fn late_fixed_goes_to_the_first_occurrence_after_today() {
        // Every Monday, three weeks late: no pile of catch-up dates.
        let r = fixed("week", 1, 0b000_0010);
        assert_eq!(
            r.next_due("2026-09-28", "2026-10-21").unwrap(),
            "2026-10-26"
        );
        // Due today, done today: tomorrow's rhythm, not today again.
        assert_eq!(
            fixed("day", 1, 0)
                .next_due("2026-10-21", "2026-10-21")
                .unwrap(),
            "2026-10-22"
        );
    }

    #[test]
    fn early_fixed_goes_one_step_after_the_due_date() {
        let friday = fixed("week", 1, 0b010_0000);
        assert_eq!(
            friday.next_due("2026-10-23", "2026-10-21").unwrap(),
            "2026-10-30"
        );
        assert_eq!(
            fixed("day", 3, 0)
                .next_due("2026-10-24", "2026-10-21")
                .unwrap(),
            "2026-10-27"
        );
    }

    #[test]
    fn after_done_counts_from_the_day_it_is_done() {
        let every_3 = rule("day", 3, None, "after").unwrap();
        assert_eq!(
            every_3.next_due("2026-10-18", "2026-10-21").unwrap(),
            "2026-10-24"
        );
        // Even when done early, the due date plays no part.
        assert_eq!(
            every_3.next_due("2026-12-01", "2026-10-21").unwrap(),
            "2026-10-24"
        );
        let monthly = rule("month", 1, None, "after").unwrap();
        assert_eq!(
            monthly.next_due("2026-01-05", "2026-01-31").unwrap(),
            "2026-02-28"
        );
        assert_eq!(
            monthly.next_due("2024-01-05", "2024-01-31").unwrap(),
            "2024-02-29"
        );
        let fortnight = rule("week", 2, None, "after").unwrap();
        assert_eq!(
            fortnight.next_due("2026-10-01", "2026-10-21").unwrap(),
            "2026-11-04"
        );
    }

    #[test]
    fn after_done_is_refused_with_chosen_weekdays() {
        assert!(rule("week", 1, Some(MON_THU), "after").is_err());
        assert!(rule("week", 1, Some(0), "after").is_ok());
        assert!(rule("week", 1, None, "after").is_ok());
    }

    #[test]
    fn intervals_are_one_to_ninety_nine() {
        assert!(rule("day", 0, None, "fixed").is_err());
        assert!(rule("day", 100, None, "fixed").is_err());
        assert!(rule("day", 1, None, "fixed").is_ok());
        assert!(rule("day", 99, None, "fixed").is_ok());
    }

    #[test]
    fn unknown_parts_are_refused() {
        assert!(rule("decade", 1, None, "fixed").is_err());
        assert!(rule("day", 1, None, "weird").is_err());
        assert!(rule("week", 1, Some(128), "fixed").is_err());
        assert!(rule("month", 1, Some(32), "fixed").is_err());
    }

    #[test]
    fn checking_settles_the_days_for_storage() {
        assert_eq!(
            rule("day", 2, Some(5), "fixed").unwrap().to_wire().days,
            None
        );
        assert_eq!(
            rule("month", 1, Some(0), "fixed").unwrap().to_wire().days,
            None
        );
        assert_eq!(
            rule("week", 1, Some(MON_THU), "fixed")
                .unwrap()
                .to_wire()
                .days,
            Some(MON_THU)
        );
    }

    #[test]
    fn the_calendar_ends_in_9999() {
        assert_eq!(fixed("year", 1, 0).step("9999-06-01"), None);
        assert!(
            fixed("year", 1, 0)
                .next_due("9999-06-01", "2026-10-21")
                .is_err()
        );
        assert!(fixed("day", 1, 0).preview("9999-12-30", 5).len() < 5);
    }

    #[test]
    fn weekdays_are_counted_from_sunday() {
        assert_eq!(weekday("2026-10-21"), Some(3));
        assert_eq!(weekday("2026-10-25"), Some(0));
        assert_eq!(weekday("1970-01-01"), Some(4));
        assert_eq!(weekday("nope"), None);
    }

    // Shared fixtures: src/lib/ipc/fixtures/recurrence-rules.json runs here and in
    // src/lib/ui/when-repeat.test.ts against the TypeScript copy of this engine.
    const FIXTURES: &str = include_str!("../../../src/lib/ipc/fixtures/recurrence-rules.json");

    fn fixture_rule(v: &Value) -> Result<Rule, TodoError> {
        let weekdays = v["weekdays"].as_array().map_or(0, |ds| {
            ds.iter()
                .fold(0, |mask, d| mask | (1u32 << d.as_u64().unwrap()))
        });
        let anchor = v["anchor"].as_u64().unwrap_or(0);
        let freq = v["freq"].as_str().unwrap();
        let days = if freq == "week" {
            u64::from(weekdays)
        } else {
            anchor
        };
        RepeatRule {
            freq: freq.into(),
            every: u32::try_from(v["every"].as_u64().unwrap()).unwrap(),
            days: Some(u32::try_from(days).unwrap()),
            mode: v["mode"].as_str().unwrap().into(),
        }
        .check()
    }

    #[test]
    fn the_shared_recurrence_fixtures_hold() {
        let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
        let cases = fixtures["cases"].as_array().unwrap();
        assert!(cases.len() > 20);
        for c in cases {
            let name = c["name"].as_str().unwrap();
            let rule = fixture_rule(&c["rule"]);
            let (date, expect) = (|k: &str| c[k].as_str().unwrap(), &c["expect"]);
            let got = match c["op"].as_str().unwrap() {
                "invalid" => {
                    assert!(rule.is_err(), "{name}");
                    continue;
                }
                "first_match" => Value::from(rule.unwrap().first_match(date("today"))),
                "next_due" => {
                    Value::from(rule.unwrap().next_due(date("due"), date("today")).unwrap())
                }
                "preview" => Value::from(rule.unwrap().preview(date("from"), 3)),
                other => panic!("{name}: unknown op {other}"),
            };
            assert_eq!(&got, expect, "{name}");
        }
    }
}
