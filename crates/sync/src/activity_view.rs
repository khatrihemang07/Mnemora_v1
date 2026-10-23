//! The Activity tab: a page of the log at a time, newest first, in words. Computed here so the
//! screen only draws: sentences are made from the row, from the viewer's side (a session logged
//! by the other device reads the other way round), and a session's breakdown is a line of counts
//! with no content.

use mnemora_core::error::AppResult;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::activity::{COLUMNS, Kind, Part, Row, Why, read};
use crate::peers;

/// Rows a page holds at most.
pub const MAX_PAGE: u32 = 200;

/// Whose events to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActivityFilter {
    /// Every device's.
    All,
    /// Those this device took part in.
    ThisDevice,
    /// Those the given device took part in.
    Device {
        /// Its id.
        id: String,
    },
}

/// A run of a sentence; `strong` ones are names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct Piece {
    /// The words.
    pub text: String,
    /// Shown bold (a device or List name).
    pub strong: bool,
}

/// Where a line can take the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ActivityLink {
    /// To the Conflicts tab.
    Conflicts,
}

/// One line of the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ActivityRow {
    /// Its id.
    pub id: String,
    /// What kind of event: picks the icon.
    pub kind: String,
    /// The sentence.
    pub text: Vec<Piece>,
    /// The device it happened on, when that is neither this one nor the one the sentence names:
    /// "· on Pixel 8".
    pub on: Option<String>,
    /// When: UTC milliseconds. Rows group by the local day of this.
    #[specta(type = u32)]
    pub at: i64,
    /// A session's per-Module breakdown ("Todo: 9 Tasks, 1 List · Journal: 2 Entries").
    pub breakdown: Option<String>,
    /// Where the line leads.
    pub link: Option<ActivityLink>,
}

/// A page of the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ActivityPage {
    /// Newest first.
    pub rows: Vec<ActivityRow>,
    /// Pass this back for the next (older) page; `None` at the end.
    pub next: Option<String>,
}

/// Who is looking.
#[derive(Debug, Clone, Copy)]
pub struct Viewer<'a> {
    /// This device's id.
    pub device: &'a str,
    /// Its name.
    pub name: &'a str,
}

fn piece(text: impl Into<String>, strong: bool) -> Piece {
    Piece {
        text: text.into(),
        strong,
    }
}

/// How a Module is named in a breakdown, and the order they come in.
fn module_label(module: &str) -> (u8, String) {
    match module {
        "todo" => (0, "Todo".into()),
        "journal" => (1, "Journal".into()),
        "core" => (2, "Settings".into()),
        "sync" => (3, "Sync".into()),
        other => {
            let mut c = other.chars();
            let name = c
                .next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default();
            (9, name)
        }
    }
}

/// The singular and plural noun of a kind of record, if it has one worth saying.
fn noun(entity: &str) -> Option<(&'static str, &'static str)> {
    Some(match entity {
        "todo_task" => ("Task", "Tasks"),
        "todo_list" => ("List", "Lists"),
        "todo_comment" => ("Comment", "Comments"),
        "journal_entry" => ("Entry", "Entries"),
        "sync_conflict" => ("Conflict", "Conflicts"),
        "sync_resolution" => ("automatic decision", "automatic decisions"),
        _ => return None,
    })
}

/// "Todo: 9 Tasks, 1 List · Journal: 2 Entries · Settings: 1". Counts only.
pub(crate) fn breakdown_text(parts: &[Part]) -> String {
    let mut modules: Vec<(u8, String, String)> = Vec::new();
    for module in parts.iter().map(|p| p.m.as_str()) {
        if modules.iter().any(|(_, _, m)| m == module) {
            continue;
        }
        let (rank, label) = module_label(module);
        modules.push((rank, label, module.to_owned()));
    }
    modules.sort();
    let mut out = Vec::new();
    for (_, label, module) in modules {
        // Kinds with no noun of their own add up ("Settings: 1").
        let bare: i64 = parts
            .iter()
            .filter(|p| p.m == module && p.n > 0 && noun(&p.e).is_none())
            .map(|p| p.n)
            .sum();
        let mut texts: Vec<String> = parts
            .iter()
            .filter(|p| p.m == module && p.n > 0)
            .filter_map(|p| {
                noun(&p.e)
                    .map(|(one, many)| format!("{} {}", p.n, if p.n == 1 { one } else { many }))
            })
            .collect();
        if bare > 0 {
            texts.push(bare.to_string());
        }
        if !texts.is_empty() {
            out.push(format!("{label}: {}", texts.join(", ")));
        }
    }
    out.join(" · ")
}

