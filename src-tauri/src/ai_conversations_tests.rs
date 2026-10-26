//! Command-layer tests for managing Conversations: the history list, rename, delete with restore,
//! and copying as Markdown. They drive the real commands through the mock runtime (ADR 0009).

use std::sync::mpsc;

use mnemora_harness::scripted::Reply;
use serde_json::{Value, json};

use crate::ai_tracer_tests::{ask, get, install, scripted, until_end};
use crate::test_support::TestApp;

const RENAME_RULES: &str = include_str!("../../src/lib/ipc/fixtures/ai-rename.json");
const HISTORY_RULES: &str = include_str!("../../src/lib/ipc/fixtures/ai-history.json");
const MARKDOWN_RULES: &str = include_str!("../../src/lib/ipc/fixtures/ai-markdown.json");
const CITED: &str = "01990000-0000-7000-8000-0000000000e1";

/// Inserts a live Conversation straight into the table, for paging and search (no Run needed).
fn seed_conversation(app: &TestApp, id: &str, title: &str, updated_at: i64) {
    app.with_db(|c| {
        c.execute(
            "INSERT INTO ai_conversations(id, title, created_at, updated_at) VALUES(?1, ?2, 1, ?3)",
            (id, title, updated_at),
        )
        .unwrap();
    });
}

/// Inserts a plain question entry into a Conversation, after `parent`.
fn seed_question(app: &TestApp, id: &str, conversation: &str, parent: Option<&str>, text: &str) {
    app.with_db(|c| {
        c.execute(
            "INSERT INTO ai_entries(id, conversation_id, parent_id, kind, payload, created_at)
             VALUES(?1, ?2, ?3, 'user', ?4, 1)",
            (
                id,
                conversation,
                parent,
                json!({ "kind": "user", "text": text }).to_string(),
            ),
        )
        .unwrap();
    });
}

/// Inserts an answer that cites an Entry, the way a Run saves it (the citation is text only).
fn seed_answer(app: &TestApp, id: &str, conversation: &str, parent: &str, text: &str) {
    let payload = json!({
        "kind": "assistant",
        "text": text,
        "stop": "stop",
        "provider": "Scripted",
        "model": "scripted-model",
        "calls": [],
        "sources": [],
        "budget_note": null,
    });
    app.with_db(|c| {
        c.execute(
            "INSERT INTO ai_entries(id, conversation_id, parent_id, kind, payload, created_at)
             VALUES(?1, ?2, ?3, 'assistant', ?4, 2)",
            (id, conversation, parent, payload.to_string()),
        )
        .unwrap();
    });
}

fn list(app: &TestApp, query: &str, offset: u32, limit: u32) -> Value {
    app.invoke(
        "ai_conversation_list",
        json!({ "query": query, "offset": offset, "limit": limit }),
    )
    .unwrap()
}

