//! The Journal's read-only Tools for the AI (ADR 0002 amendment, ADR 0018): list Entries by date,
//! read one in full, search with the app's own Search, and say what changed in a range of days.
//! They never write, skip deleted Entries and return refusals as data.

use mnemora_core::batch;
use mnemora_core::local_time;
use mnemora_core::recent;
use mnemora_core::search::Span;
use mnemora_core::tool::{
    Args, Calendar, Item, Kind, Output, Param, Spec, Tool, count, cut, fit_rows, number,
};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};

use crate::{MODULE_ID, changes};

/// Characters of an Entry's text shown in a list row or a title.
const PREVIEW_CHARS: usize = 120;
/// Characters of a search snippet.
const SNIPPET_CHARS: usize = 200;
/// Bytes of an Entry's body one read batch returns; the next offset continues it.
const READ_BATCH_BYTES: usize = 8 * 1024;
/// Rows a list returns by default, and at most.
const LIST_DEFAULT: usize = 30;
const LIST_MAX: i64 = 100;
/// Rows a search returns by default, and at most.
const SEARCH_DEFAULT: usize = 10;
const SEARCH_MAX: i64 = 25;

const LIST_PARAMS: &[Param] = &[
    Param {
        name: "from",
        kind: Kind::Date,
        required: false,
        about: "First Page date to include, YYYY-MM-DD.",
    },
    Param {
        name: "to",
        kind: Kind::Date,
        required: false,
        about: "Last Page date to include, YYYY-MM-DD.",
    },
    Param {
        name: "offset",
        kind: Kind::Integer {
            min: 0,
            max: 1_000_000,
        },
        required: false,
        about: "Rows to skip: the next_offset of the batch before, 0 for the first.",
    },
    Param {
        name: "limit",
        kind: Kind::Integer {
            min: 1,
            max: LIST_MAX,
        },
        required: false,
        about: "Rows in one batch, 30 by default.",
    },
];

const READ_PARAMS: &[Param] = &[
    Param {
        name: "id",
        kind: Kind::Text { max_chars: 64 },
        required: true,
        about: "The Entry's id, as a list or search returned it.",
    },
    Param {
        name: "offset",
        kind: Kind::Integer {
            min: 0,
            max: 10_000_000,
        },
        required: false,
        about: "Bytes of the text to skip: the next_offset of the batch before, 0 for the first.",
    },
];

const SEARCH_PARAMS: &[Param] = &[
    Param {
        name: "query",
        kind: Kind::Text { max_chars: 200 },
        required: true,
        about: "Words to find. Every word must match; quotes make a phrase.",
    },
    Param {
        name: "from",
        kind: Kind::Date,
        required: false,
        about: "First Page date to include, YYYY-MM-DD.",
    },
    Param {
        name: "to",
        kind: Kind::Date,
        required: false,
        about: "Last Page date to include, YYYY-MM-DD.",
    },
    Param {
        name: "offset",
        kind: Kind::Integer {
            min: 0,
            max: 1_000_000,
        },
        required: false,
        about: "Results to skip: the next_offset of the batch before, 0 for the first.",
    },
    Param {
        name: "limit",
        kind: Kind::Integer {
            min: 1,
            max: SEARCH_MAX,
        },
        required: false,
        about: "Results in one batch, 10 by default.",
    },
];

/// The changes Tool's parameters: the shared range, paging and limit (`recent`), and the kinds this
/// Module names.
const CHANGES_PARAMS: &[Param] = &[
    recent::FROM,
    recent::TO,
    Param {
        name: "kinds",
        kind: Kind::Text { max_chars: 60 },
        required: false,
        about: "Only these kinds, comma-separated: written, edited, deleted. Counts still cover the whole range.",
    },
    recent::OFFSET,
    recent::LIMIT,
];

/// The Tools the Journal offers.
pub(crate) fn all() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(List),
        Box::new(Read),
        Box::new(Search),
        Box::new(Changes),
    ]
}

