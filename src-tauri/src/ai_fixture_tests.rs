//! Command-layer tests on the recent-changes fixture (`fixtures/recent_changes.json`, replayed by
//! `ai_fixture.rs`): about three weeks of Journal Entries and Tasks, written through the commands at
//! their own instants and read back by the change Tools through a scripted native-tool Provider with
//! the device in New York (ADR 0009). Every expected value is a literal, derived from the fixture by
//! hand and checked by an independent script, not recomputed by the code under test.

use mnemora_harness::scripted::Reply;
use serde_json::{Value, json};

use crate::ai_fixture::{self, Loaded};
use crate::ai_tools_tests::tool_results_in;
use crate::test_support::TestApp;

/// The device's zone. It is UTC-4 in October, so 23:50 and 00:10 fall on different UTC days.
const ZONE: &str = "America/New_York";
/// The fixed now: 2026-10-25 12:00 in New York. Yesterday is Saturday 24 October.
const NOW: i64 = 1_792_944_000_000;

/// The fixture's first and last days (inclusive) and the days the tests ask about.
const FIRST_DAY: &str = "2026-10-04";
const TODAY: &str = "2026-10-25";
const YESTERDAY: &str = "2026-10-24";

/// The fixture loaded into a fresh app, with the clock at `NOW`.
fn loaded() -> (TestApp, Loaded) {
    let app = TestApp::new();
    let ids = ai_fixture::load(&app, NOW, ZONE).unwrap();
    assert_eq!(ids.len(), 112, "the fixture did not load in full");
    (app, ids)
}

/// The body of a Tool result: the JSON after the observation line.
fn body(result: &str) -> Value {
    let json = result.find('{').map_or("", |at| &result[at..]);
    serde_json::from_str(json).unwrap_or_else(|_| panic!("not a JSON body: {result}"))
}

/// One row of a changes Tool, as the Tool gives it.
fn row(id: &str, at: &str, change: &str, title: &str) -> Value {
    json!({ "id": id, "at": at, "change": change, "title": title })
}

