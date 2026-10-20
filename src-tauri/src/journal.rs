//! Journal commands. Thin: the rules live in `mnemora-journal`.

use mnemora_core::error::AppError;
use mnemora_journal::{Entry, NewEntry, Page, PageDirection, SearchResults, TodaySummary};
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

/// Ticks or unticks the checklist item on the 0-based source `line`. Never marks the Entry edited.
#[tauri::command]
#[specta::specta]
pub fn journal_entry_toggle_task(
    state: State<'_, AppState>,
    id: String,
    line: u32,
) -> Result<Entry, AppError> {
    mnemora_journal::toggle_task_line(&mut lock(&state.db), &id, line as usize)
}

/// Deletes an Entry. It can be restored for 30 days, then it is erased.
#[tauri::command]
#[specta::specta]
pub fn journal_entry_delete(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    mnemora_journal::delete_entry(&mut lock(&state.db), &id)
}

/// Brings a deleted Entry back to its place on its Page.
#[tauri::command]
#[specta::specta]
pub fn journal_entry_restore(state: State<'_, AppState>, id: String) -> Result<Entry, AppError> {
    mnemora_journal::restore_entry(&mut lock(&state.db), &id)
}

/// Moves an Entry to another Page. Only the Page date changes; the written timestamp never does.
/// Future dates are allowed.
#[tauri::command]
#[specta::specta]
pub fn journal_entry_move(
    state: State<'_, AppState>,
    id: String,
    page_date: String,
) -> Result<Entry, AppError> {
    mnemora_journal::move_entry(&mut lock(&state.db), &id, &page_date)
}

/// Dates (`YYYY-MM-DD`) in the month (`month` is 1-12) that have live Entries, for calendar dots.
#[tauri::command]
#[specta::specta]
pub fn journal_month_days(
    state: State<'_, AppState>,
    year: u32,
    month: u32,
) -> Result<Vec<String>, AppError> {
    mnemora_journal::month_days(&lock(&state.db), year, month)
}

/// Up to `limit` Pages, newest first, picked relative to the `cursor` date: `older` continues
/// before it (the last Page's date from the previous request), `from` starts AT it (a jump to a
/// date), `newer` takes the Pages just above it (scrolling up after a jump; needs a cursor).
/// Fewer Pages than `limit` means that end was reached.
#[tauri::command]
#[specta::specta]
pub fn journal_pages(
    state: State<'_, AppState>,
    cursor: Option<String>,
    limit: u32,
    direction: PageDirection,
) -> Result<Vec<Page>, AppError> {
    mnemora_journal::list_pages(&lock(&state.db), cursor.as_deref(), limit, direction)
}

/// Searches live Entries. Every word is required and matches by prefix; quoted text is an exact
/// phrase; case and accents are ignored. `from` and `to` are optional inclusive Page dates; with
/// no words they list every Entry in the range. `similar` holds Entries that match only with a
/// typo allowed, never repeating `exact`.
#[tauri::command]
#[specta::specta]
pub fn journal_search(
    state: State<'_, AppState>,
    query: String,
    from: Option<String>,
    to: Option<String>,
) -> Result<SearchResults, AppError> {
    mnemora_journal::search(&lock(&state.db), &query, from.as_deref(), to.as_deref())
}

/// How many Entries `today` (the user's local date, `YYYY-MM-DD`) holds and the one written
/// last, as a plain-text snippet. For the Home card.
#[tauri::command]
#[specta::specta]
pub fn journal_today(state: State<'_, AppState>, today: String) -> Result<TodaySummary, AppError> {
    mnemora_journal::today(&lock(&state.db), &today)
}

