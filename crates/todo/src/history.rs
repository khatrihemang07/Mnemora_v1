//! A Task's History: readable lines built from consecutive change-log snapshots (ADR 0008). Each
//! record is compared with the one before it; what changed becomes a line such as
//! "Due date Thu 22 Oct → Fri 23 Oct". Title and notes edits arrive as one record per opening of
//! the sheet (the editing session), so they make one line. Read-only. Lines hold dates, list
//! names and Priorities, never the Task's own text.

use mnemora_core::changelog::{self, Record};
use mnemora_core::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use specta::Type;

use crate::error::TodoError;
use crate::service::ENTITY;
use crate::when::epoch_day;
use crate::{list_repo, repo};

/// One line of a Task's History.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct HistoryLine {
    /// When it happened: UTC milliseconds.
    #[specta(type = u32)]
    pub at: i64,
    /// What kind of change: `create`, `done`, `skip`, `reopen`, `wont`, `delete`, `restore`,
    /// `list`, `date`, `time`, `repeat`, `priority`, `title` or `notes`. The UI picks an icon from it.
    pub kind: String,
    /// The readable line.
    pub text: String,
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// `Thu 22 Oct` for a `YYYY-MM-DD` date; anything else as it is.
fn day_text(date: &str) -> String {
    let (Some(epoch), Ok(month), Ok(day)) = (
        epoch_day(date),
        date.get(5..7).unwrap_or("").parse::<usize>(),
        date.get(8..10).unwrap_or("").parse::<u32>(),
    ) else {
        return date.to_owned();
    };
    let weekday = usize::try_from((epoch + 4).rem_euclid(7)).unwrap_or(0);
    format!(
        "{} {day} {}",
        WEEKDAYS[weekday],
        MONTHS[month.clamp(1, 12) - 1]
    )
}

/// `Thu` for a `YYYY-MM-DD` date, or the full `Thu 22 Oct` when `other` is a week or more away
/// (a weekday alone would then be ambiguous). For "Skipped (Thu → Sat)".
fn skip_days(from: &str, to: &str) -> (String, String) {
    let near = matches!((epoch_day(from), epoch_day(to)), (Some(a), Some(b)) if (b - a).abs() < 7);
    let name = |d: &str| {
        let full = day_text(d);
        if near {
            full.split(' ').next().unwrap_or(&full).to_owned()
        } else {
            full
        }
    };
    (name(from), name(to))
}

/// `22 Oct`: the day without its weekday, for "occurrence of 18 Oct".
fn short_day(date: &str) -> String {
    day_text(date)
        .split_once(' ')
        .map_or_else(|| date.to_owned(), |(_, rest)| rest.to_owned())
}

/// "Every 3 days after done", the same wording as the UI's Repeat label.
fn repeat_label(freq: &str, every: u64, days: u64, mode: &str) -> String {
    let unit = match freq {
        "day" => "day",
        "week" => "week",
        "month" => "month",
        _ => "year",
    };
    let mut text = if every == 1 {
        format!("Every {unit}")
    } else {
        format!("Every {every} {unit}s")
    };
    if freq == "week" && days != 0 {
        text = if every == 1 && days == 0b011_1110 {
            "Every weekday".to_owned()
        } else {
            let names: Vec<&str> = [1, 2, 3, 4, 5, 6, 0]
                .into_iter()
                .filter(|d| days & (1 << d) != 0)
                .map(|d| WEEKDAYS[d])
                .collect();
            format!(
                "{} {}",
                if every == 1 {
                    "Every".to_owned()
                } else {
                    format!("Every {every} weeks on")
                },
                names.join(", ")
            )
        };
    }
    if mode == "after" {
        text.push_str(" after done");
    }
    text
}

/// What a snapshot holds, read leniently: a missing or odd field is "none".
struct Snap<'a>(&'a Value);

impl Snap<'_> {
    fn text(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.as_str()
    }
    fn number(&self, key: &str) -> Option<u64> {
        self.0.get(key)?.as_u64()
    }
    fn repeat(&self) -> Option<String> {
        Some(repeat_label(
            self.text("repeat_freq")?,
            self.number("repeat_every")?,
            self.number("repeat_days").unwrap_or(0),
            self.text("repeat_mode").unwrap_or("fixed"),
        ))
    }
}

