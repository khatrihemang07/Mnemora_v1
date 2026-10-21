//! The words the Todo widget shows: Rust ports of the app's TypeScript text rules, so Kotlin does
//! no date or wording logic. `src/lib/ipc/fixtures/widget-text.json` runs the same cases here and
//! in Vitest.

use crate::when::epoch_day;

const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// How a time of day is written: the `core.time_format` Setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFormat {
    /// `21:30`.
    H24,
    /// `9:30 PM`.
    H12,
}

impl TimeFormat {
    /// Reads the Setting's value; anything but `12h` is the 24-hour default.
    pub fn from_setting(value: Option<&str>) -> Self {
        if value == Some("12h") {
            Self::H12
        } else {
            Self::H24
        }
    }
}

/// Longest title shown in a toast before it is cut with an ellipsis.
const TOAST_TITLE_CHARS: usize = 34;

/// A title without its marks in curly quotes, cut at 34 characters: port of `quotedTitle`.
fn quoted_title(title: &str) -> String {
    let plain = plain_title(title);
    let cut: String = plain.chars().take(TOAST_TITLE_CHARS).collect();
    let more = if plain.chars().count() > TOAST_TITLE_CHARS {
        "…"
    } else {
        ""
    };
    format!("“{}{more}”", cut.trim_end())
}

/// The Undo bar after a tick: `Completed “Buy oat milk”`. Port of `completedMessage`.
pub fn completed_message(title: &str) -> String {
    format!("Completed {}", quoted_title(title))
}

/// The Undo bar after ticking a repeating Task: `Completed “Call mum” · next Wed 28 Oct`. Port of
/// `completedRepeatMessage`.
pub fn completed_repeat_message(title: &str, next: &str, today: &str) -> String {
    format!(
        "Completed {} · next {}",
        quoted_title(title),
        day_text(next, today)
    )
}

/// `21:30` as the Setting wants it: `21:30` or `9:30 PM`. A malformed time is returned as given.
pub fn format_hm(hm: &str, format: TimeFormat) -> String {
    if format == TimeFormat::H24 {
        return hm.to_owned();
    }
    let parsed = hm
        .split_once(':')
        .and_then(|(h, m)| Some((h.parse::<u32>().ok()?, m.parse::<u32>().ok()?)));
    let Some((h, m)) = parsed else {
        return hm.to_owned();
    };
    let hour = if h % 12 == 0 { 12 } else { h % 12 };
    let mark = if h < 12 { "AM" } else { "PM" };
    format!("{hour}:{m:02} {mark}")
}

/// Whole days from `a` to `b` (negative when `b` is earlier), or none for a malformed date.
fn days_between(a: &str, b: &str) -> Option<i64> {
    Some(epoch_day(b)? - epoch_day(a)?)
}

/// `Mon`, `Tue`, ... of a real date.
fn weekday_short(date: &str) -> Option<&'static str> {
    // 1970-01-01 was a Thursday.
    let index = usize::try_from((epoch_day(date)? + 4).rem_euclid(7)).ok()?;
    WEEKDAYS.get(index).copied()
}

/// `(year, month, day)` of a real date.
fn ymd(date: &str) -> Option<(i64, usize, i64)> {
    epoch_day(date)?;
    let year = date.get(0..4)?.parse().ok()?;
    let month = date.get(5..7)?.parse().ok()?;
    let day = date.get(8..10)?.parse().ok()?;
    Some((year, month, day))
}

fn month_short(month: usize) -> &'static str {
    MONTHS.get(month.wrapping_sub(1)).copied().unwrap_or("")
}

/// `28 Oct`, with the year when it is not `today`'s year.
fn short_date(date: &str, today: &str) -> String {
    let Some((y, m, d)) = ymd(date) else {
        return date.to_owned();
    };
    let this_year = ymd(today).map(|t| t.0);
    let year = if this_year == Some(y) {
        String::new()
    } else {
        format!(" {y}")
    };
    format!("{d} {}{year}", month_short(m))
}

/// `Fri 23 Oct`, with the year when it is not `today`'s year: port of `dayText`.
fn day_text(date: &str, today: &str) -> String {
    match weekday_short(date) {
        Some(weekday) => format!("{weekday} {}", short_date(date, today)),
        None => date.to_owned(),
    }
}

