//! Todo commands. Thin: the rules live in `mnemora-todo`.

use mnemora_core::error::AppError;
use mnemora_todo::{
    AllView, Comment, Completion, DayCount, DeleteChoice, DoneView, DueChange, GroupBy, GroupKey,
    HistoryLine, InboxView, ListDeletion, ListView, LocalNow, NewTaskDetails, NewWhen, OverdueMove,
    ParseRequest, Parsed, QuickEdit, RepeatRule, SidebarView, Task, TaskSearchResults, TaskSort,
    TodayView, TodoCounts, TodoList, UpcomingView,
};
use tauri::State;

use crate::state::{AppState, lock};

/// The Inbox: open Tasks that belong to no List, grouped and sorted as the Preferences say, then
/// "No date, in a List" for the undated Tasks that sit in a List (not counted in the badge).
#[tauri::command]
#[specta::specta]
pub fn todo_inbox(state: State<'_, AppState>, local: LocalNow) -> Result<InboxView, AppError> {
    mnemora_todo::inbox(&lock(&state.db), &local)
}

/// Adds a Task to the bottom of a List, or of the Inbox when `list_id` is none. Rejects a blank
/// title, one over 500 characters and one with a line break. `when` gives it a Due date and time
/// under the rules of `todo_task_set_when`. `details` adds what capture reads out of the text:
/// notes, a Priority, a Repeat (which needs `when`, for the clock) and the record of understood
/// words.
#[tauri::command]
#[specta::specta]
pub fn todo_task_create(
    state: State<'_, AppState>,
    title: String,
    list_id: Option<String>,
    when: Option<NewWhen>,
    details: Option<NewTaskDetails>,
) -> Result<Task, AppError> {
    mnemora_todo::create_task(
        &mut lock(&state.db),
        &title,
        list_id.as_deref(),
        when.as_ref(),
        details.as_ref(),
    )
}

/// Reads typed text as the capture box does: dates, times, Repeats, `#List` and `p1`-`p4`, with
/// what was recognised, what lost to a later word of the same kind, and what the user keeps as
/// text. Writes nothing.
#[tauri::command]
#[specta::specta]
pub fn todo_parse(state: State<'_, AppState>, request: ParseRequest) -> Result<Parsed, AppError> {
    mnemora_todo::parse_text(&lock(&state.db), &request)
}

/// Saves a Quick edit in one write: the title, the record of understood words, the When, Repeat,
/// Priority and List as they should be afterwards. A When that differs goes through the rules of
/// `todo_task_set_when` (past dates refused). Records an `update` change.
#[tauri::command]
#[specta::specta]
pub fn todo_task_quick_edit(
    state: State<'_, AppState>,
    id: String,
    edit: QuickEdit,
    local: LocalNow,
) -> Result<Task, AppError> {
    mnemora_todo::quick_edit(&mut lock(&state.db), &id, &edit, &local, false)
}

/// Undo for a Quick edit: puts the old values back, a past date included.
#[tauri::command]
#[specta::specta]
pub fn todo_task_quick_edit_undo(
    state: State<'_, AppState>,
    id: String,
    edit: QuickEdit,
    local: LocalNow,
) -> Result<Task, AppError> {
    mnemora_todo::quick_edit(&mut lock(&state.db), &id, &edit, &local, true)
}

