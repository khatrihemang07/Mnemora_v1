//! Typo tolerance: bounded edit distance and the choice of near words from the index vocabulary.

use crate::fold;
use crate::query::Term;

/// Words shorter than this must match exactly.
const MIN_FUZZY_LEN: usize = 4;
/// Words from this length on may have two typos; shorter ones one.
const TWO_TYPOS_LEN: usize = 8;
/// Most near words kept per query word, so the expanded MATCH stays small.
pub const MAX_NEAR_PER_WORD: usize = 20;

/// Typos allowed for a word of `len` letters.
pub fn tolerance(len: usize) -> usize {
    match len {
        0..MIN_FUZZY_LEN => 0,
        MIN_FUZZY_LEN..TWO_TYPOS_LEN => 1,
        _ => 2,
    }
}

/// Levenshtein distance between `a` and `b` if it is at most `max`, else `None`. Stops as soon as
/// a whole row exceeds `max`, so a far word costs a few cells, not the full table.
pub fn bounded_distance(a: &[char], b: &[char], max: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > max {
        return None;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut row = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        row[0] = i + 1;
        let mut best = row[0];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            row[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(row[j] + 1);
            best = best.min(row[j + 1]);
        }
        if best > max {
            return None;
        }
        std::mem::swap(&mut prev, &mut row);
    }
    let d = prev[b.len()];
    (d <= max).then_some(d)
}

/// For each term, the vocabulary words within its tolerance (nearest first, then the most used),
/// excluding the word itself. Phrases and short words get none. `vocab` yields `(term, doc count)`.
pub fn near_terms(terms: &[Term], vocab: impl Iterator<Item = (String, u32)>) -> Vec<Vec<String>> {
    struct Probe {
        index: usize,
        word: Vec<char>,
        max: usize,
        found: Vec<(usize, u32, String)>,
    }
    let mut probes: Vec<Probe> = terms
        .iter()
        .enumerate()
        .filter_map(|(index, t)| match t {
            Term::Word(w) if tolerance(w.chars().count()) > 0 => Some(Probe {
                index,
                word: w.chars().collect(),
                max: tolerance(w.chars().count()),
                found: Vec::new(),
            }),
            _ => None,
        })
        .collect();

    for (term, docs) in vocab {
        let chars: Vec<char> = fold::fold(&term).chars().collect();
        for p in &mut probes {
            if chars == p.word {
                continue;
            }
            if let Some(d) = bounded_distance(&p.word, &chars, p.max) {
                p.found.push((d, docs, term.clone()));
            }
        }
    }

    let mut out = vec![Vec::new(); terms.len()];
    for mut p in probes {
        p.found
            .sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        out[p.index] = p
            .found
            .into_iter()
            .take(MAX_NEAR_PER_WORD)
            .map(|(_, _, t)| t)
            .collect();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{bounded_distance, near_terms, tolerance};
    use crate::query::Term;

    fn d(a: &str, b: &str, max: usize) -> Option<usize> {
        let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
        bounded_distance(&a, &b, max)
    }

    #[test]
    fn distance_counts_inserts_deletes_and_substitutions() {
        assert_eq!(d("journal", "journal", 2), Some(0));
        assert_eq!(d("jurnal", "journal", 2), Some(1));
        assert_eq!(d("kitten", "sitten", 1), Some(1));
        assert_eq!(d("abcd", "abdc", 2), Some(2));
        assert_eq!(d("meetng", "meeting", 1), Some(1));
    }

    #[test]
    fn distance_beyond_the_bound_is_none() {
        assert_eq!(d("journal", "jungle", 1), None);
        assert_eq!(d("a", "abcd", 2), None);
        assert_eq!(d("kitten", "sitting", 2), None);
        assert_eq!(d("kitten", "sitting", 3), Some(3));
    }

    #[test]
    fn tolerance_follows_word_length() {
        let t: Vec<usize> = [1, 3, 4, 7, 8, 12].map(tolerance).to_vec();
        assert_eq!(t, [0, 0, 1, 1, 2, 2]);
    }

    fn vocab(words: &[&str]) -> impl Iterator<Item = (String, u32)> {
        words
            .iter()
            .map(|w| ((*w).to_owned(), 1))
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn short_words_and_phrases_never_get_near_terms() {
        let terms = vec![Term::Word("cat".into()), Term::Phrase(vec!["cot".into()])];
        let near = near_terms(&terms, vocab(&["cot", "car", "cats"]));
        assert!(near.iter().all(Vec::is_empty));
    }

    #[test]
    fn near_terms_are_ranked_and_exclude_the_word_itself() {
        let terms = vec![Term::Word("jurnal".into())];
        let near = near_terms(&terms, vocab(&["journal", "jurnal", "journey", "jaundice"]));
        assert_eq!(near[0], ["journal"]);
    }

    #[test]
    fn eight_letter_words_allow_two_typos_and_the_list_is_capped() {
        let terms = vec![Term::Word("wonderful".into())];
        let near = near_terms(
            &terms,
            vocab(&["wnderfl", "wonderfully", "wanderfal", "wxxderful1"]),
        );
        assert_eq!(near[0], ["wanderfal", "wnderfl", "wonderfully"]);

        let many: Vec<String> = (0..40).map(|i| format!("wonderful{i}")).collect();
        let refs: Vec<&str> = many.iter().map(String::as_str).collect();
        // One digit is one typo, two digits two: all 40 qualify, only the cap is kept.
        assert_eq!(
            near_terms(&terms, vocab(&refs))[0].len(),
            super::MAX_NEAR_PER_WORD
        );
    }
}