/// "a Task's notes": what a Conflict is about, in words.
fn conflict_subject(entity: &str, field: &str) -> String {
    let what = match entity {
        "todo_task" => "a Task’s",
        "todo_comment" => "a Comment’s",
        "journal_entry" => "an Entry’s",
        _ => "a record’s",
    };
    let part = match field {
        "notes" => "notes",
        "title" => "title",
        _ => "text",
    };
    format!("{what} {part}")
}

fn why_words(why: Option<Why>) -> &'static str {
    match why {
        Some(Why::Update) => "one of you needs an update",
        Some(Why::Clock) => "the clocks are too far apart",
        Some(Why::Environment) => "different Environments",
        Some(Why::Revoked) => "it no longer knows this device",
        Some(Why::Lost) | None => "the connection was lost",
    }
}

/// Whether the Conflict is still open on this device (a line without its id leads nowhere).
fn conflict_waits(conn: &Connection, id: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_conflicts WHERE id = ?1 AND status = 'open')",
        [id],
        |r| r.get(0),
    )
    .unwrap_or(false)
}

/// The row as the viewer reads it.
#[allow(clippy::too_many_lines)] // one sentence per kind: reads best in order
fn describe(conn: &Connection, v: Viewer<'_>, row: &Row) -> ActivityRow {
    let name_of = |id: &str, stored: &str| -> String {
        if id == v.device {
            return v.name.to_owned();
        }
        if let Ok(Some(n)) = peers::name_of(conn, id) {
            return n;
        }
        if stored.is_empty() {
            "another device".to_owned()
        } else {
            stored.to_owned()
        }
    };
    let kind = Kind::parse(&row.kind);
    let logger = name_of(&row.device, &row.device_name);
    let about_me = row.peer == v.device;
    // The device across the line from the viewer: the peer, or the logger when the peer is us.
    let other = if about_me {
        logger.clone()
    } else {
        name_of(&row.peer, &row.peer_name)
    };
    let why = row
        .extra
        .get("why")
        .and_then(|w| w.as_str())
        .and_then(Why::parse);
    let str_of = |k: &str| {
        row.extra
            .get(k)
            .and_then(|w| w.as_str())
            .unwrap_or_default()
            .to_owned()
    };
    let mut link = None;
    let mut breakdown = None;
    let text = match kind {
        Some(Kind::Session) => {
            let (n_in, n_out) = if about_me {
                (row.n_out, row.n_in)
            } else {
                (row.n_in, row.n_out)
            };
            breakdown = Some(breakdown_text(&row.detail)).filter(|b| !b.is_empty());
            vec![
                piece("Synced with ", false),
                piece(other, true),
                piece(format!(" · {n_in} in, {n_out} out"), false),
            ]
        }
        Some(Kind::Paired) => vec![piece("Paired with ", false), piece(other, true)],
        Some(Kind::Joined) => vec![
            piece(name_of(&row.peer, &row.peer_name), true),
            piece(" joined", false),
        ],
        Some(Kind::Left) => vec![piece("Left the group", false)],
        Some(Kind::Forgot) => vec![
            piece("Forgot ", false),
            piece(name_of(&row.peer, &row.peer_name), true),
        ],
        Some(Kind::Renamed) => vec![
            piece(str_of("old"), true),
            piece(" renamed to ", false),
            piece(str_of("new"), true),
        ],
        Some(Kind::Paused) => vec![
            piece("Paused with ", false),
            piece(other, true),
            piece(format!(": {}", why_words(why)), false),
        ],
        Some(Kind::Resumed) => vec![piece("Resumed with ", false), piece(other, true)],
        Some(Kind::Failed) => match why {
            Some(Why::Revoked | Why::Environment) => vec![
                piece(other, true),
                piece(format!(" refused: {}", why_words(why)), false),
            ],
            _ => vec![
                piece("Connection to ", false),
                piece(other, true),
                piece(" lost; retrying", false),
            ],
        },
        Some(Kind::ConflictRaised) => {
            // Only while the Conflict still waits: afterwards there is nothing to review.
            if conflict_waits(conn, &str_of("conflict")) {
                link = Some(ActivityLink::Conflicts);
            }
            vec![piece(
                format!(
                    "Conflict in {} needs you",
                    conflict_subject(&str_of("entity"), &str_of("field"))
                ),
                false,
            )]
        }
        Some(Kind::ConflictResolved) => vec![piece(
            format!(
                "Conflict in {} resolved",
                conflict_subject(&str_of("entity"), &str_of("field"))
            ),
            false,
        )],
        Some(Kind::ListsMerged) => {
            let mut t = vec![piece(str_of("from"), true)];
            if !row.peer.is_empty() {
                t.push(piece(" (", false));
                t.push(piece(name_of(&row.peer, &row.peer_name), true));
                t.push(piece(")", false));
            }
            t.push(piece(" merged into ", false));
            t.push(piece(str_of("into"), true));
            t
        }
        None => vec![piece("Something happened", false)],
    };
    // Said when someone else did it and the sentence does not already name them.
    let named_in_text = matches!(
        kind,
        Some(Kind::Renamed | Kind::Left | Kind::Joined | Kind::Forgot)
    ) && row.device != v.device;
    let others_event = row.device != v.device && !about_me && !named_in_text;
    let on = (others_event && !matches!(kind, Some(Kind::Renamed))).then_some(logger);
    ActivityRow {
        id: row.id.clone(),
        kind: row.kind.clone(),
        text,
        on,
        at: row.at,
        breakdown,
        link,
    }
}

