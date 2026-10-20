//! Journal commands. Thin: the rules live in `mnemora-journal`.

use mnemora_core::error::AppError;
use mnemora_journal::{Entry, NewEntry, Page};
use tauri::State;

use crate::state::{AppState, lock};

/// Writes a new Entry on a Page. Rejects blank text and invalid dates or zones.
#[tauri::command]
#[specta::specta]
pub fn journal_entry_create(
    state: State<'_, AppState>,
    entry: NewEntry,
) -> Result<Entry, AppError> {
    mnemora_journal::create_entry(&mut lock(&state.db), entry)
}

/// Replaces an Entry's text. `session_id` groups a composer session's saves in the change log.
#[tauri::command]
#[specta::specta]
pub fn journal_entry_update(
    state: State<'_, AppState>,
    id: String,
    text: String,
    session_id: String,
) -> Result<Entry, AppError> {
    mnemora_journal::update_entry(&mut lock(&state.db), &id, text, &session_id)
}

/// Up to `limit` Pages, newest first, continuing before the `cursor` date (the last Page's date
/// from the previous request). Fewer Pages than `limit` means the end.
#[tauri::command]
#[specta::specta]
pub fn journal_pages(
    state: State<'_, AppState>,
    cursor: Option<String>,
    limit: u32,
) -> Result<Vec<Page>, AppError> {
    mnemora_journal::list_pages(&lock(&state.db), cursor.as_deref(), limit)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::test_support::TestApp;

    fn create(app: &TestApp, date: &str, text: &str) -> Result<Value, Value> {
        app.invoke(
            "journal_entry_create",
            json!({ "entry": {
                "page_date": date, "text": text, "written_tz": "Asia/Kolkata",
                "written_offset_min": 330, "session_id": "s1",
            }}),
        )
    }

    fn pages(app: &TestApp, cursor: Option<&str>, limit: u32) -> Vec<Value> {
        app.invoke("journal_pages", json!({ "cursor": cursor, "limit": limit }))
            .unwrap()
    }

    fn log_ops(app: &TestApp) -> Vec<(String, String)> {
        app.with_db(|c| {
            let mut stmt = c
                .prepare("SELECT op, session_id FROM change_log WHERE module = 'journal' ORDER BY at, id")
                .unwrap();
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        })
    }

    #[test]
    fn blank_text_is_rejected_and_nothing_is_stored() {
        let app = TestApp::new();
        for blank in ["", "   ", "\n\t \n"] {
            let err = create(&app, "2026-10-20", blank).unwrap_err();
            assert_eq!(err["code"], "validation");
        }
        assert!(pages(&app, None, 10).is_empty());
        assert!(log_ops(&app).is_empty());
    }

    #[test]
    fn invalid_page_date_and_zone_are_rejected() {
        let app = TestApp::new();
        assert_eq!(
            create(&app, "2026-02-30", "x").unwrap_err()["code"],
            "validation"
        );
        let err = app
            .invoke::<Value>(
                "journal_entry_create",
                json!({ "entry": { "page_date": "2026-10-20", "text": "x", "written_tz": "",
                    "written_offset_min": 0, "session_id": "s" }}),
            )
            .unwrap_err();
        assert_eq!(err["code"], "validation");
        let err = app
            .invoke::<Value>(
                "journal_entry_create",
                json!({ "entry": { "page_date": "2026-10-20", "text": "x", "written_tz": "UTC",
                    "written_offset_min": 15 * 60, "session_id": "s" }}),
            )
            .unwrap_err();
        assert_eq!(err["code"], "validation");
    }

    #[test]
    fn create_keeps_zone_offset_and_a_future_page_date() {
        let app = TestApp::new();
        let e = create(&app, "2999-01-01", "Hello **world**").unwrap();
        assert_eq!(e["page_date"], "2999-01-01");
        assert_eq!(e["written_tz"], "Asia/Kolkata");
        assert_eq!(e["written_offset_min"], 330);
        assert_eq!(e["word_count"], 2);
        assert!(e["written_at"].as_i64().unwrap() > 1_704_067_200_000);
        assert_eq!(e["edited_at"], Value::Null);
        let listed = pages(&app, None, 10);
        assert_eq!(
            listed[0]["entries"][0], e,
            "a later read shows exactly what create returned"
        );
    }

    #[test]
    fn pages_are_newest_first_with_entries_oldest_first() {
        let app = TestApp::new();
        let a = create(&app, "2026-10-19", "mon").unwrap();
        let b = create(&app, "2026-10-20", "first").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        let c = create(&app, "2026-10-20", "second").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        // Backfilled: written last, belongs to an old Page.
        let d = create(&app, "2026-10-18", "backfill").unwrap();

        let listed = pages(&app, None, 10);
        let dates: Vec<_> = listed.iter().map(|p| p["date"].as_str().unwrap()).collect();
        assert_eq!(dates, ["2026-10-20", "2026-10-19", "2026-10-18"]);
        assert_eq!(listed[0]["entries"], json!([b, c]));
        assert_eq!(listed[1]["entries"], json!([a]));
        assert_eq!(listed[2]["entries"], json!([d]));
    }

    #[test]
    fn pages_carry_entry_and_word_counts() {
        let app = TestApp::new();
        create(&app, "2026-10-20", "one two").unwrap();
        create(&app, "2026-10-20", "# three **four** five").unwrap();
        create(&app, "2026-10-19", "solo").unwrap();
        let listed = pages(&app, None, 10);
        assert_eq!(listed[0]["entry_count"], 2);
        assert_eq!(listed[0]["word_count"], 5);
        assert_eq!(listed[1]["entry_count"], 1);
        assert_eq!(listed[1]["word_count"], 1);
    }

    #[test]
    fn pages_are_cursor_paged_by_page_count() {
        let app = TestApp::new();
        for d in ["2026-10-17", "2026-10-18", "2026-10-19", "2026-10-20"] {
            create(&app, d, "x").unwrap();
        }
        create(&app, "2026-10-20", "again").unwrap();
        let first = pages(&app, None, 2);
        assert_eq!(first.len(), 2);
        assert_eq!(first[0]["entries"].as_array().unwrap().len(), 2);
        let next = pages(&app, Some("2026-10-19"), 2);
        let dates: Vec<_> = next.iter().map(|p| p["date"].as_str().unwrap()).collect();
        assert_eq!(dates, ["2026-10-18", "2026-10-17"]);
        assert!(pages(&app, Some("2026-10-17"), 2).is_empty());
    }

    #[test]
    fn create_and_update_in_one_session_leave_one_create_record() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "a").unwrap();
        for text in ["a b", "a b c"] {
            let _: Value = app
                .invoke(
                    "journal_entry_update",
                    json!({ "id": e["id"], "text": text, "sessionId": "s1" }),
                )
                .unwrap();
        }
        assert_eq!(log_ops(&app), [("create".into(), "s1".into())]);
        let snapshot: String = app.with_db(|c| {
            c.query_row("SELECT snapshot FROM change_log", [], |r| r.get(0))
                .unwrap()
        });
        assert!(
            snapshot.contains("a b c"),
            "the record holds the latest state"
        );
    }

    #[test]
    fn update_changes_text_and_word_count_and_a_new_session_adds_a_record() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "one").unwrap();
        let u: Value = app
            .invoke(
                "journal_entry_update",
                json!({ "id": e["id"], "text": "# one two three", "sessionId": "s2" }),
            )
            .unwrap();
        assert_eq!(u["body"], "# one two three");
        assert_eq!(u["word_count"], 3);
        assert_eq!(
            u["written_at"], e["written_at"],
            "the written instant never changes"
        );
        assert_eq!(pages(&app, None, 1)[0]["entries"][0], u);
        assert_eq!(
            log_ops(&app),
            [
                ("create".into(), "s1".into()),
                ("update".into(), "s2".into())
            ]
        );
    }

    #[test]
    fn update_of_an_unknown_id_or_blank_text_fails() {
        let app = TestApp::new();
        let err = app
            .invoke::<Value>(
                "journal_entry_update",
                json!({ "id": "nope", "text": "x", "sessionId": "s" }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "unknown_key");

        let e = create(&app, "2026-10-20", "keep").unwrap();
        let err = app
            .invoke::<Value>(
                "journal_entry_update",
                json!({ "id": e["id"], "text": "  ", "sessionId": "s" }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "validation");
        assert_eq!(pages(&app, None, 1)[0]["entries"][0]["body"], "keep");
    }

    fn update(app: &TestApp, id: &Value, text: &str, session: &str) -> Value {
        app.invoke(
            "journal_entry_update",
            json!({ "id": id, "text": text, "sessionId": session }),
        )
        .unwrap()
    }

    /// Moves the written instant into the past: the command layer reads the real clock.
    fn written_minutes_ago(app: &TestApp, id: &Value, minutes: i64) {
        app.with_db(|c| {
            // Ids are generated UUIDs, so formatting them in is safe.
            c.execute(
                &format!(
                    "UPDATE journal_entries SET written_at = written_at - {} WHERE id = '{}'",
                    minutes * 60_000,
                    id.as_str().unwrap()
                ),
                [],
            )
            .unwrap();
        });
    }

    #[test]
    fn update_within_ten_minutes_of_writing_is_not_an_edit() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "a").unwrap();
        written_minutes_ago(&app, &e["id"], 9);
        let u = update(&app, &e["id"], "a b", "s2");
        assert_eq!(u["edited_at"], Value::Null);
        assert!(u["updated_at"].as_i64() >= e["updated_at"].as_i64());
        assert_eq!(
            pages(&app, None, 1)[0]["entries"][0]["edited_at"],
            Value::Null
        );
    }

    #[test]
    fn update_after_ten_minutes_marks_edited_and_keeps_the_mark() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "a").unwrap();
        written_minutes_ago(&app, &e["id"], 11);
        let u = update(&app, &e["id"], "a b", "s2");
        let edited = u["edited_at"].as_i64().expect("edited_at is set");
        assert_eq!(u["updated_at"].as_i64(), Some(edited));
        std::thread::sleep(std::time::Duration::from_millis(3));
        let again = update(&app, &e["id"], "a b c", "s2");
        assert!(
            again["edited_at"].as_i64().unwrap() == edited,
            "the first edit time stays"
        );
        assert!(
            again["updated_at"].as_i64().unwrap() > edited,
            "updated_at always moves"
        );
    }

    #[test]
    fn same_session_updates_make_one_record_and_a_new_session_appends() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "a").unwrap();
        for text in ["a1", "a2", "a3"] {
            update(&app, &e["id"], text, "s2");
        }
        update(&app, &e["id"], "a4", "s3");
        update(&app, &e["id"], "a5", "s3");
        assert_eq!(
            log_ops(&app),
            [
                ("create".into(), "s1".into()),
                ("update".into(), "s2".into()),
                ("update".into(), "s3".into())
            ]
        );
        let last: String = app.with_db(|c| {
            c.query_row(
                "SELECT snapshot FROM change_log WHERE session_id = 's3'",
                [],
                |r| r.get(0),
            )
            .unwrap()
        });
        assert!(
            last.contains("a5"),
            "a session's record holds its latest state"
        );
    }
}
