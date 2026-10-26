//! Command-layer tests for controlling a Run (#89): stopping it, sending queued notes after the
//! Step's Tools, sending a note now, the queue, interruption on Android, and two Conversations
//! answering together. A scripted Provider plays the Model; the real commands drive the Run.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use mnemora_ai::{RunEvent, RunOutcome, failure_message};
use mnemora_harness::scripted::Reply;
use mnemora_harness::{Failure, Message};
use serde_json::{Value, json};

use crate::AiRunEvent;
use crate::ai_tracer_tests::{ask, ended_outcome, get, install, label, scripted, until_end};
use crate::ai_wire_tests::{add_wrapper, finish, reply};
use crate::test_support::TestApp;
use mnemora_providers::testing::{Canned, FakeServer};

const JOURNAL: &str = r#"{"from":"2026-10-18","to":"2026-10-25"}"#;

/// Every event up to and including the first one labelled `wanted`.
fn until_label(events: &Receiver<AiRunEvent>, wanted: &str) -> Vec<AiRunEvent> {
    let mut seen = Vec::new();
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(5))
            .expect("the event arrives");
        let hit = label(&event) == wanted;
        seen.push(event);
        if hit {
            return seen;
        }
    }
}

/// The labels of the events, in order: `entrySaved:user` and the rest.
fn labels(run: &[AiRunEvent]) -> Vec<String> {
    run.iter().map(label).collect()
}

