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

// ------------------------------------------------------------------ every view

fn view(source: &dyn Source, name: &str) -> Value {
    let reply = call(
        source,
        &json!({ "method": "snapshot", "view": name, "local": at_json(TODAY) }),
    );
    assert!(reply["ok"].is_object(), "{reply}");
    reply["ok"].clone()
}

fn set_pref(conn: &db::Connection, module: &str, key: &str, value: &str) {
    conn.execute(
        &format!(
            "INSERT OR REPLACE INTO settings(module, key, value, updated_at)
             VALUES ('{module}', '{key}', '\"{value}\"', 0)"
        ),
        [],
    )
    .unwrap();
}

/// A Setting stored as the JSON text given (a boolean, say).
fn set_raw_pref(conn: &db::Connection, module: &str, key: &str, json: &str) {
    conn.execute(
        "INSERT OR REPLACE INTO settings(module, key, value, updated_at) VALUES (?1, ?2, ?3, 0)",
        [module, key, json],
    )
    .unwrap();
}

fn titles(group: &Value) -> Vec<String> {
    rows(group).into_iter().map(|r| r.0).collect()
}

fn all_headings(snap: &Value) -> Vec<Value> {
    snap["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["heading"].clone())
        .collect()
}

#[test]
fn upcoming_is_overdue_then_each_day_of_the_next_seven() {
    let (phone, mut conn) = Phone::migrated();
    let work = mnemora_todo::create_list(&mut conn, "Work").unwrap();
    add(&mut conn, "Late", Some("2026-10-19"), None, None, None);
    add(&mut conn, "Today's", Some(TODAY), None, None, None);
    add(
        &mut conn,
        "Dentist",
        Some("2026-10-22"),
        Some("11:00"),
        None,
        None,
    );
    add(
        &mut conn,
        "Send invoice",
        Some("2026-10-22"),
        None,
        Some(2),
        Some(&work.id),
    );
    add(&mut conn, "Expenses", Some("2026-10-23"), None, None, None);
    add(&mut conn, "Last day", Some("2026-10-28"), None, None, None);
    add(&mut conn, "Too far", Some("2026-10-29"), None, None, None);
    add(&mut conn, "Undated", None, None, None, None);

    let snap = view(&phone.bridge(), "upcoming");

    assert_eq!(snap["view"], "upcoming");
    assert_eq!(snap["title"], "Upcoming");
    assert_eq!(
        all_headings(&snap),
        ["Overdue", "Tomorrow · Thu 22", "Fri 23", "Wed 28"]
    );
    assert_eq!(snap["groups"][0]["tone"], "overdue");
    // Inside a day: timed Tasks first, then the rest; the List chip shows in this mixed view.
    assert_eq!(titles(&snap["groups"][1]), ["Dentist", "Send invoice"]);
    assert_eq!(snap["groups"][1]["rows"][1]["list"], "Work");
    assert_eq!(titles(&snap["groups"][3]), ["Last day"]);
    // The header counts the open Tasks shown.
    assert_eq!(snap["count"], json!({ "text": "5", "tone": "normal" }));
}

#[test]
fn upcoming_ignores_the_apps_grouping_and_names_a_day_in_the_next_month() {
    let (phone, mut conn) = Phone::migrated();
    set_pref(&conn, "todo", "group_upcoming", "priority");
    add(&mut conn, "Soon", Some("2026-10-30"), None, Some(1), None);
    add(
        &mut conn,
        "Next month",
        Some("2026-11-02"),
        None,
        None,
        None,
    );
    // Tuesday 27 October: the week ahead reaches into November.
    let reply = call(
        &phone.bridge(),
        &json!({ "method": "snapshot", "view": "upcoming", "local": at_json("2026-10-27") }),
    );
    assert_eq!(all_headings(&reply["ok"]), ["Fri 30", "Mon 2 Nov"]);
}

#[test]
fn an_empty_upcoming_says_so_and_overdue_alone_still_lists() {
    let (phone, mut conn) = Phone::migrated();
    add(&mut conn, "Far away", Some("2026-12-01"), None, None, None);
    let snap = view(&phone.bridge(), "upcoming");
    assert_eq!(snap["groups"], json!([]));
    assert_eq!(snap["count"], Value::Null);
    assert_eq!(
        snap["empty"],
        json!({ "title": "Nothing coming up", "line": "Nothing in the next 7 days" })
    );

    add(&mut conn, "Late", Some("2026-10-20"), None, None, None);
    let snap = view(&phone.bridge(), "upcoming");
    assert_eq!(all_headings(&snap), ["Overdue"]);
    assert_eq!(snap["empty"], Value::Null);
}

#[test]
fn inbox_has_a_second_group_of_undated_tasks_in_lists() {
    let (phone, mut conn) = Phone::migrated();
    let home = mnemora_todo::create_list(&mut conn, "Home").unwrap();
    mnemora_todo::set_list_emoji(&mut conn, &home.id, Some("🏠")).unwrap();
    add(&mut conn, "Zebra idea", None, None, None, None);
    add(&mut conn, "Apple idea", None, None, None, None);
    add(
        &mut conn,
        "Dated inbox",
        Some("2026-10-24"),
        None,
        None,
        None,
    );
    add(
        &mut conn,
        "Sort the bulbs",
        None,
        None,
        None,
        Some(&home.id),
    );
    add(
        &mut conn,
        "Dated in a List",
        Some("2026-10-24"),
        None,
        None,
        Some(&home.id),
    );

    let snap = view(&phone.bridge(), "inbox");

    assert_eq!(snap["title"], "Inbox");
    assert_eq!(
        all_headings(&snap),
        [Value::Null, json!("No date, in a List")]
    );
    // The app's manual order, no List chips: they are all in the Inbox.
    assert_eq!(
        titles(&snap["groups"][0]),
        ["Zebra idea", "Apple idea", "Dated inbox"]
    );
    assert_eq!(snap["groups"][0]["rows"][0]["list"], Value::Null);
    assert_eq!(titles(&snap["groups"][1]), ["Sort the bulbs"]);
    assert_eq!(snap["groups"][1]["rows"][0]["list"], "🏠 Home");
    assert_eq!(snap["count"], json!({ "text": "4", "tone": "normal" }));

    // The Inbox's own Sort applies to both groups.
    set_pref(&conn, "todo", "inbox_sort", "title");
    let snap = view(&phone.bridge(), "inbox");
    assert_eq!(
        titles(&snap["groups"][0]),
        ["Apple idea", "Dated inbox", "Zebra idea"]
    );
}

#[test]
fn rows_not_due_today_name_their_day_in_grey_except_where_a_heading_does() {
    let (phone, mut conn) = Phone::migrated();
    let home = mnemora_todo::create_list(&mut conn, "Home").unwrap();
    add(
        &mut conn,
        "Today timed",
        Some(TODAY),
        Some("09:00"),
        None,
        Some(&home.id),
    );
    add(
        &mut conn,
        "Tomorrow timed",
        Some("2026-10-22"),
        Some("11:00"),
        None,
        Some(&home.id),
    );
    add(
        &mut conn,
        "Tomorrow untimed",
        Some("2026-10-22"),
        None,
        None,
        Some(&home.id),
    );
    add(
        &mut conn,
        "Later",
        Some("2026-11-04"),
        None,
        None,
        Some(&home.id),
    );
    add(&mut conn, "Undated", None, None, None, Some(&home.id));
    add(
        &mut conn,
        "Late",
        Some("2026-10-20"),
        Some("10:00"),
        None,
        Some(&home.id),
    );
    let bridge = phone.bridge();

    let snap = view(&bridge, &format!("list:{}", home.id));
    let by = |t: &str| {
        snap["groups"][0]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["title"] == t)
            .unwrap()
            .clone()
    };
    assert_eq!(by("Today timed")["time"], "09:00");
    assert_eq!(by("Tomorrow timed")["time"], "Tomorrow 11:00");
    assert_eq!(by("Tomorrow untimed")["time"], "Tomorrow");
    assert_eq!(by("Later")["time"], "Wed 4 Nov");
    assert_eq!(by("Undated")["time"], Value::Null);
    // Overdue keeps the red day and no time.
    assert_eq!(by("Late")["day"], "Yesterday");
    assert_eq!(by("Late")["time"], Value::Null);

    // All and Inbox name days too; Today and Upcoming's day groups do not.
    let all = view(&bridge, "all");
    assert!(
        all["groups"][0]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["time"] == "Tomorrow 11:00")
    );
    let upcoming = view(&bridge, "upcoming");
    assert_eq!(upcoming["groups"][1]["rows"][0]["time"], "11:00");
    assert_eq!(upcoming["groups"][1]["rows"][1]["time"], Value::Null);
}

