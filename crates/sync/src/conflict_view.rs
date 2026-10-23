//! What the Conflicts tab and Compare show, ready to draw: labels, the two versions with their
//! differing words marked, the original, and the clashes for Edit merged. All computed here; the
//! window only lays them out.
//!
//! "Showing" is the live text of the record, so it follows the person's own edits after the
//! Conflict was raised. "Other" is the version that lost the merge.

use mnemora_core::error::AppResult;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::conflicts::{self, Resolution, Stored};
use crate::error::SyncError;
use crate::peers;
use crate::resolved_view::{self, ResolvedRow};
use crate::textmerge::{self, DiffSpan, Piece};
use crate::texts;

/// What a Conflict is about, as the list names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConflictSubject {
    /// A Journal Entry's text: "Wed 21 Oct · 09:12 Entry".
    Entry {
        /// The Page it is on, `YYYY-MM-DD`.
        page_date: String,
        /// When it was written, `HH:MM` in the zone it was written in.
        written_time: String,
    },
    /// A Task's title or notes: "‘Plan weekend hike’ · notes".
    Task {
        /// The Task's title now.
        title: String,
        /// Which text.
        field: TaskText,
    },
    /// A Comment on a Task.
    Comment {
        /// The Task's title now.
        task_title: String,
    },
}

/// Which text of a Task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TaskText {
    /// Its title.
    Title,
    /// Its notes.
    Notes,
}

/// One side of a Conflict: whose it was and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ConflictSide {
    /// The device's name.
    pub device: String,
    /// When it was written: UTC milliseconds.
    #[specta(type = u32)]
    pub at: i64,
}

/// A row of "Needs you".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ConflictRow {
    /// The Conflict's id.
    pub id: String,
    /// What it is about.
    pub subject: ConflictSubject,
    /// The Module it belongs to, for the row's icon: `journal` or `todo`.
    pub module: String,
    /// When the newest version was written: UTC milliseconds. Rows group by this day.
    #[specta(type = u32)]
    pub raised_at: i64,
    /// The newer version, the one that was applied.
    pub showing: ConflictSide,
    /// The other version.
    pub other: ConflictSide,
}

/// The Conflicts tab's "Needs you".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ConflictList {
    /// Conflicts that wait, newest first. Its length is the count.
    pub needs_you: Vec<ConflictRow>,
    /// What was decided without asking, newest first.
    pub resolved: Vec<ResolvedRow>,
}

/// A part of the text for Edit merged: agreed, or a clash to choose for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClashPiece {
    /// Text both versions have.
    Agreed {
        /// The text.
        text: String,
    },
    /// Words both rewrote.
    Clash {
        /// The newer version's words.
        showing: String,
        /// The other version's words.
        other: String,
        /// What both started from.
        original: String,
    },
}

/// The choice for one clash in Edit merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ClashChoice {
    /// The newer version's words.
    Showing,
    /// The other version's words.
    Other,
    /// Both, the newer one first.
    Both,
}

/// One Conflict for Compare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ConflictView {
    /// The header.
    pub row: ConflictRow,
    /// Still waiting for a decision.
    pub open: bool,
    /// How it ended, once it has.
    pub resolution: Option<String>,
    /// The text showing now, words the other version lacks marked.
    pub showing: Vec<DiffSpan>,
    /// The other version, words the showing text lacks marked.
    pub other: Vec<DiffSpan>,
    /// The text both devices started from.
    pub original: String,
    /// The text, cut at the clashes, for Edit merged.
    pub pieces: Vec<ClashPiece>,
}

/// Who made a version: this device's name, a paired device's name, or a device that is no
/// longer paired.
fn device_name(conn: &Connection, device: &str, me: &str, my_name: &str) -> String {
    if device == me {
        return my_name.to_owned();
    }
    peers::name_of(conn, device)
        .ok()
        .flatten()
        .unwrap_or_else(|| "Another device".to_owned())
}

fn side(conn: &Connection, device: &str, at: i64, me: &str, my_name: &str) -> ConflictSide {
    ConflictSide {
        device: device_name(conn, device, me, my_name),
        at,
    }
}

