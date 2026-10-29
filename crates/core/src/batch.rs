//! Batches: the fields every paged Tool result carries, so the Model knows whether it has seen all of
//! it. A Batch is one part of a Tool's result when there is more than fits in one answer; the Model
//! asks for the next one with the offset it was given. This file only says how much a batch holds,
//! where the next one starts and how a body is cut to fit the cap. Each Tool decides what a row is and
//! what it pages over.

use serde_json::{Map, Value, json};

use crate::tool::{fit_rows, fits, number};

/// Bytes of a text one read batch returns (a Task's notes, an Entry's body); the next offset continues it.
pub const READ_BATCH_BYTES: usize = 8 * 1024;

/// What a paged Tool returns when its first row alone is larger than the result cap. Titles, list
/// names and snippets are capped, so no row reaches the cap in practice; the message is the refusal
/// a Model reads if one ever does, rather than an empty batch it would ask for again.
pub const ROW_TOO_LARGE: &str = "The result is too large to read. Narrow the request.";

/// One batch of a paged result: where it starts, how many rows or bytes it holds, how many the
/// query matches, and where the next batch starts when more follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Batch {
    /// Where the batch starts: the rows or bytes skipped before it.
    pub offset: usize,
    /// How many rows or bytes the batch holds.
    pub shown: usize,
    /// How many rows or bytes the query matches, in all.
    pub total: usize,
    /// Where the next batch starts, when more follows.
    pub next_offset: Option<usize>,
}

impl Batch {
    /// The batch of `shown` items that starts at `offset` in a result of `total` items.
    pub fn new(offset: usize, shown: usize, total: usize) -> Self {
        let next_offset = (offset + shown < total).then_some(offset + shown);
        Self {
            offset,
            shown,
            total,
            next_offset,
        }
    }

    /// The fields of a list or search: `next_offset`, `shown` and `total`. When more follows, also
    /// `note`, which counts from the start of the list, and `noun` and `scope` for the note on unread
    /// batches (`ai`'s unresolved note). `scope` is the Module the rows belong to, when there is one.
    pub fn list_fields(&self, noun: &str, scope: Option<&str>) -> Map<String, Value> {
        self.fields(noun, "list", noun, scope)
    }

    /// The fields of a read cut into byte parts, such as a Task's notes or an Entry's body. `noun` is
    /// what the unread note says was read, such as "bytes of the Entry". The byte offsets count from
    /// the start of the text, as the list note counts from the start of the list.
    pub fn byte_fields(&self, noun: &str) -> Map<String, Value> {
        self.fields("bytes", "text", noun, None)
    }

    /// The shared fields. `unit` and `what` word the note; `noun` and `scope` are the unread note's.
    fn fields(
        &self,
        unit: &str,
        what: &str,
        noun: &str,
        scope: Option<&str>,
    ) -> Map<String, Value> {
        let mut fields = Map::new();
        fields.insert("next_offset".to_owned(), json!(self.next_offset));
        fields.insert("shown".to_owned(), Value::from(number(self.shown)));
        fields.insert("total".to_owned(), Value::from(number(self.total)));
        if let Some(next) = self.next_offset {
            let done = self.offset + self.shown;
            let total = self.total;
            let note = format!(
                "{done} of {total} {unit} shown. The {what} is incomplete: call again with offset {next} for the rest."
            );
            fields.insert("note".to_owned(), Value::from(note));
            fields.insert("noun".to_owned(), Value::from(noun));
            if let Some(scope) = scope {
                fields.insert("scope".to_owned(), Value::from(scope));
            }
        }
        fields
    }
}

/// The body of a paged list or search: its rows under `key` and the list fields of the batch. `rows`
/// are the rows the query returns from `offset` on, at most the Tool's limit, and `total` the rows it
/// matches. The batch holds the most leading rows whose whole body fits under the result cap, so no
/// field is cut and no row is lost: the next batch starts where this one ends. Returns the body and
/// the rows it holds, or `None` when the first row alone is over the cap.
pub fn page(
    key: &str,
    rows: &[Value],
    offset: usize,
    total: usize,
    noun: &str,
    scope: Option<&str>,
) -> Option<(Value, usize)> {
    // `fit_rows` keeps the rows alone under the cap, an upper bound on what the body can hold.
    let upper = fit_rows(rows.to_vec()).0.len();
    let (body, shown) = fit(upper, |shown| {
        let mut body = Map::new();
        body.insert(key.to_owned(), Value::Array(rows[..shown].to_vec()));
        body.extend(Batch::new(offset, shown, total).list_fields(noun, scope));
        Value::Object(body)
    });
    (shown > 0 || rows.is_empty()).then_some((body, shown))
}

