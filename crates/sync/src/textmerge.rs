//! Three-way text merge (spec "Text merge"): `base` is the text two edits started from, `ours`
//! and `theirs` the two results. Pure and deterministic; it knows nothing of devices.
//!
//! By line first: a line range only one side changed is taken from that side. Where both sides
//! changed the same lines differently, the lines are merged again by word (a word is whatever
//! sits between spaces, punctuation and Markdown marks included, so "tomorrow" becoming
//! "tomorrow!" changes the word), and only words both sides changed are a **clash**. Edits that merely touch (neighbouring lines or words) do not overlap. Two
//! different insertions at the same spot do.
//!
//! A [`Merge`] keeps the clashes apart as [`Piece::Clash`], so Compare can show them and Edit
//! merged can ask which side to use; [`Merge::text`] is the merged text with every clash settled
//! for `theirs`, which callers pass as the newer side.

use std::hash::Hash;
use std::ops::Range;

use similar::{Algorithm, DiffTag, capture_diff_slices};

/// Both sides changed the same words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    /// What both started from, for the clashing part.
    pub base: String,
    /// Our side's words.
    pub ours: String,
    /// Their side's words.
    pub theirs: String,
}

/// One stretch of a merged text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Text both sides agree on, or only one side changed.
    Agreed(String),
    /// A part both sides rewrote differently.
    Clash(Clash),
}

/// The result of merging: pieces in text order, adjacent agreed pieces joined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merge {
    /// The pieces.
    pub pieces: Vec<Piece>,
}

impl Merge {
    /// The merged text; each clash takes `theirs`.
    pub fn text(&self) -> String {
        self.pieces
            .iter()
            .map(|p| match p {
                Piece::Agreed(t) => t.as_str(),
                Piece::Clash(c) => c.theirs.as_str(),
            })
            .collect()
    }

    /// How many clashes there are.
    pub fn clashes(&self) -> usize {
        self.pieces
            .iter()
            .filter(|p| matches!(p, Piece::Clash(_)))
            .count()
    }
}

/// Merges `ours` and `theirs`, both edits of `base`.
pub fn merge(base: &str, ours: &str, theirs: &str) -> Merge {
    let mut out = Pieces::default();
    if ours == theirs || base == theirs {
        out.agreed(ours);
    } else if base == ours {
        out.agreed(theirs);
    } else {
        let lines = |t| lines_of(t);
        for seg in merge_seq(&lines(base), &lines(ours), &lines(theirs)) {
            match seg {
                Seg::Same(parts) => out.agreed(&parts.concat()),
                Seg::Both { base, ours, theirs } => {
                    words(&base.concat(), &ours.concat(), &theirs.concat(), &mut out);
                }
            }
        }
    }
    Merge { pieces: out.0 }
}

/// Collects pieces, joining agreed ones.
#[derive(Default)]
struct Pieces(Vec<Piece>);

impl Pieces {
    fn agreed(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(Piece::Agreed(last)) = self.0.last_mut() {
            last.push_str(text);
        } else {
            self.0.push(Piece::Agreed(text.to_owned()));
        }
    }

    fn clash(&mut self, clash: Clash) {
        self.0.push(Piece::Clash(clash));
    }
}

/// The word-level merge of a region both sides changed.
fn words(base: &str, ours: &str, theirs: &str, out: &mut Pieces) {
    if ours == theirs {
        out.agreed(ours);
        return;
    }
    for seg in merge_seq(&tokens_of(base), &tokens_of(ours), &tokens_of(theirs)) {
        match seg {
            Seg::Same(parts) => out.agreed(&parts.concat()),
            Seg::Both { base, ours, theirs } => settle(&base, &ours, &theirs, out),
        }
    }
}