/// The text of a question or answer entry, as the thread shows it.
fn entry_texts(view: &Value, kind: &str) -> Vec<String> {
    view["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["payload"]["kind"] == kind)
        .map(|entry| entry["payload"]["text"].as_str().unwrap_or("").to_owned())
        .collect()
}

fn stop(app: &TestApp, conversation: &str) {
    app.invoke::<Value>("ai_run_abort", json!({ "conversationId": conversation }))
        .unwrap();
}

fn steer(app: &TestApp, conversation: &str, text: &str) -> Value {
    app.invoke(
        "ai_run_steer",
        json!({ "conversationId": conversation, "text": text }),
    )
    .unwrap()
}

#[test]
fn stopping_a_run_keeps_what_was_written_and_the_conversation_can_go_on() {
    let app = TestApp::new();
    let (release, gate) = mpsc::channel::<()>();
    let provider = scripted(vec![
        Reply::text(&["Partly", " the rest"]).held_after_first(gate),
        Reply::text(&["Continued"]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, None, "Tell me").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_label(&events, "textDelta");
    stop(&app, &conversation);
    release.send(()).unwrap();

    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Stopped);
    let stopped = get(&app, &conversation);
    assert_eq!(stopped["running"], false);
    let answer = stopped["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["payload"]["kind"] == "assistant")
        .expect("the part written is saved");
    assert_eq!(answer["payload"]["text"], "Partly");
    assert_eq!(answer["payload"]["stop"], "aborted");

    ask(&app, Some(&conversation), "And now?").unwrap();
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
    assert_eq!(
        provider.requests()[1].messages,
        vec![
            Message::User("Tell me".into()),
            Message::Assistant {
                text: "Partly".into(),
                calls: Vec::new(),
            },
            Message::User("And now?".into()),
        ],
        "the Model is sent the partial answer as its own, then the next question"
    );
}

#[test]
fn a_run_stopped_before_it_wrote_anything_is_marked_stopped_and_leaves_no_empty_answer_behind() {
    let app = TestApp::new();
    let (release, gate) = mpsc::channel::<()>();
    let provider = scripted(vec![
        Reply::text(&["Never shown"]).held(gate),
        Reply::text(&["Second"]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, None, "First question").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_label(&events, "stepStarted");
    stop(&app, &conversation);
    release.send(()).unwrap();
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Stopped);

    let view = get(&app, &conversation);
    assert_eq!(entry_texts(&view, "assistant"), vec![""]);
    ask(&app, Some(&conversation), "Second question").unwrap();
    until_end(&events);
    assert_eq!(
        provider.requests()[1].messages,
        vec![
            Message::User("First question".into()),
            Message::User("Second question".into()),
        ]
    );
}

#[test]
fn stopping_in_the_middle_of_the_calls_keeps_the_ones_not_run_as_stopped_results() {
    let app = TestApp::new();
    let (release, gate) = mpsc::channel::<()>();
    let provider = scripted(vec![
        Reply::calls(&[("journal_list", JOURNAL), ("journal_list", JOURNAL)])
            .held_after_first(gate),
        Reply::text(&["Unused"]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, None, "What did I write?").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_label(&events, "stepStarted");
    stop(&app, &conversation);
    release.send(()).unwrap();
    let run = until_end(&events);
    assert_eq!(ended_outcome(&run), RunOutcome::Stopped);
    // The call that was streamed is answered, so the transcript stays valid, but nothing runs.
    for event in &run {
        if let RunEvent::ToolEnded { is_error, .. } = event.event {
            assert!(is_error, "no call ran");
        }
    }
    let view = get(&app, &conversation);
    let results: Vec<&Value> = view["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["payload"]["kind"] == "tool_result")
        .map(|entry| &entry["payload"])
        .collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["is_error"], true);
    assert_eq!(results[0]["observation_id"], "");
    assert!(
        results[0]["result"]
            .as_str()
            .unwrap()
            .contains("stopped before this call ran")
    );
}

#[test]
fn a_note_sent_during_a_step_goes_to_the_model_after_its_tools_finish() {
    let app = TestApp::new();
    let (release, gate) = mpsc::channel::<()>();
    let provider = scripted(vec![
        Reply::calls(&[("journal_list", JOURNAL)]).held(gate),
        Reply::text(&["Answer with the note"]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, None, "What is on my plate?").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_label(&events, "stepStarted");
    let kept = steer(&app, &conversation, "  Only the work tasks  ");
    assert_eq!(kept["text"], "Only the work tasks");
    assert_eq!(
        get(&app, &conversation)["queued"][0]["text"],
        "Only the work tasks",
        "a page opened now shows the waiting note"
    );
    let dropped = steer(&app, &conversation, "Drop me");
    app.invoke::<Value>(
        "ai_run_unqueue",
        json!({ "conversationId": conversation, "noteId": dropped["id"] }),
    )
    .unwrap();
    release.send(()).unwrap();

    let run = until_end(&events);
    assert_eq!(ended_outcome(&run), RunOutcome::Answered);
    let names = labels(&run);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let tool_done = names.iter().position(|l| *l == "toolEnded").unwrap();
    let note_saved = names
        .iter()
        .rposition(|l| *l == "entrySaved:user")
        .expect("the note is saved as a question");
    let second_step = names.iter().rposition(|l| *l == "stepStarted").unwrap();
    assert!(
        tool_done < note_saved && note_saved < second_step,
        "{names:?}"
    );

    let sent = &provider.requests()[1].messages;
    assert_eq!(
        sent.last(),
        Some(&Message::User("Only the work tasks".into()))
    );
    assert!(
        !provider
            .requests()
            .iter()
            .any(|request| format!("{:?}", request.messages).contains("Drop me")),
        "a removed note is never sent"
    );
    let view = get(&app, &conversation);
    assert_eq!(
        entry_texts(&view, "user"),
        vec!["What is on my plate?", "Only the work tasks"]
    );
}

#[test]
fn send_now_ends_the_step_being_written_and_sends_the_note_next() {
    let app = TestApp::new();
    let (release, gate) = mpsc::channel::<()>();
    let provider = scripted(vec![
        Reply::text(&["First part", " and the rest"]).held_after_first(gate),
        Reply::text(&["Answer to the note"]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, None, "Plan my week").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_label(&events, "textDelta");
    let note = steer(&app, &conversation, "Only Monday");
    app.invoke::<Value>(
        "ai_run_send_now",
        json!({ "conversationId": conversation, "noteId": note["id"] }),
    )
    .unwrap();
    release.send(()).unwrap();

    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
    assert_eq!(
        provider.requests()[1].messages,
        vec![
            Message::User("Plan my week".into()),
            Message::Assistant {
                text: "First part".into(),
                calls: Vec::new(),
            },
            Message::User("Only Monday".into()),
        ]
    );
    let view = get(&app, &conversation);
    assert_eq!(
        entry_texts(&view, "assistant"),
        vec!["First part", "Answer to the note"]
    );
    // Send now is not a stop: the step it ended is not shown as "Stopped".
    let first = view["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["payload"]["kind"] == "assistant")
        .expect("the step ended by Send now is saved");
    assert_eq!(first["payload"]["stop"], "stop");
}

#[test]
fn a_note_still_queued_when_the_run_stops_is_handed_back_and_not_sent() {
    let app = TestApp::new();
    let (release, gate) = mpsc::channel::<()>();
    let provider = scripted(vec![Reply::text(&["Never"]).held(gate)]);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, None, "A question").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_label(&events, "stepStarted");
    steer(&app, &conversation, "Left over");
    stop(&app, &conversation);
    release.send(()).unwrap();

    let run = until_end(&events);
    assert_eq!(ended_outcome(&run), RunOutcome::Stopped);
    let handed_back = run
        .iter()
        .find_map(|event| match &event.event {
            RunEvent::QueueChanged { notes } => Some(notes.clone()),
            _ => None,
        })
        .expect("the queue is announced once the Run has ended");
    assert_eq!(handed_back.len(), 1);
    assert_eq!(handed_back[0].text, "Left over");
    assert!(
        steer_fails(&app, &conversation),
        "a Run that has ended takes no more notes"
    );
}

fn steer_fails(app: &TestApp, conversation: &str) -> bool {
    app.invoke::<Value>(
        "ai_run_steer",
        json!({ "conversationId": conversation, "text": "Too late" }),
    )
    .is_err()
}

#[test]
fn leaving_the_app_in_the_background_interrupts_the_run_and_says_so() {
    let app = TestApp::new();
    let (release, gate) = mpsc::channel::<()>();
    let provider = scripted(vec![Reply::text(&["Never"]).held(gate)]);
    install(&app, &provider);
    let events = app.ai_events();

    ask(&app, None, "Keep going").unwrap();
    until_label(&events, "stepStarted");
    crate::sync_power::foreground(&app.handle(), false);
    release.send(()).unwrap();

    assert_eq!(
        ended_outcome(&until_end(&events)),
        RunOutcome::Failed {
            kind: Failure::Interrupted,
            message: failure_message(Failure::Interrupted).to_owned(),
        }
    );
}

#[test]
fn a_second_conversation_answers_while_the_first_is_still_going() {
    let app = TestApp::new();
    let (release, gate) = mpsc::channel::<()>();
    let provider = scripted(vec![
        Reply::text(&["Slow one"]).held(gate),
        Reply::text(&["Quick one"]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();

    let first = ask(&app, None, "The slow question").unwrap();
    let first_id = first["conversation_id"].as_str().unwrap().to_owned();
    until_label(&events, "stepStarted");
    // The first Run's Step has asked the scripted Provider for its reply by now.
    while provider.requests().is_empty() {
        std::thread::sleep(Duration::from_millis(5));
    }

    let second = ask(&app, None, "The quick question").unwrap();
    let second_id = second["conversation_id"].as_str().unwrap().to_owned();
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
    assert_eq!(get(&app, &first_id)["running"], true);
    assert_eq!(get(&app, &second_id)["running"], false);

    release.send(()).unwrap();
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
    assert_eq!(get(&app, &first_id)["running"], false);
}

#[test]
fn send_now_on_the_answer_the_budget_forces_ends_the_run_and_hands_the_note_back() {
    let app = TestApp::new();
    let (release, gate) = mpsc::channel::<()>();
    // Eleven Steps of distinct calls, then the answer-only request, which is held.
    let arguments: Vec<String> = (1..=11).map(|n| format!(r#"{{"limit":{n}}}"#)).collect();
    let mut replies: Vec<Reply> = arguments
        .iter()
        .map(|a| Reply::calls(&[("journal_list", a.as_str())]))
        .collect();
    replies.push(Reply::text(&["First", " rest"]).held_after_first(gate));
    let provider = scripted(replies);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, None, "Keep looking").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    while provider.requests().len() < 12 {
        std::thread::sleep(Duration::from_millis(5));
    }
    until_label(&events, "textDelta");
    let note = steer(&app, &conversation, "Too late");
    app.invoke::<Value>(
        "ai_run_send_now",
        json!({ "conversationId": conversation, "noteId": note["id"] }),
    )
    .unwrap();
    release.send(()).unwrap();

    let run = until_end(&events);
    assert_eq!(ended_outcome(&run), RunOutcome::Stopped);
    assert_eq!(
        provider.requests().len(),
        12,
        "no thirteenth request is made"
    );
    let handed_back = run
        .iter()
        .find_map(|event| match &event.event {
            RunEvent::QueueChanged { notes } => Some(notes.clone()),
            _ => None,
        })
        .expect("the note that could not be sent is handed back");
    assert_eq!(handed_back[0].text, "Too late");
}

#[test]
fn stopping_a_run_waiting_on_a_blocked_reply_ends_it_at_once_and_the_late_reply_is_never_written() {
    // The Wrapper answers whole, and only after the reply is due: the Run must not wait for it.
    let late = Duration::from_millis(2500);
    let server = FakeServer::start(move |_| {
        Canned::json(200, &reply(finish("Too late to show", &[]))).delayed(late)
    });
    let app = TestApp::new();
    add_wrapper(&app, &server.address());
    let events = app.ai_events();

    let started = ask(&app, None, "Tell me").unwrap();
    let conversation = started["conversation_id"].as_str().unwrap().to_owned();
    until_label(&events, "stepStarted");
    std::thread::sleep(Duration::from_millis(100));
    let pressed = Instant::now();
    stop(&app, &conversation);
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Stopped);
    assert!(
        pressed.elapsed() < Duration::from_secs(1),
        "the Run ends at once, not when the blocked reply arrives"
    );

    // Let the late reply arrive: it must not reach the Conversation.
    std::thread::sleep(late + Duration::from_millis(500));
    let view = get(&app, &conversation);
    assert_eq!(view["running"], false);
    assert!(!view.to_string().contains("Too late to show"));
    assert_eq!(entry_texts(&view, "assistant"), vec![""]);
}
