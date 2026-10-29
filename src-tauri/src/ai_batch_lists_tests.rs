//! Command-layer tests for the batches of the list, search and read Tools (2026-10-29 follow-up,
//! decision 4; ADR 0009). A list or search that is cut off says how many rows it shows and how many
//! match, and its note names the offset of the next batch; a read cut into 8 KiB parts says the same
//! in bytes. A scripted native-tool Provider asks the Tools through the commands. No network.

use mnemora_core::batch::READ_BATCH_BYTES;
use mnemora_harness::scripted::Reply;
use serde_json::{Value, json};

use crate::ai_tools_tests::{seed_entry, seed_task, tool_results_in};
use crate::test_support::TestApp;

/// The device's zone: London in October is UTC+1.
const ZONE: &str = "Europe/London";

/// What a paged result counts, so how `walk` checks it: rows under `key`, which the note calls
/// `noun`, or the bytes of a text under `key`.
#[derive(Clone, Copy)]
enum Paged {
    Rows {
        key: &'static str,
        noun: &'static str,
    },
    Bytes {
        key: &'static str,
    },
}

const TODO_LIST: Paged = Paged::Rows {
    key: "tasks",
    noun: "tasks",
};
const TODO_SEARCH: Paged = Paged::Rows {
    key: "tasks",
    noun: "matches",
};
const JOURNAL_LIST: Paged = Paged::Rows {
    key: "entries",
    noun: "entries",
};
const JOURNAL_SEARCH: Paged = Paged::Rows {
    key: "results",
    noun: "matches",
};
const TODO_NOTES: Paged = Paged::Bytes { key: "notes" };
const JOURNAL_BODY: Paged = Paged::Bytes { key: "body" };

impl Paged {
    /// The note an incomplete batch carries, once `done` of `total` are shown and the next starts at `next`.
    fn note(self, done: usize, total: usize, next: usize) -> String {
        match self {
            Self::Rows { noun, .. } => format!(
                "{done} of {total} {noun} shown. The list is incomplete: call again with offset {next} for the rest."
            ),
            Self::Bytes { .. } => format!(
                "{done} of {total} bytes shown. The text is incomplete: call again with offset {next} for the rest."
            ),
        }
    }
}

/// The body of a Tool result: the JSON after the observation line.
fn body(result: &str) -> Value {
    let json = result.find('{').map_or("", |at| &result[at..]);
    serde_json::from_str(json).unwrap_or_else(|_| panic!("not a JSON body: {result}"))
}

/// The ids of a body's rows under `key` ("tasks", "entries" or "results"), in the order given.
fn ids_under(body: &Value, key: &str) -> Vec<String> {
    body[key]
        .as_array()
        .unwrap_or_else(|| panic!("no {key} in {body}"))
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_owned())
        .collect()
}

/// Sorted ids, so two batches are compared without relying on the order a Tool gives them.
fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

/// Runs `calls` in one Step on a scripted Provider and returns each Tool result, in order.
fn results_of(app: &TestApp, calls: &[(&str, &str)]) -> Vec<String> {
    tool_results_in(
        app,
        ZONE,
        vec![Reply::calls(calls), Reply::text(&["Done."])],
    )
}

/// A text of `len` bytes, all ASCII, so a byte offset is also a character offset.
fn long_text(len: usize) -> String {
    "abcdefghijklmnopqrstuvwxyz"
        .chars()
        .cycle()
        .take(len)
        .collect()
}

/// Creates a Task whose notes are `notes`, and returns its id.
fn task_with_notes(app: &TestApp, title: &str, notes: &str) -> String {
    let task: Value = app
        .invoke(
            "todo_task_create",
            json!({ "title": title, "list_id": null, "when": null, "details": { "notes": notes } }),
        )
        .unwrap();
    task["id"].as_str().unwrap().to_owned()
}

/// Checks the batches of one paged Tool, in the order the calls ran. `batches` gives each call's
/// (offset, shown) and `total` what the query matches. A batch that is followed by another says so
/// in its note and `next_offset`, and the next batch starts exactly where it ends; the last batch
/// reaches `total` and has neither. Returns the ids of every row, in order (a read has none).
fn walk(results: &[String], what: Paged, total: usize, batches: &[(usize, usize)]) -> Vec<String> {
    assert_eq!(results.len(), batches.len(), "one result per call");
    let mut ids = Vec::new();
    for (i, (result, &(offset, shown))) in results.iter().zip(batches).enumerate() {
        let whole = body(result);
        assert_eq!(whole["shown"], shown, "{result}");
        assert_eq!(whole["total"], total, "{result}");
        match what {
            Paged::Rows { key, .. } => {
                let rows = ids_under(&whole, key);
                assert_eq!(rows.len(), shown, "{result}");
                ids.extend(rows);
            }
            Paged::Bytes { key } => {
                let text = whole[key]
                    .as_str()
                    .unwrap_or_else(|| panic!("no {key} in {result}"));
                assert_eq!(text.len(), shown, "{result}");
            }
        }
        if let Some(&(next, _)) = batches.get(i + 1) {
            assert_eq!(next, offset + shown, "{result}");
            assert_eq!(whole["next_offset"], next, "{result}");
            assert_eq!(
                whole["note"],
                what.note(offset + shown, total, next),
                "{result}"
            );
        } else {
            assert_eq!(offset + shown, total, "{result}");
            assert!(whole["next_offset"].is_null(), "{result}");
            assert!(whole.get("note").is_none(), "{result}");
        }
    }
    ids
}

