//! Pauses (#72): a paired device whose data versions or clock differ is not synced with, either
//! way, and sync picks up by itself once the cause is gone. Pairing across data versions is
//! refused. Both devices keep working on their own meanwhile.

use mnemora_sync::SyncError;
use serde_json::{Value, json};

use crate::sync_live_tests::{create, has, meet, paired_pair, peer_of, quick, until};
use crate::sync_pairing_tests::Device;
use crate::test_support::{Cluster, TestApp};

const HOUR: i64 = 3_600_000;

/// What "updating Mnemora" does to a device's data version of the Todo Module.
fn set_todo_version(device: &Device, delta: i64) {
    device.app.with_db(|c| {
        c.execute(
            "UPDATE _module_migrations SET version = version + ?1 WHERE module = 'todo'",
            [delta],
        )
        .unwrap();
    });
}

fn lines(device: &Device, kind: &str) -> usize {
    let page: Value = device
        .app
        .invoke(
            "sync_activity",
            json!({ "filter": { "kind": "all" }, "cursor": null, "limit": 100 }),
        )
        .unwrap();
    page["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == kind)
        .count()
}

/// How many lines of `kind` the group's Activity holds (the one device that leads writes it; the
/// other gets the same line when they next sync).
fn logged(a: &Device, b: &Device, kind: &str) -> usize {
    lines(a, kind).max(lines(b, kind))
}

fn paused(device: &Device) -> bool {
    peer_of(device)["state"] == "paused"
}

async fn running(mac: &Device, phone: &Device) {
    until("both up to date", || {
        peer_of(mac)["state"] == "up_to_date" && peer_of(phone)["state"] == "up_to_date"
    })
    .await;
}

#[tokio::test]
async fn different_data_versions_pause_both_sides_until_the_older_one_updates() {
    let (mac, phone) = paired_pair(quick()).await;
    // The Mac was updated; the phone was not.
    set_todo_version(&mac, 1);
    let on_phone = create(&phone, "Written on the phone");
    let on_mac = create(&mac, "Written on the Mac");
    meet(&mac, &phone);
    until("both paused", || paused(&mac) && paused(&phone)).await;
    // The Mac says the other device is older, the phone says it is.
    assert_eq!(
        peer_of(&mac)["pause"],
        json!({ "kind": "update", "older_here": [], "older_there": ["todo"] })
    );
    assert_eq!(
        peer_of(&phone)["pause"],
        json!({ "kind": "update", "older_here": ["todo"], "older_there": [] })
    );
    // Let the retries run: nothing crosses, and the pause is logged once.
    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    assert!(!has(&mac, &on_phone) && !has(&phone, &on_mac));
    assert!(paused(&mac) && paused(&phone));
    for device in [&mac, &phone] {
        let attention: Value = device.app.invoke("sync_attention", json!({})).unwrap();
        assert_eq!(attention["paused"].as_array().unwrap().len(), 1);
        assert_eq!(attention["needed"], true);
    }
    // The device that leads logs it (the lower id), and only once.
    assert_eq!(logged(&mac, &phone, "paused"), 1);
    // Both keep working on their own.
    create(&phone, "Still works");
    // The phone updates.
    set_todo_version(&phone, 1);
    running(&mac, &phone).await;
    until("what was written meanwhile arrives", || {
        has(&mac, &on_phone) && has(&phone, &on_mac)
    })
    .await;
    assert_eq!(peer_of(&mac)["pause"], Value::Null);
    until("the resume to be logged", || {
        logged(&mac, &phone, "resumed") == 1
    })
    .await;
    assert_eq!(logged(&mac, &phone, "paused"), 1);
}

#[tokio::test]
async fn a_clock_more_than_five_minutes_ahead_pauses_until_the_time_is_fixed() {
    let (mac, phone) = paired_pair(quick()).await;
    let from_phone = create(&phone, "From the phone");
    phone.app.clock().skew(3 * HOUR);
    meet(&mac, &phone);
    until("both paused", || paused(&mac) && paused(&phone)).await;
    let (theirs, ours) = (
        peer_of(&mac)["pause"].clone(),
        peer_of(&phone)["pause"].clone(),
    );
    assert_eq!(theirs["kind"], "clock");
    assert_eq!(
        theirs["here"], false,
        "the Mac sees the phone's clock ahead"
    );
    assert_eq!(ours["here"], true, "the phone sees its own clock ahead");
    let ahead = theirs["ahead_ms"].as_i64().unwrap();
    assert!(
        (3 * HOUR - 60_000..=3 * HOUR + 60_000).contains(&ahead),
        "{ahead}"
    );
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    assert!(!has(&mac, &from_phone), "nothing crosses while paused");
    assert_eq!(logged(&mac, &phone, "paused"), 1);
    // The person fixes the time.
    phone.app.clock().skew(0);
    running(&mac, &phone).await;
    until("the record arrives", || has(&mac, &from_phone)).await;
    until("the resume to be logged", || {
        logged(&mac, &phone, "resumed") == 1
    })
    .await;
}

#[tokio::test]
async fn a_record_from_a_wrong_clock_pauses_even_after_the_handshake_looked_fine() {
    let (mac, phone) = paired_pair(quick()).await;
    phone.app.clock().skew(3 * HOUR);
    let from_future = create(&phone, "Made on a fast clock");
    phone.app.clock().skew(0);
    meet(&mac, &phone);
    until("the Mac pauses", || paused(&mac)).await;
    // The clock is fine now; what is wrong is the date on the records, and the banner says that.
    assert_eq!(peer_of(&mac)["pause"]["kind"], "stamped");
    let wait = peer_of(&mac)["pause"]["resumes_in_ms"].as_i64().unwrap();
    assert!(
        (3 * HOUR - 6 * 60_000..=3 * HOUR).contains(&wait),
        "resumes once real time is within five minutes of the records: {wait}"
    );
    assert!(!has(&mac, &from_future), "never stored");
    // Retries meet a good handshake each time, but the pause stays and is logged once: it is
    // not "resumed" until a sync really ran.
    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    assert_eq!(logged(&mac, &phone, "paused"), 1);
    assert_eq!(logged(&mac, &phone, "resumed"), 0);
    assert!(paused(&mac));
    assert!(!has(&mac, &from_future));
}

#[tokio::test]
async fn records_from_a_wrong_clock_are_taken_once_real_time_catches_up() {
    let (mac, phone) = paired_pair(quick()).await;
    phone.app.clock().skew(3 * HOUR);
    let from_future = create(&phone, "Made on a fast clock");
    phone.app.clock().skew(0);
    meet(&mac, &phone);
    until("the Mac pauses", || paused(&mac)).await;
    // Three hours pass on both devices.
    mac.app.clock().skew(3 * HOUR);
    phone.app.clock().skew(3 * HOUR);
    running(&mac, &phone).await;
    until("the records arrive", || has(&mac, &from_future)).await;
    until("the resume to be logged", || {
        logged(&mac, &phone, "resumed") == 1
    })
    .await;
    assert_eq!(logged(&mac, &phone, "paused"), 1);
}

#[tokio::test]
async fn a_clock_five_minutes_ahead_or_less_is_fine() {
    let (mac, phone) = paired_pair(quick()).await;
    phone.app.clock().skew(4 * 60_000);
    meet(&mac, &phone);
    running(&mac, &phone).await;
}

#[test]
fn a_record_stamped_more_than_five_minutes_ahead_is_refused_and_never_seen_by_the_clock() {
    let c = Cluster::new(2);
    c[1].clock().advance(3 * HOUR);
    create_task(&c[1], "From the future");
    let refused = c.try_sync(0, 1, None).unwrap_err();
    assert!(
        matches!(refused, SyncError::ClockAhead { .. }),
        "{refused:?}"
    );
    assert_eq!(task_titles(&c[0]), Vec::<String>::new());
    // Its own next record is stamped by its own time, not pulled into the future.
    create_task(&c[0], "Here and now");
    let newest: i64 = c[0].with_db(|d| {
        d.query_row("SELECT MAX(hlc_ms) FROM change_log", [], |r| r.get(0))
            .unwrap()
    });
    assert!(newest < Cluster::START + 60_000, "{newest}");
}

#[test]
fn edits_after_the_clock_is_fixed_are_not_stamped_further_ahead() {
    let c = Cluster::new(1);
    c[0].clock().advance(3 * HOUR);
    create_task(&c[0], "On a fast clock");
    let ahead = newest_stamp(&c[0]);
    c[0].clock().advance(-3 * HOUR);
    create_task(&c[0], "After the fix");
    create_task(&c[0], "And another");
    let now = newest_stamp(&c[0]);
    assert_eq!(now.0, ahead.0, "the pause is not extended by new edits");
    assert!(now > ahead, "but they still sort after what came before");
}

fn newest_stamp(app: &TestApp) -> (i64, i64) {
    app.with_db(|d| {
        d.query_row(
            "SELECT hlc_ms, hlc_counter FROM change_log ORDER BY hlc_ms DESC, hlc_counter DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    })
}

#[test]
fn a_record_a_few_minutes_ahead_is_taken() {
    let c = Cluster::new(2);
    c[1].clock().advance(4 * 60_000);
    create_task(&c[1], "A little fast");
    c.sync(0, 1);
    assert_eq!(task_titles(&c[0]), ["A little fast"]);
}

#[tokio::test]
async fn pairing_across_data_versions_is_refused_with_the_fix_and_nothing_is_stored() {
    let (mac, phone) = (Device::new("Mac").await, Device::new("Pixel 8").await);
    set_todo_version(&mac, 1);
    mac.call::<()>("sync_pair_show", json!({}));
    let code = mac.pairing()["code"].as_str().unwrap().replace(' ', "");
    phone.found.set(vec![mac.address()]);
    phone.call::<()>("sync_pair_enter", json!({ "code": code }));
    let failed = phone.until("failed").await;
    assert_eq!(failed["error"]["kind"], "version_mismatch");
    assert_eq!(failed["error"]["name"], "Mac");
    assert_eq!(failed["error"]["older"], "me");
    assert!(phone.listed().is_empty() && mac.listed().is_empty());
}

fn create_task(app: &TestApp, title: &str) {
    app.invoke::<Value>("todo_task_create", json!({ "title": title }))
        .unwrap();
}

fn task_titles(app: &TestApp) -> Vec<String> {
    app.with_db(|c| {
        c.prepare("SELECT title FROM todo_tasks ORDER BY title")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    })
}
