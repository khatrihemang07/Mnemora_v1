//! The first-merge preview and the backups before it (#73), between two `TestApp`s over real
//! loopback sockets: the commands the UI calls, in the order it calls them.

use std::path::{Path, PathBuf};

use mnemora_core::db::{self, Connection};
use serde_json::{Value, json};

use crate::sync_live_tests::{meet, until};
use crate::sync_pairing_tests::Device;
use crate::test_support::TestApp;

fn journal(app: &TestApp, text: &str) {
    app.invoke::<Value>(
        "journal_entry_create",
        json!({ "entry": {
            "page_date": "2026-04-01", "text": text, "written_tz": "Asia/Kolkata",
            "written_offset_min": 330, "session_id": format!("c-{text}"),
        }}),
    )
    .unwrap();
}

fn task(app: &TestApp, title: &str, list: Option<&Value>) {
    app.invoke::<Value>(
        "todo_task_create",
        json!({ "title": title, "listId": list }),
    )
    .unwrap();
}

fn list(app: &TestApp, name: &str) -> Value {
    app.invoke::<Value>("todo_list_create", json!({ "name": name }))
        .unwrap()["id"]
        .clone()
}

/// Both devices hold something. Mac: 2 Entries, 3 Tasks, Lists Groceries and Work. Phone: 1 Entry,
/// 1 Task, Lists groceries and Café.
fn with_data(mac: &Device, phone: &Device) {
    journal(&mac.app, "mac one");
    journal(&mac.app, "mac two");
    let groceries = list(&mac.app, "Groceries");
    let work = list(&mac.app, "Work");
    task(&mac.app, "Milk", Some(&groceries));
    task(&mac.app, "Report", Some(&work));
    task(&mac.app, "Inbox thing", None);
    journal(&phone.app, "phone one");
    let lower = list(&phone.app, "groceries");
    list(&phone.app, "Café");
    task(&phone.app, "Eggs", Some(&lower));
}

/// The code is typed on `typer`, shown on `shower`; returns once the shower is asked.
async fn type_code(shower: &Device, typer: &Device) {
    shower.call::<()>("sync_pair_show", json!({}));
    let code = shower.pairing()["code"].as_str().unwrap().replace(' ', "");
    typer.found.set(vec![shower.address()]);
    typer.call::<()>("sync_pair_enter", json!({ "code": code }));
    shower.until("asking").await;
}

fn backups(app: &TestApp) -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(app.backup_dir()) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "db"))
        .collect();
    files.sort();
    files
}

fn count(db: &Connection, sql: &str) -> i64 {
    db.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn open(path: &Path) -> Connection {
    db::open_existing(path).unwrap().unwrap()
}

/// The names of the live Lists, sorted.
fn list_names(app: &TestApp) -> Vec<String> {
    let view: Value = app
        .invoke(
            "todo_lists",
            json!({ "local": { "today": "2026-04-01", "now": "14:05", "offset_min": 0 } }),
        )
        .unwrap();
    let mut names: Vec<String> = view["lists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["list"]["name"].as_str().unwrap().to_lowercase())
        .collect();
    names.sort();
    names
}

fn tasks_on(app: &TestApp) -> i64 {
    app.with_db(|c| count(c, "SELECT COUNT(*) FROM todo_tasks"))
}

#[tokio::test]
async fn both_with_data_see_the_counts_and_the_lists_that_combine_before_anything_changes() {
    let (mac, phone) = (Device::new("Mac").await, Device::new("Pixel 8").await);
    with_data(&mac, &phone);
    type_code(&mac, &phone).await;
    mac.call::<()>("sync_pair_decide", json!({ "allow": true }));

    // The device that typed the code shows the preview; the other waits.
    let preview = phone.until("preview").await;
    assert_eq!(preview["name"], "Mac");
    assert_eq!(
        preview["there"],
        json!({ "entries": 2, "tasks": 3, "lists": 2 })
    );
    assert_eq!(
        preview["here"],
        json!({ "entries": 1, "tasks": 1, "lists": 2 })
    );
    assert_eq!(preview["combine"], json!(["groceries"]));
    assert_eq!(mac.until("confirming").await["name"], "Pixel 8");

    // Nothing is paired, stored or backed up until Merge.
    assert!(mac.listed().is_empty() && phone.listed().is_empty());
    assert!(backups(&mac.app).is_empty() && backups(&phone.app).is_empty());

    phone.call::<()>("sync_pair_merge", json!({ "merge": true }));
    assert_eq!(mac.until("paired").await["name"], "Pixel 8");
    assert_eq!(phone.until("paired").await["name"], "Mac");
    assert_eq!(mac.listed(), ["Pixel 8"]);
    assert_eq!(phone.listed(), ["Mac"]);
}

#[tokio::test]
async fn both_databases_are_backed_up_before_the_pairing_is_recorded() {
    let (mac, phone) = (Device::new("Mac").await, Device::new("Pixel 8").await);
    with_data(&mac, &phone);
    type_code(&mac, &phone).await;
    mac.call::<()>("sync_pair_decide", json!({ "allow": true }));
    phone.until("preview").await;
    phone.call::<()>("sync_pair_merge", json!({ "merge": true }));
    mac.until("paired").await;
    phone.until("paired").await;

    for (app, tasks, entries) in [(&mac.app, 3, 2), (&phone.app, 1, 1)] {
        let found = backups(app);
        assert_eq!(found.len(), 1, "one backup on each device");
        let db = open(&found[0]);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM todo_tasks"), tasks);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM journal_entries"), entries);
        // It was taken before the pairing was recorded, so before any record could arrive.
        assert_eq!(count(&db, "SELECT COUNT(*) FROM sync_peers"), 0);
    }

    // The merge then does what the preview said: Groceries/groceries became one List.
    meet(&mac, &phone);
    until("both devices hold all 4 Tasks", || {
        tasks_on(&mac.app) == 4 && tasks_on(&phone.app) == 4
    })
    .await;
    until("the Lists combined", || {
        list_names(&mac.app) == ["café", "groceries", "work"]
            && list_names(&phone.app) == ["café", "groceries", "work"]
    })
    .await;
}

