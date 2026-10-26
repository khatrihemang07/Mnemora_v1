//! Sync for the AI Module (ADR 0019): Conversations, their entries and Saved questions across
//! paired devices. Each test drives the real commands on each device and syncs the devices over
//! the in-memory line (`Cluster`). Providers, keys, usage and the visible Branch stay on the
//! device that holds them.

use mnemora_harness::scripted::Reply;
use serde_json::{Value, json};

use crate::ai_tracer_tests::{ask, get, install, provider_id, scripted, until_end};
use crate::sync_address_tests::group;
use crate::sync_live_tests::{meet, paired_pair, peer_of, quick, until};
use crate::sync_pairing_tests::Device;
use crate::test_support::{Cluster, TestApp};

/// Records the Module's own kinds of change, and nothing else.
const AI_ENTITIES: [&str; 3] = ["ai_conversation", "ai_entry", "ai_saved_question"];

/// Two paired devices, each with its own Provider answering with a scripted reply.
fn pair() -> Cluster {
    let c = group(2);
    for d in 0..2 {
        answering(&c[d]);
    }
    c
}

/// Gives a device its own scripted Provider, answering every question.
pub(crate) fn answering(app: &TestApp) {
    let replies: Vec<Reply> = (0..200)
        .map(|n| {
            let text = format!("Answer {n}");
            Reply::text(&[text.as_str()])
        })
        .collect();
    install(app, &scripted(replies));
}

/// Asks a question on `app` and waits for its Run to end. Returns the Conversation it went into.
fn ask_and_wait(app: &TestApp, conversation: Option<&str>, text: &str) -> String {
    let events = app.ai_events();
    let started = ask(app, conversation, text).unwrap();
    until_end(&events);
    started["conversation_id"].as_str().unwrap().to_owned()
}

fn rename(app: &TestApp, id: &str, title: &str) {
    app.invoke::<()>(
        "ai_conversation_rename",
        json!({ "id": id, "title": title }),
    )
    .unwrap();
}

/// The live Conversations on a device, by id.
fn live_ids(app: &TestApp) -> Vec<String> {
    let page: Value = app
        .invoke(
            "ai_conversation_list",
            json!({ "query": "", "offset": 0, "limit": 100 }),
        )
        .unwrap();
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_owned())
        .collect()
}

fn title_of(app: &TestApp, id: &str) -> Option<String> {
    let page: Value = app
        .invoke(
            "ai_conversation_list",
            json!({ "query": "", "offset": 0, "limit": 100 }),
        )
        .unwrap();
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .map(|c| c["title"].as_str().unwrap().to_owned())
}

/// The questions and answers a device shows for a Conversation, oldest first.
fn read_texts(view: &Value) -> Vec<String> {
    view["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| {
            let p = &e["payload"];
            p["kind"] == "user" || (p["kind"] == "assistant" && p["calls"] == json!([]))
        })
        .map(|e| e["payload"]["text"].as_str().unwrap_or("").to_owned())
        .collect()
}

