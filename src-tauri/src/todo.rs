//! Todo commands. Thin: the rules live in `mnemora-todo`.

use mnemora_core::error::AppError;
use mnemora_todo::{InboxView, Task};
use tauri::State;

use crate::state::{AppState, lock};

/// The Inbox: open Tasks that belong to no List, in manual order (new Tasks last).
#[tauri::command]
#[specta::specta]
pub fn todo_inbox(state: State<'_, AppState>) -> Result<InboxView, AppError> {
    mnemora_todo::inbox(&lock(&state.db))
}

/// Adds a Task to the bottom of the Inbox. Rejects a blank title, one over 500 characters and
/// one with a line break.
#[tauri::command]
#[specta::specta]
pub fn todo_task_create(state: State<'_, AppState>, title: String) -> Result<Task, AppError> {
    mnemora_todo::create_task(&mut lock(&state.db), &title)
}

/// Marks an open Task done, now.
#[tauri::command]
#[specta::specta]
pub fn todo_task_complete(state: State<'_, AppState>, id: String) -> Result<Task, AppError> {
    mnemora_todo::complete_task(&mut lock(&state.db), &id)
}

/// Brings a done Task back to open, in its old place.
#[tauri::command]
#[specta::specta]
pub fn todo_task_reopen(state: State<'_, AppState>, id: String) -> Result<Task, AppError> {
    mnemora_todo::reopen_task(&mut lock(&state.db), &id)
}

#[cfg(test)]
mod tests {
    use mnemora_core::db;
    use mnemora_core::module::{CoreModule, Module};
    use mnemora_journal::{JournalModule, NewEntry};
    use mnemora_todo::TodoModule;
    use serde_json::{Value, json};

    use crate::test_support::TestApp;

    fn create(app: &TestApp, title: &str) -> Result<Value, Value> {
        app.invoke("todo_task_create", json!({ "title": title }))
    }

