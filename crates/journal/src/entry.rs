//! The Entry types and the rules that decide whether input is acceptable.

use crate::error::JournalError;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Widest UTC offset in use is +14:00; the narrowest is -12:00. Allow a little slack either way.
const MAX_OFFSET_MIN: i32 = 14 * 60;
/// Longest IANA zone name is well under this.
const MAX_ZONE_CHARS: usize = 64;

// Millisecond instants are i64, which specta refuses to export. They fit a JS number (< 2^53), so
// they are typed `u32` for the bindings only: it makes TypeScript see a plain `number`.
/// One untitled piece of writing on a Page.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct Entry {
    /// `UUIDv7`.
    pub id: String,
    /// The Page this Entry sits on, `YYYY-MM-DD`.
    pub page_date: String,
    /// Markdown text.
    pub body: String,
    /// When it was written: UTC milliseconds. Never changes.
    #[specta(type = u32)]
    pub written_at: i64,
    /// UTC offset in minutes where it was written.
    pub written_offset_min: i32,
    /// IANA zone where it was written.
    pub written_tz: String,
    /// When it was last meaningfully edited: UTC milliseconds.
    #[specta(type = Option<u32>)]
    pub edited_at: Option<i64>,
    /// Words in the body, not counting Markdown symbols.
    pub word_count: u32,
    /// Row creation time: UTC milliseconds.
    #[specta(type = u32)]
    pub created_at: i64,
    /// Row update time: UTC milliseconds.
    #[specta(type = u32)]
    pub updated_at: i64,
}

/// Which Pages a `cursor` date selects. Results are always newest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageDirection {
    /// Pages strictly before the cursor (older). No cursor: the newest Pages.
    Older,
    /// The cursor's own Page and older ones: the feed starting AT a date. No cursor: the newest.
    From,
    /// The Pages just after the cursor (newer), nearest first picked; needs a cursor.
    Newer,
}

/// Everything the UI supplies to write a new Entry.
#[derive(Debug, Clone, Deserialize, Type)]
pub struct NewEntry {
    /// The Page to put it on, `YYYY-MM-DD`. Future dates are fine.
    pub page_date: String,
    /// Markdown text.
    pub text: String,
    /// IANA zone of the writer's device.
    pub written_tz: String,
    /// UTC offset in minutes of the writer's device.
    pub written_offset_min: i32,
    /// The composer session writing this Entry; groups its autosaves in the change log.
    pub session_id: String,
}

/// All Entries of one date, oldest first.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct Page {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// How many Entries the Page holds.
    pub entry_count: u32,
    /// Words across all its Entries, for the header summary.
    pub word_count: u32,
    /// Entries by written instant, oldest first.
    pub entries: Vec<Entry>,
}

fn invalid(message: &'static str) -> JournalError {
    JournalError::Validation(message)
}

/// Text must contain something visible.
pub fn validate_text(text: &str) -> Result<(), JournalError> {
    if text.trim().is_empty() {
        return Err(invalid("An Entry can't be blank"));
    }
    Ok(())
}

/// A real calendar date written `YYYY-MM-DD`.
pub fn validate_page_date(date: &str) -> Result<(), JournalError> {
    parse_date(date)
        .map(|_| ())
        .ok_or_else(|| invalid("Not a valid date"))
}

fn parse_date(date: &str) -> Option<(u32, u32, u32)> {
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
    let year: u32 = date[0..4].parse().ok()?;
    let month: u32 = date[5..7].parse().ok()?;
    let day: u32 = date[8..10].parse().ok()?;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    (year >= 1 && (1..=days_in_month).contains(&day)).then_some((year, month, day))
}

/// A plausible written zone and offset. Stored as given, so only sanity is checked.
pub fn validate_written_zone(tz: &str, offset_min: i32) -> Result<(), JournalError> {
    if tz.trim().is_empty() || tz.chars().count() > MAX_ZONE_CHARS {
        return Err(invalid("Not a valid time zone"));
    }
    if offset_min.abs() > MAX_OFFSET_MIN {
        return Err(invalid("Not a valid time zone offset"));
    }
    Ok(())
}

/// Byte index of the `[ ]`/`[x]` state character if `line` is a task item: optional quote marks
/// and indent, a bullet (`-`, `*`, `+`) or number (`1.`, `1)`), a space, then the box. This is
/// what the Markdown parser treats as a task, so the read view and this agree on what ticks.
fn task_state_index(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    let mut i = 0;
    // Quote prefixes and indent, in any mix.
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'>') {
        i += 1;
    }
    match b.get(i)? {
        b'-' | b'*' | b'+' => i += 1,
        b'0'..=b'9' => {
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if !matches!(b.get(i)?, b'.' | b')') {
                return None;
            }
            i += 1;
        }
        _ => return None,
    }
    if !matches!(b.get(i)?, b' ' | b'\t') {
        return None;
    }
    while i < b.len() && matches!(b[i], b' ' | b'\t') {
        i += 1;
    }
    if b.get(i) == Some(&b'[')
        && matches!(b.get(i + 1)?, b' ' | b'x' | b'X')
        && b.get(i + 2) == Some(&b']')
        && b.get(i + 3)
            .is_none_or(|c| matches!(c, b' ' | b'\t' | b'\r'))
    {
        return Some(i + 1);
    }
    None
}

/// Flips the checkbox on the 0-based `line` of `body`, leaving every other byte alone.
pub fn toggle_task(body: &str, line: usize) -> Result<String, JournalError> {
    let mut start = 0;
    for (n, text) in body.split('\n').enumerate() {
        if n == line {
            let at = task_state_index(text)
                .ok_or_else(|| invalid("That line is not a checklist item"))?;
            let flipped = if text.as_bytes()[at] == b' ' {
                'x'
            } else {
                ' '
            };
            let mut out = String::with_capacity(body.len());
            out.push_str(&body[..start + at]);
            out.push(flipped);
            out.push_str(&body[start + at + 1..]);
            return Ok(out);
        }
        start += text.len() + 1;
    }
    Err(invalid("That line is not a checklist item"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_task_flips_one_marker_and_rejects_everything_else() {
        assert_eq!(
            toggle_task("a\n- [ ] b\n- [ ] c", 2).unwrap(),
            "a\n- [ ] b\n- [x] c"
        );
        assert_eq!(toggle_task("  + [X] b", 0).unwrap(), "  + [ ] b");
        assert_eq!(toggle_task("- [ ]\r\n", 0).unwrap(), "- [x]\r\n");
        for (body, line) in [
            ("- [ ]x", 0),
            ("- [] a", 0),
            ("-[ ] a", 0),
            ("[ ] a", 0),
            ("1 [ ] a", 0),
            ("- [ ] a", 1),
            ("", 0),
        ] {
            assert!(toggle_task(body, line).is_err(), "{body:?} line {line}");
        }
    }

    #[test]
    fn page_dates_must_be_real_calendar_dates() {
        for ok in ["2026-10-20", "2024-02-29", "2999-12-31"] {
            assert!(validate_page_date(ok).is_ok(), "{ok}");
        }
        for bad in [
            "2026-02-29",
            "2026-13-01",
            "2026-00-10",
            "2026-04-31",
            "26-1-1",
            "2026/10/20",
            "",
            "2026-10-2a",
        ] {
            assert!(validate_page_date(bad).is_err(), "{bad}");
        }
    }
}
