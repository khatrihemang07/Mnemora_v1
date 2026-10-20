//! Turns what the user typed into an FTS5 MATCH expression. Only letters and digits ever reach
//! the expression, each inside double quotes, so no FTS5 syntax (operators, columns, `*`, `-`,
//! parentheses, `NEAR`) can be injected.

use crate::fold;

/// One requirement of a query. Every term must match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    /// A bare word: matches any word that starts with it.
    Word(String),
    /// Quoted text: these words, whole and in this order.
    Phrase(Vec<String>),
}

/// Parses `input`: text between double quotes is a phrase (an unclosed quote runs to the end),
/// everything else splits into words. Terms are folded; empty ones are dropped.
pub fn parse(input: &str) -> Vec<Term> {
    let mut terms = Vec::new();
    for (i, part) in input.split('"').enumerate() {
        let words = fold::words(part);
        if i % 2 == 1 {
            if !words.is_empty() {
                terms.push(Term::Phrase(words));
            }
        } else {
            terms.extend(words.into_iter().map(Term::Word));
        }
    }
    terms
}

fn quoted(words: &[String]) -> String {
    format!("\"{}\"", words.join(" "))
}

fn prefix(word: &str) -> String {
    format!("\"{word}\"*")
}

/// MATCH expression for exactly `terms`: prefix words and phrases, all required.
/// `None` when there is nothing to search for.
pub fn exact_match(terms: &[Term]) -> Option<String> {
    expand_match(terms, &[])
}

/// Like [`exact_match`], but a word may also be any of the `near` terms listed for it (same
/// index as `terms`; phrases ignore theirs). Near terms are whole words, not prefixes.
pub fn expand_match(terms: &[Term], near: &[Vec<String>]) -> Option<String> {
    if terms.is_empty() {
        return None;
    }
    let parts: Vec<String> = terms
        .iter()
        .enumerate()
        .map(|(i, term)| match term {
            Term::Phrase(words) => quoted(words),
            Term::Word(word) => match near.get(i).filter(|n| !n.is_empty()) {
                None => prefix(word),
                Some(near) => {
                    let alternatives: Vec<String> = std::iter::once(prefix(word))
                        .chain(near.iter().map(|n| format!("\"{n}\"")))
                        .collect();
                    format!("({})", alternatives.join(" OR "))
                }
            },
        })
        .collect();
    Some(parts.join(" AND "))
}

#[cfg(test)]
mod tests {
    use super::{Term, exact_match, expand_match, parse};

    fn word(w: &str) -> Term {
        Term::Word(w.into())
    }

    #[test]
    fn words_and_phrases() {
        assert_eq!(
            parse("Café  \"red door\" walk"),
            [
                word("cafe"),
                Term::Phrase(vec!["red".into(), "door".into()]),
                word("walk")
            ]
        );
    }

    #[test]
    fn unclosed_quote_is_a_phrase_and_empty_quotes_vanish() {
        assert_eq!(
            parse("\"red door"),
            [Term::Phrase(vec!["red".into(), "door".into()])]
        );
        assert!(parse("\"\" \"  \"").is_empty());
        assert_eq!(exact_match(&parse("  ")), None);
    }

    #[test]
    fn fts_syntax_becomes_plain_quoted_words() {
        let m = exact_match(&parse("a OR b NOT c* -d NEAR(x y) body:z ((")).unwrap();
        assert_eq!(
            m,
            "\"a\"* AND \"or\"* AND \"b\"* AND \"not\"* AND \"c\"* AND \"d\"* AND \"near\"* AND \"x\"* AND \"y\"* AND \"body\"* AND \"z\"*"
        );
    }

    #[test]
    fn near_terms_become_an_or_group() {
        let terms = parse("jurnal \"x y\" cat");
        let near = vec![vec!["journal".to_owned()], vec![], vec![]];
        assert_eq!(
            expand_match(&terms, &near).unwrap(),
            "(\"jurnal\"* OR \"journal\") AND \"x y\" AND \"cat\"*"
        );
    }
}