/// Sets an open Task's Due date, time and Repeat, or clears them when all are none. A time
/// without a date means today if it is still ahead and tomorrow otherwise; a Repeat without a date
/// gives the first day it lands on. Past dates are refused. `repeat` is the Repeat after the
/// change (none removes it), so callers that keep it pass it back. A date changed by hand is the
/// new anchor of the Repeat; no date means no Repeat. `local` is the device's today and now. Records
/// an `update` change.
#[tauri::command]
#[specta::specta]
pub fn todo_task_set_when(
    state: State<'_, AppState>,
    id: String,
    date: Option<String>,
    time: Option<String>,
    repeat: Option<RepeatRule>,
    local: LocalNow,
) -> Result<Task, AppError> {
    mnemora_todo::set_when(
        &mut lock(&state.db),
        &id,
        date.as_deref(),
        time.as_deref(),
        repeat.as_ref(),
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

/// What Todo's badge and the Home card show: the counts due today and Overdue, up to five of
/// today's open Tasks, the soonest Task after today and whether any Task exists. `local` is the
/// device's today and now.
#[tauri::command]
#[specta::specta]
pub fn todo_counts(state: State<'_, AppState>, local: LocalNow) -> Result<TodoCounts, AppError> {
    mnemora_todo::counts(&lock(&state.db), &local)
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

/// Marks an open Task done, now. A repeating Task stays open: that occurrence is recorded as a
/// done copy (taking the Task's Comments), the Checklist is reset and the Due date moves on, with
/// its time. Records a `tick` change for the Task and a `create` for the copy. `local` is the
/// device's today.
#[tauri::command]
#[specta::specta]
pub fn todo_task_complete(
    state: State<'_, AppState>,
    id: String,
    local: LocalNow,
) -> Result<Completion, AppError> {
    mnemora_todo::complete_task(&mut lock(&state.db), &id, &local)
}

/// Moves a repeating Task to its next date without recording anything done. Undo is
/// `todo_tasks_restore_when`. Records an `update` change.
#[tauri::command]
#[specta::specta]
pub fn todo_task_skip(
    state: State<'_, AppState>,
    id: String,
    local: LocalNow,
) -> Result<Task, AppError> {
    mnemora_todo::skip_task(&mut lock(&state.db), &id, &local)
}

/// What has been done, a page of days at a time, newest day first. Pass the `next_cursor` of the
/// last page as `before` to continue.
#[tauri::command]
#[specta::specta]
pub fn todo_done(
    state: State<'_, AppState>,
    before: Option<String>,
    days: u32,
    local: LocalNow,
) -> Result<DoneView, AppError> {
    mnemora_todo::done(&lock(&state.db), before.as_deref(), days, &local)
}

/// Brings a done Task back to open, in its old place. The latest occurrence of a repeating Task
/// rewinds that Task to its date and returns it; older ones are refused.
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

/// One Task in any state, with its Comment count: what the Task sheet opens.
#[tauri::command]
#[specta::specta]
pub fn todo_task(state: State<'_, AppState>, id: String) -> Result<Task, AppError> {
    mnemora_todo::task(&lock(&state.db), &id)
}

/// Searches live Tasks by title, notes and Comments. Every word is required and matches by
/// prefix; quoted text is an exact phrase; case and accents are ignored. Open Tasks come first,
/// then Done and Won't do ones. Each Hit carries the title with its matches marked and the note
/// or Comment line that matched. `similar` holds Tasks that match only with a typo allowed, never
/// repeating `exact`. Deleted Tasks and Comments are never found.
#[tauri::command]
#[specta::specta]
pub fn todo_search(
    state: State<'_, AppState>,
    query: String,
) -> Result<TaskSearchResults, AppError> {
    mnemora_todo::search(&lock(&state.db), &query)
}

/// Replaces a Task's title and notes: the Task sheet's autosave. All the saves of one opening of
/// the sheet carry the same `session_id`, so the change log keeps one `update` record for them.
/// A blank, multi-line or over-long title is refused; the same text changes nothing.
#[tauri::command]
#[specta::specta]
pub fn todo_task_update(
    state: State<'_, AppState>,
    id: String,
    title: String,
    notes: String,
    session_id: String,
) -> Result<Task, AppError> {
    mnemora_todo::update_task(&mut lock(&state.db), &id, &title, &notes, &session_id)
}

/// Sets an open Task's Priority: 1 (highest) to 4 (none, the default). Records an `update` change.
#[tauri::command]
#[specta::specta]
pub fn todo_task_set_priority(
    state: State<'_, AppState>,
    id: String,
    priority: u32,
) -> Result<Task, AppError> {
    mnemora_todo::set_priority(&mut lock(&state.db), &id, priority)
}

/// Marks an open Task Won't do: it joins Done with a ⊗ and never counts as done. A repeating Task
/// is refused (it is skipped instead). Undo is `todo_task_reopen`. Records a `tick` change.
#[tauri::command]
#[specta::specta]
pub fn todo_task_wont_do(state: State<'_, AppState>, id: String) -> Result<Task, AppError> {
    mnemora_todo::wont_do(&mut lock(&state.db), &id)
}

/// Deletes a Task at once; it is erased for good after 30 days. Occurrences of a repeating Task
/// stay in Done. Records a `delete` change.
#[tauri::command]
#[specta::specta]
pub fn todo_task_delete(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    mnemora_todo::delete_task(&mut lock(&state.db), &id)
}

/// Undoes `todo_task_delete`. A Task whose List was deleted meanwhile returns to the Inbox.
/// Records a `restore` change.
#[tauri::command]
#[specta::specta]
pub fn todo_task_restore(state: State<'_, AppState>, id: String) -> Result<Task, AppError> {
    mnemora_todo::restore_task(&mut lock(&state.db), &id)
}

/// A Task's Comments, oldest first.
#[tauri::command]
#[specta::specta]
pub fn todo_comments(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<Vec<Comment>, AppError> {
    mnemora_todo::comments(&lock(&state.db), &task_id)
}

/// Adds a Comment (line breaks allowed, up to 5000 characters) to a Task.
#[tauri::command]
#[specta::specta]
pub fn todo_comment_add(
    state: State<'_, AppState>,
    task_id: String,
    body: String,
) -> Result<Comment, AppError> {
    mnemora_todo::add_comment(&mut lock(&state.db), &task_id, &body)
}

/// Replaces a Comment's text; it then shows "edited".
#[tauri::command]
#[specta::specta]
pub fn todo_comment_edit(
    state: State<'_, AppState>,
    id: String,
    body: String,
) -> Result<Comment, AppError> {
    mnemora_todo::edit_comment(&mut lock(&state.db), &id, &body)
}

/// Deletes a Comment (erased after 30 days); `todo_comment_restore` is Undo.
#[tauri::command]
#[specta::specta]
pub fn todo_comment_delete(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    mnemora_todo::delete_comment(&mut lock(&state.db), &id)
}

/// Undoes `todo_comment_delete`.
#[tauri::command]
#[specta::specta]
pub fn todo_comment_restore(state: State<'_, AppState>, id: String) -> Result<Comment, AppError> {
    mnemora_todo::restore_comment(&mut lock(&state.db), &id)
}

/// A Task's History: readable lines built from the change log, newest first.
#[tauri::command]
#[specta::specta]
pub fn todo_task_history(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<HistoryLine>, AppError> {
    mnemora_todo::task_history(&lock(&state.db), &id)
}

/// The sidebar's data: the Inbox's open count, Today's count (due today plus Overdue, with the
/// Overdue part apart) and every List, in order, with its open count.
#[tauri::command]
#[specta::specta]
pub fn todo_lists(state: State<'_, AppState>, local: LocalNow) -> Result<SidebarView, AppError> {
    mnemora_todo::sidebar(&lock(&state.db), &local)
}

/// One List: its open Tasks grouped and sorted as the List says, and its Done Tasks as the
/// Done-in-Lists Preference says.
#[tauri::command]
#[specta::specta]
pub fn todo_list(
    state: State<'_, AppState>,
    id: String,
    local: LocalNow,
) -> Result<ListView, AppError> {
    mnemora_todo::list_view(&lock(&state.db), &id, &local)
}

/// Open Tasks grouped as the Preference says (by List, the Inbox first, unless changed). Empty
/// groups are left out.
#[tauri::command]
#[specta::specta]
pub fn todo_all(state: State<'_, AppState>, local: LocalNow) -> Result<AllView, AppError> {
    mnemora_todo::all(&lock(&state.db), &local)
}

/// Moves an open Task between two neighbours of the shared manual order: `before` is the Task
/// directly above its new place and `after` the one directly below (either may be none). `target`
/// is what the group it lands in adopts, as `TaskGroup.adopt` says: a day sets the Due date (a
/// past day is refused, and dragging among Overdue Tasks adopts nothing), a Priority sets the
/// Priority, a List moves it there. Records one change: `move` when the List changed, `update`
/// otherwise, none when nothing changed.
#[tauri::command]
#[specta::specta]
pub fn todo_task_reorder(
    state: State<'_, AppState>,
    id: String,
    before: Option<String>,
    after: Option<String>,
    target: Vec<GroupKey>,
    local: LocalNow,
) -> Result<Task, AppError> {
    mnemora_todo::reorder_task(
        &mut lock(&state.db),
        &id,
        before.as_deref(),
        after.as_deref(),
        &target,
        &local,
    )
}

/// Moves a List between two neighbours of the manual List order (`before` above, `after` below).
#[tauri::command]
#[specta::specta]
pub fn todo_list_reorder(
    state: State<'_, AppState>,
    id: String,
    before: Option<String>,
    after: Option<String>,
) -> Result<TodoList, AppError> {
    mnemora_todo::reorder_list(
        &mut lock(&state.db),
        &id,
        before.as_deref(),
        after.as_deref(),
    )
}

/// Sets how a List's page sorts the Tasks inside each group.
#[tauri::command]
#[specta::specta]
pub fn todo_list_set_sort(
    state: State<'_, AppState>,
    id: String,
    sort: TaskSort,
) -> Result<TodoList, AppError> {
    mnemora_todo::set_list_sort(&mut lock(&state.db), &id, sort)
}

/// Sets how a List's page groups its Tasks: none, priority or due.
#[tauri::command]
#[specta::specta]
pub fn todo_list_set_group(
    state: State<'_, AppState>,
    id: String,
    group_by: GroupBy,
) -> Result<TodoList, AppError> {
    mnemora_todo::set_list_group(&mut lock(&state.db), &id, group_by)
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

    use crate::test_support::{TestApp, inbox_tasks_of, overdue_of, tasks_of};

    /// The clock the view commands that need one run against.
    fn clock() -> Value {
        json!({ "today": "2026-10-21", "now": "14:05", "offset_min": 0 })
    }

    fn create(app: &TestApp, title: &str) -> Result<Value, Value> {
        app.invoke("todo_task_create", json!({ "title": title }))
    }

    fn inbox_titles(app: &TestApp) -> Vec<String> {
        let view: Value = app
            .invoke("todo_inbox", json!({ "local": clock() }))
            .unwrap();
        inbox_tasks_of(&view)
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
                    "SELECT op, entity, COALESCE(json_extract(snapshot, '$.state'), '') FROM change_log
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
            .invoke(
                "todo_task_complete",
                json!({ "id": id_of(&a), "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
            )
            .unwrap();
        assert_eq!(done["task"]["state"], "done");
        assert_eq!(done["occurrence"], Value::Null);
        assert!(done["task"]["closed_at"].as_i64().unwrap() >= a["created_at"].as_i64().unwrap());
        assert_eq!(inbox_titles(&app), ["b"]);
    }

    #[test]
    fn reopening_returns_the_task_to_open_in_its_old_place() {
        let app = TestApp::new();
        let a = create(&app, "a").unwrap();
        create(&app, "b").unwrap();
        app.invoke::<Value>(
            "todo_task_complete",
            json!({ "id": id_of(&a), "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
        )
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
        app.invoke::<Value>(
            "todo_task_complete",
            json!({ "id": id, "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
        )
        .unwrap();
        let err = app
            .invoke::<Value>(
                "todo_task_complete",
                json!({ "id": id, "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "validation");
        for cmd in ["todo_task_complete", "todo_task_reopen"] {
            let err = app
                .invoke::<Value>(
                    cmd,
                    json!({ "id": "nope", "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
                )
                .unwrap_err();
            assert_eq!(err["code"], "not_found", "{cmd}");
        }
    }

    #[test]
    fn create_and_tick_are_recorded_in_the_change_log() {
        let app = TestApp::new();
        let a = create(&app, "a").unwrap();
        let id = id_of(&a);
        app.invoke::<Value>(
            "todo_task_complete",
            json!({ "id": id, "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
        )
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
        let _ = app.invoke::<Value>(
            "todo_task_complete",
            json!({ "id": "nope", "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
        );
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
        mnemora_todo::create_task(&mut conn, "Works after the upgrade", None, None, None).unwrap();
        let local = mnemora_todo::LocalNow {
            today: "2026-10-21".into(),
            now: "14:05".into(),
            offset_min: 0,
        };
        assert_eq!(
            mnemora_todo::inbox(&conn, &local).unwrap().groups[0]
                .tasks
                .len(),
            1
        );

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
            .invoke("todo_list", json!({ "id": id_of(&home), "local": clock() }))
            .unwrap();
        assert_eq!(view["list"]["name"], "Home");
        let titles: Vec<_> = tasks_of(&view)
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].clone())
            .collect();
        assert_eq!(titles, [json!("a"), json!("b")]);
        let err = app
            .invoke::<Value>("todo_list", json!({ "id": "nope", "local": clock() }))
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
            .invoke("todo_list", json!({ "id": id_of(&home), "local": clock() }))
            .unwrap();
        assert_eq!(tasks_of(&view).as_array().unwrap().len(), 2);
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
    fn counts_give_the_badge_the_card_and_the_next_task() {
        let app = TestApp::new();
        let counts = |today: &str| -> Value {
            app.invoke("todo_counts", json!({ "local": local(today, "14:05") }))
                .unwrap()
        };
        let first = counts("2026-10-21");
        assert_eq!(first["any_tasks"], false, "a first run has no Tasks");
        assert_eq!(first["due_today"], 0);
        assert_eq!(first["next"], Value::Null);

        let late = create(&app, "late").unwrap();
        backdate(&app, &late, "2026-10-20");
        for n in 1..=6 {
            let t = create(&app, &format!("today {n}")).unwrap();
            set_when(&app, &t, json!("2026-10-21"), Value::Null, "2026-10-21");
        }
        let timed = create(&app, "timed").unwrap();
        set_when(&app, &timed, json!("2026-10-21"), json!("17:00"), "2026-10-21");
        let soon = create(&app, "soon").unwrap();
        set_when(&app, &soon, json!("2026-10-24"), Value::Null, "2026-10-21");
        let later = create(&app, "later").unwrap();
        set_when(&app, &later, json!("2026-11-02"), Value::Null, "2026-10-21");

        let c = counts("2026-10-21");
        assert_eq!(c["any_tasks"], true);
        assert_eq!(c["due_today"], 7);
        assert_eq!(c["overdue"], 1);
        let titles: Vec<_> = c["card"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap())
            .collect();
        assert_eq!(titles, ["timed", "today 1", "today 2", "today 3", "today 4"]);
        assert_eq!(c["next"]["title"], "soon");

        // The same Tasks on another day: nothing is due and the next one moves on.
        let c = counts("2026-10-25");
        assert_eq!((c["due_today"].clone(), c["overdue"].clone()), (json!(0), json!(9)));
        assert_eq!(c["card"], json!([]));
        assert_eq!(c["next"]["title"], "later");

        // A ticked Task leaves the counts but a first run is over for good.
        app.invoke::<Value>(
            "todo_task_complete",
            json!({ "id": id_of(&timed), "local": local("2026-10-21", "14:05") }),
        )
        .unwrap();
        assert_eq!(counts("2026-10-21")["due_today"], 6);
    }

    #[test]
    fn counts_refuse_a_malformed_clock() {
        let app = TestApp::new();
        let err = app
            .invoke::<Value>("todo_counts", json!({ "local": local("soon", "14:05") }))
            .unwrap_err();
        assert_eq!(err["code"], "validation");
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
            app.invoke::<Value>(
                "todo_task_complete",
                json!({ "id": id_of(t), "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
            )
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
        assert_eq!(titles(&tasks_of(&view)), json!(["c"]));
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
        app.invoke::<Value>(
            "todo_task_complete",
            json!({ "id": id_of(&due), "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
        )
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
            (
                "todo_task_complete",
                json!({ "id": id_of(&a), "local": bad }),
            ),
            ("todo_task_skip", json!({ "id": id_of(&a), "local": bad })),
            (
                "todo_done",
                json!({ "before": null, "days": 3, "local": bad }),
            ),
        ] {
            let err = app.invoke::<Value>(cmd, args).unwrap_err();
            assert_eq!(err["code"], "validation", "{cmd}");
        }
    }

    // Repeating Tasks. 2026-10-21 is a Wednesday.

    fn rule(freq: &str, every: u32, days: Value, mode: &str) -> Value {
        json!({ "freq": freq, "every": every, "days": days, "mode": mode })
    }

    fn repeat_when(app: &TestApp, task: &Value, date: Value, time: Value, repeat: Value) -> Value {
        app.invoke(
            "todo_task_set_when",
            json!({ "id": id_of(task), "date": date, "time": time, "repeat": repeat,
                    "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
        )
        .unwrap()
    }

    fn complete_on(app: &TestApp, task: &Value, today: &str) -> Value {
        app.invoke(
            "todo_task_complete",
            json!({ "id": id_of(task), "local": local(today, DEFAULT_NOW) }),
        )
        .unwrap()
    }

    fn open_task(app: &TestApp, task: &Value) -> Value {
        let id = task["id"].as_str().unwrap();
        let view: Value = app.invoke("todo_all", json!({ "local": clock() })).unwrap();
        view["groups"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g["tasks"].as_array().unwrap().clone())
            .find(|t| t["id"] == id)
            .unwrap_or(Value::Null)
    }

    /// A Task due `date` that repeats by `rule`, with a time, notes and a List.
    fn repeating(app: &TestApp, title: &str, date: &str, rule: Value) -> Value {
        let list = new_list(app, &format!("Home {title}"));
        let task = create(app, title).unwrap();
        app.invoke::<Value>(
            "todo_task_move",
            json!({ "id": id_of(&task), "listId": id_of(&list) }),
        )
        .unwrap();
        set_when(app, &task, json!(date), json!("09:00"), DEFAULT_TODAY);
        app.with_db(|c| {
            c.execute(
                "UPDATE todo_tasks SET notes = 'Water:\n- [x] ferns\n- [ ] cactus' WHERE id = ?1",
                [task["id"].as_str().unwrap()],
            )
            .unwrap();
        });
        repeat_when(app, &task, json!(date), json!("09:00"), rule)
    }

    fn occurrences_of(app: &TestApp, task: &Value) -> Vec<Value> {
        app.with_db(|c| {
            let mut stmt = c
                .prepare(
                    "SELECT id, title, list_id, notes, due_date, due_time, state, repeat_freq,
                            deleted_at IS NOT NULL
                     FROM todo_tasks WHERE repeat_of = ?1 ORDER BY closed_at, id",
                )
                .unwrap();
            stmt.query_map([task["id"].as_str().unwrap()], |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?,
                    "list_id": r.get::<_, Option<String>>(2)?, "notes": r.get::<_, String>(3)?,
                    "due_date": r.get::<_, Option<String>>(4)?,
                    "due_time": r.get::<_, Option<String>>(5)?,
                    "state": r.get::<_, String>(6)?, "repeat": r.get::<_, Option<String>>(7)?,
                    "deleted": r.get::<_, bool>(8)?,
                }))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect()
        })
    }

    fn add_comment(app: &TestApp, task_id: &str, body: &str) -> String {
        app.with_db(|c| {
            let id = format!("c-{body}");
            c.execute(
                &format!(
                    "INSERT INTO todo_comments(id, task_id, body, created_at, updated_at)
                     VALUES ('{id}', '{task_id}', '{body}', 1, 1)"
                ),
                [],
            )
            .unwrap();
            id
        })
    }

    fn comment_owner(app: &TestApp, id: &str) -> String {
        app.with_db(|c| {
            c.query_row(
                "SELECT task_id FROM todo_comments WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap()
        })
    }

    #[test]
    fn a_repeat_on_an_undated_task_dates_it_to_the_first_matching_day() {
        let app = TestApp::new();
        let a = create(&app, "Gym").unwrap();
        let before = ops_since(&app, 0).len();
        // Set on Wednesday 21 October, every Monday and Thursday: Thursday the 22nd.
        let got = repeat_when(
            &app,
            &a,
            Value::Null,
            Value::Null,
            rule("week", 1, json!(0b001_0010), "fixed"),
        );
        assert_eq!(got["due_date"], "2026-10-22");
        assert_eq!(
            (
                got["repeat_freq"].clone(),
                got["repeat_every"].clone(),
                got["repeat_days"].clone()
            ),
            (json!("week"), json!(1), json!(18))
        );
        assert_eq!(got["repeat_mode"], "fixed");
        assert_eq!(ops_since(&app, before), [pair("update", "todo_task")]);
        // The same Repeat again changes and logs nothing.
        repeat_when(
            &app,
            &a,
            json!("2026-10-22"),
            Value::Null,
            rule("week", 1, json!(18), "fixed"),
        );
        assert_eq!(ops_since(&app, before).len(), 1);
    }

    #[test]
    fn a_repeat_on_a_dated_task_keeps_its_date_and_a_new_date_is_the_new_anchor() {
        let app = TestApp::new();
        let a = create(&app, "Rent").unwrap();
        let got = repeat_when(
            &app,
            &a,
            json!("2026-10-31"),
            json!("08:30"),
            rule("month", 1, Value::Null, "fixed"),
        );
        assert_eq!(got["due_date"], "2026-10-31");
        assert_eq!(
            got["repeat_days"], 31,
            "months keep to the day of the Due date"
        );
        // The date changed by hand moves the anchor, and the Repeat survives.
        let moved = repeat_when(
            &app,
            &a,
            json!("2026-11-05"),
            json!("08:30"),
            rule("month", 1, json!(31), "fixed"),
        );
        assert_eq!(moved["repeat_days"], 5);
        assert_eq!(moved["repeat_freq"], "month");
        // Only the time changes: the anchor stays (a clamped 31st would otherwise drift).
        app.with_db(|c| {
            c.execute(
                "UPDATE todo_tasks SET due_date = '2026-11-30', repeat_days = 31",
                [],
            )
            .unwrap();
        });
        let same = repeat_when(
            &app,
            &a,
            json!("2026-11-30"),
            json!("09:00"),
            rule("month", 1, json!(31), "fixed"),
        );
        assert_eq!(same["repeat_days"], 31);
    }

    #[test]
    fn bad_repeats_are_refused_and_change_nothing() {
        let app = TestApp::new();
        let a = create(&app, "a").unwrap();
        for bad in [
            rule("day", 0, Value::Null, "fixed"),
            rule("day", 100, Value::Null, "fixed"),
            rule("decade", 1, Value::Null, "fixed"),
            rule("day", 1, Value::Null, "sometimes"),
            rule("week", 1, json!(18), "after"),
            rule("week", 1, json!(200), "fixed"),
        ] {
            let err = app
                .invoke::<Value>(
                    "todo_task_set_when",
                    json!({ "id": id_of(&a), "date": "2026-10-23", "time": null, "repeat": bad,
                            "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
                )
                .unwrap_err();
            assert_eq!(err["code"], "validation", "{bad}");
        }
        assert_eq!(open_task(&app, &a)["due_date"], Value::Null);
        assert_eq!(ops_since(&app, 0).len(), 1);
    }

    #[test]
    fn removing_the_date_of_a_repeating_task_removes_the_repeat_and_undo_brings_both_back() {
        let app = TestApp::new();
        let a = repeating(
            &app,
            "Water the plants",
            "2026-10-23",
            rule("day", 3, Value::Null, "after"),
        );
        let cleared = set_when(&app, &a, Value::Null, Value::Null, DEFAULT_TODAY);
        for field in [
            "due_date",
            "due_time",
            "repeat_freq",
            "repeat_every",
            "repeat_days",
            "repeat_mode",
        ] {
            assert_eq!(cleared[field], Value::Null, "{field}");
        }
        // Undo is what the UI captured before: date, time and Repeat together.
        let restored: Value = app
            .invoke(
                "todo_tasks_restore_when",
                json!({ "changes": [{ "id": id_of(&a), "due_date": "2026-10-23", "due_time": "09:00",
                                      "repeat": rule("day", 3, Value::Null, "after") }] }),
            )
            .unwrap();
        assert_eq!(restored[0]["due_date"], "2026-10-23");
        assert_eq!(restored[0]["repeat_every"], 3);
        assert_eq!(restored[0]["repeat_mode"], "after");
    }

    #[test]
    fn move_all_overdue_keeps_the_repeat_and_undo_restores_the_old_date_and_anchor() {
        let app = TestApp::new();
        let a = repeating(
            &app,
            "Rent",
            "2026-10-31",
            rule("month", 1, Value::Null, "fixed"),
        );
        backdate(&app, &a, "2026-10-10");
        let moved: Value = app
            .invoke(
                "todo_tasks_move_overdue",
                json!({ "date": "2026-10-21", "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
            )
            .unwrap();
        let now = open_task(&app, &a);
        assert_eq!(
            (now["due_date"].clone(), now["repeat_days"].clone()),
            (json!("2026-10-21"), json!(21))
        );
        assert_eq!(moved["previous"][0]["repeat"]["days"], 31);
        app.invoke::<Value>(
            "todo_tasks_restore_when",
            json!({ "changes": moved["previous"] }),
        )
        .unwrap();
        let back = open_task(&app, &a);
        assert_eq!(
            (back["due_date"].clone(), back["repeat_days"].clone()),
            (json!("2026-10-10"), json!(31))
        );
    }

    #[test]
    fn completing_a_repeating_task_records_one_copy_resets_the_checklist_and_moves_the_date() {
        let app = TestApp::new();
        let a = repeating(
            &app,
            "Water the plants",
            "2026-10-21",
            rule("day", 3, Value::Null, "fixed"),
        );
        let before = log(&app).len();
        let done = complete_on(&app, &a, DEFAULT_TODAY);
        assert_eq!(done["task"]["state"], "open");
        assert_eq!(done["task"]["due_date"], "2026-10-24");
        assert_eq!(done["task"]["due_time"], "09:00", "the time is kept");
        assert_eq!(done["task"]["notes"], "Water:\n- [ ] ferns\n- [ ] cactus");
        assert_eq!(done["occurrence"]["state"], "done");
        assert_eq!(done["occurrence"]["repeat_of"], a["id"]);

        let copies = occurrences_of(&app, &a);
        assert_eq!(copies.len(), 1, "exactly one occurrence copy");
        let copy = &copies[0];
        assert_eq!(copy["title"], "Water the plants");
        assert_eq!(copy["list_id"], a["list_id"].clone());
        assert_eq!(
            copy["notes"], "Water:\n- [x] ferns\n- [ ] cactus",
            "the notes as they were"
        );
        assert_eq!(
            (copy["due_date"].clone(), copy["due_time"].clone()),
            (json!("2026-10-21"), json!("09:00"))
        );
        assert_eq!(
            (copy["state"].clone(), copy["repeat"].clone()),
            (json!("done"), Value::Null)
        );

        let tail: Vec<_> = log(&app).into_iter().skip(before).collect();
        let tick = ("tick".to_owned(), "todo_task".to_owned(), "open".to_owned());
        let create = (
            "create".to_owned(),
            "todo_task".to_owned(),
            "done".to_owned(),
        );
        assert_eq!(
            tail,
            [create, tick],
            "the copy is created, the Task is ticked"
        );
    }

    #[test]
    fn a_completed_occurrence_shows_in_done_and_the_task_leaves_today() {
        let app = TestApp::new();
        let a = repeating(
            &app,
            "Water the plants",
            "2026-10-21",
            rule("day", 3, Value::Null, "fixed"),
        );
        let day = local(DEFAULT_TODAY, DEFAULT_NOW);
        let today: Value = app.invoke("todo_today", json!({ "local": day })).unwrap();
        assert_eq!(titles(&tasks_of(&today)), json!(["Water the plants"]));
        complete_on(&app, &a, DEFAULT_TODAY);
        let today: Value = app.invoke("todo_today", json!({ "local": day })).unwrap();
        assert_eq!(titles(&tasks_of(&today)), json!([]));
        assert_eq!(titles(&today["done"]), json!(["Water the plants"]));
        assert_eq!(today["total"], 1);
        let done: Value = app
            .invoke(
                "todo_done",
                json!({ "before": null, "days": 7, "local": day }),
            )
            .unwrap();
        assert_eq!(done["days"].as_array().unwrap().len(), 1);
        assert_eq!(done["days"][0]["tasks"][0]["repeat_of"], a["id"]);
    }

    #[test]
    fn the_comments_of_the_occurrence_go_with_its_copy_and_come_back_on_a_rewind() {
        let app = TestApp::new();
        let a = repeating(
            &app,
            "Water the plants",
            "2026-10-21",
            rule("day", 1, Value::Null, "fixed"),
        );
        let id = a["id"].as_str().unwrap();
        let (c1, c2) = (add_comment(&app, id, "one"), add_comment(&app, id, "two"));
        let done = complete_on(&app, &a, DEFAULT_TODAY);
        let copy = done["occurrence"]["id"].as_str().unwrap();
        assert_eq!(
            (comment_owner(&app, &c1), comment_owner(&app, &c2)),
            (copy.to_owned(), copy.to_owned())
        );
        let late = add_comment(&app, id, "new");
        app.invoke::<Value>("todo_task_reopen", json!({ "id": copy }))
            .unwrap();
        assert_eq!(comment_owner(&app, &c1), id);
        assert_eq!(comment_owner(&app, &late), id);
        // The moves are logged by id, never by text.
        let moves: i64 = app.with_db(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM change_log WHERE entity = 'todo_comment' AND op = 'move'
                 AND snapshot NOT LIKE '%body%'",
                [],
                |r| r.get(0),
            )
            .unwrap()
        });
        assert_eq!(moves, 4);
    }

    #[test]
    fn a_late_fixed_task_goes_to_the_first_date_after_today_an_early_one_a_step_on_its_due_date() {
        let app = TestApp::new();
        let mondays = repeating(
            &app,
            "Bins",
            "2026-10-21",
            rule("week", 1, json!(0b000_0010), "fixed"),
        );
        backdate(&app, &mondays, "2026-09-28");
        let got = complete_on(&app, &mondays, DEFAULT_TODAY);
        assert_eq!(
            got["task"]["due_date"], "2026-10-26",
            "three weeks late: next Monday"
        );

        let fridays = repeating(
            &app,
            "Report",
            "2026-10-23",
            rule("week", 1, json!(0b010_0000), "fixed"),
        );
        let got = complete_on(&app, &fridays, DEFAULT_TODAY);
        assert_eq!(
            got["task"]["due_date"], "2026-10-30",
            "done early: one step after Friday"
        );
    }

    #[test]
    fn an_after_done_task_counts_from_the_day_it_is_done() {
        let app = TestApp::new();
        let a = repeating(
            &app,
            "Water the plants",
            "2026-10-21",
            rule("day", 3, Value::Null, "after"),
        );
        backdate(&app, &a, "2026-10-18");
        let got = complete_on(&app, &a, DEFAULT_TODAY);
        assert_eq!(got["task"]["due_date"], "2026-10-24");
        let later = complete_on(&app, &a, "2026-10-30");
        assert_eq!(later["task"]["due_date"], "2026-11-02");
    }

    #[test]
    fn undoing_a_completion_deletes_the_copy_and_rewinds_the_task() {
        let app = TestApp::new();
        let a = repeating(
            &app,
            "Water the plants",
            "2026-10-21",
            rule("day", 3, Value::Null, "fixed"),
        );
        let done = complete_on(&app, &a, DEFAULT_TODAY);
        let before = log(&app).len();
        let back: Value = app
            .invoke(
                "todo_task_reopen",
                json!({ "id": done["occurrence"]["id"] }),
            )
            .unwrap();
        assert_eq!(
            back["id"], a["id"],
            "reopening the occurrence returns the Task"
        );
        assert_eq!(back["due_date"], "2026-10-21");
        assert_eq!(back["due_time"], "09:00");
        assert_eq!(
            back["notes"], "Water:\n- [x] ferns\n- [ ] cactus",
            "the ticks come back"
        );
        assert_eq!(back["repeat_every"], 3);
        let copies = occurrences_of(&app, &a);
        assert_eq!(copies.len(), 1);
        assert_eq!(copies[0]["deleted"], true, "the copy is deleted (soft)");
        let ops: Vec<String> = log(&app).into_iter().skip(before).map(|l| l.0).collect();
        assert_eq!(ops, ["tick", "delete"]);
        // Nothing is left in Done.
        let day = local(DEFAULT_TODAY, DEFAULT_NOW);
        let list: Value = app
            .invoke(
                "todo_done",
                json!({ "before": null, "days": 7, "local": day }),
            )
            .unwrap();
        assert_eq!(list["days"], json!([]));
    }

    #[test]
    fn a_rewind_keeps_notes_that_were_edited_since() {
        let app = TestApp::new();
        let a = repeating(
            &app,
            "Water the plants",
            "2026-10-21",
            rule("day", 3, Value::Null, "fixed"),
        );
        let done = complete_on(&app, &a, DEFAULT_TODAY);
        app.with_db(|c| {
            c.execute(
                "UPDATE todo_tasks SET notes = 'New plan' WHERE id = ?1",
                [a["id"].as_str().unwrap()],
            )
            .unwrap();
        });
        let back: Value = app
            .invoke(
                "todo_task_reopen",
                json!({ "id": done["occurrence"]["id"] }),
            )
            .unwrap();
        assert_eq!(back["notes"], "New plan");
        assert_eq!(back["due_date"], "2026-10-21");
    }

    #[test]
    fn only_the_latest_occurrence_can_be_reopened() {
        let app = TestApp::new();
        let a = repeating(
            &app,
            "Water the plants",
            "2026-10-21",
            rule("day", 1, Value::Null, "fixed"),
        );
        let first = complete_on(&app, &a, DEFAULT_TODAY);
        let second = complete_on(&app, &a, "2026-10-22");
        assert_eq!(second["task"]["due_date"], "2026-10-23");
        let before = (log(&app).len(), open_task(&app, &a)["due_date"].clone());
        let err = app
            .invoke::<Value>(
                "todo_task_reopen",
                json!({ "id": first["occurrence"]["id"] }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "validation");
        assert!(err["message"].as_str().unwrap().contains("latest"), "{err}");
        assert_eq!(
            (log(&app).len(), open_task(&app, &a)["due_date"].clone()),
            before
        );
        // The newest one still rewinds, and then the older one becomes the latest.
        let back: Value = app
            .invoke(
                "todo_task_reopen",
                json!({ "id": second["occurrence"]["id"] }),
            )
            .unwrap();
        assert_eq!(back["due_date"], "2026-10-22");
        let back: Value = app
            .invoke(
                "todo_task_reopen",
                json!({ "id": first["occurrence"]["id"] }),
            )
            .unwrap();
        assert_eq!(back["due_date"], "2026-10-21");
    }

    #[test]
    fn skipping_moves_the_date_on_and_records_nothing() {
        let app = TestApp::new();
        let a = repeating(
            &app,
            "Water the plants",
            "2026-10-21",
            rule("day", 3, Value::Null, "fixed"),
        );
        let before = ops_since(&app, 0).len();
        let skipped: Value = app
            .invoke(
                "todo_task_skip",
                json!({ "id": id_of(&a), "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
            )
            .unwrap();
        assert_eq!(skipped["due_date"], "2026-10-24");
        assert_eq!(skipped["due_time"], "09:00");
        assert_eq!(skipped["notes"], a["notes"], "the Checklist is not reset");
        assert!(
            occurrences_of(&app, &a).is_empty(),
            "no occurrence is recorded"
        );
        assert_eq!(ops_since(&app, before), [pair("update", "todo_task")]);
        let plain = create(&app, "plain").unwrap();
        let err = app
            .invoke::<Value>(
                "todo_task_skip",
                json!({ "id": id_of(&plain), "local": local(DEFAULT_TODAY, DEFAULT_NOW) }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "validation");
    }

    #[test]
    fn deleting_a_repeating_task_keeps_its_occurrences_in_done() {
        let app = TestApp::new();
        let a = repeating(
            &app,
            "Water the plants",
            "2026-10-21",
            rule("day", 1, Value::Null, "fixed"),
        );
        let first = complete_on(&app, &a, DEFAULT_TODAY);
        complete_on(&app, &a, "2026-10-22");
        app.invoke::<()>("todo_task_delete", json!({ "id": a["id"] }))
            .unwrap();
        let day = local("2026-10-22", DEFAULT_NOW);
        let done: Value = app
            .invoke(
                "todo_done",
                json!({ "before": null, "days": 7, "local": day }),
            )
            .unwrap();
        let shown: usize = done["days"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["tasks"].as_array().unwrap().len())
            .sum();
        assert_eq!(shown, 2, "both occurrences stay");
        let err = app
            .invoke::<Value>(
                "todo_task_reopen",
                json!({ "id": first["occurrence"]["id"] }),
            )
            .unwrap_err();
        assert_eq!(
            err["code"], "validation",
            "an occurrence of a deleted Task explains instead of failing"
        );
        let err = app
            .invoke::<Value>(
                "todo_task_complete",
                json!({ "id": id_of(&a), "local": day }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "not_found");
    }

    #[test]
    fn done_pages_by_day_newest_first_with_a_cursor() {
        let app = TestApp::new();
        // Five Tasks done on four days; closing times are UTC and the device is at +05:30.
        let stamps = [
            ("a", "2026-10-21 10:00:00"),
            ("b", "2026-10-21 12:00:00"),
            ("c", "2026-10-20 08:00:00"),
            ("d", "2026-10-18 20:00:00"),
            ("e", "2026-10-15 04:00:00"),
        ];
        for (title, at) in stamps {
            let t = create(&app, title).unwrap();
            complete_on(&app, &t, DEFAULT_TODAY);
            let id = t["id"].as_str().unwrap();
            app.with_db(|c| {
                c.execute(
                    &format!(
                        "UPDATE todo_tasks SET closed_at = CAST(strftime('%s', '{at}') AS INTEGER) * 1000 WHERE id = '{id}'"
                    ),
                    [],
                )
                .unwrap();
            });
        }
        let kolkata = json!({ "today": "2026-10-21", "now": "14:05", "offset_min": 330 });
        let page = |before: Value, days: u32| -> Value {
            app.invoke(
                "todo_done",
                json!({ "before": before, "days": days, "local": kolkata }),
            )
            .unwrap()
        };
        let shape = |v: &Value| -> Value {
            Value::Array(
                v["days"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| json!([d["date"], titles(&d["tasks"])]))
                    .collect(),
            )
        };
        // d (18th 20:00 UTC) is the 19th at 01:30 for the device.
        let first = page(Value::Null, 2);
        assert_eq!(
            shape(&first),
            json!([["2026-10-21", ["b", "a"]], ["2026-10-20", ["c"]]])
        );
        assert_eq!(first["next_cursor"], "2026-10-20");
        let second = page(first["next_cursor"].clone(), 2);
        assert_eq!(
            shape(&second),
            json!([["2026-10-19", ["d"]], ["2026-10-15", ["e"]]])
        );
        assert_eq!(
            second["next_cursor"],
            Value::Null,
            "the last page has no cursor"
        );
        let all = page(Value::Null, 60);
        assert_eq!(all["days"].as_array().unwrap().len(), 4);
        assert_eq!(all["next_cursor"], Value::Null);
        let none = page(json!("2026-10-15"), 2);
        assert_eq!(none["days"], json!([]));
        assert_eq!(none["next_cursor"], Value::Null);
        assert_eq!(
            app.invoke::<Value>(
                "todo_done",
                json!({ "before": "nope", "days": 2, "local": kolkata })
            )
            .unwrap_err()["code"],
            "validation"
        );
    }

    #[test]
    fn a_task_done_after_midnight_on_the_device_belongs_to_the_next_day() {
        let app = TestApp::new();
        let t = create(&app, "late").unwrap();
        complete_on(&app, &t, DEFAULT_TODAY);
        app.with_db(|c| {
            c.execute("UPDATE todo_tasks SET closed_at = CAST(strftime('%s', '2026-10-20 19:00:00') AS INTEGER) * 1000", [])
                .unwrap();
        });
        let day = |offset: i32| -> Value {
            let local = json!({ "today": "2026-10-21", "now": "14:05", "offset_min": offset });
            let v: Value = app
                .invoke(
                    "todo_done",
                    json!({ "before": null, "days": 3, "local": local }),
                )
                .unwrap();
            v["days"][0]["date"].clone()
        };
        assert_eq!(day(0), "2026-10-20");
        assert_eq!(day(330), "2026-10-21");
        assert_eq!(day(-480), "2026-10-20");
    }

    // Shared rule fixtures: the same file runs against the browser mock in
    // src/lib/ipc/mock-todo.test.ts (ADR 0009). Keep `run_rule_case` in step with `run` there.
    const RULES: &str = include_str!("../../src/lib/ipc/fixtures/todo-rules.json");
    const ORDER_RULES: &str = include_str!("../../src/lib/ipc/fixtures/todo-order-rules.json");

    /// The All view as `[[group name, [titles]]]`, the shape the fixtures expect.
    fn all_groups(app: &TestApp) -> Value {
        let view: Value = app.invoke("todo_all", json!({ "local": clock() })).unwrap();
        let sidebar: Value = app
            .invoke("todo_lists", json!({ "local": clock() }))
            .unwrap();
        let groups = view["groups"].as_array().unwrap().iter().map(|g| {
            let name = sidebar["lists"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["list"]["id"] == g["key"]["list_id"])
                .map_or("Inbox", |e| e["list"]["name"].as_str().unwrap());
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

    /// Every open repeating Task as `[title, freq, every, days, mode]`, in the All view's order.
    fn repeats(app: &TestApp) -> Value {
        let view: Value = app.invoke("todo_all", json!({ "local": clock() })).unwrap();
        let rows = view["groups"].as_array().unwrap().iter().flat_map(|g| {
            let open = g["tasks"].as_array().unwrap().iter();
            open.filter(|t| !t["repeat_freq"].is_null()).map(|t| {
                json!([
                    t["title"],
                    t["repeat_freq"],
                    t["repeat_every"],
                    t["repeat_days"],
                    t["repeat_mode"]
                ])
            })
        });
        Value::Array(rows.collect())
    }

    /// The titles in Done, newest first.
    fn done_titles(app: &TestApp, local: &Value) -> Value {
        let view: Value = app
            .invoke(
                "todo_done",
                json!({ "before": null, "days": 60, "local": local }),
            )
            .unwrap();
        Value::Array(
            view["days"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|d| {
                    d["tasks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|t| t["title"].clone())
                })
                .collect(),
        )
    }

    /// The titles of Won't do Tasks in Done, newest first.
    fn wont_titles(app: &TestApp, local: &Value) -> Value {
        let view: Value = app
            .invoke(
                "todo_done",
                json!({ "before": null, "days": 60, "local": local }),
            )
            .unwrap();
        Value::Array(
            view["days"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|d| d["tasks"].as_array().unwrap().iter())
                .filter(|t| t["state"] == "wont")
                .map(|t| t["title"].clone())
                .collect(),
        )
    }

    /// Every open Task as `[title, priority]`, in the All view's order.
    fn priorities(app: &TestApp) -> Value {
        let view: Value = app.invoke("todo_all", json!({ "local": clock() })).unwrap();
        let rows = view["groups"].as_array().unwrap().iter().flat_map(|g| {
            g["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| json!([t["title"], t["priority"]]))
        });
        Value::Array(rows.collect())
    }

    /// `[[owner, [bodies]]]` for every live created Task, then every live occurrence copy
    /// (`title (occurrence)`); an edited comment has a `*` after it.
    fn comment_rows(app: &TestApp, tasks: &[Value], occurrences: &[Value]) -> Value {
        let rows = tasks
            .iter()
            .map(|id| (id, ""))
            .chain(occurrences.iter().map(|id| (id, " (occurrence)")))
            .filter_map(|(id, suffix)| {
                let task: Value = app.invoke("todo_task", json!({ "id": id })).ok()?;
                let list: Value = app.invoke("todo_comments", json!({ "taskId": id })).ok()?;
                let bodies: Vec<String> = list
                    .as_array()?
                    .iter()
                    .map(|c| {
                        let mark = if c["edited"] == true { "*" } else { "" };
                        format!("{}{mark}", c["body"].as_str().unwrap_or_default())
                    })
                    .collect();
                Some(json!([
                    format!("{}{suffix}", task["title"].as_str()?),
                    bodies
                ]))
            });
        Value::Array(rows.collect())
    }

    /// The lines of one Task's History, newest first; none for a Task that is gone.
    fn history_lines(app: &TestApp, id: Option<&Value>) -> Value {
        let lines: Value = id
            .and_then(|id| app.invoke("todo_task_history", json!({ "id": id })).ok())
            .unwrap_or(json!([]));
        Value::Array(
            lines
                .as_array()
                .unwrap()
                .iter()
                .map(|l| l["text"].clone())
                .collect(),
        )
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
        let view: Value = app.invoke("todo_all", json!({ "local": clock() })).unwrap();
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
        json!({ "overdue": titles(&overdue_of(&view)), "tasks": titles(&tasks_of(&view)) })
    }

    /// Upcoming as `{ overdue, days: [[date, titles]] }`.
    fn upcoming_groups(app: &TestApp, local: &Value) -> Value {
        let view: Value = app
            .invoke("todo_upcoming", json!({ "local": local }))
            .unwrap();
        let days = view["groups"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|g| g["key"]["kind"] == "day");
        json!({
            "overdue": titles(&overdue_of(&view)),
            "days": days
                .map(|d| json!([d["key"]["date"], titles(&d["tasks"])]))
                .collect::<Vec<_>>(),
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

    /// A fixture's Repeat (`weekdays` or `anchor` instead of the wire's `days`) as the wire shape.
    fn wire_repeat(v: &Value) -> Value {
        if v.is_null() {
            return Value::Null;
        }
        let days = if v["freq"] == "week" {
            v["weekdays"].as_array().map_or(0, |ds| {
                ds.iter().fold(0, |m, d| m | (1u64 << d.as_u64().unwrap()))
            })
        } else {
            v["anchor"].as_u64().unwrap_or(0)
        };
        json!({ "freq": v["freq"], "every": v["every"], "days": days, "mode": v["mode"] })
    }

    // One match arm per fixture op: it reads best as a single table.
    #[allow(clippy::too_many_lines)]
    fn run_rule_case(c: &Value) -> Value {
        let app = TestApp::new();
        // The cases predate grouping: they read Today flat unless they choose otherwise.
        app.set_pref("todo.group_today", json!("none"));
        let mut tasks: Vec<Value> = Vec::new();
        let mut lists: Vec<Value> = Vec::new();
        let mut deletions: Vec<Value> = Vec::new();
        let mut moves: Vec<Value> = Vec::new();
        let mut occurrences: Vec<Value> = Vec::new();
        let mut comments: Vec<Value> = Vec::new();
        // The change-log ops each `reorder` step recorded.
        let mut reorder_ops: Vec<Value> = Vec::new();
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
                        "repeat": wire_repeat(&step["repeat"]), "local": clock(step)
                    }),
                ),
                "reopen_occurrence" => app.invoke(
                    "todo_task_reopen",
                    json!({ "id": pick(&step["occurrence"], &occurrences) }),
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
                "priority" => app.invoke(
                    "todo_task_set_priority",
                    json!({ "id": task, "priority": step["priority"] }),
                ),
                "update" => app.invoke(
                    "todo_task_update",
                    json!({
                        "id": task, "title": step["title"], "notes": step["notes"],
                        "sessionId": step["session"]
                    }),
                ),
                "comment_add" => app.invoke(
                    "todo_comment_add",
                    json!({ "taskId": task, "body": step["body"] }),
                ),
                "comment_edit" => app.invoke(
                    "todo_comment_edit",
                    json!({ "id": pick(&step["comment"], &comments), "body": step["body"] }),
                ),
                "comment_delete" | "comment_restore" => app.invoke(
                    &format!("todo_{op}"),
                    json!({ "id": pick(&step["comment"], &comments) }),
                ),
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
                "pref" => {
                    app.set_pref(
                        &format!("todo.{}", step["key"].as_str().unwrap()),
                        step["value"].clone(),
                    );
                    Ok(Value::Null)
                }
                "reorder" => {
                    let logged = ops_since(&app, 0).len();
                    // A fixture's `{kind: list, list: n}` names the n-th created List.
                    let target: Vec<Value> = step["target"]
                        .as_array()
                        .map(|keys| {
                            keys.iter()
                                .map(|k| {
                                    if k["kind"] == "list" {
                                        json!({ "kind": "list", "list_id": pick(&k["list"], &lists) })
                                    } else {
                                        k.clone()
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let out = app.invoke(
                        "todo_task_reorder",
                        json!({
                            "id": task,
                            "before": pick(&step["before"], &tasks),
                            "after": pick(&step["after"], &tasks),
                            "target": target,
                            "local": clock(step)
                        }),
                    );
                    reorder_ops.push(op_names(&app, logged));
                    out
                }
                "list_reorder" => app.invoke(
                    "todo_list_reorder",
                    json!({
                        "id": list,
                        "before": pick(&step["before"], &lists),
                        "after": pick(&step["after"], &lists)
                    }),
                ),
                "list_sort" => app.invoke(
                    "todo_list_set_sort",
                    json!({ "id": list, "sort": step["sort"] }),
                ),
                "list_group" => app.invoke(
                    "todo_list_set_group",
                    json!({ "id": list, "groupBy": step["group_by"] }),
                ),
                _ => app.invoke(
                    &format!("todo_task_{op}"),
                    json!({ "id": task, "local": clock(step) }),
                ),
            };
            match outcome {
                Ok(out) => {
                    match op {
                        "create" => tasks.push(out["id"].clone()),
                        "list_create" => lists.push(out["id"].clone()),
                        "list_delete" => deletions.push(out),
                        "comment_add" => comments.push(out["id"].clone()),
                        "move_overdue" => moves.push(out),
                        "complete" if !out["occurrence"].is_null() => {
                            occurrences.push(out["occurrence"]["id"].clone());
                        }
                        _ => {}
                    }
                    results.push(json!("ok"));
                }
                Err(e) => results.push(e["code"].clone()),
            }
        }
        let history_of = usize::try_from(c["history_of"].as_u64().unwrap_or(0)).unwrap();
        let mut out = rule_outputs(&app, results, &clock(&Value::Null));
        out["views"] = view_groups(&app, &clock(&Value::Null), &lists);
        out["reorder_ops"] = Value::Array(reorder_ops);
        out["comments"] = comment_rows(&app, &tasks, &occurrences);
        out["history"] = history_lines(&app, tasks.get(history_of));
        out
    }

    /// The ops of the Todo changes from the n-th on, oldest first.
    fn op_names(app: &TestApp, logged: usize) -> Value {
        Value::Array(
            ops_since(app, logged)
                .into_iter()
                .map(|(op, _)| json!(op))
                .collect(),
        )
    }

    /// A group's name in the fixtures: `plain`, `overdue`, `p1`, `list:Home` (`list:Inbox`),
    /// `day:2026-10-22`, `due:today`, `no_date_in_list`.
    fn group_label(key: &Value, list_names: &Value) -> String {
        let kind = key["kind"].as_str().unwrap();
        match kind {
            "priority" => format!("p{}", key["priority"]),
            "list" => format!(
                "list:{}",
                key["list_id"]
                    .as_str()
                    .map_or("Inbox", |id| list_names[id].as_str().unwrap_or("?"))
            ),
            "day" => format!("day:{}", key["date"].as_str().unwrap()),
            "due" => format!("due:{}", key["bucket"].as_str().unwrap()),
            other => other.to_owned(),
        }
    }

    /// Every view's groups as `{view: [[group name, [titles]]]}`: today, upcoming, inbox, all, and
    /// `list0`, `list1`, … for the created Lists that still exist.
    fn view_groups(app: &TestApp, view_clock: &Value, lists: &[Value]) -> Value {
        let sidebar: Value = app
            .invoke("todo_lists", json!({ "local": view_clock }))
            .unwrap();
        let names: serde_json::Map<String, Value> = sidebar["lists"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e["list"]["id"].as_str().unwrap().to_owned(),
                    e["list"]["name"].clone(),
                )
            })
            .collect();
        let names = Value::Object(names);
        let shape = |view: &Value| {
            Value::Array(
                view["groups"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|g| json!([group_label(&g["key"], &names), titles(&g["tasks"])]))
                    .collect(),
            )
        };
        let local = json!({ "local": view_clock });
        let mut out = serde_json::Map::new();
        for (name, command) in [
            ("today", "todo_today"),
            ("upcoming", "todo_upcoming"),
            ("inbox", "todo_inbox"),
            ("all", "todo_all"),
        ] {
            let view: Value = app.invoke(command, local.clone()).unwrap();
            out.insert(name.to_owned(), shape(&view));
        }
        for (i, id) in lists.iter().enumerate() {
            if let Ok(view) =
                app.invoke::<Value>("todo_list", json!({ "id": id, "local": view_clock }))
            {
                out.insert(format!("list{i}"), shape(&view));
            }
        }
        Value::Object(out)
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
            "repeats": repeats(app),
            "done": done_titles(app, view_clock),
            "wont": wont_titles(app, view_clock),
            "priorities": priorities(app),
        })
    }

    #[test]
    fn the_shared_rule_fixtures_hold() {
        let mut cases = Vec::new();
        for file in [RULES, ORDER_RULES] {
            let rules: Value = serde_json::from_str(file).unwrap();
            cases.extend(rules["cases"].as_array().unwrap().clone());
        }
        assert!(!cases.is_empty());
        // Every miss is reported, not just the first.
        let mut misses = Vec::new();
        for c in &cases {
            let actual = run_rule_case(c);
            let who = format!(
                "{}: {}",
                c["rule"].as_str().unwrap(),
                c["name"].as_str().unwrap()
            );
            for (key, want) in c["expect"].as_object().unwrap() {
                // `views` holds one entry per view: a case names only the ones it checks.
                if key == "views" {
                    for (view, groups) in want.as_object().unwrap() {
                        if &actual["views"][view] != groups {
                            misses.push(format!(
                                "{who} (views.{view})\n  want {groups}\n  got  {}",
                                actual["views"][view]
                            ));
                        }
                    }
                } else if &actual[key] != want {
                    misses.push(format!(
                        "{who} ({key})\n  want {want}\n  got  {}",
                        actual[key]
                    ));
                }
            }
        }
        assert!(misses.is_empty(), "{}", misses.join("\n"));
    }
}
