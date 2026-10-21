//! Search snippets for Entries: a short window of the Entry's plain reading text with the
//! matching words marked.

use mnemora_core::search::{Matcher, Span, window};

use mnemora_core::search::plain::plain_text;

/// The snippet for `body`, shown as plain text (Markdown removed): around its first match, or its
/// start when nothing matches (a search by date alone).
pub fn snippet(body: &str, matcher: &Matcher) -> Vec<Span> {
    window(&plain_text(body), matcher)
}

#[cfg(test)]
mod tests {
    use mnemora_core::search::{Matcher, parse};

    use super::snippet;

    #[test]
    fn markdown_and_extra_whitespace_are_gone_before_marking() {
        let out = snippet(
            "the **red**   door",
            &Matcher::new(&parse("\"red door\""), &[]),
        );
        let texts: Vec<_> = out.iter().map(|x| (x.text.as_str(), x.matched)).collect();
        assert_eq!(
            texts,
            [("the ", false), ("red", true), (" ", false), ("door", true)]
        );
    }
}