#[cfg(test)]
mod tests {
    use mnemora_core::module::Module;
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
        pages_in(app, cursor, limit, "older").unwrap()
    }

    fn pages_in(
        app: &TestApp,
        cursor: Option<&str>,
        limit: u32,
        direction: &str,
    ) -> Result<Vec<Value>, Value> {
        app.invoke(
            "journal_pages",
            json!({ "cursor": cursor, "limit": limit, "direction": direction }),
        )
    }

    fn log_ops(app: &TestApp) -> Vec<(String, String)> {
        app.with_db(|c| {
            let mut stmt = c
                .prepare("SELECT op, COALESCE(session_id, '') FROM change_log WHERE module = 'journal' ORDER BY at, id")
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

    fn toggle(app: &TestApp, id: &Value, line: u32) -> Result<Value, Value> {
        app.invoke(
            "journal_entry_toggle_task",
            json!({ "id": id, "line": line }),
        )
    }

    fn delete(app: &TestApp, id: &Value) -> Result<(), Value> {
        app.invoke("journal_entry_delete", json!({ "id": id }))
    }

    fn restore(app: &TestApp, id: &Value) -> Result<Value, Value> {
        app.invoke("journal_entry_restore", json!({ "id": id }))
    }

    #[test]
    fn toggling_flips_only_that_line_and_never_sets_edited() {
        let app = TestApp::new();
        let body = "Plan\n- [ ] one\n- [x] two\n* [ ] three\n1. [ ] four\n> - [ ] five";
        let e = create(&app, "2026-10-20", body).unwrap();
        written_minutes_ago(&app, &e["id"], 60);
        let e = pages(&app, None, 1)[0]["entries"][0].clone();

        let t = toggle(&app, &e["id"], 1).unwrap();
        assert_eq!(
            t["body"],
            "Plan\n- [x] one\n- [x] two\n* [ ] three\n1. [ ] four\n> - [ ] five"
        );
        assert_eq!(t["edited_at"], Value::Null, "ticking is not editing");
        assert_eq!(t["word_count"], e["word_count"]);
        assert_eq!(t["written_at"], e["written_at"]);

        let t = toggle(&app, &e["id"], 2).unwrap();
        assert!(t["body"].as_str().unwrap().contains("- [ ] two"), "untick");
        for line in [3, 4, 5] {
            toggle(&app, &e["id"], line).unwrap();
        }
        let body = pages(&app, None, 1)[0]["entries"][0]["body"].clone();
        assert_eq!(
            body,
            "Plan\n- [x] one\n- [ ] two\n* [x] three\n1. [x] four\n> - [x] five"
        );
    }

    #[test]
    fn toggling_a_non_task_line_or_unknown_entry_fails_and_changes_nothing() {
        let app = TestApp::new();
        let e = create(
            &app,
            "2026-10-20",
            "Plan\n- [ ] one\n- plain\n[ ] not a list",
        )
        .unwrap();
        for line in [0, 2, 3, 99] {
            let err = toggle(&app, &e["id"], line).unwrap_err();
            assert_eq!(err["code"], "validation", "line {line}");
        }
        assert_eq!(
            toggle(&app, &json!("nope"), 1).unwrap_err()["code"],
            "unknown_key"
        );
        assert_eq!(pages(&app, None, 1)[0]["entries"][0], e);
        assert_eq!(log_ops(&app), [("create".into(), "s1".into())]);
    }

    #[test]
    fn each_tick_is_its_own_change_log_record_without_a_session() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "- [ ] a\n- [ ] b").unwrap();
        toggle(&app, &e["id"], 0).unwrap();
        toggle(&app, &e["id"], 1).unwrap();
        let ticks: i64 = app.with_db(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM change_log WHERE op = 'tick' AND session_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap()
        });
        assert_eq!(ticks, 2);
    }

    #[test]
    fn delete_hides_the_entry_and_restore_brings_it_back_in_place() {
        let app = TestApp::new();
        let a = create(&app, "2026-10-20", "first").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        let b = create(&app, "2026-10-20", "second").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        let c = create(&app, "2026-10-20", "third").unwrap();

        delete(&app, &b["id"]).unwrap();
        assert_eq!(pages(&app, None, 10)[0]["entries"], json!([a, c]));
        assert_eq!(pages(&app, None, 10)[0]["entry_count"], 2);

        assert_eq!(restore(&app, &b["id"]).unwrap(), b);
        assert_eq!(pages(&app, None, 10)[0]["entries"], json!([a, b, c]));

        let ops: Vec<String> = log_ops(&app).into_iter().map(|(op, _)| op).collect();
        assert_eq!(ops, ["create", "create", "create", "delete", "restore"]);
    }

    #[test]
    fn deleting_the_only_entry_removes_its_page_and_blocks_further_changes() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "- [ ] a").unwrap();
        delete(&app, &e["id"]).unwrap();
        assert!(pages(&app, None, 10).is_empty());
        assert_eq!(delete(&app, &e["id"]).unwrap_err()["code"], "unknown_key");
        assert_eq!(
            toggle(&app, &e["id"], 0).unwrap_err()["code"],
            "unknown_key"
        );
        let err = app
            .invoke::<Value>(
                "journal_entry_update",
                json!({ "id": e["id"], "text": "x", "sessionId": "s" }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "unknown_key");
        assert_eq!(
            restore(&app, &json!("nope")).unwrap_err()["code"],
            "unknown_key"
        );
    }

    #[test]
    fn restoring_a_live_entry_is_a_harmless_no_op() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "x").unwrap();
        assert_eq!(restore(&app, &e["id"]).unwrap_err()["code"], "unknown_key");
    }

    #[test]
    fn change_log_snapshots_carry_the_deletion_state() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "keep me").unwrap();
        delete(&app, &e["id"]).unwrap();
        restore(&app, &e["id"]).unwrap();
        let rows: Vec<(String, Value)> = app.with_db(|c| {
            let mut stmt = c
                .prepare("SELECT op, snapshot FROM change_log WHERE module = 'journal' ORDER BY at, rowid")
                .unwrap();
            stmt.query_map([], |r| {
                let raw: String = r.get(1)?;
                Ok((r.get(0)?, serde_json::from_str(&raw).unwrap()))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect()
        });
        let ops: Vec<&str> = rows.iter().map(|(op, _)| op.as_str()).collect();
        assert_eq!(ops, ["create", "delete", "restore"]);
        assert_eq!(rows[0].1["deleted_at"], Value::Null);
        assert!(rows[1].1["deleted_at"].as_i64().unwrap() > 1_704_067_200_000);
        assert_eq!(rows[1].1["body"], "keep me");
        assert_eq!(rows[2].1["deleted_at"], Value::Null);
    }

    fn deleted_days_ago(app: &TestApp, id: &Value, days: i64) {
        app.with_db(|c| {
            // Ids are generated UUIDs, so formatting them in is safe.
            c.execute(
                &format!(
                    "UPDATE journal_entries SET deleted_at = {} WHERE id = '{}'",
                    mnemora_core::util::now_ms() - days * 86_400_000,
                    id.as_str().unwrap()
                ),
                [],
            )
            .unwrap();
        });
    }

    fn row_count(app: &TestApp, table: &str) -> i64 {
        app.with_db(|c| {
            c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap()
        })
    }

    #[test]
    fn purge_removes_only_entries_deleted_over_30_days_ago_and_their_history() {
        let app = TestApp::new();
        let old = create(&app, "2026-10-18", "old").unwrap();
        let recent = create(&app, "2026-10-19", "recent").unwrap();
        let live = create(&app, "2026-10-20", "live").unwrap();
        for e in [&old, &recent] {
            delete(&app, &e["id"]).unwrap();
        }
        deleted_days_ago(&app, &old["id"], 31);
        deleted_days_ago(&app, &recent["id"], 29);

        // The startup hook is what the app runs; it purges through the Module contract.
        app.with_db_mut(|c| {
            mnemora_journal::JournalModule
                .on_startup(c, &Default::default())
                .unwrap()
        });

        assert_eq!(row_count(&app, "journal_entries"), 2);
        let logged: Vec<String> = app.with_db(|c| {
            let mut stmt = c
                .prepare("SELECT DISTINCT entity_id FROM change_log ORDER BY entity_id")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        });
        assert!(!logged.contains(&old["id"].as_str().unwrap().to_owned()));
        assert!(logged.contains(&recent["id"].as_str().unwrap().to_owned()));
        assert!(logged.contains(&live["id"].as_str().unwrap().to_owned()));

        // The recently deleted one can still be undone; a second purge finds nothing.
        restore(&app, &recent["id"]).unwrap();
        assert_eq!(
            app.with_db_mut(|c| mnemora_journal::purge_deleted(c).unwrap()),
            0
        );
    }

    fn move_to(app: &TestApp, id: &Value, date: &str) -> Result<Value, Value> {
        app.invoke("journal_entry_move", json!({ "id": id, "pageDate": date }))
    }

    fn month_days(app: &TestApp, year: u32, month: u32) -> Vec<String> {
        app.invoke(
            "journal_month_days",
            json!({ "year": year, "month": month }),
        )
        .unwrap()
    }

    #[test]
    fn move_changes_only_the_page_and_logs_a_move() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "- [ ] keep").unwrap();
        written_minutes_ago(&app, &e["id"], 60);
        let e = pages(&app, None, 1)[0]["entries"][0].clone();

        let m = move_to(&app, &e["id"], "2026-10-19").unwrap();
        assert_eq!(m["page_date"], "2026-10-19");
        for field in [
            "written_at",
            "written_tz",
            "written_offset_min",
            "body",
            "edited_at",
        ] {
            assert_eq!(m[field], e[field], "{field} is untouched by a move");
        }
        let listed = pages(&app, None, 10);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["date"], "2026-10-19");
        assert_eq!(listed[0]["entries"][0], m);
        let ops: Vec<String> = log_ops(&app).into_iter().map(|(op, _)| op).collect();
        assert_eq!(ops, ["create", "move"]);
    }

    #[test]
    fn move_accepts_a_future_date_and_rejects_bad_ones() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "x").unwrap();
        assert_eq!(
            move_to(&app, &e["id"], "2999-01-01").unwrap()["page_date"],
            "2999-01-01"
        );
        assert_eq!(
            move_to(&app, &e["id"], "2026-02-30").unwrap_err()["code"],
            "validation"
        );
        assert_eq!(pages(&app, None, 1)[0]["date"], "2999-01-01");
        // Moving to where it already is changes nothing and logs nothing.
        move_to(&app, &e["id"], "2999-01-01").unwrap();
        assert_eq!(log_ops(&app).len(), 2);
    }

    #[test]
    fn a_deleted_or_unknown_entry_cannot_be_moved() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "x").unwrap();
        delete(&app, &e["id"]).unwrap();
        assert_eq!(
            move_to(&app, &e["id"], "2026-10-19").unwrap_err()["code"],
            "unknown_key"
        );
        assert_eq!(
            move_to(&app, &json!("nope"), "2026-10-19").unwrap_err()["code"],
            "unknown_key"
        );
        // Restoring it brings it back where it was, not where the move was attempted.
        assert_eq!(restore(&app, &e["id"]).unwrap()["page_date"], "2026-10-20");
    }

    #[test]
    fn future_pages_come_before_today_and_cursor_paging_still_works() {
        let app = TestApp::new();
        for d in ["2026-10-20", "2026-12-25", "2026-10-19", "2027-01-01"] {
            create(&app, d, "x").unwrap();
        }
        let dates = |p: Vec<Value>| -> Vec<String> {
            p.iter()
                .map(|p| p["date"].as_str().unwrap().into())
                .collect()
        };
        assert_eq!(
            dates(pages(&app, None, 10)),
            ["2027-01-01", "2026-12-25", "2026-10-20", "2026-10-19"]
        );
        assert_eq!(
            dates(pages(&app, Some("2026-12-25"), 10)),
            ["2026-10-20", "2026-10-19"]
        );
    }

    fn page_dates(listed: &[Value]) -> Vec<&str> {
        listed.iter().map(|p| p["date"].as_str().unwrap()).collect()
    }

    /// A Journal with Pages spread over two years, one of them with two Entries, plus a future one.
    fn spread_journal() -> TestApp {
        let app = TestApp::new();
        for d in [
            "2025-03-01",
            "2025-03-02",
            "2025-03-02",
            "2025-06-10",
            "2026-01-05",
            "2026-10-19",
            "2026-10-20",
            "2026-12-25",
        ] {
            create(&app, d, "x").unwrap();
        }
        app
    }

    #[test]
    fn from_starts_at_the_date_itself_and_goes_older() {
        let app = spread_journal();
        let listed = pages_in(&app, Some("2025-06-10"), 2, "from").unwrap();
        assert_eq!(page_dates(&listed), ["2025-06-10", "2025-03-02"]);
        assert_eq!(listed[1]["entries"].as_array().unwrap().len(), 2);
        // A date with no Page starts at the next older one; no cursor is the newest Pages.
        assert_eq!(
            page_dates(&pages_in(&app, Some("2025-12-31"), 1, "from").unwrap()),
            ["2025-06-10"]
        );
        assert_eq!(
            page_dates(&pages_in(&app, None, 2, "from").unwrap()),
            ["2026-12-25", "2026-10-20"]
        );
    }

    #[test]
    fn older_stays_strictly_before_the_cursor() {
        let app = spread_journal();
        let listed = pages_in(&app, Some("2025-06-10"), 10, "older").unwrap();
        assert_eq!(page_dates(&listed), ["2025-03-02", "2025-03-01"]);
        assert!(
            pages_in(&app, Some("2025-03-01"), 10, "older")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn newer_takes_the_nearest_pages_above_the_cursor_newest_first() {
        let app = spread_journal();
        // The two nearest above 2025-03-02, shown newest first like everything else.
        let listed = pages_in(&app, Some("2025-03-02"), 2, "newer").unwrap();
        assert_eq!(page_dates(&listed), ["2026-01-05", "2025-06-10"]);
        // Future Pages are included, and a short result means the top was reached.
        let top = pages_in(&app, Some("2026-10-19"), 10, "newer").unwrap();
        assert_eq!(page_dates(&top), ["2026-12-25", "2026-10-20"]);
        assert!(
            pages_in(&app, Some("2026-12-25"), 10, "newer")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn newer_pages_keep_entries_oldest_first_and_counts() {
        let app = TestApp::new();
        let a = create(&app, "2026-10-19", "one two").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        let b = create(&app, "2026-10-19", "three").unwrap();
        create(&app, "2026-10-10", "old").unwrap();
        let listed = pages_in(&app, Some("2026-10-10"), 5, "newer").unwrap();
        assert_eq!(listed[0]["entries"], json!([a, b]));
        assert_eq!(listed[0]["entry_count"], 2);
        assert_eq!(listed[0]["word_count"], 3);
    }

    #[test]
    fn windows_chain_without_gaps_or_overlaps() {
        let app = spread_journal();
        // Start in the middle, then extend both ways: together they are the whole Journal.
        let mid = pages_in(&app, Some("2025-06-10"), 2, "from").unwrap();
        let above = pages_in(&app, Some("2025-06-10"), 2, "newer").unwrap();
        let below = pages_in(&app, Some("2025-03-02"), 2, "older").unwrap();
        let all: Vec<&str> = [&above, &mid, &below]
            .into_iter()
            .flat_map(|g| page_dates(g))
            .collect();
        assert_eq!(
            all,
            [
                "2026-10-19",
                "2026-01-05",
                "2025-06-10",
                "2025-03-02",
                "2025-03-01"
            ]
        );
    }

    #[test]
    fn newer_needs_a_cursor_and_bad_directions_or_dates_are_rejected() {
        let app = spread_journal();
        let err = pages_in(&app, None, 5, "newer").unwrap_err();
        assert_eq!(err["code"], "validation");
        assert!(pages_in(&app, Some("nope"), 5, "from").is_err());
        assert!(pages_in(&app, Some("2025-03-01"), 5, "sideways").is_err());
    }

    #[test]
    fn month_days_lists_exactly_the_dates_with_live_entries_in_that_month() {
        let app = TestApp::new();
        let gone = create(&app, "2026-10-12", "gone").unwrap();
        delete(&app, &gone["id"]).unwrap();
        for d in [
            "2026-09-30",
            "2026-10-01",
            "2026-10-05",
            "2026-10-05",
            "2026-10-31",
            "2026-11-01",
            "2027-10-05",
        ] {
            create(&app, d, "x").unwrap();
        }
        let both = create(&app, "2026-10-20", "x").unwrap();
        create(&app, "2026-10-20", "y").unwrap();
        assert_eq!(
            month_days(&app, 2026, 10),
            ["2026-10-01", "2026-10-05", "2026-10-20", "2026-10-31"]
        );
        // Deleting one of two Entries on a day keeps the day; a move shifts it.
        delete(&app, &both["id"]).unwrap();
        assert!(month_days(&app, 2026, 10).contains(&"2026-10-20".to_owned()));
        assert!(month_days(&app, 2026, 12).is_empty());
        for (y, m) in [(2026, 0), (2026, 13), (0, 1)] {
            let err = app
                .invoke::<Value>("journal_month_days", json!({ "year": y, "month": m }))
                .unwrap_err();
            assert_eq!(err["code"], "validation", "{y}-{m}");
        }
    }

    fn search(app: &TestApp, query: &str, from: Option<&str>, to: Option<&str>) -> Value {
        app.invoke(
            "journal_search",
            json!({ "query": query, "from": from, "to": to }),
        )
        .unwrap()
    }

    /// Ids of a group of Hits mapped back to the text they were created with.
    fn found(app: &TestApp, query: &str, group: &str) -> Vec<String> {
        let result = search(app, query, None, None);
        result[group]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| entry_text(app, h["id"].as_str().unwrap()))
            .collect()
    }

    fn entry_text(app: &TestApp, id: &str) -> String {
        app.with_db(|c| {
            c.query_row(
                "SELECT body FROM journal_entries WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap()
        })
    }

    #[test]
    fn matching_ignores_case_and_accents_both_ways() {
        let app = TestApp::new();
        create(&app, "2026-10-20", "Coffee at the Café").unwrap();
        create(&app, "2026-10-19", "plain cafe visit").unwrap();
        assert_eq!(found(&app, "CAFE", "exact").len(), 2);
        assert_eq!(found(&app, "café", "exact").len(), 2);
    }

    #[test]
    fn words_match_by_prefix_and_every_word_is_required_in_any_order() {
        let app = TestApp::new();
        create(&app, "2026-10-20", "my journal entry").unwrap();
        create(&app, "2026-10-19", "journal only").unwrap();
        assert_eq!(found(&app, "jour", "exact").len(), 2);
        assert_eq!(found(&app, "entry journal", "exact"), ["my journal entry"]);
        assert!(found(&app, "journal zebra", "exact").is_empty());
    }

    #[test]
    fn quoted_text_is_an_exact_phrase() {
        let app = TestApp::new();
        create(&app, "2026-10-20", "the red door opened").unwrap();
        create(&app, "2026-10-19", "a door that was red").unwrap();
        assert_eq!(
            found(&app, "\"red door\"", "exact"),
            ["the red door opened"]
        );
        assert_eq!(found(&app, "red door", "exact").len(), 2);
        // A quoted word is whole: no prefix matching inside quotes.
        assert!(found(&app, "\"do\"", "exact").is_empty());
    }

    #[test]
    fn results_are_newest_page_first_then_oldest_written_first() {
        let app = TestApp::new();
        create(&app, "2026-10-19", "note old").unwrap();
        create(&app, "2026-10-20", "note a").unwrap();
        create(&app, "2026-10-20", "note b").unwrap();
        create(&app, "2027-01-01", "note future").unwrap();
        assert_eq!(
            found(&app, "note", "exact"),
            ["note future", "note a", "note b", "note old"]
        );
    }

    #[test]
    fn a_range_narrows_words_and_alone_lists_every_entry_in_it() {
        let app = TestApp::new();
        for (d, t) in [
            ("2026-10-01", "walk one"),
            ("2026-10-10", "walk two"),
            ("2026-10-10", "swim"),
            ("2026-10-20", "walk three"),
        ] {
            create(&app, d, t).unwrap();
        }
        let with_words = search(&app, "walk", Some("2026-10-10"), Some("2026-10-20"));
        assert_eq!(with_words["exact"].as_array().unwrap().len(), 2);
        let alone = search(&app, "  ", Some("2026-10-10"), Some("2026-10-10"));
        assert_eq!(alone["exact"].as_array().unwrap().len(), 2);
        assert!(alone["similar"].as_array().unwrap().is_empty());
        let open_ended = search(&app, "", Some("2026-10-10"), None);
        assert_eq!(open_ended["exact"].as_array().unwrap().len(), 3);
        // Nothing to search for and no range finds nothing.
        assert!(
            search(&app, "", None, None)["exact"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_backwards_or_invalid_range_is_rejected() {
        let app = TestApp::new();
        for (from, to) in [("2026-10-20", "2026-10-10"), ("2026-02-30", "2026-03-01")] {
            let err = app
                .invoke::<Value>(
                    "journal_search",
                    json!({ "query": "x", "from": from, "to": to }),
                )
                .unwrap_err();
            assert_eq!(err["code"], "validation");
        }
    }

    #[test]
    fn typos_find_similar_entries_by_word_length_and_never_repeat_exact() {
        let app = TestApp::new();
        create(&app, "2026-10-20", "my journal today").unwrap();
        create(&app, "2026-10-19", "journals everywhere").unwrap();
        // 6 letters: one typo is fine ("jurnal" -> "journal"); the prefix match of the second
        // Entry is exact for "journal" but not for the typo.
        assert!(found(&app, "jurnal", "exact").is_empty());
        let similar = found(&app, "jurnal", "similar");
        assert_eq!(similar, ["my journal today"]);
        // The exact word finds both exactly and leaves nothing for similar.
        assert_eq!(found(&app, "journal", "exact").len(), 2);
        assert!(found(&app, "journal", "similar").is_empty());
    }

    #[test]
    fn tolerance_is_one_typo_for_four_to_seven_letters_and_two_for_eight_plus() {
        let app = TestApp::new();
        create(&app, "2026-10-20", "meeting wonderful").unwrap();
        // 7 letters, two typos: nothing.
        assert!(found(&app, "mxxting", "similar").is_empty());
        // 7 letters, one typo.
        assert_eq!(found(&app, "meetng", "similar").len(), 1);
        // 9 letters, two typos.
        assert_eq!(found(&app, "wnderful", "similar").len(), 1);
        assert_eq!(found(&app, "wxnderfxl", "similar").len(), 1);
        // 9 letters, three typos.
        assert!(found(&app, "wxnderfxx", "similar").is_empty());
    }

    #[test]
    fn short_words_are_never_fuzzy() {
        let app = TestApp::new();
        create(&app, "2026-10-20", "the cat sat").unwrap();
        for typo in ["cot", "cta", "sit"] {
            assert!(found(&app, typo, "similar").is_empty(), "{typo}");
        }
    }

    #[test]
    fn similar_needs_every_word_and_phrases_stay_exact() {
        let app = TestApp::new();
        create(&app, "2026-10-20", "journal about gardens").unwrap();
        assert_eq!(found(&app, "jurnal gardens", "similar").len(), 1);
        assert!(found(&app, "jurnal zebra", "similar").is_empty());
        assert!(found(&app, "\"jurnal about\"", "similar").is_empty());
    }

    #[test]
    fn range_applies_to_similar_too() {
        let app = TestApp::new();
        create(&app, "2026-10-01", "old journal").unwrap();
        create(&app, "2026-10-20", "new journal").unwrap();
        let r = search(&app, "jurnal", Some("2026-10-10"), None);
        assert_eq!(r["similar"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn deleted_entries_are_never_returned_and_come_back_with_restore() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "secret journal").unwrap();
        delete(&app, &e["id"]).unwrap();
        for q in ["secret", "sekret", "journal"] {
            let r = search(&app, q, None, None);
            assert!(r["exact"].as_array().unwrap().is_empty(), "{q}");
            assert!(r["similar"].as_array().unwrap().is_empty(), "{q}");
        }
        assert!(
            search(&app, "", Some("2026-10-01"), Some("2026-10-31"))["exact"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        restore(&app, &e["id"]).unwrap();
        assert_eq!(found(&app, "secret", "exact").len(), 1);
    }

    #[test]
    fn snippets_mark_the_matches_as_plain_spans() {
        let app = TestApp::new();
        create(&app, "2026-10-20", "Coffee at the <b>Café</b> today").unwrap();
        let r = search(&app, "cafe", None, None);
        let hit = &r["exact"][0];
        assert_eq!(hit["page_date"], "2026-10-20");
        assert_eq!(hit["written_tz"], "Asia/Kolkata");
        assert_eq!(hit["written_offset_min"], 330);
        let spans = hit["snippet"].as_array().unwrap();
        let marked: Vec<&str> = spans
            .iter()
            .filter(|s| s["matched"] == true)
            .map(|s| s["text"].as_str().unwrap())
            .collect();
        assert_eq!(marked, ["Café"]);
        // Markup in the text stays literal text.
        assert_eq!(spans[0]["text"], "Coffee at the <b>");
        let sim = search(&app, "cafee", None, None);
        let marked: Vec<&str> = sim["similar"][0]["snippet"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["matched"] == true)
            .map(|s| s["text"].as_str().unwrap())
            .collect();
        assert_eq!(marked, ["Café"]);
    }

    #[test]
    fn fts_syntax_in_the_query_is_plain_text_and_never_errors() {
        let app = TestApp::new();
        create(&app, "2026-10-20", "cats and dogs, near the body").unwrap();
        for q in [
            "\"",
            "\"\"\"",
            "*",
            "-",
            "( ",
            "cats OR",
            "NOT dogs",
            "body:cats",
            "cats AND dogs",
            "NEAR(cats dogs)",
            "^cats",
            "cats*",
            "{body}: x",
            "a\"b",
            "'; DROP TABLE x; --",
        ] {
            let r: Result<Value, Value> = app.invoke(
                "journal_search",
                json!({ "query": q, "from": null, "to": null }),
            );
            assert!(r.is_ok(), "{q}: {r:?}");
        }
        // Operators are just words: "cats AND dogs" needs the word "and" too, which it has.
        assert_eq!(found(&app, "cats AND dogs", "exact").len(), 1);
        assert!(found(&app, "cats OR zebra", "exact").is_empty());
        assert!(found(&app, "body:zebra", "exact").is_empty());
    }

    #[test]
    fn edit_move_and_toggle_keep_the_index_in_sync() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "- [ ] apple pie").unwrap();
        assert_eq!(found(&app, "apple", "exact").len(), 1);
        update(&app, &e["id"], "banana split", "s2");
        assert!(found(&app, "apple", "exact").is_empty());
        assert_eq!(found(&app, "banana", "exact").len(), 1);
        move_to(&app, &e["id"], "2026-01-02").unwrap();
        let r = search(&app, "banana", None, None);
        assert_eq!(r["exact"][0]["page_date"], "2026-01-02");
        assert!(
            search(&app, "banana", Some("2026-10-01"), None)["exact"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let t = create(&app, "2026-10-20", "- [ ] tulip").unwrap();
        toggle(&app, &t["id"], 0).unwrap();
        assert_eq!(found(&app, "tulip", "exact"), ["- [x] tulip"]);
    }

    #[test]
    fn purging_removes_the_entry_from_the_index_and_vocabulary() {
        let app = TestApp::new();
        let e = create(&app, "2026-10-20", "ephemeral thought").unwrap();
        delete(&app, &e["id"]).unwrap();
        deleted_days_ago(&app, &e["id"], 31);
        app.with_db_mut(|c| mnemora_journal::purge_deleted(c).unwrap());
        let terms: i64 = app.with_db(|c| {
            c.query_row("SELECT COUNT(*) FROM journal_fts_vocab", [], |r| r.get(0))
                .unwrap()
        });
        assert_eq!(terms, 0);
    }

    fn spans(app: &TestApp, query: &str) -> Vec<(String, bool)> {
        search(app, query, None, None)["exact"][0]["snippet"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| (s["text"].as_str().unwrap().to_owned(), s["matched"] == true))
            .collect()
    }

    #[test]
    fn snippets_are_plain_text_with_the_match_marked() {
        let app = TestApp::new();
        create(
            &app,
            "2026-10-20",
            "# A morning plan\n\nSome **bold** words and `code`.\n- [ ] Call the dentist\n- [x] Reply",
        )
        .unwrap();
        let all: String = spans(&app, "bold").iter().map(|s| s.0.as_str()).collect();
        assert_eq!(
            all,
            "A morning plan · Some bold words and code. · Call the dentist · Reply"
        );
        assert!(spans(&app, "bold").contains(&("bold".to_owned(), true)));
        assert!(spans(&app, "dentist").contains(&("dentist".to_owned(), true)));
    }

    #[test]
    fn a_match_in_link_text_is_marked_and_the_url_is_not_shown() {
        let app = TestApp::new();
        create(
            &app,
            "2026-10-20",
            "see [my site](https://example.com/x) or https://bare.io",
        )
        .unwrap();
        let s = spans(&app, "site");
        assert!(s.contains(&("site".to_owned(), true)));
        let all: String = s.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(all, "see my site or https://bare.io");
    }

    fn today(app: &TestApp, date: &str) -> Result<Value, Value> {
        app.invoke("journal_today", json!({ "today": date }))
    }

    #[test]
    fn an_empty_day_has_no_count_and_no_latest() {
        let app = TestApp::new();
        create(&app, "2026-10-19", "yesterday").unwrap();
        let t = today(&app, "2026-10-20").unwrap();
        assert_eq!(t["count"], 0);
        assert_eq!(t["latest"], Value::Null);
    }

    #[test]
    fn today_counts_live_entries_and_the_latest_is_the_last_written() {
        let app = TestApp::new();
        create(&app, "2026-10-20", "first").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        let last = create(&app, "2026-10-20", "second").unwrap();
        create(&app, "2026-10-19", "other day").unwrap();
        let t = today(&app, "2026-10-20").unwrap();
        assert_eq!(t["count"], 2);
        assert_eq!(t["latest"]["id"], last["id"]);
        assert_eq!(t["latest"]["snippet"], "second");
        assert_eq!(t["latest"]["written_at"], last["written_at"]);
    }

    #[test]
    fn a_deleted_entry_leaves_the_summary_and_comes_back_with_undo() {
        let app = TestApp::new();
        let a = create(&app, "2026-10-20", "keep").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        let b = create(&app, "2026-10-20", "drop").unwrap();
        delete(&app, &b["id"]).unwrap();
        let t = today(&app, "2026-10-20").unwrap();
        assert_eq!(
            (t["count"].clone(), t["latest"]["id"].clone()),
            (json!(1), a["id"].clone())
        );
        let _: Value = app
            .invoke("journal_entry_restore", json!({ "id": b["id"] }))
            .unwrap();
        assert_eq!(today(&app, "2026-10-20").unwrap()["count"], 2);
    }

    #[test]
    fn the_today_snippet_is_plain_text_and_cut_short() {
        let app = TestApp::new();
        create(
            &app,
            "2026-10-20",
            "# Plan\n\nSome **bold** and [a link](https://x.io)",
        )
        .unwrap();
        assert_eq!(
            today(&app, "2026-10-20").unwrap()["latest"]["snippet"],
            "Plan · Some bold and a link"
        );
        create(&app, "2026-10-21", &"word ".repeat(100)).unwrap();
        let snippet = today(&app, "2026-10-21").unwrap()["latest"]["snippet"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(snippet.ends_with('…') && snippet.chars().count() <= 141);
    }

    #[test]
    fn today_rejects_a_bad_date() {
        let app = TestApp::new();
        assert_eq!(today(&app, "yesterday").unwrap_err()["code"], "validation");
    }

    fn save_pref(app: &TestApp, key: &str, value: Value) -> Result<(), Value> {
        app.invoke(
            "settings_save",
            json!({ "changes": [{ "key": key, "value": value }] }),
        )
    }

    #[test]
    fn journal_preferences_have_defaults_and_accept_only_allowed_values() {
        let app = TestApp::new();
        let snap: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(snap["values"]["journal.font"], "serif");
        assert_eq!(snap["values"]["journal.width"], "default");
        assert_eq!(snap["values"]["journal.page_summary"], true);

        save_pref(&app, "journal.font", json!("sans")).unwrap();
        save_pref(&app, "journal.width", json!("wide")).unwrap();
        save_pref(&app, "journal.page_summary", json!(false)).unwrap();
        let snap: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(snap["values"]["journal.font"], "sans");
        assert_eq!(snap["values"]["journal.width"], "wide");
        assert_eq!(snap["values"]["journal.page_summary"], false);

        for (key, bad) in [
            ("journal.font", json!("mono")),
            ("journal.font", json!(1)),
            ("journal.width", json!("huge")),
            ("journal.page_summary", json!("yes")),
        ] {
            let err = save_pref(&app, key, bad).unwrap_err();
            assert_eq!(err["code"], "validation", "{key}");
        }
        let snap: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(snap["values"]["journal.font"], "sans");
    }
}
