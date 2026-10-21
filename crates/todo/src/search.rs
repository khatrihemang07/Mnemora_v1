//! Task search with the Journal's matching (`mnemora_core::search`): word prefixes, case and
//! accents ignored, quoted phrases, and a "similar" fallback that allows typos. It reads Task
//! titles, notes and Comments. The text of a query never reaches a log line.

use mnemora_core::error::AppResult;
use mnemora_core::search::plain::plain_text;
use mnemora_core::search::query::{self, Term};
use mnemora_core::search::{Matcher, Span, highlight, near_terms, tolerance, window};
use rusqlite::Connection;
use serde::Serialize;
use specta::Type;

use crate::task::Task;
use crate::{comment_repo, search_repo};

/// Most exact Hits one search returns; more sets `truncated`.
const MAX_EXACT: usize = 500;
/// Most similar Hits one search returns; more sets `similar_truncated`.
const MAX_SIMILAR: usize = 100;
/// The vocabulary is read for words within this many letters of a query word, the widest
/// tolerance, so far-off lengths are never fetched.
const MAX_TOLERANCE: usize = 2;

/// Where a Hit's snippet line comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SnippetSource {
    /// A line of the Task's notes.
    Notes,
    /// A Comment.
    Comment,
}

/// One Task found by a search.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct Hit {
    /// The Task, as every list shows it.
    pub task: Task,
    /// Its title cut into plain and matched runs, for highlighting.
    pub title: Vec<Span>,
    /// The note or Comment line that matched, with the matches marked; empty when only the title
    /// matched.
    pub snippet: Vec<Span>,
    /// Where `snippet` came from.
    pub snippet_source: Option<SnippetSource>,
}

/// The outcome of a search. Within each group open Tasks come first (manual order), then Done and
/// Won't do ones (latest closed first); the UI splits on `task.state`.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SearchResults {
    /// Tasks that match every word and phrase.
    pub exact: Vec<Hit>,
    /// Tasks that match only when a word is allowed a typo; never also in `exact`.
    pub similar: Vec<Hit>,
    /// Whether `exact` was cut short at the limit.
    pub truncated: bool,
    /// Whether `similar` was cut short at the limit.
    pub similar_truncated: bool,
}

fn hit(conn: &Connection, task: Task, matcher: &Matcher) -> AppResult<Hit> {
    let title = highlight(&task.title, matcher);
    let from_notes = task
        .notes
        .lines()
        .map(plain_text)
        .find(|line| matcher.matches(line));
    let (line, snippet_source) = match from_notes {
        Some(line) => (Some(line), Some(SnippetSource::Notes)),
        None => {
            let comment = comment_repo::for_task(conn, &task.id)?
                .into_iter()
                .map(|c| plain_text(&c.body))
                .find(|body| matcher.matches(body));
            (comment, Some(SnippetSource::Comment))
        }
    };
    let (snippet, snippet_source) = match line {
        Some(line) => (window(&line, matcher), snippet_source),
        None => (Vec::new(), None),
    };
    Ok(Hit {
        task,
        title,
        snippet,
        snippet_source,
    })
}

fn hits(conn: &Connection, tasks: Vec<Task>, matcher: &Matcher) -> AppResult<Vec<Hit>> {
    tasks.into_iter().map(|t| hit(conn, t, matcher)).collect()
}

/// Searches live Tasks. Every word is required and matches by prefix; quoted text is an exact
/// phrase. Nothing to search for returns nothing; deleted Tasks and Comments are never found.
pub fn search(conn: &Connection, query: &str) -> AppResult<SearchResults> {
    let terms = query::parse(query);
    let mut results = SearchResults {
        exact: Vec::new(),
        similar: Vec::new(),
        truncated: false,
        similar_truncated: false,
    };
    let Some(exact_match) = query::exact_match(&terms) else {
        return Ok(results);
    };

    let mut rows = search_repo::search(conn, &exact_match, None, MAX_EXACT + 1)?;
    results.truncated = rows.len() > MAX_EXACT;
    rows.truncate(MAX_EXACT);
    results.exact = hits(conn, rows, &Matcher::new(&terms, &[]))?;

    let near = near_for(conn, &terms)?;
    if near.iter().any(|n| !n.is_empty())
        && let Some(expanded) = query::expand_match(&terms, &near)
    {
        let mut rows = search_repo::search(conn, &expanded, Some(&exact_match), MAX_SIMILAR + 1)?;
        results.similar_truncated = rows.len() > MAX_SIMILAR;
        rows.truncate(MAX_SIMILAR);
        results.similar = hits(conn, rows, &Matcher::new(&terms, &near))?;
    }
    Ok(results)
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
    let vocab =
        search_repo::vocabulary(conn, min.saturating_sub(MAX_TOLERANCE), max + MAX_TOLERANCE)?;
    Ok(near_terms(terms, vocab.into_iter()))
}
