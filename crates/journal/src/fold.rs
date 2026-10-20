//! Word splitting and case/accent folding that mirror the index's `unicode61` tokenizer closely
//! enough for fuzzy distance and snippet highlighting. The index itself decides what matches.

use std::ops::Range;

/// Combining marks are dropped, as the tokenizer's `remove_diacritics` does.
fn is_combining(c: char) -> bool {
    ('\u{0300}'..='\u{036F}').contains(&c)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || is_combining(c)
}

/// Lowercase `c` without its accent (common Latin letters; anything else is just lowercased).
fn fold_char(c: char) -> Option<char> {
    if is_combining(c) {
        return None;
    }
    let lower = c.to_lowercase().next().unwrap_or(c);
    Some(match lower {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' | 'đ' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'ĺ' | 'ļ' | 'ľ' | 'ł' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        other => other,
    })
}

/// Lowercases `word` and strips accents.
pub fn fold(word: &str) -> String {
    word.chars().filter_map(fold_char).collect()
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
    use super::{fold, words};

    #[test]
    fn folds_case_and_accents() {
        assert_eq!(fold("Café"), "cafe");
        assert_eq!(fold("NAÏVE"), "naive");
        assert_eq!(fold("cafe\u{301}"), "cafe");
    }

    #[test]
    fn splits_on_anything_that_is_not_a_letter_or_digit() {
        assert_eq!(
            words("a-b, c_d \"e\"* NEAR(f)"),
            ["a", "b", "c", "d", "e", "near", "f"]
        );
        assert!(words("  --- ").is_empty());
    }
}
