//! The Entry types and the rules that decide whether input is acceptable.

use mnemora_core::error::{AppError, AppResult, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Widest UTC offset in use is +14:00; the narrowest is -12:00. Allow a little slack either way.
const MAX_OFFSET_MIN: i32 = 14 * 60;
/// Longest IANA zone name is well under this.
const MAX_ZONE_CHARS: usize = 64;

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
    #[specta(type = f64)]
    pub written_at: i64,
    /// UTC offset in minutes where it was written.
    pub written_offset_min: i32,
    /// IANA zone where it was written.
    pub written_tz: String,
    /// When it was last meaningfully edited: UTC milliseconds.
    #[specta(type = Option<f64>)]
    pub edited_at: Option<i64>,
    /// Words in the body, not counting Markdown symbols.
    pub word_count: u32,
    /// Row creation time: UTC milliseconds.
    #[specta(type = f64)]
    pub created_at: i64,
    /// Row update time: UTC milliseconds.
    #[specta(type = f64)]
    pub updated_at: i64,
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
    /// Entries by written instant, oldest first.
    pub entries: Vec<Entry>,
}

fn invalid(message: &str) -> AppError {
    AppError::new(ErrorCode::Validation, message)
}

/// Text must contain something visible.
pub fn validate_text(text: &str) -> AppResult<()> {
    if text.trim().is_empty() {
        return Err(invalid("An Entry can't be blank"));
    }
    Ok(())
}

/// A real calendar date written `YYYY-MM-DD`.
pub fn validate_page_date(date: &str) -> AppResult<()> {
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
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
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
pub fn validate_written_zone(tz: &str, offset_min: i32) -> AppResult<()> {
    if tz.trim().is_empty() || tz.chars().count() > MAX_ZONE_CHARS {
        return Err(invalid("Not a valid time zone"));
    }
    if offset_min.abs() > MAX_OFFSET_MIN {
        return Err(invalid("Not a valid time zone offset"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
