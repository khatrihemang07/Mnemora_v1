//! Batches: the fields every paged Tool result carries, so the Model knows whether it has seen all of
//! it. A Batch is one part of a Tool's result when there is more than fits in one answer; the Model
//! asks for the next one with the offset it was given. This file only says how much a batch holds
//! and where the next one starts. Each Tool decides what a row is and what it pages over.

use serde_json::{Map, Value, json};

use crate::tool::{fit_rows, fits, number};

/// What a paged Tool returns when its first row alone is larger than the result cap. No row is
/// ever this large on the Modules' current limits (titles, previews and snippets are capped), so the
/// message is a guard, not a path a Model reaches in practice.
pub const ROW_TOO_LARGE: &str = "The result is too large to read. Narrow the request.";

/// One batch of a paged list or search: its rows under `key`, `next_offset` and the batch fields, as
/// the Model reads them. `rows` are the rows the query returns from `offset` on, at most the Tool's
/// limit, and `total` the rows it matches. The batch holds the most leading rows whose whole body fits
/// under the result cap, so no field is cut and no row is lost: the next batch starts where this one
/// ends. Returns the body and the rows it holds, or `None` when the first row alone is over the cap.
pub fn page(
    key: &str,
    rows: &[Value],
    offset: usize,
    total: usize,
    noun: &str,
) -> Option<(Value, usize)> {
    let body = |shown: usize| {
        let next = (offset + shown < total).then_some(offset + shown);
        let mut body = Map::new();
        body.insert(key.to_owned(), Value::Array(rows[..shown].to_vec()));
        body.insert("next_offset".to_owned(), json!(next));
        body.extend(fields(offset, shown, total, next, noun, None));
        Value::Object(body)
    };
    // `fit_rows` keeps the rows alone under the cap, so it is an upper bound; the body's own fields
    // take room too, so step down until the whole body fits.
    let mut shown = fit_rows(rows.to_vec()).0.len();
    while shown > 0 && !fits(&body(shown)) {
        shown -= 1;
    }
    if shown == 0 && !rows.is_empty() {
        return None;
    }
    Some((body(shown), shown))
}

/// The fields a paged list or search carries. `offset` is where this batch starts, `shown` the items
/// in this batch and `total` the items the query matches. `note` is there only when `next_offset` is
/// `Some`: it says in plain words that the list is incomplete and where the next batch starts. The
/// note counts from the start of the list, so a later batch says how far the list has got. `noun` is
/// the plural the note names, such as "changes"; `scope`, when given, goes before it.
pub fn fields(
    offset: usize,
    shown: usize,
    total: usize,
    next_offset: Option<usize>,
    noun: &str,
    scope: Option<&str>,
) -> Map<String, Value> {
    let noun = match scope {
        Some(scope) => format!("{scope} {noun}"),
        None => noun.to_owned(),
    };
    counted(offset, shown, total, next_offset, &noun, "list")
}

/// The fields a read cut into byte parts carries, such as a Task's notes or an Entry's body. `offset`
/// is the byte where this part starts, `shown` the bytes in this part and `total` the bytes of the
/// whole text. `note` is there only when `next_offset` is `Some`: the text is incomplete, and the
/// offset is a byte offset.
pub fn byte_fields(
    offset: usize,
    shown: usize,
    total: usize,
    next_offset: Option<usize>,
) -> Map<String, Value> {
    counted(offset, shown, total, next_offset, "bytes", "text")
}

/// `shown`, `total` and, when there is a next offset, the note "{done} of {total} {unit} shown. The
/// {what} is incomplete: call again with offset {next} for the rest.", where {done} is `offset` plus
/// `shown`: the count from the start, which is `shown` alone for the first batch.
fn counted(
    offset: usize,
    shown: usize,
    total: usize,
    next_offset: Option<usize>,
    unit: &str,
    what: &str,
) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("shown".to_owned(), Value::from(number(shown)));
    fields.insert("total".to_owned(), Value::from(number(total)));
    if let Some(next) = next_offset {
        let done = offset + shown;
        let note = format!(
            "{done} of {total} {unit} shown. The {what} is incomplete: call again with offset {next} for the rest."
        );
        fields.insert("note".to_owned(), Value::from(note));
    }
    fields
}