#[test]
fn an_empty_inbox_is_zero() {
    let (phone, mut conn) = Phone::migrated();
    add(&mut conn, "Dated in a list", Some(TODAY), None, None, None);
    let list = mnemora_todo::create_list(&mut conn, "Home").unwrap();
    add(&mut conn, "Filed", Some(TODAY), None, None, Some(&list.id));
    let snap = view(&phone.bridge(), "inbox");
    // The dated Inbox Task is still in the Inbox.
    assert_eq!(snap["empty"], Value::Null);

    let (phone, _conn) = Phone::migrated();
    let snap = view(&phone.bridge(), "inbox");
    assert_eq!(
        snap["empty"],
        json!({ "title": "Inbox zero", "line": "Everything has a List" })
    );
    assert_eq!(snap["count"], Value::Null);
}

#[test]
fn all_is_grouped_by_list_with_the_inbox_first() {
    let (phone, mut conn) = Phone::migrated();
    let home = mnemora_todo::create_list(&mut conn, "Home").unwrap();
    mnemora_todo::set_list_emoji(&mut conn, &home.id, Some("🏠")).unwrap();
    let work = mnemora_todo::create_list(&mut conn, "Work").unwrap();
    mnemora_todo::create_list(&mut conn, "Empty list").unwrap();
    add(&mut conn, "Work first", None, None, None, Some(&work.id));
    add(
        &mut conn,
        "Home one",
        Some("2026-11-20"),
        None,
        None,
        Some(&home.id),
    );
    add(&mut conn, "Inbox one", None, None, None, None);
    add(&mut conn, "Home two", None, None, None, Some(&home.id));
    let done = add(&mut conn, "Finished", None, None, None, Some(&home.id));
    mnemora_todo::complete_task(&mut conn, &done.id, &at(TODAY)).unwrap();

    let snap = view(&phone.bridge(), "all");

    assert_eq!(snap["title"], "All");
    // Lists in the sidebar's order (Home, Work); the empty one is left out; done is hidden.
    assert_eq!(
        all_headings(&snap),
        [json!("Inbox"), json!("🏠 Home"), json!("Work")]
    );
    assert_eq!(titles(&snap["groups"][1]), ["Home one", "Home two"]);
    // The group names the List, so rows carry no chip.
    assert_eq!(snap["groups"][1]["rows"][0]["list"], Value::Null);
    assert_eq!(snap["count"], json!({ "text": "4", "tone": "normal" }));

    // Following the sidebar's Sort for Lists.
    set_pref(&conn, "todo", "lists_sort", "name_desc");
    let snap = view(&phone.bridge(), "all");
    assert_eq!(
        all_headings(&snap),
        [json!("Inbox"), json!("Work"), json!("🏠 Home")]
    );
}

#[test]
fn an_empty_all_says_there_are_no_open_tasks() {
    let (phone, _conn) = Phone::migrated();
    let snap = view(&phone.bridge(), "all");
    assert_eq!(
        snap["empty"],
        json!({ "title": "No open Tasks", "line": "Everything is done" })
    );
}

#[test]
fn a_list_follows_its_own_sort_and_grouping() {
    let (phone, mut conn) = Phone::migrated();
    let home = mnemora_todo::create_list(&mut conn, "Home").unwrap();
    mnemora_todo::set_list_emoji(&mut conn, &home.id, Some("🏠")).unwrap();
    let other = mnemora_todo::create_list(&mut conn, "Other").unwrap();
    add(&mut conn, "Water plants", None, None, None, Some(&home.id));
    add(&mut conn, "Bulb", None, None, Some(1), Some(&home.id));
    add(
        &mut conn,
        "Recycling",
        Some("2026-10-19"),
        None,
        None,
        Some(&home.id),
    );
    add(&mut conn, "Elsewhere", None, None, None, Some(&other.id));
    let bridge = phone.bridge();
    let name = format!("list:{}", home.id);

    let snap = view(&bridge, &name);
    assert_eq!(snap["view"], Value::String(name.clone()));
    assert_eq!(snap["title"], "🏠 Home");
    assert_eq!(all_headings(&snap), [Value::Null]);
    assert_eq!(
        titles(&snap["groups"][0]),
        ["Water plants", "Bulb", "Recycling"]
    );
    // Nothing names the List again, and a late Task keeps its red day.
    assert_eq!(snap["groups"][0]["rows"][0]["list"], Value::Null);
    assert_eq!(snap["groups"][0]["rows"][2]["day"], "Mon");
    assert_eq!(snap["groups"][0]["rows"][2]["overdue"], true);
    assert_eq!(snap["count"], json!({ "text": "3", "tone": "normal" }));

    mnemora_todo::set_list_sort(&mut conn, &home.id, mnemora_todo::TaskSort::Title).unwrap();
    let snap = view(&bridge, &name);
    assert_eq!(
        titles(&snap["groups"][0]),
        ["Bulb", "Recycling", "Water plants"]
    );

    mnemora_todo::set_list_sort(&mut conn, &home.id, mnemora_todo::TaskSort::Priority).unwrap();
    mnemora_todo::set_list_group(&mut conn, &home.id, mnemora_todo::GroupBy::Priority).unwrap();
    let snap = view(&bridge, &name);
    assert_eq!(all_headings(&snap), [json!("P1"), json!("No priority")]);
}

