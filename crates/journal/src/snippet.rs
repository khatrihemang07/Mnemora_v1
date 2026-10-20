//! Search snippets: a short window of an Entry's text with the matching words marked, as plain
//! spans so the UI never injects markup.

use std::ops::Range;

use serde::Serialize;
use specta::Type;

use crate::fold;
use crate::query::Term;

/// Longest snippet, in characters (not counting the ellipses).
const SNIPPET_CHARS: usize = 160;
/// How much text to show before the first match.
const LEAD_CHARS: usize = 40;
const ELLIPSIS: &str = "…";

/// A run of snippet text, either plain or a match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct Span {
    /// The text, with whitespace collapsed to single spaces.
    pub text: String,
    /// Whether it matched the search.
    pub matched: bool,
}

/// Decides which words of an Entry count as matches.
pub struct Matcher {
    prefixes: Vec<String>,
    whole: Vec<String>,
}

impl Matcher {
    /// Bare words match by prefix, phrase words and `near` words (per term index) whole.
    pub fn new(terms: &[Term], near: &[Vec<String>]) -> Self {
        let mut prefixes = Vec::new();
        let mut whole = Vec::new();
        for (i, term) in terms.iter().enumerate() {
            match term {
                Term::Word(w) => {
                    prefixes.push(w.clone());
                    whole.extend(near.get(i).into_iter().flatten().map(|n| fold::fold(n)));
                }
                Term::Phrase(words) => whole.extend(words.iter().cloned()),
            }
        }
        Self { prefixes, whole }
    }

    fn hits(&self, folded: &str) -> bool {
        self.whole.iter().any(|w| w == folded)
            || self.prefixes.iter().any(|p| folded.starts_with(p))
    }
}

/// The snippet for `body`: around its first match, or its start when nothing matches (a search
/// by date alone).
pub fn snippet(body: &str, matcher: &Matcher) -> Vec<Span> {
    let chars: Vec<char> = body
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .collect();
    let marks: Vec<Range<usize>> = fold::words_with_ranges(&chars)
        .into_iter()
        .filter(|(_, w)| matcher.hits(w))
        .map(|(r, _)| r)
        .collect();

    let first = marks.first().map_or(0, |r| r.start);
    let mut start = first.saturating_sub(LEAD_CHARS);
    // Begin on a word boundary, never after the match.
    while start > 0 && start < first && chars[start - 1] != ' ' {
        start += 1;
    }
    let mut end = (start + SNIPPET_CHARS).min(chars.len());
    if let Some(r) = marks.iter().find(|r| r.start < end && r.end > end) {
        end = r.end;
    } else {
        let limit = end;
        while end > start && end < chars.len() && chars[end] != ' ' {
            end -= 1;
        }
        if end == start {
            end = limit;
        }
    }

    let mut spans = Vec::new();
    if start > 0 {
        spans.push(plain(ELLIPSIS.to_owned()));
    }
    let mut cursor = start;
    for r in marks.iter().filter(|r| r.start >= start && r.end <= end) {
        if r.start > cursor {
            spans.push(plain(chars[cursor..r.start].iter().collect()));
        }
        spans.push(Span {
            text: chars[r.clone()].iter().collect(),
            matched: true,
        });
        cursor = r.end;
    }
    let tail: String = chars[cursor..end].iter().collect();
    let tail = if end < chars.len() {
        format!("{}{ELLIPSIS}", tail.trim_end())
    } else {
        tail
    };
    if !tail.is_empty() {
        spans.push(plain(tail));
    }
    spans
}

fn plain(text: String) -> Span {
    Span {
        text,
        matched: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Matcher, Span, snippet};
    use crate::query::parse;

    fn spans(body: &str, query: &str) -> Vec<(String, bool)> {
        let terms = parse(query);
        snippet(body, &Matcher::new(&terms, &[]))
            .into_iter()
            .map(|Span { text, matched }| (text, matched))
            .collect()
    }

    fn s(text: &str, matched: bool) -> (String, bool) {
        (text.to_owned(), matched)
    }

    #[test]
    fn marks_prefix_matches_ignoring_case_and_accents() {
        assert_eq!(
            spans("Coffee at the Café today", "cafe"),
            [
                s("Coffee at the ", false),
                s("Café", true),
                s(" today", false)
            ]
        );
        assert_eq!(
            spans("A journal entry", "jour"),
            [s("A ", false), s("journal", true), s(" entry", false)]
        );
    }

    #[test]
    fn phrase_words_match_whole_and_whitespace_collapses() {
        assert_eq!(
            spans("the red\n\ndoor   opened", "\"red door\""),
            [
                s("the ", false),
                s("red", true),
                s(" ", false),
                s("door", true),
                s(" opened", false)
            ]
        );
    }

    #[test]
    fn no_match_shows_the_start_and_long_text_is_cut_with_ellipses() {
        let long = format!("{} needle {}", "word ".repeat(60), "tail ".repeat(60));
        let out = spans(&long, "needle");
        assert_eq!(out.first().map(|x| x.0.as_str()), Some("…"));
        assert!(out.iter().any(|x| x.1 && x.0 == "needle"));
        assert!(out.last().is_some_and(|x| x.0.ends_with('…')));

        let start = spans(&long, "");
        assert!(start.iter().all(|x| !x.1));
        assert_eq!(start.first().map(|x| x.0.starts_with("word")), Some(true));
    }

    #[test]
    fn near_words_are_marked_whole() {
        let terms = parse("jurnal");
        let near = vec![vec!["journal".to_owned()]];
        let out = snippet("my journal", &Matcher::new(&terms, &near));
        assert_eq!(
            out[1],
            Span {
                text: "journal".into(),
                matched: true
            }
        );
    }
}