/// The questions beside each other in a Conversation, in the order the pager shows them: each
/// device switches to every position in turn and reads which question sits there.
fn branch_order(app: &TestApp, id: &str) -> Vec<String> {
    let view = get(app, id);
    let question = view["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["payload"]["kind"] == "user" && e["sibling_count"] == 2)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    (1..=2)
        .map(|position| {
            app.invoke::<Value>(
                "ai_conversation_show_branch",
                json!({ "id": id, "entryId": question, "position": position }),
            )
            .unwrap();
            let shown = get(app, id);
            shown["entries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| {
                    e["payload"]["kind"] == "user"
                        && e["sibling_index"] == position
                        && e["sibling_count"] == 2
                })
                .unwrap()["payload"]["text"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

fn saved_texts(app: &TestApp) -> Vec<String> {
    let list: Value = app.invoke("ai_saved_list", json!({})).unwrap();
    list.as_array()
        .unwrap()
        .iter()
        .map(|q| q["text"].as_str().unwrap().to_owned())
        .collect()
}

fn saved_id(app: &TestApp, text: &str) -> String {
    let list: Value = app.invoke("ai_saved_list", json!({})).unwrap();
    list.as_array()
        .unwrap()
        .iter()
        .find(|q| q["text"] == text)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The first column of every row of `sql`, as text.
fn column(app: &TestApp, sql: &str) -> Vec<String> {
    app.with_db(|db| {
        let mut stmt = db.prepare(sql).unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    })
}

#[test]
fn a_conversation_asked_on_one_device_reads_the_same_on_the_other() {
    let c = pair();
    let id = ask_and_wait(&c[0], None, "Where is the museum?");
    c.sync(0, 1);

    assert_eq!(live_ids(&c[1]), vec![id.clone()]);
    assert_eq!(title_of(&c[1], &id), title_of(&c[0], &id));
    assert_eq!(get(&c[1], &id)["entries"], get(&c[0], &id)["entries"]);
    // The Provider's name and the Model id travel as text, so the phone can show them.
    let change = &get(&c[1], &id)["entries"][0]["payload"];
    assert_eq!(change["kind"], "model_change");
    assert_eq!(change["provider_name"], "Scripted");
    assert_eq!(change["model"], "scripted-model");
}

#[test]
fn a_continuation_on_each_device_becomes_two_branches_on_both() {
    let c = pair();
    let id = ask_and_wait(&c[0], None, "First question");
    c.sync(0, 1);
    ask_and_wait(&c[0], Some(&id), "Follow up from the Mac");
    ask_and_wait(&c[1], Some(&id), "Follow up from the phone");
    c.sync_all();

    // Each device shows the Branch it continued, and never the other's question.
    let mac = read_texts(&get(&c[0], &id));
    let phone = read_texts(&get(&c[1], &id));
    assert!(mac.contains(&"Follow up from the Mac".to_owned()));
    assert!(!mac.contains(&"Follow up from the phone".to_owned()));
    assert!(phone.contains(&"Follow up from the phone".to_owned()));
    assert!(!phone.contains(&"Follow up from the Mac".to_owned()));

    // Both questions are beside each other on both devices, in the same order.
    let mut first = branch_order(&c[0], &id);
    let mut second = branch_order(&c[1], &id);
    assert_eq!(first, second);
    first.sort();
    assert_eq!(
        first,
        vec![
            "Follow up from the Mac".to_owned(),
            "Follow up from the phone".to_owned()
        ]
    );
    second.sort();
    assert_eq!(second.len(), 2);
}

#[test]
fn the_history_is_in_the_same_order_on_both_devices_when_an_answer_arrives_from_the_other() {
    let c = pair();
    let older = ask_and_wait(&c[0], None, "Older talk");
    c[1].clock().advance(1_000);
    let newer = ask_and_wait(&c[1], None, "Newer talk");
    c.sync(0, 1);
    assert_eq!(live_ids(&c[0]), vec![newer.clone(), older.clone()]);
    assert_eq!(live_ids(&c[1]), vec![newer.clone(), older.clone()]);

    // An answer that the Mac gives to the older Conversation moves it to the top on both devices.
    c[0].clock().advance(5_000);
    ask_and_wait(&c[0], Some(&older), "Follow up");
    c.sync(0, 1);
    assert_eq!(live_ids(&c[0]), vec![older.clone(), newer.clone()]);
    assert_eq!(live_ids(&c[1]), vec![older.clone(), newer.clone()]);
}

#[test]
fn a_rename_reaches_the_other_device() {
    let c = pair();
    let id = ask_and_wait(&c[0], None, "Plan the hike");
    c.sync(0, 1);
    rename(&c[1], &id, "Hike in May");
    c.sync(1, 0);

    assert_eq!(title_of(&c[0], &id).as_deref(), Some("Hike in May"));
    assert_eq!(title_of(&c[1], &id).as_deref(), Some("Hike in May"));
}

#[test]
fn renames_of_the_same_title_on_both_devices_are_one_conflict_that_both_show() {
    let c = pair();
    let id = ask_and_wait(&c[0], None, "Plan the hike");
    c.sync(0, 1);
    rename(&c[0], &id, "Hike with Ana");
    c[1].clock().advance(1_000);
    rename(&c[1], &id, "Hike with Bruno");
    c.sync_all();

    let waiting = crate::sync_conflicts_tests::ids(&c[0]);
    assert_eq!(waiting.len(), 1);
    assert_eq!(crate::sync_conflicts_tests::ids(&c[1]), waiting);
    // The newest rename shows until the Conflict is settled, on both devices.
    assert_eq!(title_of(&c[0], &id).as_deref(), Some("Hike with Bruno"));
    assert_eq!(title_of(&c[1], &id).as_deref(), Some("Hike with Bruno"));
}

#[test]
fn keep_other_on_a_conversation_title_conflict_writes_it_back_and_converges() {
    let c = pair();
    let id = ask_and_wait(&c[0], None, "Plan the hike");
    c.sync(0, 1);
    rename(&c[0], &id, "Hike with Ana");
    c[1].clock().advance(1_000);
    rename(&c[1], &id, "Hike with Bruno");
    c.sync_all();
    let conflict = crate::sync_conflicts_tests::ids(&c[0])[0].clone();

    // The older device keeps the other title: its own, which the newer rename set aside.
    crate::sync_conflicts_tests::resolve(&c[0], &conflict, json!({ "kind": "keep_other" }));
    assert_eq!(title_of(&c[0], &id).as_deref(), Some("Hike with Ana"));
    assert!(crate::sync_conflicts_tests::ids(&c[0]).is_empty());
    c.sync_all();
    for d in 0..2 {
        assert!(
            crate::sync_conflicts_tests::ids(&c[d]).is_empty(),
            "device {d}"
        );
        assert_eq!(
            title_of(&c[d], &id).as_deref(),
            Some("Hike with Ana"),
            "device {d}"
        );
    }
}

#[test]
fn keep_other_on_a_saved_question_text_conflict_writes_it_back_and_converges() {
    let c = pair();
    c[0].invoke::<Value>("ai_saved_create", json!({ "text": "Coffee shops" }))
        .unwrap();
    c.sync(0, 1);
    let id = saved_id(&c[0], "Coffee shops");
    update_saved(&c[0], &id, "Coffee shops near the canal");
    c[1].clock().advance(1_000);
    update_saved(&c[1], &id, "Coffee shops near the park");
    c.sync_all();
    let conflict = crate::sync_conflicts_tests::ids(&c[0])[0].clone();

    crate::sync_conflicts_tests::resolve(&c[0], &conflict, json!({ "kind": "keep_other" }));
    assert_eq!(
        saved_texts(&c[0]),
        vec!["Coffee shops near the canal".to_owned()]
    );
    assert!(crate::sync_conflicts_tests::ids(&c[0]).is_empty());
    c.sync_all();
    for d in 0..2 {
        assert!(
            crate::sync_conflicts_tests::ids(&c[d]).is_empty(),
            "device {d}"
        );
        assert_eq!(
            saved_texts(&c[d]),
            vec!["Coffee shops near the canal".to_owned()],
            "device {d}"
        );
    }
}

fn update_saved(app: &TestApp, id: &str, text: &str) {
    app.invoke::<Value>("ai_saved_update", json!({ "id": id, "text": text }))
        .unwrap();
}

#[test]
fn a_deleted_conversation_is_hidden_on_the_other_device_and_undo_brings_it_back() {
    let c = pair();
    let id = ask_and_wait(&c[0], None, "Where to eat?");
    ask_and_wait(&c[0], Some(&id), "And for dessert?");
    c.sync(0, 1);

    c[0].invoke::<()>("ai_conversation_delete", json!({ "id": id }))
        .unwrap();
    c.sync(0, 1);
    assert!(!live_ids(&c[1]).contains(&id));
    assert!(
        c[1].invoke::<Value>("ai_conversation_get", json!({ "id": id }))
            .is_err()
    );

    c[0].invoke::<()>("ai_conversation_restore", json!({ "id": id }))
        .unwrap();
    c.sync(0, 1);
    assert!(live_ids(&c[1]).contains(&id));
    assert_eq!(read_texts(&get(&c[1], &id)), read_texts(&get(&c[0], &id)));
}

#[test]
fn saved_questions_order_converges_when_each_device_moves_a_different_one() {
    let c = pair();
    for text in ["Coffee shops", "Museum hours", "Train times"] {
        c[0].invoke::<Value>("ai_saved_create", json!({ "text": text }))
            .unwrap();
    }
    c.sync(0, 1);
    let (coffee, train) = (
        saved_id(&c[1], "Coffee shops"),
        saved_id(&c[1], "Train times"),
    );

    // The Mac moves Coffee shops to the bottom while the phone moves Train times to the top.
    c[0].invoke::<()>(
        "ai_saved_reorder",
        json!({ "id": coffee, "afterId": train, "beforeId": null }),
    )
    .unwrap();
    c[1].invoke::<()>(
        "ai_saved_reorder",
        json!({ "id": train, "afterId": null, "beforeId": coffee }),
    )
    .unwrap();
    c.sync_all();

    let expected = ["Train times", "Museum hours", "Coffee shops"];
    assert_eq!(saved_texts(&c[0]), expected);
    assert_eq!(saved_texts(&c[1]), expected);
}

#[test]
fn a_deleted_saved_question_is_hidden_on_the_other_device_and_undo_puts_it_back_in_place() {
    let c = pair();
    for text in ["First", "Second", "Third"] {
        c[0].invoke::<Value>("ai_saved_create", json!({ "text": text }))
            .unwrap();
    }
    c.sync(0, 1);
    let second = saved_id(&c[0], "Second");

    c[0].invoke::<()>("ai_saved_delete", json!({ "id": second }))
        .unwrap();
    c.sync(0, 1);
    assert_eq!(saved_texts(&c[1]), ["First", "Third"]);

    c[0].invoke::<()>("ai_saved_restore", json!({ "id": second }))
        .unwrap();
    c.sync(0, 1);
    assert_eq!(saved_texts(&c[1]), ["First", "Second", "Third"]);
}

#[test]
fn providers_usage_and_the_visible_branch_never_leave_their_device() {
    let c = pair();
    let id = ask_and_wait(&c[0], None, "Question from the Mac");
    ask_and_wait(&c[0], Some(&id), "Second from the Mac");
    c.sync(0, 1);

    // The phone has its own Provider only, and the Mac's address is nowhere in what it holds.
    let phone_providers = column(&c[1], "SELECT id FROM ai_providers");
    assert_eq!(phone_providers, vec![provider_id(&c[1])]);
    assert_ne!(provider_id(&c[0]), provider_id(&c[1]));
    // Usage and the visible Branch are per device: the phone has used nothing and shows nothing
    // on a Conversation it never continued.
    let phone_usage: i64 = c[1].with_db(|db| {
        db.query_row("SELECT COUNT(*) FROM ai_model_usage", [], |r| r.get(0))
            .unwrap()
    });
    assert_eq!(phone_usage, 0);
    let phone_visible: i64 = c[1].with_db(|db| {
        db.query_row("SELECT COUNT(*) FROM ai_visible_branches", [], |r| r.get(0))
            .unwrap()
    });
    assert_eq!(phone_visible, 0);

    // Only the AI's own three kinds of record are ever logged, so no Provider, usage row or visible
    // Branch can be sent, and the Provider's address is in no record on either device.
    for d in 0..2 {
        let entities = column(
            &c[d],
            "SELECT DISTINCT entity FROM change_log WHERE module = 'ai'",
        );
        assert!(
            entities.iter().all(|e| AI_ENTITIES.contains(&e.as_str())),
            "device {d} logged {entities:?}"
        );
        let leaked: i64 = c[d].with_db(|db| {
            db.query_row(
                "SELECT COUNT(*) FROM change_log WHERE snapshot LIKE '%127.0.0.1%' OR fields LIKE '%127.0.0.1%'",
                [],
                |r| r.get(0),
            )
            .unwrap()
        });
        assert_eq!(leaked, 0, "device {d} logged a Provider's address");
    }
}

#[test]
fn a_conversation_continued_on_a_device_without_its_provider_works_with_one_of_its_own() {
    let c = pair();
    let id = ask_and_wait(&c[0], None, "Mac question");
    c.sync(0, 1);

    // The phone reads the Conversation with the Mac's Provider name and Model as text, and its own
    // Provider id is not the one the Conversation names.
    let view = get(&c[1], &id);
    assert_eq!(view["provider_name"], "Scripted");
    assert_ne!(view["provider_id"], json!(provider_id(&c[1])));

    // Continuing it on the phone uses the phone's own Provider, which the Conversation then names.
    let phone = provider_id(&c[1]);
    ask_and_wait(&c[1], Some(&id), "Phone follow-up");
    assert!(read_texts(&get(&c[1], &id)).contains(&"Phone follow-up".to_owned()));
    c.sync(1, 0);
    assert_eq!(get(&c[0], &id)["provider_id"], json!(phone));
}

#[test]
fn rows_that_predate_the_change_log_reach_the_other_device() {
    let c = pair();
    c[0].with_db(|db| {
        db.execute_batch(
            "INSERT INTO ai_conversations(id, title, created_at, updated_at)
                 VALUES ('old-talk', 'Old talk', 1000, 1000);
             INSERT INTO ai_entries(id, conversation_id, parent_id, kind, payload, created_at)
                 VALUES ('old-question', 'old-talk', NULL, 'user',
                         '{\"kind\":\"user\",\"text\":\"Old question\"}', 1000);
             INSERT INTO ai_saved_questions(id, text, sort_key, created_at, updated_at)
                 VALUES ('old-saved', 'Old saved', 'V', 1000, 1000);",
        )
        .unwrap();
    });
    c.sync(0, 1);

    assert_eq!(title_of(&c[1], "old-talk").as_deref(), Some("Old talk"));
    assert_eq!(read_texts(&get(&c[1], "old-talk")), ["Old question"]);
    assert_eq!(saved_texts(&c[1]), ["Old saved"]);
}

/// What "updating Mnemora" does to a device's data version of the AI Module.
fn set_ai_version(device: &Device, delta: i64) {
    device.app.with_db(|c| {
        c.execute(
            "UPDATE _module_migrations SET version = version + ?1 WHERE module = 'ai'",
            [delta],
        )
        .unwrap();
    });
}

fn saved_on(device: &Device) -> Vec<String> {
    saved_texts(&device.app)
}

#[tokio::test]
async fn ai_data_versions_that_differ_pause_sync_until_the_older_one_updates() {
    let (mac, phone) = paired_pair(quick()).await;
    // The Mac was updated; the phone was not.
    set_ai_version(&mac, 1);
    mac.app
        .invoke::<Value>("ai_saved_create", json!({ "text": "Written on the Mac" }))
        .unwrap();
    phone
        .app
        .invoke::<Value>("ai_saved_create", json!({ "text": "Written on the phone" }))
        .unwrap();
    meet(&mac, &phone);
    until("both paused", || {
        peer_of(&mac)["state"] == "paused" && peer_of(&phone)["state"] == "paused"
    })
    .await;
    assert_eq!(
        peer_of(&mac)["pause"],
        json!({ "kind": "update", "older_here": [], "older_there": ["ai"] })
    );
    // Let the retries run: nothing crosses while the pause holds.
    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    assert_eq!(saved_on(&mac), ["Written on the Mac"]);
    assert_eq!(saved_on(&phone), ["Written on the phone"]);

    // The phone updates: the two sides catch up.
    set_ai_version(&phone, 1);
    until("the phone is up to date with the Mac", || {
        peer_of(&mac)["state"] == "up_to_date" && peer_of(&phone)["state"] == "up_to_date"
    })
    .await;
    until("what was written meanwhile arrives", || {
        saved_on(&mac).len() == 2 && saved_on(&phone).len() == 2
    })
    .await;
    assert_eq!(saved_on(&mac), saved_on(&phone));
}

#[test]
fn rows_received_while_the_ai_module_is_off_are_kept_and_show_once_it_is_turned_on() {
    let c = pair();
    c[1].set_pref("core.enabled_optional_modules", json!([]));
    let id = ask_and_wait(&c[0], None, "Where is the museum?");
    c.sync(0, 1);

    // Sync does not ask whether the Module is on: the rows are stored all the same.
    assert_eq!(
        column(
            &c[1],
            "SELECT id FROM ai_conversations WHERE deleted_at IS NULL"
        ),
        vec![id.clone()]
    );
    assert!(!column(&c[1], "SELECT id FROM ai_entries").is_empty());

    c[1].set_pref("core.enabled_optional_modules", json!(["ai"]));
    assert_eq!(live_ids(&c[1]), vec![id]);
}