/// A row's date: Today, Tomorrow, Yesterday, a weekday within the week ahead, then `28 Oct` with
/// the year when it is not this year. Port of `rowDateLabel`.
pub fn row_date_label(date: &str, today: &str) -> String {
    match days_between(today, date) {
        Some(0) => "Today".to_owned(),
        Some(1) => "Tomorrow".to_owned(),
        Some(-1) => "Yesterday".to_owned(),
        Some(n) if (2..7).contains(&n) => weekday_short(date).unwrap_or("").to_owned(),
        _ => short_date(date, today),
    }
}

/// The day a widget row shows for a Task due after today: `Tomorrow`, a weekday within the week,
/// then `Wed 4 Nov` (with the year when it is not this year). Overdue rows use
/// [`overdue_day_label`] instead.
pub fn row_day_label(date: &str, today: &str) -> String {
    match days_between(today, date) {
        Some(1) => "Tomorrow".to_owned(),
        Some(n) if (2..7).contains(&n) => weekday_short(date).unwrap_or("").to_owned(),
        _ => match weekday_short(date) {
            Some(weekday) => format!("{weekday} {}", short_date(date, today)),
            None => date.to_owned(),
        },
    }
}

/// How late an Overdue row is: `Yesterday`, the weekday for the days before that within the week,
/// then `19 Oct`. The widget shows it in red where a time would go.
pub fn overdue_day_label(date: &str, today: &str) -> String {
    match days_between(today, date) {
        Some(-1) => "Yesterday".to_owned(),
        Some(n) if (-6..-1).contains(&n) => weekday_short(date).unwrap_or("").to_owned(),
        _ => short_date(date, today),
    }
}

/// An Upcoming day's heading: `Tomorrow · Thu 22`, then `Fri 23`; a day in another month (or
/// year) names it too: `Mon 2 Nov`.
pub fn day_heading(date: &str, today: &str) -> String {
    let (Some(weekday), Some((y, m, d)), Some(now)) = (weekday_short(date), ymd(date), ymd(today))
    else {
        return date.to_owned();
    };
    let day = if (y, m) == (now.0, now.1) {
        format!("{weekday} {d}")
    } else {
        format!("{weekday} {}", short_date(date, today))
    };
    if days_between(today, date) == Some(1) {
        format!("Tomorrow · {day}")
    } else {
        day
    }
}

/// A title without its Markdown marks, for a sentence or a widget row that shows it as text.
/// Bold, italic, code and links lose their marks; block syntax (`#`, `-`) and `~~`/`==` stay as
/// the text they are, like the app's `plainTitle`.
pub fn plain_title(title: &str) -> String {
    let chars: Vec<char> = title.chars().collect();
    strip(&chars)
}

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

/// Index of the next `needle` in `chars` at or after `from`.
fn find(chars: &[char], from: usize, needle: &[char]) -> Option<usize> {
    let end = chars.len().checked_sub(needle.len())?;
    (from..=end).find(|&i| chars.get(i..i + needle.len()) == Some(needle))
}

/// A closing run of `mark` that may end an emphasis opened at `open_end`.
fn closing(chars: &[char], open_end: usize, mark: &[char], underscore: bool) -> Option<usize> {
    let mut from = open_end;
    while let Some(found) = find(chars, from, mark) {
        // In a run such as `***`, the last marks close; the earlier ones belong to what is inside.
        let mut i = found;
        while chars.get(i + mark.len()) == mark.first() {
            i += 1;
        }
        let before = i.checked_sub(1).and_then(|p| chars.get(p)).copied();
        let after = chars.get(i + mark.len()).copied();
        let flanks = before.is_some_and(|c| !c.is_whitespace()) && i > open_end;
        let boundary = !underscore || !is_word(after);
        if flanks && boundary {
            return Some(i);
        }
        from = i + 1;
    }
    None
}

