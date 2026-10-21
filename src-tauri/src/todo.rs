//! Todo commands. Thin: the rules live in `mnemora-todo`.

use mnemora_core::error::AppError;
use mnemora_todo::{
    AllView, DayCount, DeleteChoice, DueChange, InboxView, ListDeletion, ListView, LocalNow,
    NewWhen, OverdueMove, SidebarView, Task, TodayView, TodoList, UpcomingView,
};
use tauri::State;

use crate::state::{AppState, lock};

/// The Inbox: open Tasks that belong to no List, in manual order (new Tasks last).
#[tauri::command]
#[specta::specta]
pub fn todo_inbox(state: State<'_, AppState>) -> Result<InboxView, AppError> {
    mnemora_todo::inbox(&lock(&state.db))
}

/// Adds a Task to the bottom of a List, or of the Inbox when `list_id` is none. Rejects a blank
/// title, one over 500 characters and one with a line break. `when` gives it a Due date and time
/// under the rules of `todo_task_set_when`.
#[tauri::command]
#[specta::specta]
pub fn todo_task_create(
    state: State<'_, AppState>,
    title: String,
    list_id: Option<String>,
    when: Option<NewWhen>,
) -> Result<Task, AppError> {
    mnemora_todo::create_task(
        &mut lock(&state.db),
        &title,
        list_id.as_deref(),
        when.as_ref(),
    )
}

/// Sets an open Task's Due date and time, or clears them when both are none. A time without a
/// date means today if it is still ahead and tomorrow otherwise. Past dates are refused. `local`
/// is the device's today and now. Records an `update` change.
#[tauri::command]
#[specta::specta]
pub fn todo_task_set_when(
    state: State<'_, AppState>,
    id: String,
    date: Option<String>,
    time: Option<String>,
    local: LocalNow,
) -> Result<Task, AppError> {
    mnemora_todo::set_when(
        &mut lock(&state.db),
        &id,
        date.as_deref(),
        time.as_deref(),
        &local,
    )
}

/// Puts Due dates and times back as they were: Undo for a When change, past dates included.
/// Tasks that no longer exist are skipped. Records an `update` change for each Task changed.
#[tauri::command]
#[specta::specta]
pub fn todo_tasks_restore_when(
    state: State<'_, AppState>,
    changes: Vec<DueChange>,
) -> Result<Vec<Task>, AppError> {
    mnemora_todo::restore_when(&mut lock(&state.db), &changes)
}

/// Moves every Overdue Task to `date` (today or later), keeping times. Returns what
/// `todo_tasks_restore_when` needs to undo it. Records an `update` change for each Task.
#[tauri::command]
#[specta::specta]
pub fn todo_tasks_move_overdue(
    state: State<'_, AppState>,
    date: String,
    local: LocalNow,
) -> Result<OverdueMove, AppError> {
    mnemora_todo::move_overdue(&mut lock(&state.db), &date, &local)
}

/// Today: Overdue first, then today's Tasks (timed by time, then manual order), then the Tasks
/// done today.
#[tauri::command]
#[specta::specta]
pub fn todo_today(state: State<'_, AppState>, local: LocalNow) -> Result<TodayView, AppError> {
    mnemora_todo::today(&lock(&state.db), &local)
}

/// Upcoming: Overdue on top, then each day after today that has Tasks.
#[tauri::command]
#[specta::specta]
pub fn todo_upcoming(
    state: State<'_, AppState>,
    local: LocalNow,
) -> Result<UpcomingView, AppError> {
    mnemora_todo::upcoming(&lock(&state.db), &local)
}

