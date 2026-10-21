//! Word splitting and case/accent folding that mirror the index's `unicode61` tokenizer closely
//! enough for fuzzy distance and snippet highlighting. The index itself decides what matches.

use std::ops::Range;

pub use mnemora_core::text::fold;
use mnemora_core::text::{fold_char, is_combining};

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || is_combining(c)
}

/// The words of `text` (runs of letters and digits) with their char ranges, folded.
pub fn words_with_ranges(chars: &[char]) -> Vec<(Range<usize>, String)> {
    let mut out = Vec::new();
    let mut start = None;
    for i in 0..=chars.len() {
        let in_word = chars.get(i).is_some_and(|c| is_word_char(*c));
        match (in_word, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                let word: String = chars[s..i].iter().copied().filter_map(fold_char).collect();
                if !word.is_empty() {
                    out.push((s..i, word));
                }
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// The folded words of `text`.
pub fn words(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    words_with_ranges(&chars)
        .into_iter()
        .map(|(_, w)| w)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::words;

    #[test]
    fn splits_on_anything_that_is_not_a_letter_or_digit() {
        assert_eq!(
            words("a-b, c_d \"e\"* NEAR(f)"),
            ["a", "b", "c", "d", "e", "near", "f"]
        );
        assert!(words("  --- ").is_empty());
    }
}