fn plain(spans: &[Span]) -> String {
    let text: String = spans.iter().map(|span| span.text.as_str()).collect();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A live Entry's row: id, Page date, written instant, word count and body.
struct Row {
    id: String,
    page_date: String,
    written_at: i64,
    word_count: i64,
    body: String,
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    Ok(Row {
        id: r.get(0)?,
        page_date: r.get(1)?,
        written_at: r.get(2)?,
        word_count: r.get(3)?,
        body: r.get(4)?,
    })
}

fn item(row: &Row) -> Item {
    Item {
        id: row.id.clone(),
        module: MODULE_ID,
        title: recent::preview(&row.body, PREVIEW_CHARS),
        date: Some(row.page_date.clone()),
    }
}

fn list_json(row: &Row, zone: &str) -> Value {
    json!({
        "id": row.id,
        "page_date": row.page_date,
        "written_at": row.written_at,
        "written_at_local": local_time::minute(row.written_at, zone),
        "word_count": row.word_count,
        "preview": recent::preview(&row.body, PREVIEW_CHARS),
    })
}

fn unreadable() -> Output {
    Output::error("The Journal could not be read just now.")
}

struct List;

impl Tool for List {
    fn spec(&self) -> Spec {
        Spec {
            name: "journal_list",
            about: "List Journal Entries by Page date, newest first. Timestamps are epoch milliseconds (UTC); each has a _local twin, YYYY-MM-DD HH:MM in the device's time zone. Each batch has shown (its rows) and total (the rows the query matches). If next_offset is not null the list is incomplete: fetch the next batch with that offset before summarising.",
            params: LIST_PARAMS,
        }
    }

    fn run(&self, conn: &Connection, args: &Args, calendar: &Calendar) -> Output {
        let from = args.text("from").unwrap_or("0000-01-01");
        let to = args.text("to").unwrap_or("9999-12-31");
        if from > to {
            return Output::error("The start date is after the end date.");
        }
        let offset = count(args.int("offset"), 0);
        let limit = count(args.int("limit"), LIST_DEFAULT);
        // The same range counts every live Entry, so the batch can say how many there are.
        let counted = conn.query_row(
            "SELECT COUNT(*) FROM journal_entries WHERE deleted_at IS NULL AND page_date BETWEEN ?1 AND ?2",
            rusqlite::params![from, to],
            |r| r.get::<_, i64>(0),
        );
        let Ok(total) = counted else {
            return unreadable();
        };
        let total = count(Some(total), 0);
        let fetched = conn
            .prepare(
                "SELECT id, page_date, written_at, word_count, body FROM journal_entries
                 WHERE deleted_at IS NULL AND page_date BETWEEN ?1 AND ?2
                 ORDER BY page_date DESC, written_at DESC LIMIT ?3 OFFSET ?4",
            )
            .and_then(|mut stmt| {
                stmt.query_map(
                    rusqlite::params![from, to, number(limit + 1), number(offset)],
                    row,
                )?
                .collect::<rusqlite::Result<Vec<Row>>>()
            });
        let Ok(mut rows) = fetched else {
            return unreadable();
        };
        let more = rows.len() > limit;
        rows.truncate(limit);
        let zone = calendar.time_zone.as_str();
        let (kept, cut_short) = fit_rows(rows.iter().map(|row| list_json(row, zone)).collect());
        let shown = kept.len();
        let next = (more || cut_short).then_some(offset + shown);
        let items = rows.iter().take(shown).map(item).collect();
        let mut body = json!({ "entries": kept, "next_offset": next });
        if let Some(fields) = body.as_object_mut() {
            fields.extend(batch::fields(shown, total, next, "entries", None));
        }
        Output::ok(
            body,
            match shown {
                0 => "Listed entries, none".to_owned(),
                n => format!("Listed entries, {n} entries"),
            },
            items,
        )
    }
}

struct Read;

impl Tool for Read {
    fn spec(&self) -> Spec {
        Spec {
            name: "journal_read",
            about: "Read one Journal Entry in full, a batch of its text at a time. Each batch has shown and total (bytes). If next_offset is not null the text is incomplete: fetch the next batch with that offset before summarising. Timestamps are epoch milliseconds (UTC); each has a _local twin, YYYY-MM-DD HH:MM in the device's time zone.",
            params: READ_PARAMS,
        }
    }

    fn run(&self, conn: &Connection, args: &Args, calendar: &Calendar) -> Output {
        let id = args.text("id").unwrap_or_default();
        let offset = count(args.int("offset"), 0);
        let found = conn
            .query_row(
                "SELECT id, page_date, written_at, word_count, body FROM journal_entries
                 WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                row,
            )
            .optional();
        let row = match found {
            Ok(Some(row)) => row,
            Ok(None) => return Output::error("No Entry has that id."),
            Err(_) => return unreadable(),
        };
        if offset > row.body.len() {
            return Output::error("The offset is past the end of the Entry.");
        }
        let mut start = offset;
        while !row.body.is_char_boundary(start) {
            start += 1;
        }
        let rest = &row.body[start..];
        let page = cut(rest, READ_BATCH_BYTES);
        let next = (page.len() < rest.len()).then_some(start + page.len());
        let summary = format!("Read an entry, {}", row.page_date);
        let mut body = json!({
            "id": row.id,
            "page_date": row.page_date,
            "written_at": row.written_at,
            "written_at_local": local_time::minute(row.written_at, &calendar.time_zone),
            "word_count": row.word_count,
            "body": page,
            "next_offset": next,
        });
        if let Some(fields) = body.as_object_mut() {
            fields.extend(batch::byte_fields(page.len(), row.body.len(), next));
        }
        Output::ok(body, summary, vec![item(&row)])
    }
}

struct Search;

impl Tool for Search {
    fn spec(&self) -> Spec {
        Spec {
            name: "journal_search",
            about: "Find Journal Entries with the app's Search, exact matches first. Each batch has shown (its rows) and total (the rows the query matches). If next_offset is not null the list is incomplete: fetch the next batch with that offset before summarising.",
            params: SEARCH_PARAMS,
        }
    }

    fn run(&self, conn: &Connection, args: &Args, _calendar: &Calendar) -> Output {
        let query = args.text("query").unwrap_or_default();
        let from = args.text("from");
        let to = args.text("to");
        let offset = count(args.int("offset"), 0);
        let limit = count(args.int("limit"), SEARCH_DEFAULT);
        let Ok(results) = crate::search(conn, query, from, to) else {
            return Output::error("Search could not run. Check the words and dates.");
        };
        let all: Vec<(&str, &crate::Hit)> = results
            .exact
            .iter()
            .map(|hit| ("exact", hit))
            .chain(results.similar.iter().map(|hit| ("similar", hit)))
            .collect();
        let total = all.len();
        let page: Vec<&(&str, &crate::Hit)> = all.iter().skip(offset).take(limit).collect();
        let rows: Vec<Value> = page
            .iter()
            .map(|(kind, hit)| {
                json!({
                    "id": hit.id,
                    "page_date": hit.page_date,
                    "match": kind,
                    "snippet": cut(&plain(&hit.snippet), SNIPPET_CHARS),
                })
            })
            .collect();
        let (kept, _) = fit_rows(rows);
        let shown = kept.len();
        let items = page
            .iter()
            .take(shown)
            .map(|(_, hit)| Item {
                id: hit.id.clone(),
                module: MODULE_ID,
                title: cut(&plain(&hit.snippet), SNIPPET_CHARS).to_owned(),
                date: Some(hit.page_date.clone()),
            })
            .collect();
        let next = (offset + shown < total).then_some(offset + shown);
        let mut body = json!({ "results": kept, "next_offset": next });
        if let Some(fields) = body.as_object_mut() {
            fields.extend(batch::fields(shown, total, next, "matches", None));
        }
        Output::ok(body, format!("Searched entries, {total} found"), items)
    }
}

struct Changes;

impl Tool for Changes {
    fn spec(&self) -> Spec {
        Spec {
            name: "journal_changes",
            about: "Entries written, edited or deleted in a date range. Use for questions about a period. Days are local, in the device's time zone: rows are grouped by day, newest day first, and each day's rows oldest first. Each row has id, at (HH:MM, local time), change and title (the Entry's preview; a deleted Entry keeps its preview). A change counts on the day it was made, not on the Entry's Page date. Counts cover the whole range by kind. Each batch has shown (its rows) and total (the rows the query matches). If next_offset is not null the list is incomplete: fetch the next batch with that offset before summarising.",
            params: CHANGES_PARAMS,
        }
    }

    fn run(&self, conn: &Connection, args: &Args, calendar: &Calendar) -> Output {
        recent::run(args, calendar, MODULE_ID, changes::KINDS, |start, end| {
            changes::rows(conn, start, end).map_err(|_| unreadable())
        })
    }
}
