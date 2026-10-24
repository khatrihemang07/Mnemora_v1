//! Sync's lifecycle on Android (#74) through the same entry points the JNI side calls: leaving the
//! front flushes pending changes and then stops everything; mobile data stops it unless allowed.
//! Real loopback connections between two `TestApp`s; the pure rules are unit tests in `sync_power`.

use std::time::{Duration, Instant};

use mnemora_sync::Pace;
use serde_json::json;

use crate::sync_live_tests::{
    activity_lines, create, has, meet, paired_pair, peer_of, quick, until,
};
use crate::sync_power::{self, Link, Screen};

fn slow_push() -> Pace {
    Pace {
        debounce: Duration::from_secs(30),
        ..quick()
    }
}

fn stopped(device: &crate::sync_pairing_tests::Device) -> bool {
    let probe = device.app.sync_probe();
    probe.listeners == 0 && probe.advertisements == 0 && probe.tasks == 0
}

#[tokio::test]
async fn leaving_the_app_flushes_pending_changes_then_stops_everything() {
    let (mac, phone) = paired_pair(slow_push()).await;
    meet(&mac, &phone);
    until("connected", || peer_of(&mac)["state"] == "up_to_date").await;
    // Pairing left a line in Activity; let the first session finish.
    until("the first session", || {
        peer_of(&mac)["last_synced_at"].is_u64()
    })
    .await;
    assert!(!stopped(&mac));

    let id = create(&mac, "Last edit before the app is put away");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!has(&phone, &id), "the 30 s quiet period had not run out");

    let left = Instant::now();
    sync_power::foreground(&mac.app.handle(), false);
    until("the Task on the phone", || has(&phone, &id)).await;
    assert!(
        left.elapsed() < Duration::from_secs(5),
        "flushed without waiting out the quiet period"
    );
    until("everything to stop", || stopped(&mac)).await;
    until("the phone to see it gone", || {
        peer_of(&phone)["state"] == "not_nearby"
    })
    .await;
}

#[tokio::test]
async fn nothing_is_stopped_while_the_app_stays_in_front() {
    let (mac, phone) = paired_pair(quick()).await;
    meet(&mac, &phone);
    until("connected", || peer_of(&mac)["state"] == "up_to_date").await;
    // A resume or a repeated "in front" report changes nothing.
    sync_power::foreground(&mac.app.handle(), true);
    sync_power::link(&mac.app.handle(), Link::Wifi);
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(!stopped(&mac));
    assert_eq!(peer_of(&mac)["state"], "up_to_date");
}

#[tokio::test]
async fn coming_back_inside_the_flush_window_keeps_the_network() {
    let (mac, phone) = paired_pair(quick()).await;
    meet(&mac, &phone);
    until("connected", || peer_of(&mac)["state"] == "up_to_date").await;
    sync_power::foreground(&mac.app.handle(), false);
    sync_power::foreground(&mac.app.handle(), true);
    // Well past the 600 ms grace the test app uses.
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert!(
        !stopped(&mac),
        "the pending stop was cancelled by the return"
    );
}

#[tokio::test]
async fn mobile_data_stops_sync_unless_it_is_allowed() {
    let (mac, phone) = paired_pair(quick()).await;
    meet(&mac, &phone);
    until("connected", || peer_of(&mac)["state"] == "up_to_date").await;

    // Allowed: staying on Tailscale over mobile data keeps the network.
    mac.call::<()>(
        "sync_set_option",
        json!({ "option": "mobile_data", "enabled": true }),
    );
    sync_power::link(&mac.app.handle(), Link::Mobile);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!stopped(&mac));

    // Switched off while on mobile data: no listener, no dials, no sessions.
    mac.call::<()>(
        "sync_set_option",
        json!({ "option": "mobile_data", "enabled": false }),
    );
    until("everything to stop", || stopped(&mac)).await;
    until("the phone to see it gone", || {
        peer_of(&phone)["state"] == "not_nearby"
    })
    .await;
}

