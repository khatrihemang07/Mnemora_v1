//! Batches: the fields every paged Tool result carries, so the Model knows whether it has seen all of
//! it. A Batch is one part of a Tool's result when there is more than fits in one answer; the Model
//! asks for the next one with the offset it was given. This file only says how much a batch holds
//! and where the next one starts. Each Tool decides what a row is and what it pages over.

use serde_json::{Map, Value};

use crate::tool::number;

/// The fields a paged list or search carries. `shown` is the items in this batch and `total` the
/// items the query matches. `note` is there only when `next_offset` is `Some`: it says in plain words
/// that the list is incomplete and where the next batch starts. `noun` is the plural the note names,
/// such as "changes"; `scope`, when given, goes before it.
pub fn fields(
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
    counted(shown, total, next_offset, &noun, "list")
}

/// The fields a read cut into byte parts carries, such as a Task's notes or an Entry's body. `shown`
/// is the bytes in this part and `total` the bytes of the whole text. `note` is there only when
/// `next_offset` is `Some`: the text is incomplete, and the offset is a byte offset.
pub fn byte_fields(shown: usize, total: usize, next_offset: Option<usize>) -> Map<String, Value> {
    counted(shown, total, next_offset, "bytes", "text")
}

/// `shown`, `total` and, when there is a next offset, the note "{shown} of {total} {unit} shown. The
/// {what} is incomplete: call again with offset {next} for the rest."
fn counted(
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
        let note = format!(
            "{shown} of {total} {unit} shown. The {what} is incomplete: call again with offset {next} for the rest."
        );
        fields.insert("note".to_owned(), Value::from(note));
    }
    fields
}