/// A clash of tokens: what the two sides share at either end is agreed text, not part of the
/// clash ("canal, felt great" against "canal, legs heavy" clashes on the words after "canal,").
/// A change that sits inside the other settles on the longer one.
fn settle(base: &[&str], ours: &[&str], theirs: &[&str], out: &mut Pieces) {
    let pre = ours.iter().zip(theirs).take_while(|(a, b)| a == b).count();
    let suf = ours[pre..]
        .iter()
        .rev()
        .zip(theirs[pre..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let (o, t) = (
        &ours[pre..ours.len() - suf],
        &theirs[pre..theirs.len() - suf],
    );
    out.agreed(&ours[..pre].concat());
    // One side's whole change sits inside the other's: the longer one says it all.
    if (o.is_empty() && !ours.is_empty()) || (t.is_empty() && !theirs.is_empty()) {
        out.agreed(&o.concat());
        out.agreed(&t.concat());
    } else {
        // The base shares the trimmed ends only when it does; it is for display.
        let b_pre = base
            .iter()
            .zip(&ours[..pre])
            .take_while(|(a, b)| a == b)
            .count();
        let rest = &base[b_pre..];
        let b_suf = rest
            .iter()
            .rev()
            .zip(ours[ours.len() - suf..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        out.clash(Clash {
            base: rest[..rest.len() - b_suf].concat(),
            ours: o.concat(),
            theirs: t.concat(),
        });
    }
    out.agreed(&ours[ours.len() - suf..].concat());
}

/// A stretch of the three-way merge of sequences.
#[derive(Debug)]
enum Seg<T> {
    /// One result: untouched, changed by one side only, or changed the same by both.
    Same(Vec<T>),
    /// Both sides changed this part of `base` and not the same way.
    Both {
        base: Vec<T>,
        ours: Vec<T>,
        theirs: Vec<T>,
    },
}

/// Where a side replaced `base[base]` with `new[new]`.
#[derive(Debug, Clone)]
struct Hunk {
    base: Range<usize>,
    new: Range<usize>,
}

fn hunks<T: Eq + Hash + Ord>(base: &[T], new: &[T]) -> Vec<Hunk> {
    let mut out: Vec<Hunk> = Vec::new();
    let mut open: Option<Hunk> = None;
    for op in capture_diff_slices(Algorithm::Myers, base, new) {
        let (tag, old, now) = op.as_tag_tuple();
        if tag == DiffTag::Equal {
            out.extend(open.take());
        } else if let Some(h) = &mut open {
            h.base.end = old.end;
            h.new.end = now.end;
        } else {
            open = Some(Hunk {
                base: old,
                new: now,
            });
        }
    }
    out.extend(open);
    out
}

/// Whether two hunks of different sides change the same part of the base. Touching ranges and
/// an insertion at the edge of the other's range do not; an insertion inside it, and two
/// insertions at one spot, do.
fn overlap(a: &Range<usize>, b: &Range<usize>) -> bool {
    if a.start < b.end && b.start < a.end {
        return true;
    }
    let inside = |point: usize, r: &Range<usize>| r.start < point && point < r.end;
    (a.is_empty() && b.is_empty() && a.start == b.start)
        || (a.is_empty() && inside(a.start, b))
        || (b.is_empty() && inside(b.start, a))
}

/// A run of hunks that depend on each other.
#[derive(Default)]
struct Region {
    ours: Vec<Hunk>,
    theirs: Vec<Hunk>,
}

impl Region {
    fn is_empty(&self) -> bool {
        self.ours.is_empty() && self.theirs.is_empty()
    }

    fn span(&self) -> Range<usize> {
        let all = self.ours.iter().chain(&self.theirs);
        let start = all.clone().map(|h| h.base.start).min().unwrap_or(0);
        let end = all.map(|h| h.base.end).max().unwrap_or(0);
        start..end
    }

    fn touches(&self, hunk: &Hunk, ours: bool) -> bool {
        let others = if ours { &self.theirs } else { &self.ours };
        others.iter().any(|o| overlap(&hunk.base, &o.base))
    }
}

/// The side's version of `base[span]`.
fn apply<T: Clone>(base: &[T], new: &[T], span: &Range<usize>, hunks: &[Hunk]) -> Vec<T> {
    let mut out = Vec::new();
    let mut at = span.start;
    for h in hunks {
        out.extend_from_slice(&base[at..h.base.start]);
        out.extend_from_slice(&new[h.new.clone()]);
        at = h.base.end;
    }
    out.extend_from_slice(&base[at..span.end]);
    out
}

fn merge_seq<T: Eq + Hash + Ord + Clone>(base: &[T], ours: &[T], theirs: &[T]) -> Vec<Seg<T>> {
    // Both sides' hunks in base order; at one spot ours first.
    let mut all: Vec<(Hunk, bool)> = hunks(base, ours)
        .into_iter()
        .map(|h| (h, true))
        .chain(hunks(base, theirs).into_iter().map(|h| (h, false)))
        .collect();
    all.sort_by_key(|(h, ours)| (h.base.start, h.base.end, !ours));

    let mut regions: Vec<Region> = Vec::new();
    let mut current = Region::default();
    for (hunk, is_ours) in all {
        if !current.is_empty() && !current.touches(&hunk, is_ours) {
            regions.push(std::mem::take(&mut current));
        }
        if is_ours {
            current.ours.push(hunk);
        } else {
            current.theirs.push(hunk);
        }
    }
    if !current.is_empty() {
        regions.push(current);
    }

    let mut out = Vec::new();
    let mut at = 0;
    for region in regions {
        let span = region.span();
        if span.start > at {
            out.push(Seg::Same(base[at..span.start].to_vec()));
        }
        at = span.end;
        let mine = apply(base, ours, &span, &region.ours);
        let yours = apply(base, theirs, &span, &region.theirs);
        out.push(if region.theirs.is_empty() {
            Seg::Same(mine)
        } else if region.ours.is_empty() || mine == yours {
            Seg::Same(yours)
        } else {
            Seg::Both {
                base: base[span].to_vec(),
                ours: mine,
                theirs: yours,
            }
        });
    }
    if at < base.len() {
        out.push(Seg::Same(base[at..].to_vec()));
    }
    out
}

/// A stretch of one text, and whether it differs from the other version.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct DiffSpan {
    /// The text, line breaks included.
    pub text: String,
    /// Words here are not in the other version.
    pub changed: bool,
}

/// Both texts cut into stretches, with the words each has that the other lacks marked: what
/// Compare shows side by side.
pub fn diff(a: &str, b: &str) -> (Vec<DiffSpan>, Vec<DiffSpan>) {
    let (ta, tb) = (tokens_of(a), tokens_of(b));
    let mut changed_a = vec![false; ta.len()];
    let mut changed_b = vec![false; tb.len()];
    for op in capture_diff_slices(Algorithm::Myers, &ta, &tb) {
        let (tag, old, new) = op.as_tag_tuple();
        if tag != DiffTag::Equal {
            changed_a[old].fill(true);
            changed_b[new].fill(true);
        }
    }
    bridge(&ta, &mut changed_a);
    bridge(&tb, &mut changed_b);
    (spans(&ta, &changed_a), spans(&tb, &changed_b))
}

/// Spaces between two changed words belong to the change, so a changed phrase is one stretch.
fn bridge(tokens: &[&str], changed: &mut [bool]) {
    for i in 1..tokens.len().saturating_sub(1) {
        if !changed[i]
            && changed[i - 1]
            && changed[i + 1]
            && tokens[i].chars().all(char::is_whitespace)
        {
            changed[i] = true;
        }
    }
}

fn spans(tokens: &[&str], changed: &[bool]) -> Vec<DiffSpan> {
    let mut out: Vec<DiffSpan> = Vec::new();
    for (token, &flag) in tokens.iter().zip(changed) {
        match out.last_mut() {
            Some(last) if last.changed == flag => last.text.push_str(token),
            _ => out.push(DiffSpan {
                text: (*token).to_owned(),
                changed: flag,
            }),
        }
    }
    out
}

/// Lines, each with its line break.
fn lines_of(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// Words (anything between spaces: punctuation and Markdown marks stay with their word), runs
/// of spaces, and single line breaks. Accents, emoji and other scripts stay whole.
fn tokens_of(text: &str) -> Vec<&str> {
    #[derive(PartialEq, Clone, Copy)]
    enum Kind {
        Word,
        Space,
        Break,
    }
    let kind = |c: char| {
        if c == '\n' {
            Kind::Break
        } else if c.is_whitespace() {
            Kind::Space
        } else {
            Kind::Word
        }
    };
    let mut out = Vec::new();
    let mut start = 0;
    let mut last: Option<Kind> = None;
    for (i, c) in text.char_indices() {
        let k = kind(c);
        if let Some(prev) = last
            && (k != prev || k == Kind::Break)
        {
            out.push(&text[start..i]);
            start = i;
        }
        last = Some(k);
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(base: &str, ours: &str, theirs: &str) -> String {
        merge(base, ours, theirs).text()
    }

    fn clash(m: &Merge, n: usize) -> (&str, &str, &str) {
        match m
            .pieces
            .iter()
            .filter(|p| matches!(p, Piece::Clash(_)))
            .nth(n)
        {
            Some(Piece::Clash(c)) => (&c.base, &c.ours, &c.theirs),
            _ => panic!("no clash {n}"),
        }
    }

    #[test]
    fn edits_to_different_lines_merge_without_a_clash() {
        let base = "one\ntwo\nthree\nfour\n";
        let m = merge(base, "ONE\ntwo\nthree\nfour\n", "one\ntwo\nthree\nFOUR\n");
        assert_eq!(m.clashes(), 0);
        assert_eq!(m.text(), "ONE\ntwo\nthree\nFOUR\n");
    }

    #[test]
    fn neighbouring_lines_do_not_overlap() {
        let m = merge("a\nb\n", "A\nb\n", "a\nB\n");
        assert_eq!((m.clashes(), m.text().as_str()), (0, "A\nB\n"));
    }

    #[test]
    fn different_words_of_one_line_merge() {
        let base = "The quick brown fox jumps over the lazy dog.\n";
        let ours = "The quick red fox jumps over the lazy dog.\n";
        let theirs = "The quick brown fox leaps over the lazy cat.\n";
        let m = merge(base, ours, theirs);
        assert_eq!(m.clashes(), 0);
        assert_eq!(m.text(), "The quick red fox leaps over the lazy cat.\n");
    }

    #[test]
    fn the_same_words_changed_differently_clash_and_theirs_is_taken() {
        let base = "Went for a run along the canal.\nCoffee.\n";
        let ours = "Went for a run along the canal, felt great after the rain.\nCoffee.\n";
        let theirs = "Went for a run along the canal, legs heavy but good.\nCoffee.\n";
        let m = merge(base, ours, theirs);
        assert_eq!(m.clashes(), 1);
        // The word both wrote the same ("canal,") is agreed text; the rest clashes.
        assert_eq!(
            clash(&m, 0),
            (
                "canal.",
                "felt great after the rain.",
                "legs heavy but good."
            )
        );
        assert_eq!(m.text(), theirs);
    }

    #[test]
    fn a_clash_keeps_edits_around_it() {
        let base = "alpha beta gamma\ndelta\n";
        let m = merge(
            base,
            "alpha BETA gamma\nDELTA\n",
            "alpha Beta gamma\ndelta\n",
        );
        assert_eq!(m.clashes(), 1);
        assert_eq!(clash(&m, 0), ("beta", "BETA", "Beta"));
        assert_eq!(m.text(), "alpha Beta gamma\nDELTA\n");
    }

    #[test]
    fn the_same_change_on_both_sides_is_no_clash() {
        let m = merge("a b c", "a X c", "a X c");
        assert_eq!((m.clashes(), m.text().as_str()), (0, "a X c"));
        let m = merge("a\nb\nc\n", "a\nX\nc\n", "a\nX\nc\n");
        assert_eq!((m.clashes(), m.text().as_str()), (0, "a\nX\nc\n"));
    }

    #[test]
    fn two_different_insertions_at_one_spot_clash() {
        let m = merge("start\nend\n", "start\nours\nend\n", "start\ntheirs\nend\n");
        assert_eq!(m.clashes(), 1);
        assert_eq!(clash(&m, 0), ("", "ours", "theirs"));
        assert_eq!(m.text(), "start\ntheirs\nend\n");
    }

    #[test]
    fn one_insertion_containing_the_other_settles_on_the_longer() {
        let m = merge("a\n", "a, x\n", "a, x y\n");
        assert_eq!((m.clashes(), m.text().as_str()), (0, "a, x y\n"));
    }

    #[test]
    fn insertions_at_different_spots_both_land() {
        let m = merge("a\nb\nc\n", "x\na\nb\nc\n", "a\nb\nc\ny\n");
        assert_eq!((m.clashes(), m.text().as_str()), (0, "x\na\nb\nc\ny\n"));
    }

    #[test]
    fn an_empty_base_clashes_when_both_wrote_something() {
        let m = merge("", "mine", "yours");
        assert_eq!(m.clashes(), 1);
        assert_eq!(clash(&m, 0), ("", "mine", "yours"));
        assert_eq!(text("", "same", "same"), "same");
        assert_eq!(text("", "", "yours"), "yours");
        assert_eq!(text("", "mine", ""), "mine");
    }

    #[test]
    fn only_one_side_changing_takes_that_side() {
        assert_eq!(text("a b", "a b", "a c"), "a c");
        assert_eq!(text("a b", "a c", "a b"), "a c");
        assert_eq!(merge("a b", "a b", "a b").clashes(), 0);
    }

    #[test]
    fn a_trailing_newline_is_just_text() {
        // One side added the final newline, the other edited the last line.
        let m = merge("a\nb", "a\nb\n", "a\nB");
        assert_eq!(m.clashes(), 0);
        assert_eq!(m.text(), "a\nB\n");
        assert_eq!(text("a\nb\n", "a\nb", "a\nb\n"), "a\nb");
        assert_eq!(text("a\n", "a\n\n", "a\n"), "a\n\n");
    }

    #[test]
    fn accents_emoji_and_other_scripts_stay_whole() {
        let base = "Café au lait ☕ 今日は晴れ\n";
        let m = merge(
            base,
            "Café noir ☕ 今日は晴れ\n",
            "Café au lait ☕ 今日は雨\n",
        );
        assert_eq!(m.clashes(), 0);
        assert_eq!(m.text(), "Café noir ☕ 今日は雨\n");
        // Changing the same accented word clashes on the whole word, never half a character.
        let m = merge("naïve idea", "naïf idea", "naïveté idea");
        assert_eq!(m.clashes(), 1);
        assert_eq!(clash(&m, 0), ("naïve", "naïf", "naïveté"));
    }

    #[test]
    fn crlf_lines_merge_like_lf_lines() {
        let m = merge("a\r\nb\r\n", "A\r\nb\r\n", "a\r\nB\r\n");
        assert_eq!((m.clashes(), m.text().as_str()), (0, "A\r\nB\r\n"));
    }

    #[test]
    fn swapping_the_sides_gives_the_same_text_when_nothing_clashes() {
        let cases = [
            ("a b c d", "a X c d", "a b c Y"),
            ("l1\nl2\nl3\n", "l1\nL2\nl3\n", "l1\nl2\nl3\nl4\n"),
            ("x\n", "x\n\nmore", "pre\nx\n"),
        ];
        for (b, o, t) in cases {
            let (m1, m2) = (merge(b, o, t), merge(b, t, o));
            assert_eq!((m1.clashes(), m2.clashes()), (0, 0), "{b:?}");
            assert_eq!(m1.text(), m2.text(), "{b:?}");
        }
    }

    #[test]
    fn diff_marks_the_words_each_text_has_that_the_other_lacks() {
        let (a, b) = diff("Went for a run, felt great.", "Went for a run, legs heavy.");
        let marked = |spans: &[DiffSpan]| -> Vec<String> {
            spans
                .iter()
                .filter(|s| s.changed)
                .map(|s| s.text.clone())
                .collect()
        };
        assert_eq!(marked(&a), ["felt great."]);
        assert_eq!(marked(&b), ["legs heavy."]);
        assert_eq!(
            a.iter().map(|s| s.text.as_str()).collect::<String>(),
            "Went for a run, felt great."
        );
        assert_eq!(
            diff("same", "same").0,
            [DiffSpan {
                text: "same".into(),
                changed: false
            }]
        );
        assert!(diff("", "").0.is_empty());
    }

    #[test]
    fn tokens_split_words_spaces_and_line_breaks() {
        assert_eq!(
            tokens_of("Hi, you  there\n\n**ok**"),
            ["Hi,", " ", "you", "  ", "there", "\n", "\n", "**ok**"]
        );
        assert!(tokens_of("").is_empty());
    }
}