/// The body that holds the most leading rows, from `upper` down, whose whole body fits under the
/// result cap: `body` builds the body that shows that many rows. The body's own fields take room
/// too, so the count steps down until the whole body fits. Returns the body and its count; zero when
/// even one row does not fit.
pub fn fit(upper: usize, body: impl Fn(usize) -> Value) -> (Value, usize) {
    let mut shown = upper;
    loop {
        let built = body(shown);
        if shown == 0 || fits(&built) {
            return (built, shown);
        }
        shown -= 1;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{Batch, page};

    #[test]
    fn a_first_batch_says_how_many_of_how_many_and_where_the_next_starts() {
        let batch = Batch::new(0, 100, 130);
        assert_eq!(batch.next_offset, Some(100));
        let fields = batch.list_fields("changes", None);
        assert_eq!(fields["next_offset"], 100);
        assert_eq!(fields["shown"], 100);
        assert_eq!(fields["total"], 130);
        assert_eq!(
            fields["note"],
            "100 of 130 changes shown. The list is incomplete: call again with offset 100 for the rest."
        );
        assert_eq!(fields["noun"], "changes");
        assert!(fields.get("scope").is_none());
    }

    #[test]
    fn a_later_batch_counts_from_the_start_of_the_list() {
        let batch = Batch::new(30, 30, 130);
        assert_eq!(batch.next_offset, Some(60));
        assert_eq!(
            batch.list_fields("changes", None)["note"],
            "60 of 130 changes shown. The list is incomplete: call again with offset 60 for the rest."
        );
    }

    #[test]
    fn a_complete_list_has_null_next_offset_and_no_note_noun_or_scope() {
        let batch = Batch::new(100, 30, 130);
        assert_eq!(batch.next_offset, None);
        let fields = batch.list_fields("changes", Some("Journal"));
        assert_eq!(
            fields.keys().map(String::as_str).collect::<Vec<_>>(),
            ["next_offset", "shown", "total"]
        );
        assert!(fields["next_offset"].is_null());
        assert_eq!(fields["shown"], 30);
        assert_eq!(fields["total"], 130);
    }

    #[test]
    fn an_incomplete_list_names_its_scope_for_the_unread_note() {
        let fields = Batch::new(0, 100, 130).list_fields("changes", Some("Journal"));
        assert_eq!(fields["noun"], "changes");
        assert_eq!(fields["scope"], "Journal");
    }

    #[test]
    fn a_byte_part_says_how_many_bytes_of_how_many_and_its_unread_noun() {
        let fields = Batch::new(0, 8192, 13_192).byte_fields("bytes of the Entry");
        assert_eq!(fields["next_offset"], 8192);
        assert_eq!(fields["shown"], 8192);
        assert_eq!(fields["total"], 13_192);
        assert_eq!(
            fields["note"],
            "8192 of 13192 bytes shown. The text is incomplete: call again with offset 8192 for the rest."
        );
        assert_eq!(fields["noun"], "bytes of the Entry");
        assert!(fields.get("scope").is_none());
    }

    #[test]
    fn a_later_byte_part_counts_from_the_start_of_the_text() {
        let fields = Batch::new(8192, 8192, 20_000).byte_fields("bytes of the Entry");
        assert_eq!(
            fields["note"],
            "16384 of 20000 bytes shown. The text is incomplete: call again with offset 16384 for the rest."
        );
    }

    #[test]
    fn a_complete_byte_part_has_no_note() {
        let fields = Batch::new(8192, 5_000, 13_192).byte_fields("bytes of the Entry");
        assert!(fields["next_offset"].is_null());
        assert!(fields.get("note").is_none());
        assert!(fields.get("noun").is_none());
    }

    /// A row whose text is `pad` bytes, so its JSON is `pad + 11` bytes and a comma more in a list.
    fn row(pad: usize) -> Value {
        json!({ "pad": "x".repeat(pad) })
    }

    #[test]
    fn a_page_drops_rows_until_the_whole_body_fits_the_cap() {
        // Nineteen rows of 832 bytes with their commas: the rows alone (15,810 bytes) fit the 15,870
        // the rows may take, but the body's own fields do not, so the batch holds eighteen.
        let rows: Vec<Value> = (0..19).map(|_| row(821)).collect();
        let (body, shown) = page("tasks", &rows, 0, 25, "tasks", None).unwrap();
        assert_eq!(shown, 18);
        assert_eq!(body["tasks"].as_array().unwrap().len(), 18);
        assert_eq!(body["shown"], 18);
        assert_eq!(body["next_offset"], 18);
        assert_eq!(
            body["note"],
            "18 of 25 tasks shown. The list is incomplete: call again with offset 18 for the rest."
        );
    }

    #[test]
    fn a_page_of_rows_that_all_fit_keeps_them_all() {
        let rows: Vec<Value> = (0..3).map(|_| row(10)).collect();
        let (body, shown) = page("tasks", &rows, 0, 3, "tasks", None).unwrap();
        assert_eq!(shown, 3);
        assert!(body["next_offset"].is_null());
        assert!(body.get("note").is_none());
    }

    #[test]
    fn a_page_whose_first_row_is_over_the_cap_is_refused() {
        // Not reachable with the Modules' limits, where a row is at most a few kilobytes; kept so
        // that an oversized row is a refusal the Model reads, not an empty batch it asks for again.
        let rows = vec![row(16 * 1024)];
        assert_eq!(page("tasks", &rows, 0, 1, "tasks", None), None);
    }

    #[test]
    fn an_empty_page_is_a_complete_empty_batch() {
        let (body, shown) = page("tasks", &[], 0, 0, "tasks", None).unwrap();
        assert_eq!(shown, 0);
        assert_eq!(body["shown"], 0);
        assert_eq!(body["total"], 0);
        assert!(body["next_offset"].is_null());
    }
}
