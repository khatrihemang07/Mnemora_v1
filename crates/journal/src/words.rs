//! Word counting that ignores Markdown symbols.

/// Characters that only style text. Replaced by a space so `**bold**word` still splits sensibly.
const STYLE_MARKS: [char; 5] = ['*', '_', '~', '=', '`'];

/// Number of words in Markdown `text`, not counting syntax: heading, quote, list and checklist
/// markers, emphasis marks and link targets (the link text counts).
pub fn word_count(text: &str) -> u32 {
    let count: usize = text
        .lines()
        .map(|line| {
            let plain: String = strip_links(strip_line_prefix(line))
                .chars()
                .map(|c| if STYLE_MARKS.contains(&c) { ' ' } else { c })
                .collect();
            plain
                .split_whitespace()
                // A token with no letter or digit is a leftover symbol (`#`, `---`, `-`).
                .filter(|w| w.chars().any(char::is_alphanumeric))
                .count()
        })
        .sum();
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn strip_line_prefix(line: &str) -> &str {
    let mut s = line.trim_start();
    while let Some(rest) = s.strip_prefix('>') {
        s = rest.trim_start();
    }
    s = s.trim_start_matches('#').trim_start();
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = s.strip_prefix(marker) {
            s = rest.trim_start();
            break;
        }
    }
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && s[digits..].starts_with(". ") {
        s = s[digits + 2..].trim_start();
    }
    for task in ["[ ] ", "[x] ", "[X] "] {
        if let Some(rest) = s.strip_prefix(task) {
            return rest;
        }
    }
    s
}

/// `[text](url)` becomes `text`.
fn strip_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let link = after.find("](").and_then(|close| {
            let url = &after[close + 2..];
            url.find(')').map(|end| (&after[..close], &url[end + 1..]))
        });
        match link {
            Some((text, tail)) => {
                out.push_str(text);
                rest = tail;
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::word_count;

    #[test]
    fn plain_words() {
        assert_eq!(word_count("one two  three\nfour"), 4);
        assert_eq!(word_count(""), 0);
    }

    #[test]
    fn markdown_symbols_do_not_count() {
        assert_eq!(word_count("# Title here"), 2);
        assert_eq!(word_count("**bold** _it_ ~~gone~~ ==hi== `code`"), 5);
        assert_eq!(word_count("> quoted words"), 2);
        assert_eq!(word_count("---"), 0);
    }

    #[test]
    fn list_and_checklist_markers_do_not_count() {
        assert_eq!(
            word_count("- one\n* two\n1. three\n- [ ] four\n- [x] five"),
            5
        );
    }

    #[test]
    fn link_text_counts_but_the_target_does_not() {
        assert_eq!(
            word_count("see [my site](https://example.com/a?b=c) now"),
            4
        );
        assert_eq!(word_count("a [not a link"), 4);
    }
}
