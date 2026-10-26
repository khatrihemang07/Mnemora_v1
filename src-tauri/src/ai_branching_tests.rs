//! Command-layer tests for Branches (#91): Try again and Edit add sibling questions, the pager
//! switches between them, Branch from earlier continues after a chosen answer with the context up
//! to it only, a too-long Conversation has both exits, and the visible Branch is device-local. A
//! scripted Provider plays the Model; the real commands drive every Run.

use std::sync::Arc;

use mnemora_ai::RunOutcome;
use mnemora_harness::scripted::{Reply, ScriptedProvider};
use mnemora_harness::{Failure, Message};
use serde_json::{Value, json};

use crate::ai_tracer_tests::{
    ask, ended_outcome, get, install, local, provider_id, scripted, until_end,
};
use crate::test_support::TestApp;

/// A scripted Provider that makes native Tool calls, for the budget case.
fn native(replies: Vec<Reply>) -> Arc<ScriptedProvider> {
    Arc::new(ScriptedProvider::new("Scripted", "scripted-model", replies).with_native_tools())
}

/// Starts a Run in `conversation` that edits or retries the question `branch_of`, or that
/// continues the visible Branch when `branch_of` is none.
fn run_in(
    app: &TestApp,
    conversation: &str,
    text: &str,
    branch_of: Option<&str>,
) -> Result<Value, Value> {
    app.invoke(
        "ai_run_start",
        json!({ "request": {
            "conversation_id": conversation,
            "text": text,
            "provider_id": provider_id(app),
            "model": "scripted-model",
            "local": local(),
            "branch_of": branch_of,
        } }),
    )
}

/// Try again on a question: a Run that writes a sibling of it and answers it.
fn retry(app: &TestApp, conversation: &str, entry: &str) -> Result<Value, Value> {
    app.invoke(
        "ai_run_retry",
        json!({ "request": {
            "conversation_id": conversation,
            "entry_id": entry,
            "provider_id": provider_id(app),
            "model": "scripted-model",
            "local": local(),
        } }),
    )
}

fn set_leaf(app: &TestApp, conversation: &str, entry: &str) -> Result<Value, Value> {
    app.invoke(
        "ai_conversation_set_leaf",
        json!({ "id": conversation, "entryId": entry }),
    )
}

/// The pager: shows the question at `position` among those beside `question`, and returns the
/// Conversation as it now reads.
fn show_branch(app: &TestApp, conversation: &str, question: &str, position: u32) -> Value {
    app.invoke::<Value>(
        "ai_conversation_show_branch",
        json!({ "id": conversation, "entryId": question, "position": position }),
    )
    .unwrap();
    get(app, conversation)
}

/// The entries of the view that a person reads: questions and final answers, in order.
fn thread(view: &Value) -> Vec<String> {
    view["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| {
            let payload = &entry["payload"];
            payload["kind"] == "user"
                || (payload["kind"] == "assistant"
                    && payload["calls"].as_array().is_some_and(Vec::is_empty))
        })
        .map(|entry| entry["payload"]["text"].as_str().unwrap_or("").to_owned())
        .collect()
}

