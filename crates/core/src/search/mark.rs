//! Marking matches: which words of a text match a query, and the plain spans the UI renders so
//! it never injects markup.

use std::ops::Range;

use serde::Serialize;
use specta::Type;

use super::fold;
use super::query::Term;

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

    /// Whether any word of `text` matches.
    pub fn matches(&self, text: &str) -> bool {
        let chars: Vec<char> = text.chars().collect();
        !marks_of(&chars, self).is_empty()
    }

    fn hits(&self, folded: &str) -> bool {
        self.whole.iter().any(|w| w == folded)
            || self.prefixes.iter().any(|p| folded.starts_with(p))
    }
}

/// Every word of `text` that matches, as spans covering all of `text` (a title to highlight).
pub fn highlight(text: &str, matcher: &Matcher) -> Vec<Span> {
    let chars: Vec<char> = text.chars().collect();
    spans_between(&chars, &marks_of(&chars, matcher), 0, chars.len())
}

fn marks_of(chars: &[char], matcher: &Matcher) -> Vec<Range<usize>> {
    fold::words_with_ranges(chars)
        .into_iter()
        .filter(|(_, w)| matcher.hits(w))
        .map(|(r, _)| r)
        .collect()
}

/// A window of plain `text` around its first match, or its start when nothing matches (a search
/// by date alone). Whitespace is not collapsed here; callers pass already-flattened text.
pub fn window(text: &str, matcher: &Matcher) -> Vec<Span> {
    let chars: Vec<char> = text.chars().collect();
    let marks = marks_of(&chars, matcher);

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
    spans.extend(spans_between(&chars, &marks, start, end));
    if end < chars.len()
        && let Some(last) = spans.last_mut()
    {
        last.text = format!("{}{ELLIPSIS}", last.text.trim_end());
    }
    spans
}

/// Spans for `chars[start..end]` with the `marks` inside it marked.
fn spans_between(chars: &[char], marks: &[Range<usize>], start: usize, end: usize) -> Vec<Span> {
    let mut spans = Vec::new();
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
    if cursor < end {
        spans.push(plain(chars[cursor..end].iter().collect()));
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
    use super::{Matcher, Span, highlight, window};
    use crate::search::query::parse;

    fn spans(body: &str, query: &str) -> Vec<(String, bool)> {
        let terms = parse(query);
        window(body, &Matcher::new(&terms, &[]))
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
    fn phrase_words_match_whole() {
        assert_eq!(
            spans("the red door opened", "\"red door\""),
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
        let out = window("my journal", &Matcher::new(&terms, &near));
        assert_eq!(
            out[1],
            Span {
                text: "journal".into(),
                matched: true
            }
        );
    }

    #[test]
    fn highlight_covers_the_whole_text() {
        let terms = parse("cafe");
        let out = highlight("Un Café ici", &Matcher::new(&terms, &[]));
        let texts: Vec<_> = out.iter().map(|x| (x.text.as_str(), x.matched)).collect();
        assert_eq!(texts, [("Un ", false), ("Café", true), (" ici", false)]);
    }
}