/// Every row of a changes body, in the order the Tool gives them.
fn rows_of(body: &Value) -> Vec<Value> {
    body["days"]
        .as_array()
        .map(|days| {
            days.iter()
                .flat_map(|day| day["rows"].as_array().cloned().unwrap_or_default())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn todo_changes_for_yesterday_lists_exactly_the_expected_rows_and_counts() {
    let (app, ids) = loaded();
    let id = |key: &str| ids.id(key).to_owned();
    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[("todo_changes", r#"{"from":"2026-10-24","to":"2026-10-24"}"#)]),
            Reply::text(&["Done."]),
        ],
    );

    let body = body(&results[0]);
    assert_eq!(
        body["days"],
        json!([{ "date": YESTERDAY, "rows": [
            row(&id("plants"), "07:55", "Done (occurrence of 24 Oct)", "Water the plants"),
            row(&id("call_bank"), "09:02", "Created", "Call the bank about the card"),
            row(&id("council_tax"), "09:30", "Due date Mon 26 Oct → Wed 28 Oct", "Pay council tax"),
            row(&id("council_tax"), "09:35", "Commented: Moved to Wednesday, after payday.", "Pay council tax"),
            row(&id("call_bank"), "10:44", "Commented: Card is blocked. New one posted, due Tuesday.", "Call the bank about the card"),
            row(&id("call_bank"), "10:45", "Done", "Call the bank about the card"),
            row(&id("hartley"), "11:00", "Created", "Reply to the Hartley email"),
            row(&id("draft_priya"), "12:00", "Moved 📥 Inbox → ≡ Work", "Email Priya the draft"),
            row(&id("draft_priya"), "12:05", "Commented: Priya asked for the numbers tab too.", "Email Priya the draft"),
            row(&id("passport"), "13:05", "Commented: Sent the form off this morning. Still waiting on the photos.", "Renew passport"),
            row(&id("prescription"), "13:30", "Created", "Pick up the prescription"),
            row(&id("hartley"), "14:00", "Due date no date → Mon 26 Oct", "Reply to the Hartley email"),
            row(&id("dentist"), "15:10", "Priority P4 → P1", "Book the dentist"),
            row(&id("dentist"), "15:12", "Commented: Called them, they have a slot on the 27th.", "Book the dentist"),
            row(&id("violin"), "16:20", "Won’t do", "Learn the violin"),
            row(&id("candles"), "17:40", "Deleted", "Buy birthday candles"),
            row(&id("anika"), "18:00", "Created", "Reply to Anika about Sunday lunch"),
            row(&id("anika"), "18:05", "Commented: Draft is written, it just needs sending.", "Reply to Anika about Sunday lunch"),
            row(&id("dry_cleaning"), "23:50", "Done", "Pick up the dry cleaning"),
        ] }]),
        "{}",
        results[0]
    );
    assert_eq!(
        body["counts"],
        json!({ "created": 4, "done": 3, "wont": 1, "skipped": 0, "reopened": 0, "moved": 1, "due": 2, "priority": 1, "edited": 0, "commented": 6, "deleted": 1, "restored": 0, "repeat": 0 }),
        "{}",
        results[0]
    );
    assert!(body["next_offset"].is_null(), "{}", results[0]);
}

#[test]
fn journal_changes_for_yesterday_list_each_entry_by_when_it_was_written() {
    let (app, ids) = loaded();
    let id = |key: &str| ids.id(key).to_owned();
    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[
                (
                    "journal_changes",
                    r#"{"from":"2026-10-24","to":"2026-10-24"}"#,
                ),
                (
                    "journal_changes",
                    r#"{"from":"2026-10-23","to":"2026-10-23"}"#,
                ),
                ("journal_list", r#"{"from":"2026-10-23","to":"2026-10-23"}"#),
            ]),
            Reply::text(&["Done."]),
        ],
    );

    // The late-night Entry was written at 00:40 on Saturday, so it reads on Saturday, not Friday's Page.
    let saturday = body(&results[0]);
    assert_eq!(
        rows_of(&saturday),
        [
            row(&id("e16"), "00:40", "Written, 44 words", "Having been exceedingly disturbed in the night with the barking of a dog of one of our neighbours that I could not sleep"),
            row(&id("e14"), "08:30", "Edited", "In the morning called out to carry L20 to Mr. Downing, which I did and came back, and finding Mr. Pierce, the surgeon, I"),
            row(&id("e17"), "09:14", "Written, 68 words", "In the morning to my office, where, after I had drank my morning draft at Will’s with Ethell and Mr. Stevens, I went and"),
            row(&id("e13"), "11:02", "Deleted", "I went in the morning to Mr. Messum’s, where I met with W. Thurburn and sat with him in his pew. A very eloquent sermon "),
            row(&id("e18"), "19:30", "Written, 62 words", "Walked upon the hill-tops; followed the sheep tracks till we overlooked the larger coombe. Sat in the sunshine. The dist"),
        ]
        .to_vec(),
        "{}",
        results[0]
    );
    assert_eq!(
        saturday["counts"],
        json!({ "written": 3, "edited": 1, "deleted": 1 }),
        "{}",
        results[0]
    );

    let friday = body(&results[1]);
    assert_eq!(
        rows_of(&friday),
        [
            row(&id("e15"), "07:30", "Written, 41 words", "Walked between half-past three and half-past five. The evening cold and clear. The sea of a sober grey, streaked by the "),
            row(&id("e14"), "20:05", "Written, 35 words", "In the morning called out to carry L20 to Mr. Downing, which I did and came back, and finding Mr. Pierce, the surgeon, I"),
        ]
        .to_vec(),
        "{}",
        results[1]
    );
    assert!(!results[1].contains(&id("e16")), "{}", results[1]);
    // The page of Friday holds that Entry, though: a change counts on the day it was made, a Page does not.
    assert!(results[2].contains(&id("e16")), "{}", results[2]);
}

#[test]
fn todo_list_closed_on_yesterday_lists_the_done_and_wont_do_tasks_newest_first() {
    let (app, _ids) = loaded();
    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[(
                "todo_list",
                r#"{"closed_from":"2026-10-24","closed_to":"2026-10-24"}"#,
            )]),
            Reply::text(&["Done."]),
        ],
    );

    let body = body(&results[0]);
    let closed: Vec<Value> = body["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| json!([task["title"], task["state"], task["closed_at_local"]]))
        .collect();
    assert_eq!(
        closed,
        [
            json!(["Pick up the dry cleaning", "done", "2026-10-24 23:50"]),
            json!(["Learn the violin", "wont", "2026-10-24 16:20"]),
            json!(["Call the bank about the card", "done", "2026-10-24 10:45"]),
            // The occurrence a repeating Task leaves behind on completion is a done Task of its own.
            json!(["Water the plants", "done", "2026-10-24 07:55"]),
        ],
        "{}",
        results[0]
    );
    assert!(body["next_offset"].is_null(), "{}", results[0]);
}