/// A List as History names it: its emoji (or a neutral mark) and name.
fn list_label(conn: &Connection, id: Option<&str>) -> AppResult<String> {
    let Some(id) = id else {
        return Ok("📥 Inbox".to_owned());
    };
    Ok(match list_repo::get_any(conn, id)? {
        Some((list, _)) => format!("{} {}", list.emoji.as_deref().unwrap_or("≡"), list.name),
        None => "a deleted List".to_owned(),
    })
}

type Lines = Vec<(&'static str, String)>;

/// Whether a snapshot is marked deleted.
fn is_deleted(s: &Snap) -> bool {
    s.0.get("deleted_at").is_some_and(|v| !v.is_null())
}

/// Created, or (for an occurrence copy, which is created done) the day's completion.
fn first_line(cur: &Snap) -> (&'static str, String) {
    if cur.text("repeat_of").is_none() {
        return ("create", "Created".to_owned());
    }
    match cur.text("due_date") {
        Some(due) => ("done", format!("Done (occurrence of {})", short_day(due))),
        None => ("done", "Done".to_owned()),
    }
}

/// Deleted, restored, and the state: done, Won't do, reopened. A repeating Task is ticked without
/// closing, moving to its next date instead: that is an occurrence (`step`), unless the record is
/// marked `skipped` (Skip moves it on without doing it).
fn state_lines(old: &Snap, cur: &Snap, step: bool, skipped: bool, out: &mut Lines) {
    match (is_deleted(old), is_deleted(cur)) {
        (false, true) => out.push(("delete", "Deleted".to_owned())),
        (true, false) => out.push(("restore", "Restored".to_owned())),
        _ => {}
    }
    if step {
        if let (Some(a), Some(b)) = (old.text("due_date"), cur.text("due_date")) {
            out.push(if skipped {
                let (from, to) = skip_days(a, b);
                ("skip", format!("Skipped ({from} → {to})"))
            } else if b > a {
                ("done", format!("Done (occurrence of {})", short_day(a)))
            } else {
                (
                    "reopen",
                    format!("Reopened (occurrence of {})", short_day(b)),
                )
            });
        }
    } else if old.text("state") != cur.text("state") {
        out.push(match cur.text("state") {
            Some("done") => ("done", "Done".to_owned()),
            Some("wont") => ("wont", "Won’t do".to_owned()),
            _ => ("reopen", "Reopened".to_owned()),
        });
    }
}

/// Moved, Due date, time and Repeat. An occurrence step explains the new date itself.
fn schedule_lines(
    conn: &Connection,
    old: &Snap,
    cur: &Snap,
    step: bool,
    out: &mut Lines,
) -> AppResult<()> {
    if old.text("list_id") != cur.text("list_id") {
        out.push((
            "list",
            format!(
                "Moved {} → {}",
                list_label(conn, old.text("list_id"))?,
                list_label(conn, cur.text("list_id"))?
            ),
        ));
    }
    let (old_due, new_due) = (old.text("due_date"), cur.text("due_date"));
    if !step && old_due != new_due {
        out.push((
            "date",
            format!(
                "Due date {} → {}",
                old_due.map_or_else(|| "no date".to_owned(), day_text),
                new_due.map_or_else(|| "no date".to_owned(), day_text)
            ),
        ));
    }
    match (old.text("due_time"), cur.text("due_time")) {
        (a, b) if a == b => {}
        (None, Some(b)) => out.push(("time", format!("Time set to {b}"))),
        (Some(_), None) => out.push(("time", "Time removed".to_owned())),
        (Some(a), Some(b)) => out.push(("time", format!("Time {a} → {b}"))),
        (None, None) => {}
    }
    if !step {
        match (old.repeat(), cur.repeat()) {
            (a, b) if a == b => {}
            (None, Some(b)) => out.push(("repeat", format!("Repeat set: {b}"))),
            (Some(_), None) => out.push(("repeat", "Repeat removed".to_owned())),
            (Some(a), Some(b)) => out.push(("repeat", format!("Repeat {a} → {b}"))),
            (None, None) => {}
        }
    }
    Ok(())
}

/// A Task's title and notes as History last counted them: what the next session's edits are
/// compared with.
#[derive(Clone)]
struct Baseline {
    title: String,
    notes: String,
}

impl Baseline {
    fn of(s: &Snap) -> Self {
        Self {
            title: s.text("title").unwrap_or_default().to_owned(),
            notes: s.text("notes").unwrap_or_default().to_owned(),
        }
    }
}

/// Priority, and the text edits. Only an editing session edits text, and a session's record can
/// be rewritten after other changes (it carries the latest snapshot), so the text is compared with
/// the `base` from before the session, never with the neighbouring record: one opening of the
/// sheet is one "Title edited" and one "Notes edited" line however much happened in between.
fn edit_lines(old: &Snap, cur: &Snap, record: &Record, base: &Baseline, out: &mut Lines) {
    if old.number("priority") != cur.number("priority") {
        out.push((
            "priority",
            format!(
                "Priority P{} → P{}",
                old.number("priority").unwrap_or(4),
                cur.number("priority").unwrap_or(4)
            ),
        ));
    }
    if record.session_id.is_none() {
        return;
    }
    if cur.text("title").unwrap_or_default() != base.title {
        out.push(("title", "Title edited".to_owned()));
    }
    if cur.text("notes").unwrap_or_default() != base.notes {
        out.push(("notes", "Notes edited".to_owned()));
    }
}

/// The lines one record adds to its predecessor, in reading order.
fn record_lines(
    conn: &Connection,
    prev: Option<&Value>,
    base: &Baseline,
    record: &Record,
) -> AppResult<Lines> {
    let cur = Snap(&record.snapshot);
    let Some(prev) = prev else {
        return Ok(if record.op == "create" {
            vec![first_line(&cur)]
        } else {
            Vec::new()
        });
    };
    let old = Snap(prev);
    let (old_due, new_due) = (old.text("due_date"), cur.text("due_date"));
    let step = record.op == "tick"
        && old.text("state") == cur.text("state")
        && old_due.is_some()
        && new_due.is_some()
        && old_due != new_due;
    let skipped = step && cur.0.get("skipped").and_then(Value::as_bool) == Some(true);
    let mut out = Vec::new();
    state_lines(&old, &cur, step, skipped, &mut out);
    schedule_lines(conn, &old, &cur, step, &mut out)?;
    edit_lines(&old, &cur, record, base, &mut out);
    Ok(out)
}

/// A Task's History, newest line first. Records of a Task that has been purged are gone with it.
pub fn task_history(conn: &Connection, id: &str) -> AppResult<Vec<HistoryLine>> {
    repo::get(conn, id)?.ok_or(TodoError::TaskNotFound)?;
    let records = changelog::read(conn, ENTITY, id)?;
    let mut batches: Vec<Vec<HistoryLine>> = Vec::with_capacity(records.len());
    let mut prev: Option<&Value> = None;
    let mut base: Option<Baseline> = None;
    for record in &records {
        let lines = record_lines(
            conn,
            prev,
            base.as_ref()
                .unwrap_or(&Baseline::of(&Snap(&record.snapshot))),
            record,
        )?
        .into_iter()
        .map(|(kind, text)| HistoryLine {
            at: record.at,
            kind: kind.to_owned(),
            text,
        })
        .collect();
        batches.push(lines);
        // A session settles the text; so do creating and a tick (a Checklist reset). Any other
        // record only carries the text of a session that is still being written.
        if base.is_none()
            || record.session_id.is_some()
            || matches!(record.op.as_str(), "create" | "tick")
        {
            base = Some(Baseline::of(&Snap(&record.snapshot)));
        }
        prev = Some(&record.snapshot);
    }
    Ok(batches.into_iter().rev().flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_are_written_with_their_weekday() {
        assert_eq!(day_text("2026-10-22"), "Thu 22 Oct");
        assert_eq!(day_text("2026-10-18"), "Sun 18 Oct");
        assert_eq!(short_day("2026-10-18"), "18 Oct");
        assert_eq!(day_text("nonsense"), "nonsense");
    }

    #[test]
    fn a_skip_names_weekdays_alone_within_a_week_and_whole_days_beyond() {
        assert_eq!(
            skip_days("2026-10-22", "2026-10-24"),
            ("Thu".to_owned(), "Sat".to_owned())
        );
        assert_eq!(
            skip_days("2026-10-22", "2026-11-05"),
            ("Thu 22 Oct".to_owned(), "Thu 5 Nov".to_owned())
        );
    }

    #[test]
    fn repeats_read_like_the_picker_says_them() {
        assert_eq!(repeat_label("day", 1, 0, "fixed"), "Every day");
        assert_eq!(
            repeat_label("day", 3, 0, "after"),
            "Every 3 days after done"
        );
        assert_eq!(
            repeat_label("week", 1, 0b011_1110, "fixed"),
            "Every weekday"
        );
        assert_eq!(
            repeat_label("week", 2, 0b100_0010, "fixed"),
            "Every 2 weeks on Mon, Sat"
        );
        assert_eq!(repeat_label("month", 1, 31, "fixed"), "Every month");
    }
}