/// Open Tasks per date from `from` to `to`, both included (only dates that have any), leaving
/// out the Task `exclude`: the dots in the When picker.
#[tauri::command]
#[specta::specta]
pub fn todo_day_counts(
    state: State<'_, AppState>,
    from: String,
    to: String,
    exclude: Option<String>,
) -> Result<Vec<DayCount>, AppError> {
    mnemora_todo::day_counts(&lock(&state.db), &from, &to, exclude.as_deref())
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

/// Puts a Task in a List, or in the Inbox when `list_id` is none. Undo moves it back. Records
/// a `move` change.
#[tauri::command]
#[specta::specta]
pub fn todo_task_move(
    state: State<'_, AppState>,
    id: String,
    list_id: Option<String>,
) -> Result<Task, AppError> {
    mnemora_todo::move_task(&mut lock(&state.db), &id, list_id.as_deref())
}

/// The sidebar's data: the Inbox's open count, Today's count (due today plus Overdue, with the
/// Overdue part apart) and every List, in order, with its open count.
#[tauri::command]
#[specta::specta]
pub fn todo_lists(state: State<'_, AppState>, local: LocalNow) -> Result<SidebarView, AppError> {
    mnemora_todo::sidebar(&lock(&state.db), &local)
}

/// One List and its open Tasks in manual order.
#[tauri::command]
#[specta::specta]
pub fn todo_list(state: State<'_, AppState>, id: String) -> Result<ListView, AppError> {
    mnemora_todo::list_view(&lock(&state.db), &id)
}

/// Open Tasks grouped by List, the Inbox first. Empty groups are left out.
#[tauri::command]
#[specta::specta]
pub fn todo_all(state: State<'_, AppState>) -> Result<AllView, AppError> {
    mnemora_todo::all(&lock(&state.db))
}

/// Adds a List at the bottom. Names are unique ignoring case and accents, and at most 60
/// characters.
#[tauri::command]
#[specta::specta]
pub fn todo_list_create(state: State<'_, AppState>, name: String) -> Result<TodoList, AppError> {
    mnemora_todo::create_list(&mut lock(&state.db), &name)
}

/// Renames a List, under the same rules as creating one.
#[tauri::command]
#[specta::specta]
pub fn todo_list_rename(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<TodoList, AppError> {
    mnemora_todo::rename_list(&mut lock(&state.db), &id, &name)
}

/// Sets a List's emoji; none or blank goes back to the neutral icon.
#[tauri::command]
#[specta::specta]
pub fn todo_list_set_emoji(
    state: State<'_, AppState>,
    id: String,
    emoji: Option<String>,
) -> Result<TodoList, AppError> {
    mnemora_todo::set_list_emoji(&mut lock(&state.db), &id, emoji.as_deref())
}

/// Deletes a List. Its Tasks are deleted too or moved to the Inbox or another List, as `choice`
/// says. Returns what `todo_list_restore` needs to undo it.
#[tauri::command]
#[specta::specta]
pub fn todo_list_delete(
    state: State<'_, AppState>,
    id: String,
    choice: DeleteChoice,
) -> Result<ListDeletion, AppError> {
    mnemora_todo::delete_list(&mut lock(&state.db), &id, &choice)
}

/// Undoes `todo_list_delete`: the List, its deleted Tasks and its moved Tasks all return.
#[tauri::command]
#[specta::specta]
pub fn todo_list_restore(
    state: State<'_, AppState>,
    deletion: ListDeletion,
) -> Result<TodoList, AppError> {
    mnemora_todo::restore_list(&mut lock(&state.db), &deletion)
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
        mnemora_todo::create_task(&mut conn, "Works after the upgrade", None, None).unwrap();
        assert_eq!(mnemora_todo::inbox(&conn).unwrap().tasks.len(), 1);

        let again = db::migrate::run(&mut conn, &after, &backups).unwrap();
        assert!(again.applied.is_empty(), "a second run changes nothing");
    }

    fn new_list(app: &TestApp, name: &str) -> Value {
        app.invoke("todo_list_create", json!({ "name": name }))
            .unwrap()
    }

    /// `(op, entity)` of every Todo change from the n-th on, oldest first.
    fn ops_since(app: &TestApp, skip: usize) -> Vec<(String, String)> {
        app.with_db(|c| {
            let mut stmt = c
                .prepare(
                    "SELECT op, entity FROM change_log WHERE module = 'todo' ORDER BY at, rowid",
                )
                .unwrap();
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .skip(skip)
                .map(Result::unwrap)
                .collect()
        })
    }

    fn pair(op: &str, entity: &str) -> (String, String) {
        (op.to_owned(), entity.to_owned())
    }

    #[test]
    fn a_new_list_has_no_emoji_and_is_recorded_as_a_todo_list_create() {
        let app = TestApp::new();
        let list = new_list(&app, "  Home ");
        assert_eq!(list["name"], "Home");
        assert_eq!(list["emoji"], Value::Null);
        assert_eq!(ops_since(&app, 0), [pair("create", "todo_list")]);
        let err = app
            .invoke::<Value>("todo_list_create", json!({ "name": "home" }))
            .unwrap_err();
        assert_eq!(err["code"], "validation");
        assert_eq!(ops_since(&app, 0).len(), 1, "a refused write logs nothing");
    }

    #[test]
    fn rename_and_emoji_are_updates_and_unchanged_values_write_nothing() {
        let app = TestApp::new();
        let list = new_list(&app, "Home");
        let id = id_of(&list);
        app.invoke::<Value>("todo_list_rename", json!({ "id": id, "name": "House" }))
            .unwrap();
        app.invoke::<Value>("todo_list_rename", json!({ "id": id, "name": "House" }))
            .unwrap();
        let set: Value = app
            .invoke("todo_list_set_emoji", json!({ "id": id, "emoji": " 🏠 " }))
            .unwrap();
        assert_eq!(set["emoji"], "🏠");
        app.invoke::<Value>("todo_list_set_emoji", json!({ "id": id, "emoji": "🏠" }))
            .unwrap();
        let cleared: Value = app
            .invoke("todo_list_set_emoji", json!({ "id": id, "emoji": null }))
            .unwrap();
        assert_eq!(cleared["emoji"], Value::Null);
        assert_eq!(
            ops_since(&app, 0),
            [
                pair("create", "todo_list"),
                pair("update", "todo_list"),
                pair("update", "todo_list"),
                pair("update", "todo_list"),
            ]
        );
    }

    #[test]
    fn the_sidebar_and_list_view_show_lists_in_order_with_open_counts() {
        let app = TestApp::new();
        let home = new_list(&app, "Home");
        new_list(&app, "Work");
        create(&app, "inbox one").unwrap();
        for t in ["a", "b"] {
            app.invoke::<Value>(
                "todo_task_create",
                json!({ "title": t, "listId": id_of(&home) }),
            )
            .unwrap();
        }
        let sidebar: Value = app
            .invoke(
                "todo_lists",
                json!({ "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
            )
            .unwrap();
        assert_eq!(sidebar["inbox_count"], 1);
        let names: Vec<_> = sidebar["lists"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (e["list"]["name"].clone(), e["open_count"].clone()))
            .collect();
        assert_eq!(
            names,
            [(json!("Home"), json!(2)), (json!("Work"), json!(0))]
        );

        let view: Value = app
            .invoke("todo_list", json!({ "id": id_of(&home) }))
            .unwrap();
        assert_eq!(view["list"]["name"], "Home");
        let titles: Vec<_> = view["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].clone())
            .collect();
        assert_eq!(titles, [json!("a"), json!("b")]);
        let err = app
            .invoke::<Value>("todo_list", json!({ "id": "nope" }))
            .unwrap_err();
        assert_eq!(err["code"], "not_found");
    }

    #[test]
    fn moving_a_task_records_a_move_and_an_unchanged_move_records_nothing() {
        let app = TestApp::new();
        let home = new_list(&app, "Home");
        let a = create(&app, "a").unwrap();
        let before = ops_since(&app, 0).len();
        let moved: Value = app
            .invoke(
                "todo_task_move",
                json!({ "id": id_of(&a), "listId": id_of(&home) }),
            )
            .unwrap();
        assert_eq!(moved["list_id"], home["id"]);
        app.invoke::<Value>(
            "todo_task_move",
            json!({ "id": id_of(&a), "listId": id_of(&home) }),
        )
        .unwrap();
        let back: Value = app
            .invoke("todo_task_move", json!({ "id": id_of(&a), "listId": null }))
            .unwrap();
        assert_eq!(back["list_id"], Value::Null);
        assert_eq!(
            ops_since(&app, before),
            [pair("move", "todo_task"), pair("move", "todo_task")]
        );
    }

    #[test]
    fn deleting_with_the_tasks_soft_deletes_them_and_restore_undoes_it() {
        let app = TestApp::new();
        let home = new_list(&app, "Home");
        for t in ["a", "b"] {
            app.invoke::<Value>(
                "todo_task_create",
                json!({ "title": t, "listId": id_of(&home) }),
            )
            .unwrap();
        }
        let before = ops_since(&app, 0).len();
        let deletion: Value = app
            .invoke(
                "todo_list_delete",
                json!({ "id": id_of(&home), "choice": { "kind": "delete_tasks" } }),
            )
            .unwrap();
        assert_eq!(deletion["deleted_task_ids"].as_array().unwrap().len(), 2);
        assert_eq!(deletion["moved_task_ids"].as_array().unwrap().len(), 0);
        // Soft delete: the rows stay, flagged, so Undo and the 30-day purge can find them.
        app.with_db(|c| {
            let n: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM todo_tasks WHERE deleted_at IS NOT NULL",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 2);
        });
        app.invoke::<Value>("todo_list_restore", json!({ "deletion": deletion }))
            .unwrap();
        let view: Value = app
            .invoke("todo_list", json!({ "id": id_of(&home) }))
            .unwrap();
        assert_eq!(view["tasks"].as_array().unwrap().len(), 2);
        assert_eq!(
            ops_since(&app, before),
            [
                pair("delete", "todo_task"),
                pair("delete", "todo_task"),
                pair("delete", "todo_list"),
                pair("restore", "todo_list"),
                pair("restore", "todo_task"),
                pair("restore", "todo_task"),
            ]
        );
    }

    #[test]
    fn moving_the_tasks_out_records_moves_and_restore_moves_them_back() {
        let app = TestApp::new();
        let home = new_list(&app, "Home");
        let work = new_list(&app, "Work");
        let a = app
            .invoke::<Value>(
                "todo_task_create",
                json!({ "title": "a", "listId": id_of(&home) }),
            )
            .unwrap();
        let before = ops_since(&app, 0).len();
        let deletion: Value = app
            .invoke(
                "todo_list_delete",
                json!({
                    "id": id_of(&home),
                    "choice": { "kind": "move_to_list", "list_id": id_of(&work) }
                }),
            )
            .unwrap();
        assert_eq!(deletion["moved_task_ids"], json!([id_of(&a)]));
        let sidebar: Value = app
            .invoke(
                "todo_lists",
                json!({ "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
            )
            .unwrap();
        assert_eq!(sidebar["lists"][0]["open_count"], 1);
        app.invoke::<Value>("todo_list_restore", json!({ "deletion": deletion }))
            .unwrap();
        let sidebar: Value = app
            .invoke(
                "todo_lists",
                json!({ "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
            )
            .unwrap();
        assert_eq!(sidebar["lists"][0]["open_count"], 1);
        assert_eq!(sidebar["lists"][1]["open_count"], 0);
        assert_eq!(
            ops_since(&app, before),
            [
                pair("move", "todo_task"),
                pair("delete", "todo_list"),
                pair("restore", "todo_list"),
                pair("move", "todo_task"),
            ]
        );
    }

    #[test]
    fn a_task_moved_to_a_deleted_list_or_created_in_one_is_refused() {
        let app = TestApp::new();
        let home = new_list(&app, "Home");
        let a = create(&app, "a").unwrap();
        app.invoke::<Value>(
            "todo_list_delete",
            json!({ "id": id_of(&home), "choice": { "kind": "delete_tasks" } }),
        )
        .unwrap();
        let err = app
            .invoke::<Value>(
                "todo_task_move",
                json!({ "id": id_of(&a), "listId": id_of(&home) }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "not_found");
        let err = app
            .invoke::<Value>(
                "todo_task_create",
                json!({ "title": "x", "listId": id_of(&home) }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "not_found");
    }

    fn local(today: &str, now: &str) -> Value {
        json!({ "today": today, "now": now, "offset_min": 0 })
    }

    fn set_when(app: &TestApp, task: &Value, date: Value, time: Value, today: &str) -> Value {
        app.invoke(
            "todo_task_set_when",
            json!({ "id": id_of(task), "date": date, "time": time, "local": local(today, "14:05") }),
        )
        .unwrap()
    }

    /// Dates a Task directly in the database: the only way to make one Overdue is time passing.
    fn backdate(app: &TestApp, task: &Value, date: &str) {
        app.with_db(|c| {
            let id = task["id"].as_str().unwrap();
            c.execute(
                &format!("UPDATE todo_tasks SET due_date = '{date}' WHERE id = '{id}'"),
                [],
            )
            .unwrap();
        });
    }

    #[test]
    fn dating_a_task_records_one_update_and_an_unchanged_when_records_nothing() {
        let app = TestApp::new();
        let a = create(&app, "a").unwrap();
        let before = ops_since(&app, 0).len();
        let dated = set_when(&app, &a, json!("2026-10-23"), json!("09:00"), "2026-10-21");
        assert_eq!(dated["due_date"], "2026-10-23");
        assert_eq!(dated["due_time"], "09:00");
        set_when(&app, &a, json!("2026-10-23"), json!("09:00"), "2026-10-21");
        set_when(&app, &a, Value::Null, Value::Null, "2026-10-21");
        assert_eq!(
            ops_since(&app, before),
            [pair("update", "todo_task"), pair("update", "todo_task")]
        );
        let err = app
            .invoke::<Value>(
                "todo_task_set_when",
                json!({ "id": id_of(&a), "date": "2026-10-20", "time": null,
                        "local": local("2026-10-21", "14:05") }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "validation");
        assert_eq!(
            ops_since(&app, before).len(),
            2,
            "a refused write logs nothing"
        );
    }

    #[test]
    fn removing_the_date_removes_the_time_and_any_repeat() {
        let app = TestApp::new();
        let a = create(&app, "a").unwrap();
        set_when(&app, &a, json!("2026-10-23"), json!("09:00"), "2026-10-21");
        app.with_db(|c| {
            c.execute(
                "UPDATE todo_tasks SET repeat_freq = 'week', repeat_every = 1, repeat_mode = 'fixed'",
                [],
            )
            .unwrap();
        });
        let cleared = set_when(&app, &a, Value::Null, Value::Null, "2026-10-21");
        for field in [
            "due_date",
            "due_time",
            "repeat_freq",
            "repeat_every",
            "repeat_mode",
        ] {
            assert_eq!(cleared[field], Value::Null, "{field}");
        }
    }

    #[test]
    fn today_lists_what_was_done_today_in_the_devices_day_and_counts_progress() {
        let app = TestApp::new();
        let (a, b, c) = (
            create(&app, "a").unwrap(),
            create(&app, "b").unwrap(),
            create(&app, "c").unwrap(),
        );
        for t in [&a, &b, &c] {
            set_when(&app, t, json!("2026-10-21"), Value::Null, "2026-10-21");
        }
        for t in [&a, &b] {
            app.invoke::<Value>("todo_task_complete", json!({ "id": id_of(t) }))
                .unwrap();
        }
        // 2026-10-21 in Kolkata (+05:30) starts at 2026-10-20 18:30 UTC.
        let day_start_utc: i64 = 1_792_521_000_000;
        assert_eq!(day_start_utc % 86_400_000, 66_600_000, "18:30 UTC");
        let set_closed = |t: &Value, at: i64| {
            app.with_db(|conn| {
                let id = t["id"].as_str().unwrap();
                conn.execute(
                    &format!("UPDATE todo_tasks SET closed_at = {at} WHERE id = '{id}'"),
                    [],
                )
                .unwrap();
            });
        };
        set_closed(&a, day_start_utc + 1);
        set_closed(&b, day_start_utc - 1);
        let kolkata = json!({ "today": "2026-10-21", "now": "14:05", "offset_min": 330 });
        let view: Value = app
            .invoke("todo_today", json!({ "local": kolkata }))
            .unwrap();
        assert_eq!(titles(&view["done"]), json!(["a"]));
        assert_eq!(titles(&view["tasks"]), json!(["c"]));
        assert_eq!(
            view["total"], 2,
            "one open and one done today; b was done yesterday"
        );

        let utc = local("2026-10-21", "14:05");
        let view: Value = app.invoke("todo_today", json!({ "local": utc })).unwrap();
        assert_eq!(
            view["done"],
            json!([]),
            "both were closed before this UTC day began"
        );
        assert_eq!(view["total"], 1);
    }

    #[test]
    fn a_task_wont_do_today_is_not_counted_as_done() {
        let app = TestApp::new();
        let a = create(&app, "a").unwrap();
        set_when(&app, &a, json!("2026-10-21"), Value::Null, "2026-10-21");
        app.with_db(|c| {
            c.execute(
                "UPDATE todo_tasks SET state = 'wont', closed_at = 1792521000000 + 5",
                [],
            )
            .unwrap();
        });
        let kolkata = json!({ "today": "2026-10-21", "now": "14:05", "offset_min": 330 });
        let view: Value = app
            .invoke("todo_today", json!({ "local": kolkata }))
            .unwrap();
        assert_eq!(view["done"], json!([]));
        assert_eq!(view["total"], 0);
    }

    #[test]
    fn move_all_logs_each_change_and_undo_logs_them_again() {
        let app = TestApp::new();
        let (a, b) = (create(&app, "a").unwrap(), create(&app, "b").unwrap());
        for (t, d) in [(&a, "2026-10-19"), (&b, "2026-10-20")] {
            set_when(&app, t, json!("2026-10-21"), Value::Null, "2026-10-21");
            backdate(&app, t, d);
        }
        let before = ops_since(&app, 0).len();
        let moved: Value = app
            .invoke(
                "todo_tasks_move_overdue",
                json!({ "date": "2026-10-21", "local": local("2026-10-21", "14:05") }),
            )
            .unwrap();
        assert_eq!(moved["to_date"], "2026-10-21");
        assert_eq!(moved["previous"].as_array().unwrap().len(), 2);
        let restored: Value = app
            .invoke(
                "todo_tasks_restore_when",
                json!({ "changes": moved["previous"] }),
            )
            .unwrap();
        assert_eq!(restored.as_array().unwrap().len(), 2);
        assert_eq!(restored[0]["due_date"], "2026-10-19");
        assert_eq!(ops_since(&app, before).len(), 4, "two moves and two undos");
        // Nothing is left to undo the second time.
        app.invoke::<Value>(
            "todo_tasks_restore_when",
            json!({ "changes": moved["previous"] }),
        )
        .unwrap();
        assert_eq!(
            ops_since(&app, before).len(),
            4,
            "an unchanged restore logs nothing"
        );
    }

    #[test]
    fn restoring_skips_deleted_tasks_and_refuses_malformed_input() {
        let app = TestApp::new();
        let home = new_list(&app, "Home");
        let a = app
            .invoke::<Value>(
                "todo_task_create",
                json!({ "title": "a", "listId": id_of(&home) }),
            )
            .unwrap();
        app.invoke::<Value>(
            "todo_list_delete",
            json!({ "id": id_of(&home), "choice": { "kind": "delete_tasks" } }),
        )
        .unwrap();
        let skipped: Value = app
            .invoke(
                "todo_tasks_restore_when",
                json!({ "changes": [{ "id": id_of(&a), "due_date": "2026-10-01", "due_time": null }] }),
            )
            .unwrap();
        assert_eq!(skipped, json!([]));
        for bad in [
            json!({ "id": "x", "due_date": "2026-02-30", "due_time": null }),
            json!({ "id": "x", "due_date": null, "due_time": "09:00" }),
            json!({ "id": "x", "due_date": "2026-10-01", "due_time": "9am" }),
        ] {
            let err = app
                .invoke::<Value>("todo_tasks_restore_when", json!({ "changes": [bad] }))
                .unwrap_err();
            assert_eq!(err["code"], "validation");
        }
    }

    #[test]
    fn day_counts_leave_out_the_excluded_task_and_refuse_bad_ranges() {
        let app = TestApp::new();
        let (a, b) = (create(&app, "a").unwrap(), create(&app, "b").unwrap());
        for t in [&a, &b] {
            set_when(&app, t, json!("2026-10-23"), Value::Null, "2026-10-21");
        }
        let counts = |exclude: Value| -> Value {
            app.invoke(
                "todo_day_counts",
                json!({ "from": "2026-10-21", "to": "2026-10-30", "exclude": exclude }),
            )
            .unwrap()
        };
        assert_eq!(
            counts(Value::Null),
            json!([{ "date": "2026-10-23", "count": 2 }])
        );
        assert_eq!(
            counts(id_of(&a)),
            json!([{ "date": "2026-10-23", "count": 1 }])
        );
        for (from, to) in [("2026-10-30", "2026-10-21"), ("2026-10-21", "nope")] {
            let err = app
                .invoke::<Value>(
                    "todo_day_counts",
                    json!({ "from": from, "to": to, "exclude": null }),
                )
                .unwrap_err();
            assert_eq!(err["code"], "validation");
        }
    }

    #[test]
    fn the_sidebar_counts_today_as_due_today_plus_overdue_and_the_inbox_apart() {
        let app = TestApp::new();
        let (late, due, future) = (
            create(&app, "late").unwrap(),
            create(&app, "due").unwrap(),
            create(&app, "future").unwrap(),
        );
        create(&app, "undated").unwrap();
        set_when(&app, &late, json!("2026-10-21"), Value::Null, "2026-10-21");
        backdate(&app, &late, "2026-10-19");
        set_when(
            &app,
            &due,
            json!("2026-10-21"),
            json!("09:00"),
            "2026-10-21",
        );
        set_when(
            &app,
            &future,
            json!("2026-10-22"),
            Value::Null,
            "2026-10-21",
        );
        let sidebar = |today: &str| -> Value {
            app.invoke("todo_lists", json!({ "local": local(today, "14:05") }))
                .unwrap()
        };
        let s = sidebar("2026-10-21");
        assert_eq!(
            (&s["today_count"], &s["overdue_count"], &s["inbox_count"]),
            (&json!(2), &json!(1), &json!(4))
        );
        // A day later the same Tasks are all Overdue.
        let s = sidebar("2026-10-22");
        assert_eq!(
            (&s["today_count"], &s["overdue_count"]),
            (&json!(3), &json!(2))
        );
        app.invoke::<Value>("todo_task_complete", json!({ "id": id_of(&due) }))
            .unwrap();
        assert_eq!(sidebar("2026-10-21")["today_count"], 1);
    }

    #[test]
    fn a_malformed_clock_is_refused_by_every_date_command() {
        let app = TestApp::new();
        let a = create(&app, "a").unwrap();
        let bad = json!({ "today": "21/10/2026", "now": "14:05", "offset_min": 0 });
        for (cmd, args) in [
            ("todo_today", json!({ "local": bad })),
            ("todo_upcoming", json!({ "local": bad })),
            (
                "todo_tasks_move_overdue",
                json!({ "date": "2026-10-21", "local": bad }),
            ),
            (
                "todo_task_set_when",
                json!({ "id": id_of(&a), "date": null, "time": null, "local": bad }),
            ),
        ] {
            let err = app.invoke::<Value>(cmd, args).unwrap_err();
            assert_eq!(err["code"], "validation", "{cmd}");
        }
    }

    // Shared rule fixtures: the same file runs against the browser mock in
    // src/lib/ipc/mock-todo.test.ts (ADR 0009). Keep `run_rule_case` in step with `run` there.
    const RULES: &str = include_str!("../../src/lib/ipc/fixtures/todo-rules.json");

    /// The All view as `[[group name, [titles]]]`, the shape the fixtures expect.
    fn all_groups(app: &TestApp) -> Value {
        let view: Value = app.invoke("todo_all", json!({})).unwrap();
        let groups = view["groups"].as_array().unwrap().iter().map(|g| {
            let name = g["list"]["name"].as_str().unwrap_or("Inbox");
            let titles: Vec<&str> = g["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t["title"].as_str().unwrap())
                .collect();
            json!([name, titles])
        });
        Value::Array(groups.collect())
    }

    const DEFAULT_TODAY: &str = "2026-10-21";
    const DEFAULT_NOW: &str = "14:05";

    fn titles(tasks: &Value) -> Value {
        Value::Array(
            tasks
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t["title"].clone())
                .collect(),
        )
    }

    /// Every open Task as `[title, due date, due time]`, in the All view's order.
    fn dues(app: &TestApp) -> Value {
        let view: Value = app.invoke("todo_all", json!({})).unwrap();
        let rows = view["groups"].as_array().unwrap().iter().flat_map(|g| {
            g["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| json!([t["title"], t["due_date"], t["due_time"]]))
        });
        Value::Array(rows.collect())
    }

    /// Today as `{ overdue, tasks }` titles; what was done today depends on the real clock.
    fn today_groups(app: &TestApp, local: &Value) -> Value {
        let view: Value = app.invoke("todo_today", json!({ "local": local })).unwrap();
        json!({ "overdue": titles(&view["overdue"]), "tasks": titles(&view["tasks"]) })
    }

    /// Upcoming as `{ overdue, days: [[date, titles]] }`.
    fn upcoming_groups(app: &TestApp, local: &Value) -> Value {
        let view: Value = app
            .invoke("todo_upcoming", json!({ "local": local }))
            .unwrap();
        let days = view["days"].as_array().unwrap().iter();
        json!({
            "overdue": titles(&view["overdue"]),
            "days": days.map(|d| json!([d["date"], titles(&d["tasks"])])).collect::<Vec<_>>(),
        })
    }

    /// `[[date, open Tasks]]` from the case's today to the end of 2027.
    fn day_counts(app: &TestApp, local: &Value) -> Value {
        let counts: Value = app
            .invoke(
                "todo_day_counts",
                json!({ "from": local["today"], "to": "2027-12-31", "exclude": null }),
            )
            .unwrap();
        Value::Array(
            counts
                .as_array()
                .unwrap()
                .iter()
                .map(|d| json!([d["date"], d["count"]]))
                .collect(),
        )
    }

    fn run_rule_case(c: &Value) -> Value {
        let app = TestApp::new();
        let mut tasks: Vec<Value> = Vec::new();
        let mut lists: Vec<Value> = Vec::new();
        let mut deletions: Vec<Value> = Vec::new();
        let mut moves: Vec<Value> = Vec::new();
        // The clock steps run against: the case's, unless a step pins its own.
        let clock = |step: &Value| {
            json!({
                "today": step["today"].as_str().or(c["today"].as_str()).unwrap_or(DEFAULT_TODAY),
                "now": step["now"].as_str().or(c["now"].as_str()).unwrap_or(DEFAULT_NOW),
                "offset_min": 0,
            })
        };
        // A number picks the n-th created item, null stays null, a string is used as an id.
        let pick = |v: &Value, made: &[Value]| match v {
            Value::Number(n) => made[usize::try_from(n.as_u64().unwrap()).unwrap()].clone(),
            other => other.clone(),
        };
        let mut results = Vec::new();
        for step in c["steps"].as_array().unwrap() {
            let op = step["op"].as_str().unwrap();
            let task = pick(&step["task"], &tasks);
            let list = pick(&step["list"], &lists);
            let outcome: Result<Value, Value> = match op {
                "create" => {
                    let dated = !step["date"].is_null() || !step["time"].is_null();
                    let when = if dated {
                        json!({ "date": step["date"], "time": step["time"], "local": clock(step) })
                    } else {
                        Value::Null
                    };
                    app.invoke(
                        "todo_task_create",
                        json!({ "title": step["title"], "listId": list, "when": when }),
                    )
                }
                "set_when" => app.invoke(
                    "todo_task_set_when",
                    json!({
                        "id": task, "date": step["date"], "time": step["time"],
                        "local": clock(step)
                    }),
                ),
                "move_overdue" => app.invoke(
                    "todo_tasks_move_overdue",
                    json!({ "date": step["date"], "local": clock(step) }),
                ),
                "undo_move" => app.invoke(
                    "todo_tasks_restore_when",
                    json!({ "changes": moves[usize::try_from(step["move"].as_u64().unwrap()).unwrap()]["previous"] }),
                ),
                "move" => app.invoke("todo_task_move", json!({ "id": task, "listId": list })),
                "list_create" => app.invoke("todo_list_create", json!({ "name": step["name"] })),
                "list_rename" => app.invoke(
                    "todo_list_rename",
                    json!({ "id": list, "name": step["name"] }),
                ),
                "list_emoji" => app.invoke(
                    "todo_list_set_emoji",
                    json!({ "id": list, "emoji": step["emoji"] }),
                ),
                "list_delete" => {
                    let choice = match &step["choice"] {
                        Value::String(s) if s == "delete" => json!({ "kind": "delete_tasks" }),
                        Value::String(_) => json!({ "kind": "move_to_inbox" }),
                        other => {
                            json!({ "kind": "move_to_list", "list_id": pick(&other["list"], &lists) })
                        }
                    };
                    app.invoke("todo_list_delete", json!({ "id": list, "choice": choice }))
                }
                "list_restore" => app.invoke(
                    "todo_list_restore",
                    json!({ "deletion": pick(&step["deletion"], &deletions) }),
                ),
                _ => app.invoke(&format!("todo_task_{op}"), json!({ "id": task })),
            };
            match outcome {
                Ok(out) => {
                    match op {
                        "create" => tasks.push(out["id"].clone()),
                        "list_create" => lists.push(out["id"].clone()),
                        "list_delete" => deletions.push(out),
                        "move_overdue" => moves.push(out),
                        _ => {}
                    }
                    results.push(json!("ok"));
                }
                Err(e) => results.push(e["code"].clone()),
            }
        }
        rule_outputs(&app, results, &clock(&Value::Null))
    }

    /// Everything a fixture case can expect, read from the app after its steps ran.
    fn rule_outputs(app: &TestApp, results: Vec<Value>, view_clock: &Value) -> Value {
        let sidebar: Value = app
            .invoke("todo_lists", json!({ "local": view_clock }))
            .unwrap();
        let entries = sidebar["lists"].as_array().unwrap();
        let mut counts = vec![sidebar["inbox_count"].clone()];
        counts.extend(entries.iter().map(|e| e["open_count"].clone()));
        json!({
            "results": results,
            "dues": dues(app),
            "today": today_groups(app, view_clock),
            "upcoming": upcoming_groups(app, view_clock),
            "day_counts": day_counts(app, view_clock),
            "inbox": inbox_titles(app),
            "lists": entries.iter().map(|e| e["list"]["name"].clone()).collect::<Vec<_>>(),
            "emojis": entries.iter().map(|e| e["list"]["emoji"].clone()).collect::<Vec<_>>(),
            "counts": counts,
            "sidebar_today": [sidebar["today_count"], sidebar["overdue_count"]],
            "all": all_groups(app),
        })
    }

    #[test]
    fn the_shared_rule_fixtures_hold() {
        let rules: Value = serde_json::from_str(RULES).unwrap();
        let cases = rules["cases"].as_array().unwrap();
        assert!(!cases.is_empty());
        for c in cases {
            let actual = run_rule_case(c);
            let expected = c["expect"].as_object().unwrap();
            for (key, want) in expected {
                assert_eq!(
                    &actual[key],
                    want,
                    "{}: {} ({key})",
                    c["rule"].as_str().unwrap(),
                    c["name"].as_str().unwrap()
                );
            }
        }
    }
}