#[test]
fn an_empty_list_names_itself() {
    let (phone, mut conn) = Phone::migrated();
    let home = mnemora_todo::create_list(&mut conn, "Home").unwrap();
    mnemora_todo::set_list_emoji(&mut conn, &home.id, Some("🏠")).unwrap();
    let snap = view(&phone.bridge(), &format!("list:{}", home.id));
    assert_eq!(
        snap["empty"],
        json!({ "title": "No open Tasks", "line": "in 🏠 Home" })
    );
}

#[test]
fn a_missing_list_is_reported_as_not_found() {
    let (phone, mut conn) = Phone::migrated();
    let gone = mnemora_todo::create_list(&mut conn, "Gone").unwrap();
    mnemora_todo::delete_list(
        &mut conn,
        &gone.id,
        &mnemora_todo::DeleteChoice::MoveToInbox,
    )
    .unwrap();
    for id in [gone.id.as_str(), "never-existed"] {
        let reply = call(
            &phone.bridge(),
            &json!({ "method": "snapshot", "view": format!("list:{id}"), "local": at_json(TODAY) }),
        );
        assert_eq!(reply["err"]["code"], "not_found", "{reply}");
        assert_eq!(reply["err"]["message"], "That List no longer exists");
    }
}

#[test]
fn every_snapshot_carries_the_apps_theme() {
    let (phone, conn) = Phone::migrated();
    assert_eq!(view(&phone.bridge(), "today")["app_theme"], "system");
    set_pref(&conn, "core", "theme", "oled");
    for name in ["today", "upcoming", "inbox", "all"] {
        assert_eq!(view(&phone.bridge(), name)["app_theme"], "oled", "{name}");
    }
}

#[test]
fn lists_returns_the_lists_with_emoji_in_the_sidebars_order_and_the_theme() {
    let (phone, mut conn) = Phone::migrated();
    let home = mnemora_todo::create_list(&mut conn, "Home").unwrap();
    mnemora_todo::set_list_emoji(&mut conn, &home.id, Some("🏠")).unwrap();
    let work = mnemora_todo::create_list(&mut conn, "Work").unwrap();
    set_pref(&conn, "core", "theme", "dark");

    let reply = call(&phone.bridge(), &json!({ "method": "lists" }));
    assert_eq!(
        reply["ok"],
        json!({
            "app_theme": "dark",
            "lists": [
                { "id": home.id, "name": "Home", "emoji": "🏠", "label": "🏠 Home" },
                { "id": work.id, "name": "Work", "emoji": null, "label": "Work" },
            ],
        })
    );

    mnemora_todo::delete_list(
        &mut conn,
        &work.id,
        &mnemora_todo::DeleteChoice::MoveToInbox,
    )
    .unwrap();
    let reply = call(&phone.bridge(), &json!({ "method": "lists" }));
    assert_eq!(reply["ok"]["lists"].as_array().unwrap().len(), 1);
}

#[test]
fn lists_before_the_app_ran_is_not_set_up() {
    let phone = Phone::fresh();
    let reply = call(&phone.bridge(), &json!({ "method": "lists" }));
    assert_eq!(reply, json!({ "status": "not_set_up" }));
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
        r#"{"method":"snapshot","view":"list:","local":{"today":"2026-10-21","now":"14:05","offset_min":0}}"#,
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
    assert_eq!(app.widget_signals(), before);
}

#[test]
fn settings_that_change_the_drawing_signal_once_and_others_do_not() {
    let app = TestApp::new();
    for (key, value) in [
        ("core.theme", json!("dark")),
        ("core.time_format", json!("12h")),
        ("core.week_start", json!("sun")),
        ("todo.group_today", json!("list")),
        ("todo.inbox_sort", json!("title")),
    ] {
        let before = app.widget_signals();
        app.set_pref(key, value);
        assert_eq!(
            app.widget_signals() - before,
            1,
            "{key} redraws the widget once"
        );
    }
    // A batch of several is still one signal per Module saved: here, one.
    let before = app.widget_signals();
    app.invoke::<()>(
        "settings_save",
        json!({ "changes": [
            { "key": "core.theme", "value": "light" },
            { "key": "core.time_format", "value": "24h" },
        ] }),
    )
    .unwrap();
    assert_eq!(app.widget_signals() - before, 1);

    let before = app.widget_signals();
    app.set_pref("core.density", json!("compact"));
    app.set_pref("core.display_name", json!("Hemang"));
    app.set_pref("core.reduce_motion", json!(true));
    app.set_pref("todo.parse_dates", json!(false));
    assert_eq!(
        app.widget_signals(),
        before,
        "unrelated Settings stay quiet"
    );
}

#[test]
fn a_refused_write_does_not_signal() {
    let app = TestApp::new();
    let (reply, sent) = signals(&app, "todo_task_create", json!({ "title": "  " }));
    assert!(reply.is_err());
    assert_eq!(sent, 0);
}

// ------------------------------------------------------------------ ticking and Undo

fn tick(source: &dyn Source, id: &str, tick_steps: Option<bool>) -> Value {
    call(
        source,
        &json!({
            "method": "complete", "id": id, "tick_steps": tick_steps, "local": at_json(TODAY),
        }),
    )
}

fn undo(source: &dyn Source, token: &Value) -> Value {
    call(source, &json!({ "method": "undo", "token": token }))
}