#[test]
fn todo_list_cut_at_30_says_so_and_offset_30_holds_the_rest() {
    let app = TestApp::new();
    let seeded: Vec<String> = (0..35)
        .map(|i| seed_task(&app, &format!("Task {i:02}"), None))
        .collect();

    let results = results_of(
        &app,
        &[("todo_list", "{}"), ("todo_list", r#"{"offset":30}"#)],
    );

    assert_eq!(
        body(&results[0])["note"],
        "30 of 35 tasks shown. The list is incomplete: call again with offset 30 for the rest.",
        "{}",
        results[0]
    );
    let ids = walk(&results, TODO_LIST, 35, &[(0, 30), (30, 5)]);
    assert_eq!(
        sorted(ids),
        sorted(seeded),
        "every Task once across the batches"
    );
}

#[test]
fn todo_search_cut_at_10_says_so_and_offset_10_holds_the_rest() {
    let app = TestApp::new();
    let seeded: Vec<String> = (0..12)
        .map(|i| seed_task(&app, &format!("Renew passport part {i:02}"), None))
        .collect();

    let results = results_of(
        &app,
        &[
            ("todo_search", r#"{"query":"passport"}"#),
            ("todo_search", r#"{"query":"passport","offset":10}"#),
        ],
    );

    let ids = walk(&results, TODO_SEARCH, 12, &[(0, 10), (10, 2)]);
    assert_eq!(
        sorted(ids),
        sorted(seeded),
        "every match once across the batches"
    );
}

#[test]
fn journal_list_cut_at_30_says_so_and_offset_30_holds_the_rest() {
    let app = TestApp::new();
    let seeded: Vec<String> = (0..32)
        .map(|i| seed_entry(&app, "2026-10-20", &format!("Entry {i:02} about the day")))
        .collect();

    let results = results_of(
        &app,
        &[("journal_list", "{}"), ("journal_list", r#"{"offset":30}"#)],
    );

    let ids = walk(&results, JOURNAL_LIST, 32, &[(0, 30), (30, 2)]);
    assert_eq!(
        sorted(ids),
        sorted(seeded),
        "every Entry once across the batches"
    );
}

#[test]
fn journal_search_cut_at_10_says_so_and_offset_10_holds_the_rest() {
    let app = TestApp::new();
    let seeded: Vec<String> = (0..12)
        .map(|i| seed_entry(&app, "2026-10-20", &format!("Walk {i:02} along the river")))
        .collect();

    let results = results_of(
        &app,
        &[
            ("journal_search", r#"{"query":"walk"}"#),
            ("journal_search", r#"{"query":"walk","offset":10}"#),
        ],
    );

    let ids = walk(&results, JOURNAL_SEARCH, 12, &[(0, 10), (10, 2)]);
    assert_eq!(
        sorted(ids),
        sorted(seeded),
        "every match once across the batches"
    );
}

#[test]
fn todo_read_cut_at_8_kib_says_so_in_bytes_and_the_next_offset_holds_the_rest() {
    let app = TestApp::new();
    let notes = long_text(READ_BATCH_BYTES + 5_000);
    let task = task_with_notes(&app, "Long notes", &notes);

    let results = results_of(
        &app,
        &[
            ("todo_read", &format!(r#"{{"id":"{task}"}}"#)),
            (
                "todo_read",
                &format!(r#"{{"id":"{task}","offset":{READ_BATCH_BYTES}}}"#),
            ),
        ],
    );

    let first = body(&results[0]);
    assert_eq!(first["notes"], &notes[..READ_BATCH_BYTES], "{}", results[0]);
    assert_eq!(
        first["note"],
        "8192 of 13192 bytes shown. The text is incomplete: call again with offset 8192 for the rest.",
        "{}",
        results[0]
    );
    walk(
        &results,
        TODO_NOTES,
        notes.len(),
        &[(0, READ_BATCH_BYTES), (READ_BATCH_BYTES, 5_000)],
    );
    assert_eq!(
        body(&results[1])["notes"],
        &notes[READ_BATCH_BYTES..],
        "{}",
        results[1]
    );
}

#[test]
fn journal_read_cut_at_8_kib_says_so_in_bytes_and_the_next_offset_holds_the_rest() {
    let app = TestApp::new();
    let text = long_text(READ_BATCH_BYTES + 5_000);
    let entry = seed_entry(&app, "2026-10-20", &text);

    let results = results_of(
        &app,
        &[
            ("journal_read", &format!(r#"{{"id":"{entry}"}}"#)),
            (
                "journal_read",
                &format!(r#"{{"id":"{entry}","offset":{READ_BATCH_BYTES}}}"#),
            ),
        ],
    );

    assert_eq!(
        body(&results[0])["body"],
        &text[..READ_BATCH_BYTES],
        "{}",
        results[0]
    );
    walk(
        &results,
        JOURNAL_BODY,
        text.len(),
        &[(0, READ_BATCH_BYTES), (READ_BATCH_BYTES, 5_000)],
    );
    assert_eq!(
        body(&results[1])["body"],
        &text[READ_BATCH_BYTES..],
        "{}",
        results[1]
    );
}

#[test]
fn a_complete_list_and_search_show_all_their_rows_and_no_note() {
    let app = TestApp::new();
    for i in 0..3 {
        seed_task(&app, &format!("Renew passport {i}"), None);
        seed_entry(&app, "2026-10-20", &format!("Walk {i}"));
    }

    let results = results_of(
        &app,
        &[
            ("todo_list", "{}"),
            ("journal_list", "{}"),
            ("todo_search", r#"{"query":"passport"}"#),
            ("journal_search", r#"{"query":"walk"}"#),
        ],
    );

    walk(&results[0..1], TODO_LIST, 3, &[(0, 3)]);
    walk(&results[1..2], JOURNAL_LIST, 3, &[(0, 3)]);
    walk(&results[2..3], TODO_SEARCH, 3, &[(0, 3)]);
    walk(&results[3..4], JOURNAL_SEARCH, 3, &[(0, 3)]);
}

#[test]
fn a_complete_read_shows_its_whole_text_and_no_note() {
    let app = TestApp::new();
    let task = task_with_notes(&app, "Short notes", "Two lines.\nAnd a third.");
    let entry = seed_entry(&app, "2026-10-20", "A short Entry.");

    let results = results_of(
        &app,
        &[
            ("todo_read", &format!(r#"{{"id":"{task}"}}"#)),
            ("journal_read", &format!(r#"{{"id":"{entry}"}}"#)),
        ],
    );

    assert_eq!(
        body(&results[0])["notes"],
        "Two lines.\nAnd a third.",
        "{}",
        results[0]
    );
    walk(&results[0..1], TODO_NOTES, 23, &[(0, 23)]);
    assert_eq!(
        body(&results[1])["body"],
        "A short Entry.",
        "{}",
        results[1]
    );
    walk(&results[1..2], JOURNAL_BODY, 14, &[(0, 14)]);
}

#[test]
fn a_later_batch_that_is_still_incomplete_counts_from_the_start_of_the_list() {
    let app = TestApp::new();
    for i in 0..35 {
        seed_task(&app, &format!("Task {i:02}"), None);
    }

    let results = results_of(&app, &[("todo_list", r#"{"limit":10,"offset":10}"#)]);

    let middle = body(&results[0]);
    assert_eq!(ids_under(&middle, "tasks").len(), 10, "{}", results[0]);
    assert_eq!(middle["shown"], 10, "{}", results[0]);
    assert_eq!(middle["total"], 35, "{}", results[0]);
    assert_eq!(middle["next_offset"], 20, "{}", results[0]);
    assert_eq!(
        middle["note"],
        "20 of 35 tasks shown. The list is incomplete: call again with offset 20 for the rest.",
        "{}",
        results[0]
    );
}

#[test]
fn a_later_read_part_that_is_still_incomplete_counts_from_the_start_of_the_text() {
    let app = TestApp::new();
    let text = long_text(20_000);
    let entry = seed_entry(&app, "2026-10-20", &text);

    let results = results_of(
        &app,
        &[(
            "journal_read",
            &format!(r#"{{"id":"{entry}","offset":{READ_BATCH_BYTES}}}"#),
        )],
    );

    let middle = body(&results[0]);
    assert_eq!(
        middle["body"],
        &text[READ_BATCH_BYTES..2 * READ_BATCH_BYTES],
        "{}",
        results[0]
    );
    assert_eq!(middle["shown"], READ_BATCH_BYTES, "{}", results[0]);
    assert_eq!(middle["total"], 20_000, "{}", results[0]);
    assert_eq!(
        middle["next_offset"],
        2 * READ_BATCH_BYTES,
        "{}",
        results[0]
    );
    assert_eq!(
        middle["note"],
        "16384 of 20000 bytes shown. The text is incomplete: call again with offset 16384 for the rest.",
        "{}",
        results[0]
    );
}

/// A title of `len` characters that starts with `label`, so every row of a list is the same size.
fn title_of(label: &str, len: usize) -> String {
    format!("{label} {}", "x".repeat(len - label.len() - 1))
}

/// A text of `chars` copies of "é": two bytes each, so each row that carries it is large.
fn accented(chars: usize) -> String {
    "é".repeat(chars)
}

#[test]
fn todo_list_whose_rows_pass_the_cap_is_cut_short_and_no_task_is_lost() {
    let app = TestApp::new();
    // Each row is about 630 bytes, so 24 rows fill a batch and 25 would pass the 16 KiB cap.
    let seeded: Vec<String> = (0..100)
        .map(|i| seed_task(&app, &title_of(&format!("Task {i:03}"), 480), None))
        .collect();

    let results = results_of(
        &app,
        &[
            ("todo_list", r#"{"limit":100}"#),
            ("todo_list", r#"{"limit":100,"offset":24}"#),
            ("todo_list", r#"{"limit":100,"offset":48}"#),
            ("todo_list", r#"{"limit":100,"offset":72}"#),
            ("todo_list", r#"{"limit":100,"offset":96}"#),
        ],
    );

    assert!(
        !results[0].contains("too large"),
        "a batch that passes the cap is cut, not refused: {}",
        results[0]
    );
    let ids = walk(
        &results,
        TODO_LIST,
        100,
        &[(0, 24), (24, 24), (48, 24), (72, 24), (96, 4)],
    );
    assert_eq!(
        sorted(ids),
        sorted(seeded),
        "every Task once across the batches"
    );
}

#[test]
fn journal_list_whose_rows_pass_the_cap_is_cut_short_and_no_entry_is_lost() {
    let app = TestApp::new();
    // Each preview is 120 accented characters, so each row is about 405 bytes: 38 rows fill a batch.
    let seeded: Vec<String> = (0..100)
        .map(|_| seed_entry(&app, "2026-10-20", &accented(130)))
        .collect();

    let results = results_of(
        &app,
        &[
            ("journal_list", r#"{"limit":100}"#),
            ("journal_list", r#"{"limit":100,"offset":38}"#),
            ("journal_list", r#"{"limit":100,"offset":76}"#),
        ],
    );

    assert!(
        !results[0].contains("too large"),
        "a batch that passes the cap is cut, not refused: {}",
        results[0]
    );
    let ids = walk(&results, JOURNAL_LIST, 100, &[(0, 38), (38, 38), (76, 24)]);
    assert_eq!(
        sorted(ids),
        sorted(seeded),
        "every Entry once across the batches"
    );
}

#[test]
fn todo_search_whose_rows_pass_the_cap_is_cut_short_and_no_match_is_lost() {
    let app = TestApp::new();
    // A search of 25 rows, each about 630 bytes, would pass the cap: the batch holds 24.
    let seeded: Vec<String> = (0..30)
        .map(|i| seed_task(&app, &title_of(&format!("Passport {i:03}"), 480), None))
        .collect();

    let results = results_of(
        &app,
        &[
            ("todo_search", r#"{"query":"passport","limit":25}"#),
            (
                "todo_search",
                r#"{"query":"passport","limit":25,"offset":24}"#,
            ),
        ],
    );

    assert!(
        !results[0].contains("too large"),
        "a batch that passes the cap is cut, not refused: {}",
        results[0]
    );
    let ids = walk(&results, TODO_SEARCH, 30, &[(0, 24), (24, 6)]);
    assert_eq!(
        sorted(ids),
        sorted(seeded),
        "every match once across the batches"
    );
}

#[test]
fn journal_search_at_its_largest_batch_fits_the_cap_and_no_match_is_lost() {
    // Not red first: a search row holds a snippet of at most about 160 bytes, so 25 rows (about
    // 7 KiB) never reach the cap. This test guards that bound.
    let app = TestApp::new();
    let seeded: Vec<String> = (0..30)
        .map(|i| {
            seed_entry(
                &app,
                "2026-10-20",
                &format!("Walk {i:02} {}", accented(200)),
            )
        })
        .collect();

    let results = results_of(
        &app,
        &[
            ("journal_search", r#"{"query":"walk","limit":25}"#),
            (
                "journal_search",
                r#"{"query":"walk","limit":25,"offset":25}"#,
            ),
        ],
    );

    let ids = walk(&results, JOURNAL_SEARCH, 30, &[(0, 25), (25, 5)]);
    assert_eq!(
        sorted(ids),
        sorted(seeded),
        "every match once across the batches"
    );
}