fn titles_of(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["title"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn the_history_lists_newest_first_thirty_at_a_time_and_says_when_there_is_more() {
    let app = TestApp::new();
    for n in 0..31 {
        seed_conversation(
            &app,
            &format!("c{n:02}"),
            &format!("Conversation {n}"),
            1_000 + n,
        );
    }

    let first = list(&app, "", 0, 30);
    assert_eq!(first["items"].as_array().unwrap().len(), 30);
    assert_eq!(first["more"], true);
    assert_eq!(first["items"][0]["title"], "Conversation 30");
    assert_eq!(first["items"][29]["title"], "Conversation 1");

    let second = list(&app, "", 30, 30);
    assert_eq!(titles_of(&second), ["Conversation 0"]);
    assert_eq!(second["more"], false);
}

#[test]
fn the_history_search_matches_titles_only_not_the_text_of_the_entries() {
    let app = TestApp::new();
    seed_conversation(&app, "trip", "Trip to Lisbon", 2_000);
    seed_conversation(&app, "shop", "Groceries", 1_000);
    seed_question(&app, "q1", "shop", None, "Lisbon sounds lovely");

    assert_eq!(titles_of(&list(&app, "LISBON", 0, 30)), ["Trip to Lisbon"]);
    assert!(titles_of(&list(&app, "lovely", 0, 30)).is_empty());
    assert_eq!(
        titles_of(&list(&app, "   ", 0, 30)).len(),
        2,
        "a blank search lists everything"
    );
}

#[test]
fn the_history_search_follows_the_shared_rules_for_every_case_in_the_fixture() {
    let cases: Vec<Value> = serde_json::from_str(HISTORY_RULES).unwrap();
    for case in cases {
        let app = TestApp::new();
        let titles = case["titles"].as_array().unwrap();
        for (n, title) in titles.iter().enumerate() {
            seed_conversation(
                &app,
                &format!("c{n}"),
                title.as_str().unwrap(),
                10_000 - i64::try_from(n).unwrap(),
            );
        }
        let found = titles_of(&list(&app, case["query"].as_str().unwrap(), 0, 30));
        let expected: Vec<String> = case["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(found, expected, "for {}", case["query"]);
    }
}

#[test]
fn a_conversation_is_renamed_to_its_trimmed_title_and_the_history_shows_it() {
    let app = TestApp::new();
    seed_conversation(&app, "c", "Old name", 1_000);

    app.invoke::<Value>(
        "ai_conversation_rename",
        json!({ "id": "c", "title": "  Plans   for\n the move  " }),
    )
    .unwrap();

    assert_eq!(get(&app, "c")["title"], "Plans for the move");
    assert_eq!(titles_of(&list(&app, "", 0, 30)), ["Plans for the move"]);
}

#[test]
fn a_title_is_checked_against_the_shared_rename_rules() {
    let cases: Vec<Value> = serde_json::from_str(RENAME_RULES).unwrap();
    for case in cases {
        let app = TestApp::new();
        seed_conversation(&app, "c", "Start", 1_000);
        let outcome = app.invoke::<Value>(
            "ai_conversation_rename",
            json!({ "id": "c", "title": case["title"] }),
        );
        if let Some(message) = case.get("error").and_then(Value::as_str) {
            let error = outcome.unwrap_err();
            assert_eq!(error["code"], "validation", "for {:?}", case["title"]);
            assert_eq!(error["message"], message, "for {:?}", case["title"]);
            assert_eq!(
                get(&app, "c")["title"],
                "Start",
                "a refused rename saves nothing"
            );
        } else {
            outcome.unwrap();
            assert_eq!(
                get(&app, "c")["title"],
                case["saved"],
                "for {:?}",
                case["title"]
            );
        }
    }
}

#[test]
fn a_renamed_conversation_that_is_gone_cannot_be_renamed() {
    let app = TestApp::new();
    let error = app
        .invoke::<Value>(
            "ai_conversation_rename",
            json!({ "id": "nope", "title": "Name" }),
        )
        .unwrap_err();
    assert_eq!(error["code"], "not_found");
}

#[test]
fn a_deleted_conversation_leaves_the_history_and_comes_back_whole_when_restored() {
    let app = TestApp::new();
    install(&app, &scripted(vec![Reply::text(&["Answer"])]));
    let events = app.ai_events();
    let started = ask(&app, None, "What is overdue?").unwrap();
    until_end(&events);
    let id = started["conversation_id"].as_str().unwrap().to_owned();

    app.invoke::<Value>("ai_conversation_delete", json!({ "id": id }))
        .unwrap();
    assert!(titles_of(&list(&app, "", 0, 30)).is_empty());
    let gone = app
        .invoke::<Value>("ai_conversation_get", json!({ "id": id }))
        .unwrap_err();
    assert_eq!(gone["code"], "not_found");

    app.invoke::<Value>("ai_conversation_restore", json!({ "id": id }))
        .unwrap();
    assert_eq!(titles_of(&list(&app, "", 0, 30)), ["What is overdue?"]);
    let back = get(&app, &id);
    assert_eq!(back["title"], "What is overdue?");
    assert_eq!(
        back["entries"].as_array().unwrap().len(),
        3,
        "every entry comes back"
    );
}

#[test]
fn a_conversation_that_is_answering_cannot_be_deleted_until_the_answer_ends() {
    let app = TestApp::new();
    let (release, gate) = mpsc::channel::<()>();
    install(&app, &scripted(vec![Reply::text(&["Done"]).held(gate)]));
    let events = app.ai_events();
    let started = ask(&app, None, "A slow question").unwrap();
    let id = started["conversation_id"].as_str().unwrap().to_owned();

    let busy = app
        .invoke::<Value>("ai_conversation_delete", json!({ "id": id }))
        .unwrap_err();
    assert_eq!(busy["code"], "validation");
    assert_eq!(busy["message"], "This Conversation is still answering.");

    release.send(()).unwrap();
    until_end(&events);
    app.invoke::<Value>("ai_conversation_delete", json!({ "id": id }))
        .unwrap();
}

#[test]
fn a_conversation_copies_as_markdown_with_its_questions_and_answers_in_order() {
    let app = TestApp::new();
    install(
        &app,
        &scripted(vec![
            Reply::text(&["First answer"]),
            Reply::text(&["Second answer"]),
        ]),
    );
    let events = app.ai_events();
    let first = ask(&app, None, "First question").unwrap();
    until_end(&events);
    let id = first["conversation_id"].as_str().unwrap().to_owned();
    ask(&app, Some(&id), "Second question").unwrap();
    until_end(&events);

    let markdown: String = app
        .invoke(
            "ai_conversation_markdown",
            json!({ "id": id, "entryId": null }),
        )
        .unwrap();
    assert_eq!(
        markdown,
        "# First question\n\n**You:** First question\n\nFirst answer\n\n**You:** Second question\n\nSecond answer\n"
    );
}

#[test]
fn an_answer_copies_as_its_markdown_without_citation_markers() {
    let app = TestApp::new();
    seed_conversation(&app, "c", "Tasks", 1_000);
    seed_question(&app, "q", "c", None, "What did I close?");
    seed_answer(
        &app,
        "a",
        "c",
        "q",
        &format!("You closed **one** task [{CITED}] and no others."),
    );

    let copied: String = app
        .invoke(
            "ai_conversation_markdown",
            json!({ "id": "c", "entryId": "a" }),
        )
        .unwrap();
    assert_eq!(copied, "You closed **one** task and no others.");
}

#[test]
fn the_shared_markdown_rules_hold_for_every_case_in_the_fixture() {
    // The pure function has its own unit tests; this runs the shared fixture through the command
    // path too, by seeding each case's entries and comparing what the command copies.
    let cases: Vec<Value> = serde_json::from_str(MARKDOWN_RULES).unwrap();
    for case in cases {
        let app = TestApp::new();
        seed_conversation(&app, "c", case["title"].as_str().unwrap(), 1_000);
        app.with_db(|c| {
            for entry in case["entries"].as_array().unwrap() {
                c.execute(
                    "INSERT INTO ai_entries(id, conversation_id, parent_id, kind, payload, created_at)
                     VALUES(?1, 'c', ?2, ?3, ?4, ?5)",
                    (
                        entry["id"].as_str().unwrap(),
                        entry["parent_id"].as_str(),
                        entry["payload"]["kind"].as_str().unwrap(),
                        entry["payload"].to_string(),
                        entry["created_at"].as_i64().unwrap(),
                    ),
                )
                .unwrap();
            }
        });
        let markdown: String = app
            .invoke(
                "ai_conversation_markdown",
                json!({ "id": "c", "entryId": null }),
            )
            .unwrap();
        assert_eq!(markdown, case["markdown"], "for {}", case["title"]);
    }
}

#[test]
fn copying_a_conversation_that_is_gone_is_not_found() {
    let app = TestApp::new();
    let error = app
        .invoke::<Value>(
            "ai_conversation_markdown",
            json!({ "id": "nope", "entryId": null }),
        )
        .unwrap_err();
    assert_eq!(error["code"], "not_found");
}