    fn inbox_titles(app: &TestApp) -> Vec<String> {
        let view: Value = app.invoke("todo_inbox", json!({})).unwrap();
        view["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap().to_owned())
            .collect()
    }

    fn id_of(task: &Value) -> Value {
        task["id"].clone()
    }

    /// `(op, entity, state in the snapshot)` of every Todo change, oldest first.
    fn log(app: &TestApp) -> Vec<(String, String, String)> {
        app.with_db(|c| {
            let mut stmt = c
                .prepare(
                    "SELECT op, entity, json_extract(snapshot, '$.state') FROM change_log
                     WHERE module = 'todo' ORDER BY at, rowid",
                )
                .unwrap();
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        })
    }

    #[test]
    fn an_empty_inbox_is_empty() {
        let app = TestApp::new();
        assert!(inbox_titles(&app).is_empty());
    }

    #[test]
    fn a_new_task_is_open_in_the_inbox_with_no_dates() {
        let app = TestApp::new();
        let task = create(&app, "  Buy oat milk ").unwrap();
        assert_eq!(task["title"], "Buy oat milk");
        assert_eq!(task["state"], "open");
        assert_eq!(task["list_id"], Value::Null);
        assert_eq!(task["closed_at"], Value::Null);
        assert_eq!(task["due_date"], Value::Null);
        assert_eq!(task["priority"], 4);
        assert_eq!(inbox_titles(&app), ["Buy oat milk"]);
    }

    #[test]
    fn blank_overlong_and_multiline_titles_are_rejected_and_nothing_is_stored() {
        let app = TestApp::new();
        let too_long = "x".repeat(501);
        for bad in [
            "",
            "   ",
            "\n\t \n",
            "two\nlines",
            "two\r\nlines",
            &too_long,
        ] {
            let err = create(&app, bad).unwrap_err();
            assert_eq!(err["code"], "validation", "{bad:?}");
        }
        assert!(inbox_titles(&app).is_empty());
        assert!(log(&app).is_empty());
    }

    #[test]
    fn a_title_of_exactly_500_characters_is_accepted() {
        let app = TestApp::new();
        assert!(create(&app, &"é".repeat(500)).is_ok());
    }

    #[test]
    fn new_tasks_go_to_the_bottom_in_creation_order() {
        let app = TestApp::new();
        for t in ["one", "two", "three", "four"] {
            create(&app, t).unwrap();
        }
        assert_eq!(inbox_titles(&app), ["one", "two", "three", "four"]);
    }

    #[test]
    fn completing_sets_done_and_the_done_time_and_leaves_the_inbox() {
        let app = TestApp::new();
        let a = create(&app, "a").unwrap();
        create(&app, "b").unwrap();
        let done: Value = app
            .invoke("todo_task_complete", json!({ "id": id_of(&a) }))
            .unwrap();
        assert_eq!(done["state"], "done");
        assert!(done["closed_at"].as_i64().unwrap() >= a["created_at"].as_i64().unwrap());
        assert_eq!(inbox_titles(&app), ["b"]);
    }

    #[test]
    fn reopening_returns_the_task_to_open_in_its_old_place() {
        let app = TestApp::new();
        let a = create(&app, "a").unwrap();
        create(&app, "b").unwrap();
        app.invoke::<Value>("todo_task_complete", json!({ "id": id_of(&a) }))
            .unwrap();
        let back: Value = app
            .invoke("todo_task_reopen", json!({ "id": id_of(&a) }))
            .unwrap();
        assert_eq!(back["state"], "open");
        assert_eq!(back["closed_at"], Value::Null);
        assert_eq!(inbox_titles(&app), ["a", "b"]);
    }

    #[test]
    fn closing_twice_reopening_an_open_task_and_unknown_ids_are_rejected() {
        let app = TestApp::new();
        let a = create(&app, "a").unwrap();
        let id = id_of(&a);
        let err = app
            .invoke::<Value>("todo_task_reopen", json!({ "id": id }))
            .unwrap_err();
        assert_eq!(err["code"], "validation");
        app.invoke::<Value>("todo_task_complete", json!({ "id": id }))
            .unwrap();
        let err = app
            .invoke::<Value>("todo_task_complete", json!({ "id": id }))
            .unwrap_err();
        assert_eq!(err["code"], "validation");
        for cmd in ["todo_task_complete", "todo_task_reopen"] {
            let err = app
                .invoke::<Value>(cmd, json!({ "id": "nope" }))
                .unwrap_err();
            assert_eq!(err["code"], "not_found", "{cmd}");
        }
    }

    #[test]
    fn create_and_tick_are_recorded_in_the_change_log() {
        let app = TestApp::new();
        let a = create(&app, "a").unwrap();
        let id = id_of(&a);
        app.invoke::<Value>("todo_task_complete", json!({ "id": id }))
            .unwrap();
        app.invoke::<Value>("todo_task_reopen", json!({ "id": id }))
            .unwrap();
        let entity = "todo_task".to_owned();
        assert_eq!(
            log(&app),
            [
                ("create".to_owned(), entity.clone(), "open".to_owned()),
                ("tick".to_owned(), entity.clone(), "done".to_owned()),
                ("tick".to_owned(), entity, "open".to_owned()),
            ]
        );
    }

    #[test]
    fn rejected_writes_leave_no_change_log_record() {
        let app = TestApp::new();
        let a = create(&app, "a").unwrap();
        let _ = create(&app, " ");
        let _ = app.invoke::<Value>("todo_task_reopen", json!({ "id": id_of(&a) }));
        let _ = app.invoke::<Value>("todo_task_complete", json!({ "id": "nope" }));
        assert_eq!(log(&app).len(), 1);
    }

    #[test]
    fn the_full_schema_exists_and_search_indexes_follow_the_text() {
        let app = TestApp::new();
        create(&app, "Café run").unwrap();
        app.with_db(|c| {
            for table in [
                "todo_lists",
                "todo_tasks",
                "todo_comments",
                "todo_tasks_fts",
                "todo_comments_fts",
            ] {
                c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap_or_else(|e| panic!("{table}: {e}"));
            }
            let hits: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM todo_tasks_fts WHERE todo_tasks_fts MATCH '\"cafe\"*'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(hits, 1, "prefix search ignores accents, like the Journal");
        });
    }

    #[test]
    fn migrating_a_database_that_holds_journal_data_keeps_it_and_backs_it_up() {
        let dir = tempfile::tempdir().unwrap();
        let (db_file, backups) = (dir.path().join("mnemora.db"), dir.path().join("backups"));
        let before: Vec<&dyn Module> = vec![&CoreModule, &JournalModule];
        let after: Vec<&dyn Module> = vec![&CoreModule, &JournalModule, &TodoModule];

        let mut conn = db::open(&db_file).unwrap();
        db::migrate::run(&mut conn, &before, &backups).unwrap();
        mnemora_journal::create_entry(
            &mut conn,
            NewEntry {
                page_date: "2026-10-20".into(),
                text: "Written before Todo existed".into(),
                written_tz: "Asia/Kolkata".into(),
                written_offset_min: 330,
                session_id: "s1".into(),
            },
        )
        .unwrap();

        let report = db::migrate::run(&mut conn, &after, &backups).unwrap();
        assert!(report.backup.is_some(), "existing data is backed up first");
        assert_eq!(report.applied.len(), 1);
        assert_eq!(report.applied[0].0, "todo");

        let pages =
            mnemora_journal::list_pages(&conn, None, 10, mnemora_journal::PageDirection::Older)
                .unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].entries[0].body, "Written before Todo existed");
        mnemora_todo::create_task(&mut conn, "Works after the upgrade").unwrap();
        assert_eq!(mnemora_todo::inbox(&conn).unwrap().tasks.len(), 1);

        let again = db::migrate::run(&mut conn, &after, &backups).unwrap();
        assert!(again.applied.is_empty(), "a second run changes nothing");
    }
}
