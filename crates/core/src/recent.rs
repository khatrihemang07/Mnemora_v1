//! Recent changes: the shared half of the change Tools (`journal_changes`, `todo_changes`; ADR 0018).
//! A Module finds the changes it made in a range of days and says what each reads as. This file
//! puts them in reading order, counts them by kind, pages them and keeps the result under the cap.
//! Days and times are local, in the zone the client sends.

use std::collections::HashSet;

use serde_json::{Map, Value, json};

use crate::hlc;
use crate::local_time;
use crate::tool::{Args, Item, Output, cut, fits, number};

/// Characters of a title or text shown as a preview.
pub const PREVIEW_CHARS: usize = 120;

/// One change a Module found, ready to read.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The Task or Entry it is about: the id a Source is cited by.
    pub id: String,
    /// When it was made: UTC milliseconds.
    pub at: i64,
    /// Its place in the log: the clock its record was stamped with, then the device that made it.
    pub clock: hlc::Timestamp,
    /// The device that made the record.
    pub device: String,
    /// The kind a `kinds` argument names: one word of the Tool's vocabulary.
    pub kind: &'static str,
    /// The line the person reads, such as "Done" or "Written, 42 words".
    pub change: String,
    /// A Task's title, or an Entry's preview.
    pub title: String,
    /// A Task's Due date or an Entry's Page date, when it has one.
    pub date: Option<String>,
}

/// A row with the local day and time of day it was made.
struct Placed {
    day: String,
    time: String,
    row: Row,
}

/// The first `chars` characters of `text`, its whitespace collapsed to single spaces.
pub fn preview(text: &str, chars: usize) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(chars)
        .collect()
}

/// The instants a `from` and `to` argument cover in `zone`: from the start of the first day to
/// the end of the last, in UTC milliseconds. A reversed range, or a day the zone cannot read, is a
/// refusal.
pub fn bounds(args: &Args, zone: &str) -> Result<(i64, i64), Output> {
    let from = args.text("from").unwrap_or_default();
    let to = args.text("to").unwrap_or_default();
    if from > to {
        return Err(Output::error("The start date is after the end date."));
    }
    let refused = || Output::error("The days could not be read in the device's time zone.");
    let start = local_time::day_start(from, zone).ok_or_else(refused)?;
    let end = local_time::day_end(to, zone).ok_or_else(refused)?;
    Ok((start, end))
}

/// The kinds a `kinds` argument names: comma-separated words, each one of `vocabulary`. `None`
/// when none was named, which means every kind. An unknown word is a refusal that lists them.
pub fn kind_filter(
    text: Option<&str>,
    vocabulary: &'static [&'static str],
) -> Result<Option<Vec<&'static str>>, Output> {
    let Some(text) = text else {
        return Ok(None);
    };
    let mut kinds: Vec<&'static str> = Vec::new();
    for word in text.split(',').map(str::trim).filter(|w| !w.is_empty()) {
        let Some(kind) = vocabulary.iter().find(|k| k.eq_ignore_ascii_case(word)) else {
            return Err(Output::error(&format!(
                "Unknown kind: {word}. Use: {}.",
                vocabulary.join(", ")
            )));
        };
        if !kinds.contains(kind) {
            kinds.push(kind);
        }
    }
    Ok((!kinds.is_empty()).then_some(kinds))
}

/// One page of changes. Days read newest first and the rows of a day oldest first (their log
/// order). `kinds` (all when `None`) narrow the rows; the counts cover the whole range, one per
/// word of `vocabulary`. `offset` and `limit` page the rows. A page is cut to fit the result cap,
/// and its `next_offset` continues from the last row it holds.
pub fn page(
    module: &'static str,
    rows: Vec<Row>,
    vocabulary: &'static [&'static str],
    kinds: Option<Vec<&'static str>>,
    offset: usize,
    limit: usize,
    zone: &str,
) -> Output {
    let mut placed: Vec<Placed> = rows
        .into_iter()
        .filter_map(|row| {
            Some(Placed {
                day: local_time::day(row.at, zone)?,
                time: local_time::clock(row.at, zone)?,
                row,
            })
        })
        .collect();
    placed.sort_by(|a, b| {
        b.day
            .cmp(&a.day)
            .then_with(|| (a.row.clock, &a.row.device).cmp(&(b.row.clock, &b.row.device)))
    });
    let counts: Map<String, Value> = vocabulary
        .iter()
        .map(|kind| {
            let n = placed.iter().filter(|p| p.row.kind == *kind).count();
            ((*kind).to_owned(), Value::from(number(n)))
        })
        .collect();
    let counts = Value::Object(counts);

    let matching: Vec<&Placed> = placed
        .iter()
        .filter(|p| kinds.as_ref().is_none_or(|k| k.contains(&p.row.kind)))
        .collect();
    let total = matching.len();
    let page: Vec<&Placed> = matching.into_iter().skip(offset).take(limit).collect();

    let mut shown = page.len();
    let body = loop {
        let next = (offset + shown < total).then_some(number(offset + shown));
        let body =
            json!({ "days": days_of(&page[..shown]), "counts": counts, "next_offset": next });
        if fits(&body) || shown == 0 {
            break body;
        }
        shown -= 1;
    };

    let mut seen = HashSet::new();
    let items: Vec<Item> = page[..shown]
        .iter()
        .filter(|p| seen.insert(p.row.id.as_str()))
        .map(|p| Item {
            id: p.row.id.clone(),
            module,
            title: cut(&p.row.title, PREVIEW_CHARS).to_owned(),
            date: p.row.date.clone(),
        })
        .collect();
    Output::ok(
        body,
        match shown {
            0 => "Listed changes, none".to_owned(),
            n => format!("Listed changes, {n} changes"),
        },
        items,
    )
}

/// The rows of a page grouped by local day, each day's rows in the order given.
fn days_of(page: &[&Placed]) -> Vec<Value> {
    let mut days: Vec<(&str, Vec<Value>)> = Vec::new();
    for p in page {
        let line = json!({
            "id": p.row.id,
            "at": p.time,
            "change": p.row.change,
            "title": p.row.title,
        });
        match days.last_mut() {
            Some((day, lines)) if *day == p.day.as_str() => lines.push(line),
            _ => days.push((p.day.as_str(), vec![line])),
        }
    }
    days.into_iter()
        .map(|(day, lines)| json!({ "date": day, "rows": lines }))
        .collect()
}