#[test]
fn the_empty_day_has_no_rows_zero_counts_and_no_error() {
    let (app, _ids) = loaded();
    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[
                ("todo_changes", r#"{"from":"2026-10-21","to":"2026-10-21"}"#),
                (
                    "journal_changes",
                    r#"{"from":"2026-10-21","to":"2026-10-21"}"#,
                ),
                (
                    "todo_list",
                    r#"{"closed_from":"2026-10-21","closed_to":"2026-10-21"}"#,
                ),
            ]),
            Reply::text(&["Nothing changed."]),
        ],
    );

    let todo = body(&results[0]);
    assert_eq!(todo["days"], json!([]), "{}", results[0]);
    assert_eq!(
        todo["counts"],
        json!({ "created": 0, "done": 0, "wont": 0, "skipped": 0, "reopened": 0, "moved": 0, "due": 0, "priority": 0, "edited": 0, "commented": 0, "deleted": 0, "restored": 0, "repeat": 0 }),
        "{}",
        results[0]
    );
    assert!(todo["next_offset"].is_null(), "{}", results[0]);
    assert!(todo.get("error").is_none(), "{}", results[0]);

    let journal = body(&results[1]);
    assert_eq!(journal["days"], json!([]), "{}", results[1]);
    assert_eq!(
        journal["counts"],
        json!({ "written": 0, "edited": 0, "deleted": 0 }),
        "{}",
        results[1]
    );
    assert!(journal["next_offset"].is_null(), "{}", results[1]);
    assert!(journal.get("error").is_none(), "{}", results[1]);

    let closed = body(&results[2]);
    assert_eq!(closed["tasks"], json!([]), "{}", results[2]);
    assert!(closed["next_offset"].is_null(), "{}", results[2]);
}

#[test]
fn the_last_seven_days_count_each_kind_for_todo_and_journal() {
    let (app, _ids) = loaded();
    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[
                ("todo_changes", r#"{"from":"2026-10-19","to":"2026-10-25"}"#),
                (
                    "journal_changes",
                    r#"{"from":"2026-10-19","to":"2026-10-25"}"#,
                ),
            ]),
            Reply::text(&["Done."]),
        ],
    );

    assert_eq!(
        body(&results[0])["counts"],
        json!({ "created": 13, "done": 7, "wont": 1, "skipped": 1, "reopened": 0, "moved": 1, "due": 2, "priority": 2, "edited": 0, "commented": 9, "deleted": 1, "restored": 0, "repeat": 1 }),
        "{}",
        results[0]
    );
    assert_eq!(
        body(&results[1])["counts"],
        json!({ "written": 9, "edited": 1, "deleted": 1 }),
        "{}",
        results[1]
    );
}

#[test]
fn a_comment_only_update_reads_as_commented_with_its_snippet_and_the_task_stays_open() {
    let (app, ids) = loaded();
    let passport = ids.id("passport").to_owned();
    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[
                ("todo_changes", r#"{"from":"2026-10-24","to":"2026-10-24"}"#),
                ("todo_list", r#"{"state":"open","limit":100}"#),
            ]),
            Reply::text(&["Done."]),
        ],
    );

    // Only the Comment reads for this Task yesterday: the Task itself was not changed.
    let on_passport: Vec<Value> = rows_of(&body(&results[0]))
        .into_iter()
        .filter(|r| r["id"] == passport.as_str())
        .collect();
    assert_eq!(
        on_passport,
        [row(
            &passport,
            "13:05",
            "Commented: Sent the form off this morning. Still waiting on the photos.",
            "Renew passport"
        )]
        .to_vec(),
        "{}",
        results[0]
    );
    // The Comment says the form went off; the Task is still open.
    let open = body(&results[1])["tasks"].clone();
    let task = open
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == passport.as_str())
        .unwrap_or_else(|| panic!("the open list has no Renew passport: {}", results[1]));
    assert_eq!(task["state"], "open", "{}", results[1]);
}

