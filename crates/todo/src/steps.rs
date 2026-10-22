//! Open steps and "Tick all and complete": Rust ports of `openSteps` and `tickAllSteps`
//! (`src/modules/todo/notes.ts`), so the Todo widget decides exactly as the app does.
//! `src/lib/ipc/fixtures/todo-steps.json` runs the same cases here and in Vitest. Task text must
//! never reach a log line.

/// The notes' lines, each with its line end kept, and whether it is a Checklist item that is not
/// ticked: `(line, index of the character between the brackets)`.
fn checklist_lines(notes: &str) -> impl Iterator<Item = (&str, Option<(usize, bool)>)> {
    let mut fence: Option<(char, usize)> = None;
    notes.split_inclusive('\n').map(move |line| {
        if let Some(open) = fence_of(line) {
            match fence {
                None => fence = Some(open),
                Some((mark, len)) if open.0 == mark && open.1 >= len && is_bare_fence(line) => {
                    fence = None;
                }
                Some(_) => {}
            }
            return (line, None);
        }
        if fence.is_some() {
            return (line, None);
        }
        (line, step_box(line))
    })
}

/// A line that opens or closes a fenced code block: up to three spaces, then three or more
/// backticks or tildes. The mark and how many.
fn fence_of(line: &str) -> Option<(char, usize)> {
    let body = line.trim_start_matches(' ');
    if line.len() - body.len() > 3 {
        return None;
    }
    let mark = body.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let len = body.chars().take_while(|&c| c == mark).count();
    (len >= 3).then_some((mark, len))
}

/// A fence line with nothing after the marks (what closes a block).
fn is_bare_fence(line: &str) -> bool {
    line.trim()
        .chars()
        .all(|c| c == '`' || c == '~' || c.is_whitespace())
}

/// Where the box of a Checklist item is: `(byte index of the character between [ and ], ticked)`.
/// A line is an item when, after quote marks and indent, it has a bullet (`-`, `*`, `+`) or a
/// number with `.` or `)`, some space, then `[ ]`, `[x]` or `[X]` followed by a space or tab.
fn step_box(line: &str) -> Option<(usize, bool)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while matches!(bytes.get(i), Some(b' ' | b'\t' | b'>')) {
        i += 1;
    }
    match bytes.get(i)? {
        b'-' | b'*' | b'+' => i += 1,
        b'0'..=b'9' => {
            while bytes.get(i)?.is_ascii_digit() {
                i += 1;
            }
            if !matches!(bytes.get(i)?, b'.' | b')') {
                return None;
            }
            i += 1;
        }
        _ => return None,
    }
    let spaces = i;
    while matches!(bytes.get(i), Some(b' ' | b'\t')) {
        i += 1;
    }
    if i == spaces || bytes.get(i)? != &b'[' || bytes.get(i + 2)? != &b']' {
        return None;
    }
    // A bare box at the end of the line is not an item in the app.
    if !matches!(bytes.get(i + 3)?, b' ' | b'\t' | b'\r') {
        return None;
    }
    match bytes.get(i + 1)? {
        b' ' => Some((i + 1, false)),
        b'x' | b'X' => Some((i + 1, true)),
        _ => None,
    }
}

/// How many Checklist items are still unticked.
pub fn open_steps(notes: &str) -> usize {
    checklist_lines(notes)
        .filter(|(_, item)| matches!(item, Some((_, false))))
        .count()
}

/// The notes with every Checklist item ticked: "Tick all and complete".
pub fn tick_all_steps(notes: &str) -> String {
    let mut out = String::with_capacity(notes.len());
    for (line, item) in checklist_lines(notes) {
        match item {
            Some((at, false)) => {
                out.push_str(line.get(..at).unwrap_or(line));
                out.push('x');
                out.push_str(line.get(at + 1..).unwrap_or(""));
            }
            _ => out.push_str(line),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    const FIXTURE: &str = include_str!("../../../src/lib/ipc/fixtures/todo-steps.json");

    fn cases() -> Vec<Value> {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        fixture["cases"].as_array().unwrap().clone()
    }

    #[test]
    fn open_steps_match_the_app() {
        for case in cases() {
            let notes = case["notes"].as_str().unwrap();
            let open = usize::try_from(case["open"].as_u64().unwrap()).unwrap();
            assert_eq!(open_steps(notes), open, "{case}");
        }
    }

    #[test]
    fn tick_all_matches_the_app() {
        for case in cases() {
            let notes = case["notes"].as_str().unwrap();
            assert_eq!(
                tick_all_steps(notes),
                case["ticked"].as_str().unwrap(),
                "{case}"
            );
        }
    }

    #[test]
    fn ticking_all_leaves_no_open_step_and_odd_text_never_panics() {
        for notes in [
            "- [ ] a\n  - [ ] b",
            "- [ ",
            "- [",
            "-",
            "1",
            "1.",
            "é- [ ] x",
            "> ",
            "```",
        ] {
            assert_eq!(open_steps(&tick_all_steps(notes)), 0, "{notes:?}");
        }
    }
}
