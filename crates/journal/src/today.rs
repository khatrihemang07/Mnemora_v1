//! The "today" summary behind the Home card.

use mnemora_core::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;
use specta::Type;

use crate::entry::validate_page_date;
use crate::repo;
use crate::words::plain_text;

/// Longest card snippet, in characters (not counting the ellipsis).
const SNIPPET_CHARS: usize = 140;
const ELLIPSIS: &str = "…";

/// The Entry written last on the day.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct TodayLatest {
    /// `UUIDv7`.
    pub id: String,
    /// When it was written: UTC milliseconds.
    #[specta(type = u32)]
    pub written_at: i64,
    /// UTC offset in minutes where it was written.
    pub written_offset_min: i32,
    /// IANA zone where it was written.
    pub written_tz: String,
    /// Its text as plain reading text (no Markdown), cut at about 140 characters.
    pub snippet: String,
}

/// What the Home card shows about one day.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct TodaySummary {
    /// Live Entries on the day's Page.
    pub count: u32,
    /// The Entry written last, `None` on an empty day.
    pub latest: Option<TodayLatest>,
}

/// `text` cut to `SNIPPET_CHARS` on a word boundary, with an ellipsis when it was cut.
fn shorten(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= SNIPPET_CHARS {
        return text.to_owned();
    }
    let mut end = SNIPPET_CHARS;
    // Never cut mid-word unless one word fills the whole snippet.
    while end > 0 && chars[end] != ' ' {
        end -= 1;
    }
    if end == 0 {
        end = SNIPPET_CHARS;
    }
    let cut: String = chars[..end].iter().collect();
    format!("{}{ELLIPSIS}", cut.trim_end())
}

/// The summary of `date` (the user's local today, `YYYY-MM-DD`, passed in so the zone is theirs).
pub fn today(conn: &Connection, date: &str) -> AppResult<TodaySummary> {
    validate_page_date(date)?;
    let (count, latest) = repo::day_summary(conn, date)?;
    Ok(TodaySummary {
        count,
        latest: latest.map(|e| TodayLatest {
            snippet: shorten(&plain_text(&e.body)),
            id: e.id,
            written_at: e.written_at,
            written_offset_min: e.written_offset_min,
            written_tz: e.written_tz,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::shorten;

    #[test]
    fn short_text_is_kept_and_long_text_is_cut_on_a_word() {
        assert_eq!(shorten("a short one"), "a short one");
        let long = "word ".repeat(60);
        let cut = shorten(&long);
        assert!(cut.ends_with('…'));
        assert!(cut.chars().count() <= 141);
        assert!(cut.trim_end_matches('…').ends_with("word"));
    }

    #[test]
    fn one_giant_word_is_cut_hard() {
        let cut = shorten(&"x".repeat(300));
        assert_eq!(cut.chars().count(), 141);
    }
}