fn string_of(snapshot: Option<&serde_json::Value>, key: &str) -> String {
    snapshot
        .and_then(|s| s.get(key))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn subject(conn: &Connection, c: &Stored) -> Result<(ConflictSubject, &'static str), SyncError> {
    subject_of(conn, &c.target, &c.target_id, &c.field)
}

/// What a record is, as the Conflicts tab names it, and the Module it belongs to.
pub(crate) fn subject_of(
    conn: &Connection,
    target_entity: &str,
    target_id: &str,
    field: &str,
) -> Result<(ConflictSubject, &'static str), SyncError> {
    let target = texts::current(conn, target_entity, target_id)?;
    let target = target.as_ref();
    Ok(match target_entity {
        "journal_entry" => {
            let written = target
                .and_then(|t| t.get("written_at"))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_default();
            let offset = target
                .and_then(|t| t.get("written_offset_min"))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_default();
            let minute = (written / 60_000 + offset).rem_euclid(1440);
            (
                ConflictSubject::Entry {
                    page_date: string_of(target, "page_date"),
                    written_time: format!("{:02}:{:02}", minute / 60, minute % 60),
                },
                "journal",
            )
        }
        "todo_comment" => {
            let task_id = string_of(target, "task_id");
            let task = texts::current(conn, "todo_task", &task_id)?;
            (
                ConflictSubject::Comment {
                    task_title: string_of(task.as_ref(), "title"),
                },
                "todo",
            )
        }
        _ => (
            ConflictSubject::Task {
                title: string_of(target, "title"),
                field: if field == "notes" {
                    TaskText::Notes
                } else {
                    TaskText::Title
                },
            },
            "todo",
        ),
    })
}

fn row(conn: &Connection, c: &Stored, me: &str, my_name: &str) -> Result<ConflictRow, SyncError> {
    let (subject, module) = subject(conn, c)?;
    Ok(ConflictRow {
        id: c.id.clone(),
        subject,
        module: module.to_owned(),
        raised_at: c.raised_at,
        showing: side(conn, &c.a_device, c.a_at, me, my_name),
        other: side(conn, &c.b_device, c.b_at, me, my_name),
    })
}

/// "Needs you": every Conflict that waits.
pub fn list(conn: &Connection, me: &str, my_name: &str) -> AppResult<ConflictList> {
    let needs_you = conflicts::open(conn)?
        .iter()
        .map(|c| row(conn, c, me, my_name))
        .collect::<Result<_, _>>()?;
    Ok(ConflictList {
        needs_you,
        resolved: resolved_view::list(conn, me, my_name)?,
    })
}

/// The text of the field a Conflict is about as the record holds it now.
fn showing_now(conn: &Connection, c: &Stored) -> Result<String, SyncError> {
    Ok(string_of(
        texts::current(conn, &c.target, &c.target_id)?.as_ref(),
        &c.field,
    ))
}

/// One Conflict for Compare; `None` when there is none with that id.
pub fn view(
    conn: &Connection,
    id: &str,
    me: &str,
    my_name: &str,
) -> AppResult<Option<ConflictView>> {
    let Some(c) = conflicts::get(conn, id)? else {
        return Ok(None);
    };
    let showing = showing_now(conn, &c)?;
    let (showing_spans, other_spans) = textmerge::diff(&showing, &c.b_text);
    // The clashes of the merge that raised it: the older version is "ours", the newer "theirs".
    let pieces = textmerge::merge(&c.base, &c.b_text, &c.a_text)
        .pieces
        .into_iter()
        .map(|p| match p {
            Piece::Agreed(text) => ClashPiece::Agreed { text },
            Piece::Clash(k) => ClashPiece::Clash {
                showing: k.theirs,
                other: k.ours,
                original: k.base,
            },
        })
        .collect();
    Ok(Some(ConflictView {
        row: row(conn, &c, me, my_name)?,
        open: c.open,
        resolution: c.resolution.clone(),
        showing: showing_spans,
        other: other_spans,
        original: c.base.clone(),
        pieces,
    }))
}

/// The text Edit merged starts from: the agreed parts with each clash settled as chosen. A
/// clash without a choice takes the newer version.
pub fn draft(conn: &Connection, id: &str, choices: &[ClashChoice]) -> AppResult<String> {
    let Some(c) = conflicts::get(conn, id)? else {
        return Err(SyncError::Validation("That conflict is gone").into());
    };
    let mut choices = choices.iter();
    let mut out = String::new();
    for piece in textmerge::merge(&c.base, &c.b_text, &c.a_text).pieces {
        match piece {
            Piece::Agreed(text) => out.push_str(&text),
            Piece::Clash(k) => match choices.next().unwrap_or(&ClashChoice::Showing) {
                ClashChoice::Showing => out.push_str(&k.theirs),
                ClashChoice::Other => out.push_str(&k.ours),
                ClashChoice::Both => {
                    out.push_str(&k.theirs);
                    if !(k.theirs.is_empty()
                        || k.ours.is_empty()
                        || k.theirs.ends_with(char::is_whitespace)
                        || k.ours.starts_with(char::is_whitespace))
                    {
                        out.push(if k.theirs.contains('\n') || k.ours.contains('\n') {
                            '\n'
                        } else {
                            ' '
                        });
                    }
                    out.push_str(&k.ours);
                }
            },
        }
    }
    Ok(out)
}

/// What the app must write to settle a Conflict, or nothing for "Keep this".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictWrite {
    /// The change-log entity of the record.
    pub entity: String,
    /// Its id.
    pub entity_id: String,
    /// The text field.
    pub field: String,
    /// The text to write.
    pub text: String,
}

/// How the person settles a Conflict.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConflictChoice {
    /// Keep what shows.
    KeepThis,
    /// Replace with the other version.
    KeepOther,
    /// Replace with this text.
    Merged {
        /// The edited merge.
        text: String,
    },
}

/// What to write for `choice`, and how the Conflict will read afterwards. Errors if it is not
/// waiting any more (it was resolved on another device).
pub fn plan(
    conn: &Connection,
    id: &str,
    choice: &ConflictChoice,
) -> Result<(Option<ConflictWrite>, Resolution), SyncError> {
    let Some(c) = conflicts::get(conn, id)? else {
        return Err(SyncError::Validation("That conflict is gone"));
    };
    if !c.open {
        return Err(SyncError::Validation("That conflict is already resolved"));
    }
    let write = |text: String| ConflictWrite {
        entity: c.target.clone(),
        entity_id: c.target_id.clone(),
        field: c.field.clone(),
        text,
    };
    Ok(match choice {
        ConflictChoice::KeepThis => (None, Resolution::KeptThis),
        ConflictChoice::KeepOther => (Some(write(c.b_text.clone())), Resolution::KeptOther),
        ConflictChoice::Merged { text } => (Some(write(text.clone())), Resolution::Merged),
    })
}

/// Marks a Conflict resolved, as a change that reaches the other devices.
pub fn finish(
    conn: &Connection,
    id: &str,
    resolution: Resolution,
    me: &str,
    now: i64,
) -> Result<(), SyncError> {
    conflicts::resolve(conn, id, resolution, me, now)?;
    Ok(())
}