#[test]
fn deleted_items_read_as_deleted_and_stay_out_of_the_lists_and_searches() {
    let (app, ids) = loaded();
    let candles = ids.id("candles").to_owned();
    let entry = ids.id("e13").to_owned();
    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[
                ("todo_changes", r#"{"from":"2026-10-24","to":"2026-10-24"}"#),
                ("todo_list", r#"{"state":"open","limit":100}"#),
                ("todo_search", r#"{"query":"birthday candles"}"#),
                ("journal_list", r#"{"from":"2026-10-22","to":"2026-10-24"}"#),
                ("journal_search", r#"{"query":"Thurburn"}"#),
                ("journal_search", r#"{"query":"Pierce"}"#),
            ]),
            Reply::text(&["Done."]),
        ],
    );

    assert!(
        rows_of(&body(&results[0])).contains(&row(
            &candles,
            "17:40",
            "Deleted",
            "Buy birthday candles"
        )),
        "{}",
        results[0]
    );
    // Non-empty checks first, so that "not listed" cannot be a list that came back empty.
    assert!(
        results[1].contains(&ids.id("council_tax").to_owned()),
        "{}",
        results[1]
    );
    assert!(!results[1].contains(&candles), "{}", results[1]);
    assert!(!results[2].contains(&candles), "{}", results[2]);
    assert!(
        results[3].contains(&ids.id("e14").to_owned()),
        "{}",
        results[3]
    );
    assert!(!results[3].contains(&entry), "{}", results[3]);
    assert!(
        results[5].contains(&ids.id("e14").to_owned()),
        "{}",
        results[5]
    );
    assert!(!results[4].contains(&entry), "{}", results[4]);
}

#[test]
fn the_full_range_is_paged_in_batches_of_100_and_the_journal_fits_in_one() {
    let (app, _ids) = loaded();
    let whole = format!(r#"{{"from":"{FIRST_DAY}","to":"{TODAY}"}}"#);
    let whole_100 = format!(r#"{{"from":"{FIRST_DAY}","to":"{TODAY}","limit":100}}"#);
    let second_batch = format!(r#"{{"from":"{FIRST_DAY}","to":"{TODAY}","offset":30}}"#);
    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[
                ("todo_changes", &whole),
                ("todo_changes", &second_batch),
                ("todo_changes", &whole_100),
                ("journal_changes", &whole),
            ]),
            Reply::text(&["Done."]),
        ],
    );

    // The range holds 192 Task changes (58 before the catch-up day, 134 on it), so a default batch
    // holds the first 100 and says the rest follows. A batch from offset 30 holds the next 100.
    assert!(results[0].len() < 16 * 1024, "{}", results[0].len());
    let first = body(&results[0]);
    assert_eq!(rows_of(&first).len(), 100, "{}", results[0]);
    assert_eq!(first["shown"], 100, "{}", results[0]);
    assert_eq!(first["total"], 192, "{}", results[0]);
    assert_eq!(first["next_offset"], 100, "{}", results[0]);
    assert_eq!(first["counts"]["created"], 86, "{}", results[0]);
    assert_eq!(
        first["note"],
        "100 of 192 changes shown. The list is incomplete: call again with offset 100 for the rest.",
        "{}",
        results[0]
    );
    let second = body(&results[1]);
    assert_eq!(rows_of(&second).len(), 100, "{}", results[1]);
    assert_eq!(second["total"], 192, "{}", results[1]);
    assert_eq!(second["next_offset"], 130, "{}", results[1]);
    assert_eq!(
        second["note"],
        "130 of 192 changes shown. The list is incomplete: call again with offset 130 for the rest.",
        "{}",
        results[1]
    );

    // Asked for 100 by name, the first batch is the same as the default.
    assert!(results[2].len() < 16 * 1024, "{}", results[2].len());
    assert_eq!(
        rows_of(&body(&results[2])),
        rows_of(&first),
        "{}",
        results[2]
    );

    // The journal range holds 21 Entry rows before the catch-up day and 5 on it: all 26 in one batch.
    assert!(results[3].len() < 16 * 1024, "{}", results[3].len());
    let journal = body(&results[3]);
    assert_eq!(rows_of(&journal).len(), 26, "{}", results[3]);
    assert!(journal["next_offset"].is_null(), "{}", results[3]);
    assert!(journal.get("note").is_none(), "{}", results[3]);
}