/// The id of the entry whose text is `text` (a question or an answer) in the view.
fn id_of(view: &Value, text: &str) -> String {
    view["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["payload"]["text"] == text)
        .unwrap_or_else(|| panic!("no entry reads {text:?}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The ids of the questions in the view, in order.
fn question_ids(view: &Value) -> Vec<String> {
    view["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["payload"]["kind"] == "user")
        .map(|entry| entry["id"].as_str().unwrap().to_owned())
        .collect()
}

/// The pager of the question at `index` (among the questions in the view): (index, count), 1-based.
fn pager(view: &Value, index: usize) -> (u64, u64) {
    let question = view["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["payload"]["kind"] == "user")
        .nth(index)
        .expect("the question is in the view");
    (
        question["sibling_index"].as_u64().unwrap(),
        question["sibling_count"].as_u64().unwrap(),
    )
}

#[test]
fn try_again_adds_a_sibling_question_and_runs_it_and_the_pager_keeps_both() {
    let app = TestApp::new();
    let provider = scripted(vec![
        Reply::text(&["First answer"]),
        Reply::text(&["Second answer"]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, None, "Tell me about the move").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_end(&events);
    let original = question_ids(&get(&app, &conversation))[0].clone();

    retry(&app, &conversation, &original).unwrap();
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
    assert_eq!(
        provider.requests()[1].messages,
        vec![Message::User("Tell me about the move".into())],
        "the retried question is sent on its own: it follows what the original followed"
    );

    let visible = get(&app, &conversation);
    assert_ne!(
        question_ids(&visible)[0],
        original,
        "the retry is a new question"
    );
    assert_eq!(
        thread(&visible),
        vec!["Tell me about the move", "Second answer"]
    );
    assert_eq!(pager(&visible, 0), (2, 2));

    let kept = show_branch(&app, &conversation, &question_ids(&visible)[0], 1);
    assert_eq!(
        thread(&kept),
        vec!["Tell me about the move", "First answer"]
    );
    assert_eq!(pager(&kept, 0), (1, 2));
}

#[test]
fn edit_adds_an_edited_sibling_and_the_model_is_sent_only_what_it_follows() {
    let app = TestApp::new();
    let provider = scripted(vec![
        Reply::text(&["Answer one"]),
        Reply::text(&["Answer two"]),
        Reply::text(&["Edited answer"]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, None, "Start").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_end(&events);
    ask(&app, Some(&conversation), "Follow-up vague").unwrap();
    until_end(&events);
    let vague = question_ids(&get(&app, &conversation))[1].clone();

    run_in(&app, &conversation, "Follow-up, now precise", Some(&vague)).unwrap();
    until_end(&events);

    assert_eq!(
        provider.requests()[2].messages,
        vec![
            Message::User("Start".into()),
            Message::Assistant {
                text: "Answer one".into(),
                calls: Vec::new(),
            },
            Message::User("Follow-up, now precise".into()),
        ]
    );
    let visible = get(&app, &conversation);
    assert_eq!(
        thread(&visible),
        vec![
            "Start",
            "Answer one",
            "Follow-up, now precise",
            "Edited answer"
        ]
    );
    assert_eq!(
        pager(&visible, 1),
        (2, 2),
        "the edit sits beside the question it changed"
    );
    assert_eq!(pager(&visible, 0), (1, 1));
}

#[test]
fn branch_from_earlier_continues_after_the_chosen_answer_with_only_the_context_up_to_it() {
    let app = TestApp::new();
    let provider = scripted(vec![
        Reply::text(&["A1"]),
        Reply::text(&["A2"]),
        Reply::text(&["A3"]),
        Reply::text(&["A4"]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, None, "Q1").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_end(&events);
    ask(&app, Some(&conversation), "Q2").unwrap();
    until_end(&events);
    ask(&app, Some(&conversation), "Q3").unwrap();
    until_end(&events);
    let before = get(&app, &conversation);
    let first_answer = id_of(&before, "A1");

    set_leaf(&app, &conversation, &first_answer).unwrap();
    ask(&app, Some(&conversation), "Q4 from A1").unwrap();
    until_end(&events);

    assert_eq!(
        provider.requests()[3].messages,
        vec![
            Message::User("Q1".into()),
            Message::Assistant {
                text: "A1".into(),
                calls: Vec::new(),
            },
            Message::User("Q4 from A1".into()),
        ],
        "the Model sees the branch up to the chosen answer, and nothing after it"
    );
    let visible = get(&app, &conversation);
    assert_eq!(thread(&visible), vec!["Q1", "A1", "Q4 from A1", "A4"]);

    let beside = question_ids(&visible)[1].clone();
    assert!(
        app.invoke::<Value>(
            "ai_conversation_show_branch",
            json!({ "id": conversation, "entryId": beside, "position": 3 }),
        )
        .is_err(),
        "there are two questions here, so position 3 is refused"
    );
    let original = show_branch(&app, &conversation, &beside, 1);
    assert_eq!(
        thread(&original),
        vec!["Q1", "A1", "Q2", "A2", "Q3", "A3"],
        "the original Branch stays reachable"
    );
    assert_eq!(pager(&original, 1), (1, 2));
    assert_eq!(
        pager(&show_branch(&app, &conversation, &beside, 2), 1),
        (2, 2)
    );
}

#[test]
fn branch_from_earlier_refuses_an_entry_of_another_conversation_or_an_unknown_one() {
    let app = TestApp::new();
    let provider = scripted(vec![Reply::text(&["One"]), Reply::text(&["Two"])]);
    install(&app, &provider);
    let events = app.ai_events();
    let first = ask(&app, None, "Alpha").unwrap()["conversation_id"]
        .as_str()
        .unwrap()
        .to_owned();
    until_end(&events);
    let second = ask(&app, None, "Beta").unwrap()["conversation_id"]
        .as_str()
        .unwrap()
        .to_owned();
    until_end(&events);
    let beta_answer = id_of(&get(&app, &second), "Two");

    assert!(set_leaf(&app, &first, &beta_answer).is_err());
    assert!(set_leaf(&app, &first, "no-such-entry").is_err());
    assert!(
        retry(&app, &first, &beta_answer).is_err(),
        "only a question can be tried again"
    );
}

#[test]
fn a_too_long_conversation_can_branch_from_an_earlier_answer_or_start_a_new_one() {
    let app = TestApp::new();
    // Eight 8 KB reads fill the budget in two Steps, so the third request is never sent (ADR 0018).
    let body = "walk ".repeat(3_400);
    let ids: Vec<String> = (1..=8).map(|day| seed_entry(&app, day, &body)).collect();
    let first: Vec<(&str, String)> = ids
        .iter()
        .map(|id| ("journal_read", format!(r#"{{"id":"{id}"}}"#)))
        .collect();
    let second: Vec<(&str, String)> = ids
        .iter()
        .map(|id| ("journal_read", format!(r#"{{"id":"{id}","offset":8192}}"#)))
        .collect();
    let provider = native(vec![
        Reply::text(&["Short answer"]),
        Reply::calls(&borrow(&first)),
        Reply::calls(&borrow(&second)),
        Reply::text(&["After the branch"]),
        Reply::text(&["In a new Conversation"]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, None, "Short question").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_end(&events);
    let short_answer = id_of(&get(&app, &conversation), "Short answer");

    ask(&app, Some(&conversation), "Everything walk").unwrap();
    assert_eq!(
        ended_outcome(&until_end(&events)),
        RunOutcome::Failed {
            kind: Failure::TooLong,
            message: mnemora_ai::failure_message(Failure::TooLong).to_owned(),
        }
    );

    assert_eq!(
        provider.requests().len(),
        3,
        "the request that would exceed the budget is never sent"
    );

    set_leaf(&app, &conversation, &short_answer).unwrap();
    ask(&app, Some(&conversation), "Continue here").unwrap();
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
    assert_eq!(
        provider.requests()[3].messages,
        vec![
            Message::User("Short question".into()),
            Message::Assistant {
                text: "Short answer".into(),
                calls: Vec::new(),
            },
            Message::User("Continue here".into()),
        ]
    );

    ask(&app, None, "Fresh start").unwrap();
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
    assert_eq!(
        provider.requests()[4].messages,
        vec![Message::User("Fresh start".into())]
    );
}

#[test]
fn copy_as_markdown_copies_the_branch_that_is_shown() {
    let app = TestApp::new();
    let provider = scripted(vec![
        Reply::text(&["Alpha answer"]),
        Reply::text(&["Beta answer"]),
        Reply::text(&["Gamma answer"]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();
    let started = ask(&app, None, "Alpha question").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_end(&events);
    let alpha_answer = id_of(&get(&app, &conversation), "Alpha answer");
    ask(&app, Some(&conversation), "Beta question").unwrap();
    until_end(&events);
    set_leaf(&app, &conversation, &alpha_answer).unwrap();
    ask(&app, Some(&conversation), "Gamma question").unwrap();
    until_end(&events);

    let copied: String = app
        .invoke(
            "ai_conversation_markdown",
            json!({ "id": conversation, "entryId": null }),
        )
        .unwrap();
    assert!(copied.contains("Gamma question") && copied.contains("Alpha answer"));
    assert!(
        !copied.contains("Beta"),
        "the branch not shown is not copied"
    );
}

#[test]
fn the_shown_branch_is_device_local_and_writes_nothing_to_the_change_log() {
    let app = TestApp::new();
    let provider = scripted(vec![Reply::text(&["One"]), Reply::text(&["Two"])]);
    install(&app, &provider);
    let events = app.ai_events();
    let started = ask(&app, None, "Q one").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_end(&events);
    let answer = id_of(&get(&app, &conversation), "One");
    ask(&app, Some(&conversation), "Q two").unwrap();
    until_end(&events);

    let changes_before = change_log_rows(&app);
    set_leaf(&app, &conversation, &answer).unwrap();
    assert_eq!(
        change_log_rows(&app),
        changes_before,
        "showing a Branch is not synced"
    );
    assert_eq!(thread(&get(&app, &conversation)), vec!["Q one", "One"]);
}

fn seed_entry(app: &TestApp, day: i64, text: &str) -> String {
    let entry: Value = app
        .invoke(
            "journal_entry_create",
            json!({ "entry": {
                "page_date": format!("2026-10-{day:02}"), "text": text, "written_tz": "UTC",
                "written_offset_min": 0, "session_id": "seed",
            }}),
        )
        .unwrap();
    entry["id"].as_str().unwrap().to_owned()
}

fn change_log_rows(app: &TestApp) -> i64 {
    app.with_db(|c| {
        c.query_row("SELECT COUNT(*) FROM change_log", [], |r| r.get(0))
            .unwrap()
    })
}

fn borrow<'a>(calls: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    calls
        .iter()
        .map(|(name, args)| (*name, args.as_str()))
        .collect()
}