#[tokio::test]
async fn the_screen_and_link_are_remembered_for_the_next_decision() {
    let (mac, _phone) = paired_pair(quick()).await;
    let state = mac.app.handle();
    sync_power::link(&state, Link::Mobile);
    until("stopped", || stopped(&mac)).await;
    sync_power::foreground(&state, false);
    sync_power::link(&state, Link::Wifi);
    // Away: Wi-Fi alone does not bring it back.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(stopped(&mac));
    assert_eq!(sync_power::test_screen(&state), Screen::Away);
}

#[tokio::test]
async fn rapid_leaving_and_returning_never_duplicates_the_network() {
    let (mac, phone) = paired_pair(quick()).await;
    meet(&mac, &phone);
    until("connected", || peer_of(&mac)["state"] == "up_to_date").await;
    let before = mac.app.sync_probe();
    let handle = mac.app.handle();
    for _ in 0..25 {
        sync_power::foreground(&handle, false);
        sync_power::foreground(&handle, true);
    }
    // Past every flush timer that was started: none of them may stop the network now.
    tokio::time::sleep(Duration::from_millis(1_800)).await;
    let after = mac.app.sync_probe();
    assert_eq!(after.listeners, before.listeners);
    assert_eq!(after.advertisements, before.advertisements);
    assert_eq!(after.tasks, before.tasks);
    assert_eq!(sync_power::test_screen(&handle), Screen::Front);
}

#[tokio::test]
async fn on_mobile_data_devices_read_paused_and_pairing_says_why() {
    let (mac, _phone) = paired_pair(quick()).await;
    let id = peer_of(&mac)["id"].as_str().unwrap().to_owned();
    sync_power::link(&mac.app.handle(), Link::Mobile);
    until("everything to stop", || stopped(&mac)).await;

    // The row and the device sheet give the reason instead of "Not nearby".
    assert_eq!(peer_of(&mac)["state"], "paused_on_mobile_data");
    let sheet: serde_json::Value = mac.call("sync_device", json!({ "id": id }));
    assert_eq!(sheet["state"], "paused_on_mobile_data");

    // Pairing is refused up front with the reason, not "Sync is still starting".
    let args = [
        ("sync_pair_show", json!({})),
        ("sync_pair_new_code", json!({})),
        ("sync_pair_enter", json!({ "code": "12345678" })),
        (
            "sync_pair_enter_at",
            json!({ "address": "192.168.1.9", "code": "12345678" }),
        ),
    ];
    for (command, args) in args {
        let refused = mac
            .app
            .invoke::<serde_json::Value>(command, args)
            .unwrap_err();
        let message = refused["message"].as_str().unwrap();
        assert!(message.contains("mobile data"), "{command}: {message}");
    }

    // Allowing mobile data lifts both (tests do not restart the network, so pairing then waits
    // for it instead).
    mac.call::<()>(
        "sync_set_option",
        json!({ "option": "mobile_data", "enabled": true }),
    );
    assert_eq!(peer_of(&mac)["state"], "not_nearby");
    let waiting = mac
        .app
        .invoke::<serde_json::Value>("sync_pair_show", json!({}))
        .unwrap_err();
    assert!(!waiting["message"].as_str().unwrap().contains("mobile data"));
}

/// D17: a live connection that drops while nothing is being exchanged (Wi-Fi off, the phone put
/// away) is not news: Activity logs nothing and the row just reads Not nearby.
#[tokio::test]
async fn an_idle_connection_that_drops_is_not_logged() {
    let (mac, phone) = paired_pair(quick()).await;
    meet(&mac, &phone);
    until("connected", || peer_of(&mac)["state"] == "up_to_date").await;
    until("the first session", || {
        peer_of(&mac)["last_synced_at"].is_u64()
    })
    .await;
    sync_power::foreground(&mac.app.handle(), false);
    until("the phone to see it gone", || {
        peer_of(&phone)["state"] == "not_nearby"
    })
    .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(activity_lines(&mac, "failed").is_empty());
    assert!(activity_lines(&phone, "failed").is_empty());
}