/// `at:id`, where the next page starts after.
fn cursor_of(row: &Row) -> String {
    format!("{}:{}", row.at, row.id)
}

fn parse_cursor(cursor: &str) -> Option<(i64, String)> {
    let (at, id) = cursor.split_once(':')?;
    Some((at.parse().ok()?, id.to_owned()))
}

/// One page of the log, newest first, after `cursor` when given.
pub fn page(
    conn: &Connection,
    viewer: Viewer<'_>,
    filter: &ActivityFilter,
    cursor: Option<&str>,
    limit: u32,
) -> AppResult<ActivityPage> {
    let device = match filter {
        ActivityFilter::All => None,
        ActivityFilter::ThisDevice => Some(viewer.device),
        ActivityFilter::Device { id } => Some(id.as_str()),
    };
    let (after_at, after_id) = cursor.and_then(parse_cursor).unzip();
    let limit = limit.clamp(1, MAX_PAGE);
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM sync_activity
         WHERE (?1 IS NULL OR device = ?1 OR peer = ?1)
           AND (?2 IS NULL OR at < ?2 OR (at = ?2 AND id < ?3))
         ORDER BY at DESC, id DESC LIMIT ?4"
    ))?;
    let mut rows: Vec<Row> = stmt
        .query_map(
            params![device, after_at, after_id, i64::from(limit) + 1],
            read,
        )?
        .collect::<Result<_, _>>()?;
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next = more.then(|| rows.last().map(cursor_of)).flatten();
    Ok(ActivityPage {
        rows: rows.iter().map(|r| describe(conn, viewer, r)).collect(),
        next,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(m: &str, e: &str, n: i64) -> Part {
        Part {
            m: m.into(),
            e: e.into(),
            n,
        }
    }

    #[test]
    fn a_breakdown_reads_module_by_module_in_a_fixed_order() {
        let parts = vec![
            part("core", "setting", 1),
            part("journal", "journal_entry", 2),
            part("todo", "todo_task", 9),
            part("todo", "todo_list", 1),
        ];
        assert_eq!(
            breakdown_text(&parts),
            "Todo: 9 Tasks, 1 List · Journal: 2 Entries · Settings: 1"
        );
    }

    #[test]
    fn a_breakdown_is_empty_when_nothing_moved() {
        assert_eq!(breakdown_text(&[]), "");
        assert_eq!(breakdown_text(&[part("todo", "todo_task", 0)]), "");
    }

    #[test]
    fn a_cursor_round_trips() {
        assert_eq!(parse_cursor("17:abc-def"), Some((17, "abc-def".into())));
        assert_eq!(parse_cursor("nonsense"), None);
    }
}
