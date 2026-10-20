//! Journal search: exact matches from the full-text index, then "similar" matches that allow
//! typos. The text of a query never reaches a log line.

use mnemora_core::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;
use specta::Type;

use crate::entry::validate_page_date;
use crate::error::JournalError;
use crate::fuzzy::{near_terms, tolerance};
use crate::query::{self, Term};
use crate::repo::{self, SearchRow};
use crate::snippet::{Matcher, Span, snippet};

/// Most exact Hits one search returns (newest first); more sets `truncated`.
const MAX_EXACT: usize = 500;
/// Most similar Hits one search returns (newest first); more sets `similar_truncated`.
const MAX_SIMILAR: usize = 100;
/// The vocabulary is read for words within this many letters of a query word, the widest
/// tolerance, so far-off lengths are never fetched.
const MAX_TOLERANCE: usize = 2;

/// One Entry found by a search.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct Hit {
    /// The Entry's id.
    pub id: String,
    /// The Page it sits on, `YYYY-MM-DD`.
    pub page_date: String,
    /// When it was written: UTC milliseconds.
    #[specta(type = u32)]
    pub written_at: i64,
    /// UTC offset in minutes where it was written.
    pub written_offset_min: i32,
    /// IANA zone where it was written.
    pub written_tz: String,
    /// A window of its text with the matches marked.
    pub snippet: Vec<Span>,
}

/// The outcome of a search.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SearchResults {
    /// Entries that match every word and phrase, newest first.
    pub exact: Vec<Hit>,
    /// Entries that match only when a word is allowed a typo; never also in `exact`.
    pub similar: Vec<Hit>,
    /// Whether `exact` was cut short at the limit.
    pub truncated: bool,
    /// Whether `similar` was cut short at the limit.
    pub similar_truncated: bool,
}

fn hits(rows: Vec<SearchRow>, matcher: &Matcher) -> Vec<Hit> {
    rows.into_iter()
        .map(|r| Hit {
            snippet: snippet(&r.body, matcher),
            id: r.id,
            page_date: r.page_date,
            written_at: r.written_at,
            written_offset_min: r.written_offset_min,
            written_tz: r.written_tz,
        })
        .collect()
}

fn validate_range(from: Option<&str>, to: Option<&str>) -> Result<(), JournalError> {
    for date in [from, to].into_iter().flatten() {
        validate_page_date(date)?;
    }
    if let (Some(f), Some(t)) = (from, to)
        && f > t
    {
        return Err(JournalError::Validation(
            "The start date is after the end date",
        ));
    }
    Ok(())
}

/// Searches live Entries. `query` words are all required and match by prefix; quoted text is an
/// exact phrase. `from` and `to` (inclusive Page dates) are optional; with a range and no words
/// every Entry in the range is returned. Nothing at all to search for returns nothing.
pub fn search(
    conn: &Connection,
    query: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> AppResult<SearchResults> {
    validate_range(from, to)?;
    let terms = query::parse(query);
    let empty = SearchResults {
        exact: Vec::new(),
        similar: Vec::new(),
        truncated: false,
        similar_truncated: false,
    };
    let Some(exact_match) = query::exact_match(&terms) else {
        if from.is_none() && to.is_none() {
            return Ok(empty);
        }
        let rows = repo::search(conn, None, None, from, to, MAX_EXACT + 1)?;
        let truncated = rows.len() > MAX_EXACT;
        let mut exact = hits(rows, &Matcher::new(&terms, &[]));
        exact.truncate(MAX_EXACT);
        return Ok(SearchResults {
            exact,
            truncated,
            ..empty
        });
    };

    let rows = repo::search(conn, Some(&exact_match), None, from, to, MAX_EXACT + 1)?;
    let truncated = rows.len() > MAX_EXACT;
    let mut exact = hits(rows, &Matcher::new(&terms, &[]));
    exact.truncate(MAX_EXACT);

    let near = near_for(conn, &terms)?;
    let (similar, similar_truncated) =
        match query::expand_match(&terms, &near).filter(|_| near.iter().any(|n| !n.is_empty())) {
            None => (Vec::new(), false),
            Some(expanded) => {
                let rows = repo::search(
                    conn,
                    Some(&expanded),
                    Some(&exact_match),
                    from,
                    to,
                    MAX_SIMILAR + 1,
                )?;
                let cut = rows.len() > MAX_SIMILAR;
                let mut similar = hits(rows, &Matcher::new(&terms, &near));
                similar.truncate(MAX_SIMILAR);
                (similar, cut)
            }
        };
    Ok(SearchResults {
        exact,
        similar,
        truncated,
        similar_truncated,
    })
}

/// Near words per term, reading only the vocabulary words of a useful length.
fn near_for(conn: &Connection, terms: &[Term]) -> AppResult<Vec<Vec<String>>> {
    let lengths: Vec<usize> = terms
        .iter()
        .filter_map(|t| match t {
            Term::Word(w) if tolerance(w.chars().count()) > 0 => Some(w.chars().count()),
            _ => None,
        })
        .collect();
    let (Some(min), Some(max)) = (lengths.iter().min(), lengths.iter().max()) else {
        return Ok(vec![Vec::new(); terms.len()]);
    };
    let vocab = repo::vocabulary(conn, min.saturating_sub(MAX_TOLERANCE), max + MAX_TOLERANCE)?;
    Ok(near_terms(terms, vocab.into_iter()))
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::search;
    use crate::migrations;

    #[test]
    fn the_migration_indexes_entries_written_before_it() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::all().to_version(&mut conn, 1).unwrap();
        for (id, body, deleted) in [("a", "old Café note", "NULL"), ("b", "old gone note", "5")] {
            conn.execute(
                &format!(
                    "INSERT INTO journal_entries(id, page_date, body, written_at,
                         written_offset_min, written_tz, word_count, created_at, updated_at,
                         deleted_at)
                     VALUES ('{id}', '2026-10-01', '{body}', 1, 0, 'UTC', 3, 1, 1, {deleted})"
                ),
                [],
            )
            .unwrap();
        }
        migrations::all().to_latest(&mut conn).unwrap();

        let found = search(&conn, "cafe", None, None).unwrap();
        assert_eq!(found.exact.len(), 1);
        assert_eq!(found.exact[0].id, "a");
        // The deleted one is indexed (Undo) but never returned.
        assert_eq!(search(&conn, "gone", None, None).unwrap().exact.len(), 0);
    }
}
