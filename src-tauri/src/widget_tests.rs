//! Tests for the Todo widget's bridge, driven at its one JSON entry point (`widget::handle`)
//! against a temporary database, as the widget's Kotlin side would call it (ADR 0009, ADR 0014).

use std::path::{Path, PathBuf};

use mnemora_core::db;
use mnemora_core::device::DeviceSettings;
use mnemora_core::env::Environment;
use mnemora_core::module::Module;
use mnemora_core::paths::AppPaths;
use mnemora_todo::{LocalNow, NewTaskDetails, NewWhen};
use serde_json::{Value, json};

use crate::test_support::TestApp;
use crate::widget::source::{Live, Standalone};
use crate::widget::{Source, handle};

const TODAY: &str = "2026-10-21"; // a Wednesday

fn at(today: &str) -> LocalNow {
    LocalNow {
        today: today.into(),
        now: "14:05".into(),
        offset_min: 0,
    }
}

/// An Android-like data dir holding (maybe) the app's database for Production.
struct Phone {
    _dir: tempfile::TempDir,
    base: PathBuf,
    paths: AppPaths,
}

impl Phone {
    /// The folder exists, the app has not run yet.
    fn fresh() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("data/user/0/com.hemang.mnemora");
        std::fs::create_dir_all(&base).unwrap();
        let paths = AppPaths::resolve(Environment::Production, &base, None);
        Self {
            _dir: dir,
            base,
            paths,
        }
    }

    /// The app has run once: a database with every Module migrated.
    fn migrated() -> (Self, db::Connection) {
        let phone = Self::fresh();
        let conn = phone.make_db(&phone.paths.db_file, |_| true);
        (phone, conn)
    }

    /// Creates and migrates a database with the Modules `keep` accepts.
    fn make_db(&self, file: &Path, keep: impl Fn(&str) -> bool) -> db::Connection {
        let modules = crate::modules();
        let refs: Vec<&dyn Module> = modules
            .iter()
            .filter(|m| keep(m.id()))
            .map(AsRef::as_ref)
            .collect();
        let mut conn = db::open(file).unwrap();
        db::migrate::run(&mut conn, &refs, &self.paths.backup_dir).unwrap();
        conn
    }

    fn bridge(&self) -> Standalone {
        Standalone::new(Environment::Production, self.base.clone())
    }
}

fn call(source: &dyn Source, request: &Value) -> Value {
    serde_json::from_str(&handle(source, &request.to_string())).unwrap()
}

fn snapshot(source: &dyn Source, today: &str, now: &str) -> Value {
    let reply = call(
        source,
        &json!({
            "method": "snapshot", "view": "today",
            "local": { "today": today, "now": now, "offset_min": 0 },
        }),
    );
    assert!(reply["ok"].is_object(), "{reply}");
    reply["ok"].clone()
}

fn add(
    conn: &mut db::Connection,
    title: &str,
    date: Option<&str>,
    time: Option<&str>,
    priority: Option<u32>,
    list: Option<&str>,
) -> mnemora_todo::Task {
    // Created on the day it is due, so a past date is allowed.
    let when = date.map(|d| NewWhen {
        date: Some(d.into()),
        time: time.map(Into::into),
        local: at(d),
    });
    let details = NewTaskDetails {
        priority,
        ..NewTaskDetails::default()
    };
    mnemora_todo::create_task(conn, title, list, when.as_ref(), Some(&details)).unwrap()
}

fn rows(group: &Value) -> Vec<(String, String)> {
    group["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["title"].as_str().unwrap().to_owned(),
                r["time"]
                    .as_str()
                    .or_else(|| r["day"].as_str())
                    .unwrap_or("")
                    .to_owned(),
            )
        })
        .collect()
}