fn titles_today(source: &dyn Source) -> Vec<String> {
    snapshot(source, TODAY, "14:05")["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| rows(g).into_iter().map(|(title, _)| title))
        .collect()
}

fn with_notes(conn: &mut db::Connection, id: &str, notes: &str) {
    mnemora_todo::update_task(conn, id, &stored_title(conn, id), notes, "s1").unwrap();
}

fn stored_title(conn: &db::Connection, id: &str) -> String {
    mnemora_todo::task(conn, id).unwrap().title
}

fn repeat_daily(conn: &mut db::Connection, title: &str) -> mnemora_todo::Task {
    let when = NewWhen {
        date: Some(TODAY.into()),
        time: None,
        local: at(TODAY),
    };
    let details = NewTaskDetails {
        repeat: Some(mnemora_todo::RepeatRule {
            freq: "day".into(),
            every: 1,
            days: None,
            mode: "fixed".into(),
        }),
        ..NewTaskDetails::default()
    };
    mnemora_todo::create_task(conn, title, None, Some(&when), Some(&details)).unwrap()
}

#[test]
fn ticking_a_task_completes_it_and_undo_reopens_it() {
    let (phone, mut conn) = Phone::migrated();
    let task = add(&mut conn, "Buy **oat** milk", Some(TODAY), None, None, None);
    add(&mut conn, "Other", Some(TODAY), None, None, None);
    let bridge = phone.bridge();

    let reply = tick(&bridge, &task.id, None);
    assert_eq!(reply["ok"]["kind"], "completed", "{reply}");
    assert_eq!(reply["ok"]["message"], "Completed “Buy oat milk”");
    assert_eq!(reply["ok"]["lead"], "Completed “Buy oat milk”");
    assert!(reply["ok"]["suffix"].is_null());
    assert_eq!(titles_today(&bridge), ["Other"]);
    let done = mnemora_todo::task(&conn, &task.id).unwrap();
    assert_eq!(done.state, mnemora_todo::TaskState::Done);

    let undone = undo(&bridge, &reply["ok"]["token"]);
    assert!(undone["ok"].is_object(), "{undone}");
    assert_eq!(titles_today(&bridge), ["Buy oat milk", "Other"]);
    let reopened = mnemora_todo::task(&conn, &task.id).unwrap();
    assert_eq!(reopened.state, mnemora_todo::TaskState::Open);
    assert_eq!(reopened.closed_at, None);
}

#[test]
fn ticking_a_repeating_task_says_where_it_went_and_undo_rewinds_it() {
    let (phone, mut conn) = Phone::migrated();
    let task = repeat_daily(&mut conn, "Call mum");
    let bridge = phone.bridge();

    let reply = tick(&bridge, &task.id, None);
    assert_eq!(
        reply["ok"]["message"],
        "Completed “Call mum” · next Thu 22 Oct"
    );
    assert_eq!(reply["ok"]["kind"], "completed");
    // The bar keeps the date whole and shortens the title, so it gets the two parts apart.
    assert_eq!(reply["ok"]["lead"], "Completed “Call mum”");
    assert_eq!(reply["ok"]["suffix"], "· next Thu 22 Oct");
    let moved = mnemora_todo::task(&conn, &task.id).unwrap();
    assert_eq!(moved.due_date.as_deref(), Some("2026-10-22"));
    assert_eq!(moved.state, mnemora_todo::TaskState::Open);
    assert!(titles_today(&bridge).is_empty());

    let undone = undo(&bridge, &reply["ok"]["token"]);
    assert!(undone["ok"].is_object(), "{undone}");
    let back = mnemora_todo::task(&conn, &task.id).unwrap();
    assert_eq!(back.due_date.as_deref(), Some(TODAY));
    assert_eq!(titles_today(&bridge), ["Call mum"]);
    // The occurrence record is gone again: the Done view of the day is empty.
    let done = mnemora_todo::done(&conn, None, 7, &at(TODAY)).unwrap();
    assert!(done.days.iter().all(|d| d.tasks.is_empty()), "{done:?}");
}

#[test]
fn open_steps_are_asked_about_and_nothing_changes_until_answered() {
    let (phone, mut conn) = Phone::migrated();
    let task = add(&mut conn, "Pack", Some(TODAY), None, None, None);
    with_notes(&mut conn, &task.id, "- [x] tent\n- [ ] stove\n- [ ] gas");
    let bridge = phone.bridge();

    let reply = tick(&bridge, &task.id, None);
    assert_eq!(
        reply["ok"],
        json!({ "kind": "open_steps", "task_id": task.id, "title": "Pack", "count": 2 })
    );
    let still = mnemora_todo::task(&conn, &task.id).unwrap();
    assert_eq!(still.state, mnemora_todo::TaskState::Open);
    assert_eq!(still.notes, "- [x] tent\n- [ ] stove\n- [ ] gas");
}

#[test]
fn complete_without_ticking_the_steps_leaves_the_notes() {
    let (phone, mut conn) = Phone::migrated();
    let task = add(&mut conn, "Pack", Some(TODAY), None, None, None);
    with_notes(&mut conn, &task.id, "- [ ] stove");
    let bridge = phone.bridge();

    let reply = tick(&bridge, &task.id, Some(false));
    assert_eq!(reply["ok"]["kind"], "completed", "{reply}");
    let done = mnemora_todo::task(&conn, &task.id).unwrap();
    assert_eq!(done.state, mnemora_todo::TaskState::Done);
    assert_eq!(done.notes, "- [ ] stove");
}

#[test]
fn tick_all_and_complete_ticks_the_steps_and_undo_puts_the_notes_back() {
    let (phone, mut conn) = Phone::migrated();
    let task = add(&mut conn, "Pack", Some(TODAY), None, None, None);
    let notes = "Pack:\n- [x] tent\n- [ ] stove\n  * [ ] gas";
    with_notes(&mut conn, &task.id, notes);
    let bridge = phone.bridge();

    let reply = tick(&bridge, &task.id, Some(true));
    assert_eq!(reply["ok"]["kind"], "completed", "{reply}");
    let done = mnemora_todo::task(&conn, &task.id).unwrap();
    assert_eq!(done.state, mnemora_todo::TaskState::Done);
    assert_eq!(done.notes, "Pack:\n- [x] tent\n- [x] stove\n  * [x] gas");

    let undone = undo(&bridge, &reply["ok"]["token"]);
    assert!(undone["ok"].is_object(), "{undone}");
    let back = mnemora_todo::task(&conn, &task.id).unwrap();
    assert_eq!(back.state, mnemora_todo::TaskState::Open);
    assert_eq!(back.notes, notes);
}

#[test]
fn tick_all_on_a_repeating_task_undoes_back_to_the_original_notes() {
    let (phone, mut conn) = Phone::migrated();
    let task = repeat_daily(&mut conn, "Pack daily");
    with_notes(&mut conn, &task.id, "- [ ] stove");
    let bridge = phone.bridge();

    let reply = tick(&bridge, &task.id, Some(true));
    assert!(
        reply["ok"]["message"].as_str().unwrap().contains("· next"),
        "{reply}"
    );
    // The next occurrence starts with fresh steps.
    assert_eq!(
        mnemora_todo::task(&conn, &task.id).unwrap().notes,
        "- [ ] stove"
    );
    undo(&bridge, &reply["ok"]["token"]);
    let back = mnemora_todo::task(&conn, &task.id).unwrap();
    assert_eq!(back.notes, "- [ ] stove");
    assert_eq!(back.due_date.as_deref(), Some(TODAY));
}

#[test]
fn bad_ticks_and_undos_are_errors_not_panics() {
    let (phone, mut conn) = Phone::migrated();
    let task = add(&mut conn, "Once", Some(TODAY), None, None, None);
    let bridge = phone.bridge();
    let first = tick(&bridge, &task.id, None);
    assert_eq!(first["ok"]["kind"], "completed");

    for reply in [
        tick(&bridge, &task.id, None), // already closed
        tick(&bridge, "no-such-task", None),
        undo(&bridge, &json!("not a token")),
        undo(&bridge, &json!("{}")),
        undo(&bridge, &json!("{\"reopen\":\"no-such-task\"}")),
        call(
            &bridge,
            &json!({ "method": "complete", "id": task.id, "local": { "today": "junk", "now": "x", "offset_min": 0 } }),
        ),
    ] {
        assert!(reply["err"]["message"].is_string(), "{reply}");
    }
    // Reopening twice: the second Undo finds the Task already open.
    assert!(undo(&bridge, &first["ok"]["token"])["ok"].is_object());
    assert!(undo(&bridge, &first["ok"]["token"])["err"].is_object());
}

// -------------------------------------------- the same rules as the app's own command

/// The change-log records of one Task (and, for a repeating one, of its occurrences), without
/// what differs between two Tasks: ids and times.
fn log_shape(app: &TestApp, id: &str) -> Vec<Value> {
    app.with_db(|db| {
        let mut stmt = db
            .prepare("SELECT entity, entity_id, op, snapshot FROM change_log ORDER BY id")
            .unwrap();
        let rows: Vec<(String, String, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let mut out = Vec::new();
        for (entity, entity_id, op, snapshot) in rows {
            let mut snap: Value = serde_json::from_str(&snapshot).unwrap();
            if entity_id != id && snap["repeat_of"].as_str() != Some(id) {
                continue;
            }
            let snap = snap.as_object_mut().unwrap();
            for key in [
                "id",
                "created_at",
                "updated_at",
                "closed_at",
                "sort_key",
                "repeat_of",
                "at",
            ] {
                snap.remove(key);
            }
            out.push(json!({ "entity": entity, "op": op, "snapshot": snap }));
        }
        out
    })
}

fn history(app: &TestApp, id: &str) -> Vec<(String, String)> {
    app.with_db(|db| {
        mnemora_todo::task_history(db, id)
            .unwrap()
            .into_iter()
            .map(|l| (l.kind, l.text))
            .collect()
    })
}

fn create_in_app(app: &TestApp, title: &str, repeat: bool) -> String {
    let mut args = json!({
        "title": title,
        "when": { "date": TODAY, "time": "09:00", "local": at_json(TODAY) },
    });
    if repeat {
        args["details"] = json!({ "repeat": { "freq": "day", "every": 1, "mode": "fixed" } });
    }
    let task: Value = app.invoke("todo_task_create", args).unwrap();
    task["id"].as_str().unwrap().to_owned()
}

fn completed_rows(app: &TestApp, id: &str) -> (Value, Vec<Value>, Vec<(String, String)>) {
    let task = app.with_db(|db| serde_json::to_value(mnemora_todo::task(db, id).unwrap()).unwrap());
    let mut task = task;
    let task_obj = task.as_object_mut().unwrap();
    for key in [
        "id",
        "title",
        "created_at",
        "updated_at",
        "closed_at",
        "sort_key",
    ] {
        task_obj.remove(key);
    }
    (task, log_shape(app, id), history(app, id))
}

#[test]
fn ticking_on_the_widget_is_the_same_as_the_apps_complete_command() {
    for repeating in [false, true] {
        let app = TestApp::new();
        let via_app = create_in_app(&app, "Same rules", repeating);
        let via_widget = create_in_app(&app, "Same rules", repeating);

        let done: Value = app
            .invoke(
                "todo_task_complete",
                json!({ "id": via_app, "local": at_json(TODAY) }),
            )
            .unwrap();
        assert!(done["task"].is_object());
        let live = Live::new(app.handle());
        let reply = tick(&live, &via_widget, None);
        assert_eq!(reply["ok"]["kind"], "completed", "{reply}");

        let from_app = completed_rows(&app, &via_app);
        let from_widget = completed_rows(&app, &via_widget);
        assert_eq!(
            from_app.0, from_widget.0,
            "Task rows, repeating={repeating}"
        );
        assert_eq!(
            from_app.1, from_widget.1,
            "change log, repeating={repeating}"
        );
        assert!(!from_app.1.is_empty());
        assert_eq!(from_app.2, from_widget.2, "History, repeating={repeating}");
        assert!(!from_app.2.is_empty());
    }
}

#[test]
fn undoing_on_the_widget_is_the_same_as_reopening_in_the_app() {
    let app = TestApp::new();
    let via_app = create_in_app(&app, "Same undo", false);
    let via_widget = create_in_app(&app, "Same undo", false);
    app.invoke::<Value>(
        "todo_task_complete",
        json!({ "id": via_app, "local": at_json(TODAY) }),
    )
    .unwrap();
    app.invoke::<Value>("todo_task_reopen", json!({ "id": via_app }))
        .unwrap();
    let live = Live::new(app.handle());
    let reply = tick(&live, &via_widget, None);
    assert!(undo(&live, &reply["ok"]["token"])["ok"].is_object());

    assert_eq!(
        completed_rows(&app, &via_app),
        completed_rows(&app, &via_widget)
    );
}

// ------------------------------------------------------------ telling a running app

fn count_events(app: &TestApp) -> std::sync::Arc<std::sync::atomic::AtomicU32> {
    use std::sync::atomic::{AtomicU32, Ordering};
    let seen = std::sync::Arc::new(AtomicU32::new(0));
    let counter = seen.clone();
    tauri_specta::Event::listen(
        &app.handle(),
        move |_: tauri_specta::TypedEvent<crate::TodoChangedEvent>| {
            counter.fetch_add(1, Ordering::SeqCst);
        },
    );
    seen
}

#[test]
fn a_widget_write_while_the_app_is_alive_emits_todo_changed_once_per_write() {
    use std::sync::atomic::Ordering;
    let app = TestApp::new();
    let seen = count_events(&app);
    let id = create_in_app(&app, "Live tick", false);
    let after_create = seen.load(Ordering::SeqCst);
    let live = Live::new(app.handle());

    snapshot(&live, TODAY, "14:05");
    assert_eq!(seen.load(Ordering::SeqCst), after_create, "reads are quiet");

    let asked = tick(&live, &id, None);
    assert_eq!(asked["ok"]["kind"], "completed");
    wait_for(&seen, after_create + 1);

    let undone = undo(&live, &asked["ok"]["token"]);
    assert!(undone["ok"].is_object());
    wait_for(&seen, after_create + 2);

    // A refused write changes nothing and says nothing.
    tick(&live, "no-such-task", None);
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(seen.load(Ordering::SeqCst), after_create + 2);
}

fn wait_for(seen: &std::sync::atomic::AtomicU32, want: u32) {
    use std::sync::atomic::Ordering;
    for _ in 0..100 {
        if seen.load(Ordering::SeqCst) >= want {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(seen.load(Ordering::SeqCst), want, "event never arrived");
}

// ------------------------------------------------------------------------------- Quick add

fn local_json(today: &str) -> Value {
    json!({ "today": today, "now": "14:05", "offset_min": 0 })
}

fn context(source: &dyn Source, view: &str) -> Value {
    let reply = call(
        source,
        &json!({ "method": "capture_context", "view": view, "local": local_json(TODAY) }),
    );
    assert!(reply["ok"].is_object(), "{reply}");
    reply["ok"].clone()
}

fn quick(method: &str, view: &str, text: &str, extra: Value) -> Value {
    let mut request = json!({
        "method": method, "view": view, "text": text, "local": local_json(TODAY),
    });
    for (key, value) in extra.as_object().cloned().unwrap_or_default() {
        request[key] = value;
    }
    request
}

fn read(source: &dyn Source, view: &str, text: &str, extra: Value) -> Value {
    let reply = call(source, &quick("parse", view, text, extra));
    assert!(reply["ok"].is_object(), "{reply}");
    reply["ok"].clone()
}

fn added(source: &dyn Source, view: &str, text: &str, extra: Value) -> Value {
    let reply = call(source, &quick("add", view, text, extra));
    assert!(reply["ok"].is_object(), "{reply}");
    reply["ok"].clone()
}

fn the_task(conn: &db::Connection, added: &Value) -> mnemora_todo::Task {
    mnemora_todo::task(conn, added["task"]["id"].as_str().unwrap()).unwrap()
}

fn new_list(conn: &mut db::Connection, name: &str, emoji: Option<&str>) -> mnemora_todo::TodoList {
    let list = mnemora_todo::create_list(conn, name).unwrap();
    match emoji {
        Some(e) => mnemora_todo::set_list_emoji(conn, &list.id, Some(e)).unwrap(),
        None => list,
    }
}

#[test]
fn the_sheet_opens_in_the_place_each_view_stands_for() {
    let (phone, mut conn) = Phone::migrated();
    let home = new_list(&mut conn, "Home", Some("🏠"));
    let bridge = phone.bridge();

    let today = context(&bridge, "today");
    assert_eq!(today["placeholder"], "Quick add, due today");
    assert_eq!(today["place"], json!({ "date": TODAY, "list_id": null }));

    let upcoming = context(&bridge, "upcoming");
    assert_eq!(upcoming["placeholder"], "Quick add, due tomorrow");
    assert_eq!(
        upcoming["place"],
        json!({ "date": "2026-10-22", "list_id": null })
    );

    for view in ["inbox", "all"] {
        let ctx = context(&bridge, view);
        assert_eq!(ctx["placeholder"], "Quick add to Inbox", "{view}");
        assert_eq!(ctx["place"], json!({ "date": null, "list_id": null }));
    }

    let list = context(&bridge, &format!("list:{}", home.id));
    assert_eq!(list["placeholder"], "Quick add to 🏠 Home");
    assert_eq!(list["place"], json!({ "date": null, "list_id": home.id }));

    // A List deleted since the widget was set up is the Inbox, and the hint says so.
    mnemora_todo::delete_list(
        &mut conn,
        &home.id,
        &mnemora_todo::DeleteChoice::MoveToInbox,
    )
    .unwrap();
    let gone = context(&bridge, &format!("list:{}", home.id));
    assert_eq!(gone["placeholder"], "Quick add to Inbox");
    assert_eq!(gone["place"], json!({ "date": null, "list_id": null }));
}

#[test]
fn the_context_carries_the_lists_and_the_settings_that_shape_reading() {
    let (phone, mut conn) = Phone::migrated();
    new_list(&mut conn, "Home", Some("🏠"));
    new_list(&mut conn, "Work", None);
    let bridge = phone.bridge();

    let ctx = context(&bridge, "today");
    let labels: Vec<&str> = ctx["lists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["🏠 Home", "Work"]);
    assert_eq!(ctx["lists"][0]["name"], "Home");
    assert_eq!(ctx["lists"][0]["emoji"], "🏠");
    assert_eq!(ctx["parse_dates"], true);
    assert_eq!(ctx["week_start"], "mon");
    assert_eq!(ctx["time_format"], "24h");

    set_raw_pref(&conn, "todo", "parse_dates", "false");
    set_pref(&conn, "core", "week_start", "sun");
    set_pref(&conn, "core", "time_format", "12h");
    let ctx = context(&bridge, "today");
    assert_eq!(ctx["parse_dates"], false);
    assert_eq!(ctx["week_start"], "sun");
    assert_eq!(ctx["time_format"], "12h");
}

#[test]
fn before_the_app_ever_ran_the_sheet_cannot_open() {
    let phone = Phone::fresh();
    let bridge = phone.bridge();
    for request in [
        json!({ "method": "capture_context", "view": "today", "local": local_json(TODAY) }),
        quick("parse", "today", "x", json!({})),
        quick("add", "today", "x", json!({})),
    ] {
        assert_eq!(call(&bridge, &request)["status"], "not_set_up");
    }
}

#[test]
fn parse_reads_the_words_as_the_app_does_and_says_what_they_mean() {
    let (phone, mut conn) = Phone::migrated();
    let home = new_list(&mut conn, "Home", Some("🏠"));
    let bridge = phone.bridge();

    let text = "Call plumber tomorrow 9am p2 #Home";
    let got = read(&bridge, "inbox", text, json!({}));
    let seen: Vec<(String, String, String)> = got["spans"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["kind"].as_str().unwrap().to_owned(),
                s["word"].as_str().unwrap().to_owned(),
                s["meaning"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(
        seen.contains(&("date".into(), "tomorrow".into(), "Thu 22 Oct".into())),
        "{seen:?}"
    );
    assert!(
        seen.contains(&("time".into(), "9am".into(), "09:00".into())),
        "{seen:?}"
    );
    assert!(
        seen.contains(&("prio".into(), "p2".into(), "P2".into())),
        "{seen:?}"
    );
    assert!(
        seen.contains(&("list".into(), "#Home".into(), "Home".into())),
        "{seen:?}"
    );
    // The offsets are where the words sit in the text.
    let date = got["spans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "date")
        .unwrap();
    let (start, end) = (
        date["start"].as_u64().unwrap(),
        date["end"].as_u64().unwrap(),
    );
    // ASCII text: UTF-16 units and bytes agree here.
    assert_eq!(
        &text[usize::try_from(start).unwrap()..usize::try_from(end).unwrap()],
        "tomorrow"
    );

    assert_eq!(got["fields"]["date"], "2026-10-22");
    assert_eq!(got["fields"]["time"], "09:00");
    assert_eq!(got["fields"]["priority"], 2);
    assert_eq!(got["fields"]["list_id"], home.id);
    assert_eq!(got["list_label"], "🏠 Home");
    assert_eq!(got["list_is_new"], false);
    assert_eq!(got["when_label"], "Tomorrow · 09:00");
}

#[test]
fn parse_words_follow_the_time_format_and_offsets_count_utf16_units() {
    let (phone, conn) = Phone::migrated();
    set_pref(&conn, "core", "time_format", "12h");
    let bridge = phone.bridge();

    // The emoji is two UTF-16 units, as in a Kotlin string.
    let got = read(&bridge, "inbox", "🎉 party at 9pm", json!({}));
    let time = &got["spans"][0];
    assert_eq!(time["kind"], "time");
    assert_eq!(time["meaning"], "9:00 PM");
    assert_eq!(
        (
            time["start"].as_u64().unwrap(),
            time["end"].as_u64().unwrap()
        ),
        (9, 15)
    );
    assert_eq!(got["when_label"], "Today · 9:00 PM", "{got}");
}

#[test]
fn parse_honours_kept_as_text_and_use_instead() {
    let (phone, _conn) = Phone::migrated();
    let bridge = phone.bridge();

    let both = read(&bridge, "inbox", "dinner tomorrow or friday", json!({}));
    assert_eq!(both["spans"].as_array().unwrap().len(), 1);
    assert_eq!(both["alternatives"].as_array().unwrap().len(), 1);
    assert_eq!(both["alternatives"][0]["reason"], "conflict");
    let loser = both["alternatives"][0]["key"].as_str().unwrap().to_owned();
    let winner = both["spans"][0]["key"].as_str().unwrap().to_owned();

    let swapped = read(
        &bridge,
        "inbox",
        "dinner tomorrow or friday",
        json!({ "prefer": { "date": loser } }),
    );
    assert_eq!(swapped["spans"][0]["key"], loser);
    assert_eq!(swapped["alternatives"][0]["key"], winner);

    let kept = read(
        &bridge,
        "inbox",
        "dinner tomorrow or friday",
        json!({ "ignored": [winner, loser] }),
    );
    assert_eq!(kept["spans"], json!([]));
    assert_eq!(kept["ignored"].as_array().unwrap().len(), 2);
    assert_eq!(kept["fields"]["date"], Value::Null);
}

#[test]
fn parse_offers_a_new_list_for_an_unknown_name_and_turns_off_with_the_preference() {
    let (phone, conn) = Phone::migrated();
    let bridge = phone.bridge();

    let got = read(&bridge, "inbox", "Buy bolts #Garage", json!({}));
    assert_eq!(got["spans"][0]["meaning"], "New list “Garage”");
    assert_eq!(got["fields"]["new_list"], "Garage");
    assert_eq!(got["fields"]["list_id"], Value::Null);
    assert_eq!(got["list_label"], "Garage");
    assert_eq!(got["list_is_new"], true);

    set_raw_pref(&conn, "todo", "parse_dates", "false");
    let off = read(&bridge, "today", "Buy bolts tomorrow p1 #Garage", json!({}));
    assert_eq!(off["spans"], json!([]));
    // Nothing is read, so the place the sheet opened in still stands.
    assert_eq!(off["fields"]["date"], TODAY);
    assert_eq!(off["fields"]["priority"], 4);
}

#[test]
fn parse_applies_the_chips_used_by_hand() {
    let (phone, mut conn) = Phone::migrated();
    let work = new_list(&mut conn, "Work", None);
    let bridge = phone.bridge();

    let got = read(
        &bridge,
        "inbox",
        "Report",
        json!({ "hand": { "list_id": work.id, "priority": 1 } }),
    );
    assert_eq!(got["fields"]["list_id"], work.id);
    assert_eq!(got["fields"]["priority"], 1);
    assert_eq!(got["list_label"], "Work");

    // The words win over a chip.
    let got = read(
        &bridge,
        "inbox",
        "Report p3",
        json!({ "hand": { "priority": 1 } }),
    );
    assert_eq!(got["fields"]["priority"], 3);
    // The Inbox chosen by hand beats the List the widget shows.
    let got = read(
        &bridge,
        &format!("list:{}", work.id),
        "Report",
        json!({ "hand": { "list_id": null } }),
    );
    assert_eq!(got["fields"]["list_id"], Value::Null);
    assert_eq!(got["list_label"], "Inbox");
}

#[test]
fn adding_inherits_the_place_of_every_view() {
    let (phone, mut conn) = Phone::migrated();
    let home = new_list(&mut conn, "Home", Some("🏠"));
    let bridge = phone.bridge();

    let today = added(&bridge, "today", "Water plants", json!({}));
    let task = the_task(&conn, &today);
    assert_eq!(
        (task.due_date.as_deref(), task.list_id.as_deref()),
        (Some(TODAY), None)
    );
    assert_eq!(today["message"], "Added “Water plants” to Today");

    let upcoming = added(&bridge, "upcoming", "Book dentist", json!({}));
    let task = the_task(&conn, &upcoming);
    assert_eq!(
        (task.due_date.as_deref(), task.list_id.as_deref()),
        (Some("2026-10-22"), None)
    );
    assert_eq!(
        upcoming["message"],
        "Added “Book dentist” to Upcoming · Thu 22 Oct"
    );

    for view in ["inbox", "all"] {
        let got = added(&bridge, view, &format!("Sort mail {view}"), json!({}));
        let task = the_task(&conn, &got);
        assert_eq!((task.due_date, task.list_id), (None, None), "{view}");
        assert_eq!(got["message"], format!("Added “Sort mail {view}” to Inbox"));
    }

    let got = added(&bridge, &format!("list:{}", home.id), "Fix tap", json!({}));
    let task = the_task(&conn, &got);
    assert_eq!((task.due_date, task.list_id), (None, Some(home.id.clone())));
    assert_eq!(got["message"], "Added “Fix tap” to Home");

    // A deleted List lands in the Inbox rather than failing.
    mnemora_todo::delete_list(
        &mut conn,
        &home.id,
        &mnemora_todo::DeleteChoice::MoveToInbox,
    )
    .unwrap();
    let got = added(&bridge, &format!("list:{}", home.id), "Orphan", json!({}));
    assert_eq!(the_task(&conn, &got).list_id, None);
}

#[test]
fn typed_words_beat_where_the_view_opened() {
    let (phone, mut conn) = Phone::migrated();
    let work = new_list(&mut conn, "Work", None);
    let bridge = phone.bridge();

    let got = added(
        &bridge,
        "today",
        "Call plumber tomorrow 9am #Work p1",
        json!({}),
    );
    let task = the_task(&conn, &got);
    assert_eq!(task.title, "Call plumber tomorrow 9am #Work p1");
    assert_eq!(task.due_date.as_deref(), Some("2026-10-22"));
    assert_eq!(task.due_time.as_deref(), Some("09:00"));
    assert_eq!(task.priority, 1);
    assert_eq!(task.list_id.as_deref(), Some(work.id.as_str()));
    // It is not due today, so the toast says where it went.
    assert_eq!(
        got["message"],
        "Added “Call plumber tomorrow 9am #Work p1” to Upcoming · Thu 22 Oct"
    );
    let understood: Value = serde_json::from_str(task.understood.as_deref().unwrap()).unwrap();
    assert_eq!(
        understood,
        json!([
            { "word": "tomorrow", "kind": "date", "value": "2026-10-22" },
            { "word": "9am", "kind": "time", "value": "09:00" },
            { "word": "#Work", "kind": "list", "value": work.id },
            { "word": "p1", "kind": "prio", "value": "1" },
        ])
    );
}

#[test]
fn a_time_alone_in_upcoming_keeps_tomorrows_day() {
    let (phone, conn) = Phone::migrated();
    let bridge = phone.bridge();
    let got = added(&bridge, "upcoming", "Gym at 7pm", json!({}));
    let task = the_task(&conn, &got);
    assert_eq!(task.due_date.as_deref(), Some("2026-10-22"));
    assert_eq!(task.due_time.as_deref(), Some("19:00"));
}

#[test]
fn a_hash_name_that_matches_no_list_makes_one() {
    let (phone, mut conn) = Phone::migrated();
    let bridge = phone.bridge();
    let before = mnemora_todo::sidebar(&conn, &at(TODAY))
        .unwrap()
        .lists
        .len();

    let got = added(&bridge, "inbox", "Buy bolts #Garage", json!({}));
    let task = the_task(&conn, &got);
    let lists = mnemora_todo::sidebar(&conn, &at(TODAY)).unwrap().lists;
    assert_eq!(lists.len(), before + 1);
    let garage = lists.iter().find(|e| e.list.name == "Garage").unwrap();
    assert_eq!(task.list_id.as_deref(), Some(garage.list.id.as_str()));
    assert_eq!(got["message"], "Added “Buy bolts #Garage” to Garage");
    let understood: Value = serde_json::from_str(task.understood.as_deref().unwrap()).unwrap();
    assert_eq!(
        understood,
        json!([{ "word": "#Garage", "kind": "list", "value": garage.list.id }])
    );

    // The second time the List exists and is reused, not made again.
    let again = added(&bridge, "inbox", "More bolts #garage", json!({}));
    assert_eq!(
        the_task(&conn, &again).list_id.as_deref(),
        Some(garage.list.id.as_str())
    );
    assert_eq!(
        mnemora_todo::sidebar(&conn, &at(TODAY))
            .unwrap()
            .lists
            .len(),
        before + 1
    );
    let _ = &mut conn;
}

#[test]
fn priority_notes_and_the_chips_are_stored() {
    let (phone, mut conn) = Phone::migrated();
    let work = new_list(&mut conn, "Work", None);
    let bridge = phone.bridge();

    let got = added(
        &bridge,
        "inbox",
        "Plan trip",
        json!({ "notes": "- [ ] flights\n- [ ] hotel", "hand": { "priority": 2, "list_id": work.id } }),
    );
    let task = the_task(&conn, &got);
    assert_eq!(task.priority, 2);
    assert_eq!(task.notes, "- [ ] flights\n- [ ] hotel");
    assert_eq!(task.list_id.as_deref(), Some(work.id.as_str()));
    assert_eq!(task.understood, None);

    // No Priority is the app's 4.
    let plain = added(&bridge, "inbox", "Plain", json!({}));
    assert_eq!(the_task(&conn, &plain).priority, 4);
}

#[test]
fn kept_as_text_words_stay_in_the_title_and_out_of_the_fields() {
    let (phone, conn) = Phone::migrated();
    let bridge = phone.bridge();
    let read_it = read(&bridge, "inbox", "Visit mum tomorrow", json!({}));
    let key = read_it["spans"][0]["key"].clone();

    let got = added(
        &bridge,
        "inbox",
        "Visit mum tomorrow",
        json!({ "ignored": [key] }),
    );
    let task = the_task(&conn, &got);
    assert_eq!(task.title, "Visit mum tomorrow");
    assert_eq!(task.due_date, None);
    assert_eq!(task.understood, None);
}

#[test]
fn a_repeat_word_makes_a_repeating_task() {
    let (phone, conn) = Phone::migrated();
    let bridge = phone.bridge();
    let got = added(&bridge, "inbox", "Water plants every 3 days", json!({}));
    let task = the_task(&conn, &got);
    assert_eq!(task.repeat_freq.as_deref(), Some("day"));
    assert_eq!(task.repeat_every, Some(3));
    assert!(task.due_date.is_some());
}

#[test]
fn adding_writes_one_create_row_and_a_new_list_writes_its_own() {
    let (phone, conn) = Phone::migrated();
    let bridge = phone.bridge();
    let count = |entity: &str| -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM change_log WHERE module = 'todo' AND entity = ?1 AND op = 'create'",
            [entity],
            |r| r.get(0),
        )
        .unwrap()
    };
    let (tasks, lists) = (count("todo_task"), count("todo_list"));

    added(&bridge, "today", "One", json!({}));
    assert_eq!((count("todo_task"), count("todo_list")), (tasks + 1, lists));
    added(&bridge, "today", "Two #Fresh", json!({}));
    assert_eq!(
        (count("todo_task"), count("todo_list")),
        (tasks + 2, lists + 1)
    );
}

#[test]
fn a_task_added_by_the_widget_is_the_task_the_app_would_add() {
    let app = TestApp::new();
    let live = Live::new(app.handle());

    // The app's Capture box: read, then todo_task_create with what was read.
    let via_app: Value = app
        .invoke(
            "todo_task_create",
            json!({
                "title": "Call plumber tomorrow 9am p2",
                "listId": null,
                "when": { "date": "2026-10-22", "time": "09:00", "local": at_json(TODAY) },
                "details": {
                    "notes": "ring twice", "priority": 2, "repeat": null,
                    "understood": "[{\"word\":\"tomorrow\",\"kind\":\"date\",\"value\":\"2026-10-22\"},{\"word\":\"9am\",\"kind\":\"time\",\"value\":\"09:00\"},{\"word\":\"p2\",\"kind\":\"prio\",\"value\":\"2\"}]",
                },
            }),
        )
        .unwrap();
    let via_widget = added(
        &live,
        "inbox",
        "Call plumber tomorrow 9am p2",
        json!({ "notes": "ring twice" }),
    );

    let strip = |mut task: Value| {
        let task_obj = task.as_object_mut().unwrap();
        for key in ["id", "created_at", "updated_at", "closed_at", "sort_key"] {
            task_obj.remove(key);
        }
        task
    };
    assert_eq!(strip(via_app.clone()), strip(via_widget["task"].clone()));
    let id_of = |v: &Value| v["id"].as_str().unwrap().to_owned();
    assert_eq!(
        log_shape(&app, &id_of(&via_app)),
        log_shape(&app, &id_of(&via_widget["task"]))
    );
    assert!(!log_shape(&app, &id_of(&via_app)).is_empty());
    assert_eq!(
        history(&app, &id_of(&via_app)),
        history(&app, &id_of(&via_widget["task"]))
    );
}

#[test]
fn adding_tells_a_running_app_once_and_a_refused_add_says_nothing() {
    use std::sync::atomic::Ordering;
    let app = TestApp::new();
    let seen = count_events(&app);
    let live = Live::new(app.handle());

    read(&live, "today", "Quiet tomorrow", json!({}));
    context(&live, "today");
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(seen.load(Ordering::SeqCst), 0, "reading is quiet");

    added(&live, "today", "Loud", json!({}));
    wait_for(&seen, 1);

    let refused = call(&live, &quick("add", "today", "   ", json!({})));
    assert!(refused["err"]["message"].is_string(), "{refused}");
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(seen.load(Ordering::SeqCst), 1);
}

#[test]
fn bad_quick_add_requests_are_errors_and_create_nothing() {
    let (phone, conn) = Phone::migrated();
    let bridge = phone.bridge();
    let long = "x".repeat(501);
    for request in [
        quick("add", "today", "", json!({})),
        quick("add", "today", &long, json!({})),
        quick("add", "today", "two\nlines", json!({})),
        quick("add", "nowhere", "Task", json!({})),
        quick("parse", "nowhere", "Task", json!({})),
        quick(
            "add",
            "today",
            "Task",
            json!({ "hand": { "list_id": "no-such-list" } }),
        ),
        json!({ "method": "add", "view": "today", "local": local_json(TODAY) }),
        json!({ "method": "add", "text": "x", "view": "today", "local": { "today": "junk", "now": "x", "offset_min": 0 } }),
    ] {
        let reply = call(&bridge, &request);
        assert!(reply["err"]["message"].is_string(), "{request} -> {reply}");
    }
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM todo_tasks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}
