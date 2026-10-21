//! Plain reading text and word counts from Markdown, for search snippets and summaries.

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

/// Separates lines (blocks, list items) in [`plain_text`].
const LINE_SEPARATOR: &str = " · ";

/// `text` as plain reading text for a search snippet: block markers, emphasis, highlight, strike
/// and code marks and link targets are gone (link text and bare URLs stay), fence and divider
/// lines are dropped, and the remaining lines are joined with " · ".
pub fn plain_text(text: &str) -> String {
    let mut in_fence = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        // Code keeps its characters; everything else loses its Markdown.
        let plain = if in_fence {
            line.trim().to_owned()
        } else {
            strip_marks(&strip_links(strip_line_prefix(line)))
        };
        // A line with no letter or digit is only symbols (a divider).
        if plain.chars().any(char::is_alphanumeric) {
            lines.push(plain.split_whitespace().collect::<Vec<_>>().join(" "));
        }
    }
    lines.join(LINE_SEPARATOR)
}

/// Removes `*`, backticks, `~~` and `==`, and `_` at the edge of a word (`snake_case` keeps its own).
fn strip_marks(s: &str) -> String {
    let s = s.replace("~~", "").replace("==", "");
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '*' | '`' => {}
            '_' => {
                let inside = i > 0
                    && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric())
                    && chars[i - 1].is_alphanumeric();
                if inside {
                    out.push(c);
                }
            }
            _ => out.push(c),
        }
    }
    out
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
    if digits > 0 && (s[digits..].starts_with(". ") || s[digits..].starts_with(") ")) {
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
        if let Some((text, tail)) = link {
            out.push_str(text);
            rest = tail;
        } else {
            out.push('[');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::word_count;

    #[test]
    fn plain_text_drops_markdown_and_joins_lines() {
        use super::plain_text;
        assert_eq!(
            plain_text(
                "# A plan\n\nSome **bold**, *it*, ~~gone~~ ==hi== `code` and [a link](https://x.io/a) at https://y.io"
            ),
            "A plan · Some bold, it, gone hi code and a link at https://y.io"
        );
        assert_eq!(
            plain_text("- [ ] Call\n- [x] Reply\n> quoted\n1. one\n---"),
            "Call · Reply · quoted · one"
        );
        assert_eq!(
            plain_text("a snake_case _word_\n```rust\nlet x_y = 1;\n```"),
            "a snake_case word · let x_y = 1;"
        );
    }

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
        // `1)` is a list marker too, as the task toggle accepts it.
        assert_eq!(word_count("1) one\n2) two words\n10) [ ] four"), 4);
        assert_eq!(word_count("1)nospace"), 1);
    }

    #[test]
    fn plain_text_drops_paren_list_markers() {
        assert_eq!(super::plain_text("1) one\n2) two"), "one · two");
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