fn headings(snap: &Value) -> Vec<String> {
    snap["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["heading"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn before_the_app_ever_ran_the_reply_is_not_set_up_and_nothing_is_created() {
    let phone = Phone::fresh();
    let reply = call(
        &phone.bridge(),
        &json!({ "method": "snapshot", "view": "today", "local": at_json(TODAY) }),
    );
    assert_eq!(reply, json!({ "status": "not_set_up" }));
    assert!(!phone.paths.db_file.exists());
    assert!(!phone.paths.data_dir.exists(), "not even the folder");
    assert_eq!(std::fs::read_dir(&phone.base).unwrap().count(), 0);
}

fn at_json(today: &str) -> Value {
    json!({ "today": today, "now": "14:05", "offset_min": 0 })
}

#[test]
fn a_pending_migration_means_needs_update_and_nothing_is_migrated() {
    let phone = Phone::fresh();
    // An older app: core and the Journal only, so Todo's migrations are all pending.
    let conn = phone.make_db(&phone.paths.db_file, |id| id != "todo");
    drop(conn);
    let reply = call(
        &phone.bridge(),
        &json!({ "method": "snapshot", "view": "today", "local": at_json(TODAY) }),
    );
    assert_eq!(reply, json!({ "status": "needs_update" }));
    let conn = db::open_existing(&phone.paths.db_file).unwrap().unwrap();
    let todo_tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'todo_tasks'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(todo_tables, 0, "the widget never migrates");
}

#[test]
fn the_database_is_found_where_the_device_setting_moved_it() {
    let phone = Phone::fresh();
    let moved = phone.base.join("elsewhere");
    let device = DeviceSettings {
        data_dir: Some(moved.to_string_lossy().into_owned()),
        ..DeviceSettings::default()
    };
    device.save(&phone.paths.device_file()).unwrap();
    let relocated = AppPaths::resolve(Environment::Production, &phone.base, Some(&moved));
    let mut conn = phone.make_db(&relocated.db_file, |_| true);
    add(&mut conn, "Moved house", Some(TODAY), None, None, None);

    let snap = snapshot(&phone.bridge(), TODAY, "14:05");
    assert_eq!(snap["groups"][0]["rows"][0]["title"], "Moved house");
}

#[test]
fn a_sandbox_build_refuses_a_production_data_dir() {
    let phone = Phone::fresh();
    let bridge = Standalone::new(Environment::Sandbox, phone.base.clone());
    let reply = call(
        &bridge,
        &json!({ "method": "snapshot", "view": "today", "local": at_json(TODAY) }),
    );
    assert_eq!(reply["err"]["code"], "internal", "{reply}");
}

#[test]
fn today_shows_overdue_then_today_in_the_apps_order_with_done_hidden() {
    let (phone, mut conn) = Phone::migrated();
    let home = mnemora_todo::create_list(&mut conn, "Home").unwrap();
    mnemora_todo::set_list_emoji(&mut conn, &home.id, Some("🏠")).unwrap();
    add(
        &mut conn,
        "Reply to Aarav",
        Some("2026-10-20"),
        None,
        None,
        None,
    );
    add(
        &mut conn,
        "Renew passport photos",
        Some("2026-10-19"),
        Some("10:00"),
        Some(2),
        None,
    );
    add(
        &mut conn,
        "Stand-up notes",
        Some(TODAY),
        Some("09:30"),
        None,
        None,
    );
    add(
        &mut conn,
        "Pay **electricity** bill",
        Some(TODAY),
        None,
        Some(1),
        Some(&home.id),
    );
    add(
        &mut conn,
        "Call mum",
        Some(TODAY),
        Some("18:00"),
        Some(3),
        None,
    );
    add(
        &mut conn,
        "Dentist",
        Some("2026-10-22"),
        Some("11:00"),
        None,
        None,
    );
    add(&mut conn, "Undated", None, None, None, None);
    let done = add(&mut conn, "Already done", Some(TODAY), None, None, None);
    mnemora_todo::complete_task(&mut conn, &done.id, &at(TODAY)).unwrap();

    let snap = snapshot(&phone.bridge(), TODAY, "14:05");

    assert_eq!(snap["view"], "today");
    assert_eq!(snap["title"], "Today");
    assert_eq!(headings(&snap), ["Overdue", "Today"]);
    assert_eq!(snap["groups"][0]["tone"], "overdue");
    assert_eq!(snap["groups"][1]["tone"], "normal");
    assert_eq!(snap["empty"], Value::Null);
    // Overdue: oldest date first, each with the day it was due where a time would go.
    assert_eq!(
        rows(&snap["groups"][0]),
        [
            ("Renew passport photos".into(), "Mon".into()),
            ("Reply to Aarav".into(), "Yesterday".into()),
        ]
    );
    // Today, as the app's default Today lists it (by Priority, then timed Tasks by time);
    // Markdown marks gone.
    assert_eq!(
        rows(&snap["groups"][1]),
        [
            ("Pay electricity bill".into(), String::new()),
            ("Call mum".into(), "18:00".into()),
            ("Stand-up notes".into(), "09:30".into()),
        ]
    );
}

#[test]
fn today_follows_the_apps_grouping_preference() {
    let (phone, mut conn) = Phone::migrated();
    add(
        &mut conn,
        "Untimed urgent",
        Some(TODAY),
        None,
        Some(1),
        None,
    );
    add(
        &mut conn,
        "Late timed",
        Some(TODAY),
        Some("18:00"),
        None,
        None,
    );
    add(
        &mut conn,
        "Early timed",
        Some(TODAY),
        Some("09:30"),
        None,
        None,
    );
    conn.execute(
        "INSERT INTO settings(module, key, value, updated_at) VALUES ('todo', 'group_today', '\"none\"', 0)",
        [],
    )
    .unwrap();

    let snap = snapshot(&phone.bridge(), TODAY, "14:05");
    let titles: Vec<_> = rows(&snap["groups"][0]).into_iter().map(|r| r.0).collect();
    assert_eq!(titles, ["Early timed", "Late timed", "Untimed urgent"]);
}

#[test]
fn a_row_carries_its_priority_ring_list_chip_and_overdue_flag() {
    let (phone, mut conn) = Phone::migrated();
    let home = mnemora_todo::create_list(&mut conn, "Home").unwrap();
    mnemora_todo::set_list_emoji(&mut conn, &home.id, Some("🏠")).unwrap();
    let bare = mnemora_todo::create_list(&mut conn, "Errands").unwrap();
    add(&mut conn, "Late", Some("2026-10-20"), None, Some(1), None);
    add(
        &mut conn,
        "Ringed",
        Some(TODAY),
        None,
        Some(2),
        Some(&home.id),
    );
    add(
        &mut conn,
        "Violet",
        Some(TODAY),
        None,
        Some(3),
        Some(&bare.id),
    );
    add(&mut conn, "Plain", Some(TODAY), None, None, None);

    let snap = snapshot(&phone.bridge(), TODAY, "14:05");
    let late = &snap["groups"][0]["rows"][0];
    assert_eq!(
        (late["ring"].as_str(), late["overdue"].as_bool()),
        (Some("p1"), Some(true))
    );
    assert_eq!(late["list"], Value::Null);
    let today = snap["groups"][1]["rows"].as_array().unwrap();
    let by_title = |t: &str| today.iter().find(|r| r["title"] == t).unwrap();
    assert_eq!(by_title("Ringed")["ring"], "p2");
    assert_eq!(by_title("Ringed")["list"], "🏠 Home");
    assert_eq!(by_title("Ringed")["overdue"], false);
    assert_eq!(by_title("Violet")["ring"], "p3");
    assert_eq!(by_title("Violet")["list"], "Errands");
    assert_eq!(by_title("Plain")["ring"], "none");
    assert_eq!(by_title("Plain")["list"], Value::Null);
    assert!(
        by_title("Plain")["id"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
}

#[test]
fn the_header_counts_what_is_left_or_in_red_what_is_overdue() {
    let (phone, mut conn) = Phone::migrated();
    add(&mut conn, "One", Some(TODAY), None, None, None);
    add(&mut conn, "Two", Some(TODAY), None, None, None);
    let snap = snapshot(&phone.bridge(), TODAY, "14:05");
    assert_eq!(snap["count"], json!({ "text": "2 left", "tone": "normal" }));

    add(&mut conn, "Late", Some("2026-10-20"), None, None, None);
    let snap = snapshot(&phone.bridge(), TODAY, "14:05");
    assert_eq!(
        snap["count"],
        json!({ "text": "1 overdue", "tone": "overdue" })
    );
}

#[test]
fn times_follow_the_twelve_hour_setting() {
    let (phone, mut conn) = Phone::migrated();
    add(&mut conn, "Evening", Some(TODAY), Some("18:00"), None, None);
    conn.execute(
        "INSERT INTO settings(module, key, value, updated_at) VALUES ('core', 'time_format', '\"12h\"', 0)",
        [],
    )
    .unwrap();
    let snap = snapshot(&phone.bridge(), TODAY, "14:05");
    assert_eq!(snap["groups"][0]["rows"][0]["time"], "6:00 PM");
}

#[test]
fn an_empty_today_says_what_comes_next() {
    let (phone, mut conn) = Phone::migrated();
    add(
        &mut conn,
        "Dentist",
        Some("2026-10-22"),
        Some("11:00"),
        None,
        None,
    );
    add(&mut conn, "Later", Some("2026-10-30"), None, None, None);
    let snap = snapshot(&phone.bridge(), TODAY, "14:05");
    assert_eq!(snap["groups"], json!([]));
    assert_eq!(snap["count"], Value::Null);
    assert_eq!(
        snap["empty"],
        json!({ "title": "Nothing due today", "line": "Next: Dentist, tomorrow 11:00" })
    );
}

#[test]
fn an_empty_today_with_nothing_ahead_says_so() {
    let (phone, mut conn) = Phone::migrated();
    add(&mut conn, "Undated", None, None, None, None);
    let snap = snapshot(&phone.bridge(), TODAY, "14:05");
    assert_eq!(
        snap["empty"],
        json!({ "title": "Nothing due today", "line": "Nothing coming up" })
    );
}

#[test]
fn the_next_task_names_a_weekday_or_a_date_when_it_is_further_off() {
    let (phone, mut conn) = Phone::migrated();
    add(
        &mut conn,
        "Car service",
        Some("2026-10-24"),
        None,
        None,
        None,
    );
    let snap = snapshot(&phone.bridge(), TODAY, "14:05");
    assert_eq!(snap["empty"]["line"], "Next: Car service, Sat");
}

#[test]
fn the_day_rolls_over_with_the_clock_the_widget_sends() {
    let (phone, mut conn) = Phone::migrated();
    add(&mut conn, "Yesterday's job", Some(TODAY), None, None, None);
    let next_day = snapshot(&phone.bridge(), "2026-10-22", "00:10");
    assert_eq!(headings(&next_day), ["Overdue"]);
    assert_eq!(next_day["groups"][0]["rows"][0]["day"], "Yesterday");
}

#[test]
fn bad_requests_get_an_err_and_never_a_panic() {
    let (phone, _conn) = Phone::migrated();
    let bridge = phone.bridge();
    for request in [
        "",
        "not json",
        "[]",
        "{}",
        r#"{"method":"fly"}"#,
        r#"{"method":"snapshot"}"#,
        r#"{"method":"snapshot","view":"narnia","local":{"today":"2026-10-21","now":"14:05","offset_min":0}}"#,
        r#"{"method":"snapshot","view":"today","local":{"today":"garbage","now":"14:05","offset_min":0}}"#,
    ] {
        let reply: Value = serde_json::from_str(&handle(&bridge, request)).unwrap();
        assert!(reply["err"]["message"].is_string(), "{request} -> {reply}");
        assert!(reply["err"]["code"].is_string(), "{request} -> {reply}");
    }
}

#[test]
fn a_live_app_answers_from_its_own_connection() {
    // The TestApp database is in memory: only the app's own connection can see these Tasks.
    let app = TestApp::new();
    app.invoke::<Value>(
        "todo_task_create",
        json!({
            "title": "Seen live",
            "when": { "date": TODAY, "time": null, "local": at_json(TODAY) },
        }),
    )
    .unwrap();

    let live = Live::new(app.handle());
    assert!(live.is_ready());
    let snap = snapshot(&live, TODAY, "14:05");
    assert_eq!(snap["groups"][0]["rows"][0]["title"], "Seen live");
}

// ------------------------------------------------------------------ the write signal

/// Invokes `command` and returns the reply plus how many times the widget was signalled by it.
fn signals(app: &TestApp, command: &str, args: Value) -> (Result<Value, Value>, u32) {
    let before = app.widget_signals();
    let reply = app.invoke::<Value>(command, args);
    (reply, app.widget_signals() - before)
}

fn ok_once(app: &TestApp, command: &str, args: Value) -> Value {
    let (reply, sent) = signals(app, command, args);
    let value = reply.unwrap_or_else(|e| panic!("{command} failed: {e}"));
    assert_eq!(sent, 1, "{command} must signal the widget exactly once");
    value
}

fn silent(app: &TestApp, command: &str, args: Value) {
    let (reply, sent) = signals(app, command, args);
    reply.unwrap_or_else(|e| panic!("{command} failed: {e}"));
    assert_eq!(sent, 0, "{command} must not signal the widget");
}

#[test]
fn every_list_write_signals_the_widget_once() {
    let app = TestApp::new();
    let list_a = ok_once(&app, "todo_list_create", json!({ "name": "A" }));
    let list_b = ok_once(&app, "todo_list_create", json!({ "name": "B" }));
    ok_once(
        &app,
        "todo_list_rename",
        json!({ "id": list_a["id"], "name": "A2" }),
    );
    ok_once(
        &app,
        "todo_list_set_emoji",
        json!({ "id": list_a["id"], "emoji": "🏠" }),
    );
    ok_once(
        &app,
        "todo_list_set_sort",
        json!({ "id": list_a["id"], "sort": "title" }),
    );
    ok_once(
        &app,
        "todo_list_set_group",
        json!({ "id": list_a["id"], "groupBy": "priority" }),
    );
    ok_once(
        &app,
        "todo_list_reorder",
        json!({ "id": list_b["id"], "before": null, "after": list_a["id"] }),
    );
}

#[test]
fn every_task_write_signals_the_widget_once() {
    let app = TestApp::new();
    let local = at_json(TODAY);
    let when = json!({ "date": TODAY, "time": null, "local": local });
    let list_a = ok_once(&app, "todo_list_create", json!({ "name": "A" }));
    let list_b = ok_once(&app, "todo_list_create", json!({ "name": "B" }));

    let t1 = ok_once(
        &app,
        "todo_task_create",
        json!({ "title": "One", "listId": list_a["id"], "when": when }),
    );
    let t2 = ok_once(
        &app,
        "todo_task_create",
        json!({ "title": "Two", "when": when }),
    );
    let t3 = ok_once(&app, "todo_task_create", json!({ "title": "Three" }));
    let edit = json!({
        "title": "One!", "understood": null, "due_date": TODAY, "due_time": null,
        "repeat": null, "priority": 4, "list_id": list_a["id"],
    });
    let quick = json!({ "id": t1["id"], "edit": edit, "local": local });
    ok_once(&app, "todo_task_quick_edit", quick.clone());
    ok_once(&app, "todo_task_quick_edit_undo", quick);
    ok_once(
        &app,
        "todo_task_update",
        json!({ "id": t1["id"], "title": "One", "notes": "n", "sessionId": "s1" }),
    );
    ok_once(
        &app,
        "todo_task_set_priority",
        json!({ "id": t1["id"], "priority": 2 }),
    );
    ok_once(
        &app,
        "todo_task_set_when",
        json!({ "id": t2["id"], "date": "2026-10-23", "time": null, "repeat": null, "local": local }),
    );
    let change = json!({ "id": t2["id"], "due_date": TODAY, "due_time": null, "repeat": null });
    ok_once(
        &app,
        "todo_tasks_restore_when",
        json!({ "changes": [change] }),
    );
    ok_once(
        &app,
        "todo_tasks_move_overdue",
        json!({ "date": TODAY, "local": local }),
    );
    ok_once(
        &app,
        "todo_task_move",
        json!({ "id": t2["id"], "listId": list_b["id"] }),
    );
    ok_once(
        &app,
        "todo_task_reorder",
        json!({
            "id": t3["id"], "before": null, "after": t1["id"],
            "target": [], "local": local,
        }),
    );
    ok_once(
        &app,
        "todo_task_complete",
        json!({ "id": t1["id"], "local": local }),
    );
    ok_once(&app, "todo_task_reopen", json!({ "id": t1["id"] }));
    ok_once(
        &app,
        "todo_task_wont_do",
        json!({ "id": t3["id"], "local": local }),
    );
    ok_once(&app, "todo_task_reopen", json!({ "id": t3["id"] }));
    let repeating = ok_once(
        &app,
        "todo_task_create",
        json!({
            "title": "Water plants", "when": when,
            "details": { "repeat": { "freq": "day", "every": 1, "mode": "fixed" } },
        }),
    );
    ok_once(
        &app,
        "todo_task_skip",
        json!({ "id": repeating["id"], "local": local }),
    );
    ok_once(&app, "todo_task_delete", json!({ "id": t2["id"] }));
    ok_once(&app, "todo_task_restore", json!({ "id": t2["id"] }));
    let deletion = ok_once(
        &app,
        "todo_list_delete",
        json!({ "id": list_b["id"], "choice": { "kind": "move_to_inbox" } }),
    );
    ok_once(&app, "todo_list_restore", json!({ "deletion": deletion }));
}

#[test]
fn todo_reads_comments_and_settings_never_signal_the_widget() {
    let app = TestApp::new();
    let local = at_json(TODAY);
    let list = app
        .invoke::<Value>("todo_list_create", json!({ "name": "A" }))
        .unwrap();
    let task = app
        .invoke::<Value>("todo_task_create", json!({ "title": "One" }))
        .unwrap();
    let before = app.widget_signals();

    silent(&app, "todo_today", json!({ "local": local }));
    silent(&app, "todo_upcoming", json!({ "local": local }));
    silent(&app, "todo_inbox", json!({ "local": local }));
    silent(&app, "todo_all", json!({ "local": local }));
    silent(&app, "todo_lists", json!({ "local": local }));
    silent(
        &app,
        "todo_list",
        json!({ "id": list["id"], "local": local }),
    );
    silent(&app, "todo_counts", json!({ "local": local }));
    silent(&app, "todo_task", json!({ "id": task["id"] }));
    silent(&app, "todo_search", json!({ "query": "One" }));
    silent(&app, "todo_task_history", json!({ "id": task["id"] }));
    silent(
        &app,
        "todo_done",
        json!({ "before": null, "days": 3, "local": local }),
    );
    silent(&app, "todo_comments", json!({ "taskId": task["id"] }));
    let comment = app
        .invoke::<Value>(
            "todo_comment_add",
            json!({ "taskId": task["id"], "body": "hello" }),
        )
        .unwrap();
    silent(
        &app,
        "todo_comment_edit",
        json!({ "id": comment["id"], "body": "hi" }),
    );
    silent(&app, "todo_comment_delete", json!({ "id": comment["id"] }));
    silent(&app, "todo_comment_restore", json!({ "id": comment["id"] }));
    silent(
        &app,
        "todo_parse",
        json!({ "request": {
            "text": "Call mum tomorrow", "ignored": [], "prefer": {}, "frozen": [],
            "today": TODAY, "now": "14:05", "week_start": "mon",
        } }),
    );
    silent(
        &app,
        "settings_save",
        json!({ "changes": [{ "key": "core.theme", "value": "dark" }] }),
    );
    assert_eq!(app.widget_signals(), before);
}

#[test]
fn a_refused_write_does_not_signal() {
    let app = TestApp::new();
    let (reply, sent) = signals(&app, "todo_task_create", json!({ "title": "  " }));
    assert!(reply.is_err());
    assert_eq!(sent, 0);
}
