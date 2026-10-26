//! Conversations and answers as Markdown, for copying. The rules are here so that the copy the
//! screen makes and the copy a test makes are the same text.

use crate::tools::is_id;
use crate::types::{EntryPayload, EntryView};

/// A Conversation's branch as Markdown: its title, then each question as "**You:**" and each
/// answer under it. Tool steps, corrections and the Provider's switches are left out.
pub fn conversation(title: &str, branch: &[EntryView]) -> String {
    let mut blocks = vec![format!("# {title}")];
    for entry in branch {
        match &entry.payload {
            EntryPayload::User { text } => blocks.push(format!("**You:** {text}")),
            EntryPayload::Assistant { text, calls, .. } if calls.is_empty() => {
                let answer = strip_citations(text);
                if !answer.trim().is_empty() {
                    blocks.push(answer);
                }
            }
            _ => {}
        }
    }
    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}

/// An answer as Markdown: its text with the citation markers taken out.
pub fn answer(text: &str) -> String {
    strip_citations(text)
}

/// Removes each citation, a bracketed Entry or Task id and the whitespace before it. The screen
/// shows the answer the same way, so a copy reads like what was on the screen.
pub fn strip_citations(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        let (before, after) = rest.split_at(start);
        let bytes = after.as_bytes();
        let is_citation = bytes.len() >= 38 && is_id(&bytes[1..37]) && bytes[37] == b']';
        if is_citation {
            out.push_str(before.trim_end());
            rest = &after[38..];
        } else {
            out.push_str(before);
            out.push('[');
            rest = &after[1..];
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "01990000-0000-7000-8000-0000000000e1";

    #[test]
    fn citations_go_with_the_space_before_them() {
        assert_eq!(strip_citations(&format!("Two tasks [{ID}].")), "Two tasks.");
        assert_eq!(strip_citations(&format!("A\n[{ID}] B")), "A B");
    }

    #[test]
    fn brackets_that_are_not_ids_stay() {
        assert_eq!(strip_citations("see [1] and [note]"), "see [1] and [note]");
        assert_eq!(strip_citations("café [é] ok"), "café [é] ok");
    }
}