#[tokio::test]
async fn cancel_leaves_both_databases_untouched_and_tells_the_other_device() {
    let (mac, phone) = (Device::new("Mac").await, Device::new("Pixel 8").await);
    with_data(&mac, &phone);
    type_code(&mac, &phone).await;
    mac.call::<()>("sync_pair_decide", json!({ "allow": true }));
    phone.until("preview").await;
    let (mac_tasks, phone_tasks) = (tasks_on(&mac.app), tasks_on(&phone.app));

    phone.call::<()>("sync_pair_merge", json!({ "merge": false }));

    // The typing device's sheet just closes; the showing device is told (and keeps its code).
    phone.until("idle").await;
    let told = mac.until("showing").await;
    assert_eq!(
        told["notice"],
        json!({ "kind": "cancelled", "name": "Pixel 8" })
    );
    assert!(mac.listed().is_empty() && phone.listed().is_empty());
    assert!(backups(&mac.app).is_empty() && backups(&phone.app).is_empty());
    assert_eq!(
        (tasks_on(&mac.app), tasks_on(&phone.app)),
        (mac_tasks, phone_tasks)
    );
    assert_eq!(mac.app.sync_probe().sessions, 0);
}

#[tokio::test]
async fn closing_the_preview_sheet_is_a_cancel_too() {
    let (mac, phone) = (Device::new("Mac").await, Device::new("Pixel 8").await);
    with_data(&mac, &phone);
    type_code(&mac, &phone).await;
    mac.call::<()>("sync_pair_decide", json!({ "allow": true }));
    phone.until("preview").await;

    phone.call::<()>("sync_pair_cancel", json!({}));

    phone.until("idle").await;
    assert_eq!(mac.until("showing").await["notice"]["kind"], "cancelled");
    assert!(mac.listed().is_empty() && phone.listed().is_empty());
    assert!(backups(&mac.app).is_empty() && backups(&phone.app).is_empty());
}

#[tokio::test]
async fn the_showing_device_can_cancel_while_it_waits() {
    let (mac, phone) = (Device::new("Mac").await, Device::new("Pixel 8").await);
    with_data(&mac, &phone);
    type_code(&mac, &phone).await;
    mac.call::<()>("sync_pair_decide", json!({ "allow": true }));
    phone.until("preview").await;
    mac.until("confirming").await;

    mac.call::<()>("sync_pair_cancel", json!({}));

    let told = phone.until("failed").await;
    assert_eq!(told["error"], json!({ "kind": "cancelled", "name": "Mac" }));
    assert!(mac.listed().is_empty() && phone.listed().is_empty());
    assert!(backups(&mac.app).is_empty() && backups(&phone.app).is_empty());
}

#[tokio::test]
async fn the_preview_is_skipped_when_one_side_is_empty_but_both_are_still_backed_up() {
    // Either side may be the empty one.
    for mac_has_data in [true, false] {
        let (mac, phone) = (Device::new("Mac").await, Device::new("Pixel 8").await);
        let full = if mac_has_data { &mac } else { &phone };
        journal(&full.app, "only here");
        task(&full.app, "Only task", None);
        type_code(&mac, &phone).await;
        mac.call::<()>("sync_pair_decide", json!({ "allow": true }));

        // No preview: straight to paired.
        mac.until("paired").await;
        phone.until("paired").await;
        assert_eq!(mac.listed(), ["Pixel 8"]);
        assert_eq!(backups(&mac.app).len(), 1);
        assert_eq!(backups(&phone.app).len(), 1);
    }
}

#[tokio::test]
async fn a_device_that_cannot_back_up_stops_the_pairing_with_nothing_changed() {
    for typer_fails in [true, false] {
        let (mac, phone) = (Device::new("Mac").await, Device::new("Pixel 8").await);
        with_data(&mac, &phone);
        // A file where the folder should be: the backup cannot be written.
        let broken = if typer_fails { &phone } else { &mac };
        let dir = broken.app.backup_dir();
        std::fs::create_dir_all(dir.parent().unwrap()).unwrap();
        std::fs::write(dir, b"in the way").unwrap();
        type_code(&mac, &phone).await;
        mac.call::<()>("sync_pair_decide", json!({ "allow": true }));
        phone.until("preview").await;
        phone.call::<()>("sync_pair_merge", json!({ "merge": true }));

        if typer_fails {
            let failed = phone.until("failed").await;
            assert_eq!(
                failed["error"],
                json!({ "kind": "backup_failed", "name": null })
            );
            let notice = mac.until("showing").await;
            assert!(
                notice["notice"]["kind"] == "backup_failed"
                    && notice["notice"]["name"] == "Pixel 8",
                "{notice}"
            );
        } else {
            let failed = phone.until("failed").await;
            assert_eq!(
                failed["error"],
                json!({ "kind": "backup_failed", "name": "Mac" })
            );
            let notice = mac.until("showing").await;
            assert!(
                notice["notice"]["kind"] == "backup_failed" && notice["notice"]["name"].is_null(),
                "{notice}"
            );
        }
        assert!(mac.listed().is_empty() && phone.listed().is_empty());
    }
}
