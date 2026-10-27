//! Command-layer tests for Saved questions: create, rename, reorder, delete with restore, and a
//! Saved question run as a normal Run. They drive the real commands (ADR 0009).

use mnemora_harness::scripted::Reply;
use serde_json::{Value, json};

use crate::ai_tracer_tests::{ask, get, install, label, scripted, until_end};
use crate::test_support::TestApp;

fn saved(app: &TestApp) -> Vec<String> {
    let list: Value = app.invoke("ai_saved_list", json!({})).unwrap();
    list.as_array()
        .unwrap()
        .iter()
        .map(|item| item["text"].as_str().unwrap().to_owned())
        .collect()
}

fn id_of(app: &TestApp, text: &str) -> String {
    let list: Value = app.invoke("ai_saved_list", json!({})).unwrap();
    list.as_array()
        .unwrap()
        .iter()
        .find(|item| item["text"] == text)
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .unwrap()
}

fn create(app: &TestApp, text: &str) -> Value {
    app.invoke("ai_saved_create", json!({ "text": text }))
        .unwrap()
}

#[test]
fn saved_questions_are_listed_in_the_order_they_were_made_and_can_be_renamed() {
    let app = TestApp::new();
    let first = create(&app, "Summarise my last 7 days");
    create(&app, "What is overdue?");
    assert_eq!(first["text"], "Summarise my last 7 days");
    assert_eq!(
        saved(&app),
        ["Summarise my last 7 days", "What is overdue?"]
    );

    let id = first["id"].as_str().unwrap();
    app.invoke::<Value>(
        "ai_saved_update",
        json!({ "id": id, "text": "  Summarise  my week " }),
    )
    .unwrap();
    assert_eq!(saved(&app), ["Summarise my week", "What is overdue?"]);
}

#[test]
fn a_saved_question_moves_between_the_two_neighbours_it_was_placed_between() {
    let app = TestApp::new();
    for text in ["One", "Two", "Three"] {
        create(&app, text);
    }
    let three = id_of(&app, "Three");
    let one = id_of(&app, "One");
    let two = id_of(&app, "Two");

    // Three moves above Two (after One, before Two).
    app.invoke::<Value>(
        "ai_saved_reorder",
        json!({ "id": three, "afterId": one, "beforeId": two }),
    )
    .unwrap();
    assert_eq!(saved(&app), ["One", "Three", "Two"]);

    // One moves to the very end (after Two, before nothing).
    app.invoke::<Value>(
        "ai_saved_reorder",
        json!({ "id": one, "afterId": two, "beforeId": null }),
    )
    .unwrap();
    assert_eq!(saved(&app), ["Three", "Two", "One"]);

    // And to the top (before everything, after nothing).
    app.invoke::<Value>(
        "ai_saved_reorder",
        json!({ "id": one, "afterId": null, "beforeId": three }),
    )
    .unwrap();
    assert_eq!(saved(&app), ["One", "Three", "Two"]);
}

#[test]
fn a_saved_question_can_be_deleted_and_restored_in_its_place() {
    let app = TestApp::new();
    create(&app, "First");
    create(&app, "Second");
    create(&app, "Third");
    let second = id_of(&app, "Second");

    app.invoke::<Value>("ai_saved_delete", json!({ "id": second }))
        .unwrap();
    assert_eq!(saved(&app), ["First", "Third"]);

    app.invoke::<Value>("ai_saved_restore", json!({ "id": second }))
        .unwrap();
    assert_eq!(saved(&app), ["First", "Second", "Third"]);
}

#[test]
fn a_blank_saved_question_is_refused_and_nothing_is_saved() {
    let app = TestApp::new();
    let error = app
        .invoke::<Value>("ai_saved_create", json!({ "text": "  \n " }))
        .unwrap_err();
    assert_eq!(error["code"], "validation");
    assert_eq!(error["message"], "Type a question first.");
    assert!(saved(&app).is_empty());
}

#[test]
fn a_saved_question_that_is_gone_cannot_be_edited_or_moved() {
    let app = TestApp::new();
    create(&app, "Only");
    let id = id_of(&app, "Only");
    app.invoke::<Value>("ai_saved_delete", json!({ "id": id }))
        .unwrap();

    let renamed = app
        .invoke::<Value>("ai_saved_update", json!({ "id": id, "text": "Changed" }))
        .unwrap_err();
    assert_eq!(renamed["code"], "not_found");
    let moved = app
        .invoke::<Value>(
            "ai_saved_reorder",
            json!({ "id": id, "afterId": null, "beforeId": null }),
        )
        .unwrap_err();
    assert_eq!(moved["code"], "not_found");
}

#[test]
fn running_a_saved_question_is_a_normal_run_that_starts_a_conversation_titled_with_it() {
    let app = TestApp::new();
    install(&app, &scripted(vec![Reply::text(&["Here is the week"])]));
    let events = app.ai_events();
    create(&app, "Summarise my last 7 days");

    // The screen sends the saved text as a question; nothing else is different from typing it.
    let started = ask(&app, None, "Summarise my last 7 days").unwrap();
    let run = until_end(&events);

    assert_eq!(
        run.iter().map(label).collect::<Vec<_>>(),
        [
            "runStarted",
            "entrySaved:user",
            "stepStarted",
            "textDelta",
            "entrySaved:assistant",
            "runEnded",
        ]
    );
    let conversation = get(&app, started["conversation_id"].as_str().unwrap());
    assert_eq!(conversation["title"], "Summarise my last 7 days");
    // Running it does not change the list of Saved questions.
    assert_eq!(saved(&app), ["Summarise my last 7 days"]);
}