fn strip(chars: &[char]) -> String {
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        let next = chars.get(i + 1).copied();
        match c {
            '\\' if next.is_some_and(|n| n.is_ascii_punctuation()) => {
                out.extend(next);
                i += 2;
            }
            '`' => {
                if let Some(end) = find(chars, i + 1, &['`']).filter(|&e| e > i + 1) {
                    out.extend(chars.get(i + 1..end).unwrap_or(&[]));
                    i = end + 1;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            '[' => {
                // `[text](url)`: keep the text.
                let link = find(chars, i + 1, &[']', '('])
                    .and_then(|close| Some((close, find(chars, close + 2, &[')'])?)));
                if let Some((close, end)) = link {
                    out.push_str(&strip(chars.get(i + 1..close).unwrap_or(&[])));
                    i = end + 1;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            '*' | '_' => {
                let underscore = c == '_';
                let before = i.checked_sub(1).and_then(|p| chars.get(p)).copied();
                let strong = next == Some(c);
                let width = if strong { 2 } else { 1 };
                let mark = [c; 2];
                let mark = mark.get(..width).unwrap_or(&[]);
                let after = chars.get(i + width).copied();
                let opens = after.is_some_and(|a| !a.is_whitespace() && a != c)
                    && (!underscore || !is_word(before));
                let close = if opens {
                    closing(chars, i + width, mark, underscore)
                } else {
                    None
                };
                if let Some(end) = close {
                    out.push_str(&strip(chars.get(i + width..end).unwrap_or(&[])));
                    i = end + width;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    const FIXTURE: &str = include_str!("../../../src/lib/ipc/fixtures/widget-text.json");

    fn cases(name: &str) -> Vec<Value> {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        fixture[name].as_array().unwrap().clone()
    }

    fn text<'a>(case: &'a Value, key: &str) -> &'a str {
        case[key].as_str().unwrap()
    }

    #[test]
    fn plain_titles_match_the_app() {
        for case in cases("plain_title") {
            assert_eq!(
                plain_title(text(&case, "title")),
                text(&case, "plain"),
                "{case}"
            );
        }
    }

    #[test]
    fn completed_messages_match_the_app() {
        for case in cases("completed_message") {
            assert_eq!(
                completed_message(text(&case, "title")),
                text(&case, "message"),
                "{case}"
            );
        }
        for case in cases("completed_repeat_message") {
            assert_eq!(
                completed_repeat_message(
                    text(&case, "title"),
                    text(&case, "next"),
                    text(&case, "today")
                ),
                text(&case, "message"),
                "{case}"
            );
        }
    }

    #[test]
    fn row_date_labels_match_the_app() {
        for case in cases("row_date_label") {
            assert_eq!(
                row_date_label(text(&case, "date"), text(&case, "today")),
                text(&case, "label"),
                "{case}"
            );
        }
    }

    #[test]
    fn row_days_for_tasks_not_due_today() {
        for case in cases("row_day_label") {
            assert_eq!(
                row_day_label(text(&case, "date"), text(&case, "today")),
                text(&case, "label"),
                "{case}"
            );
        }
        assert_eq!(row_day_label("nope", "2026-10-21"), "nope");
    }

    #[test]
    fn times_match_the_app() {
        for case in cases("format_hm") {
            let format = if text(&case, "format") == "12h" {
                TimeFormat::H12
            } else {
                TimeFormat::H24
            };
            assert_eq!(
                format_hm(text(&case, "hm"), format),
                text(&case, "text"),
                "{case}"
            );
        }
    }

    #[test]
    fn overdue_rows_name_the_day_they_were_due() {
        let today = "2026-10-21"; // a Wednesday
        assert_eq!(overdue_day_label("2026-10-20", today), "Yesterday");
        assert_eq!(overdue_day_label("2026-10-19", today), "Mon");
        assert_eq!(overdue_day_label("2026-10-15", today), "Thu");
        assert_eq!(overdue_day_label("2026-10-14", today), "14 Oct");
        assert_eq!(overdue_day_label("2025-12-30", today), "30 Dec 2025");
    }

    #[test]
    fn upcoming_days_have_a_heading() {
        let today = "2026-10-21";
        assert_eq!(day_heading("2026-10-22", today), "Tomorrow · Thu 22");
        assert_eq!(day_heading("2026-10-23", today), "Fri 23");
        assert_eq!(day_heading("2026-11-02", today), "Mon 2 Nov");
        // Tomorrow in the next month still says so.
        assert_eq!(
            day_heading("2026-11-01", "2026-10-31"),
            "Tomorrow · Sun 1 Nov"
        );
        assert_eq!(day_heading("2027-01-01", "2026-12-30"), "Fri 1 Jan 2027");
        assert_eq!(day_heading("nope", today), "nope");
    }

    #[test]
    fn malformed_input_never_panics() {
        assert_eq!(row_date_label("nope", "2026-10-21"), "nope");
        assert_eq!(format_hm("junk", TimeFormat::H12), "junk");
        assert_eq!(
            plain_title("[unclosed](link and `tick"),
            "[unclosed](link and `tick"
        );
        assert_eq!(plain_title(""), "");
    }
}
