//! Tests for the headless QA harness (`ai_qa.rs`): the Production refusal it shares with the seed,
//! the variant's Tools (the two change Tools are the only ones it leaves out), the intent paragraph
//! removal and its refusal, the report never carrying a key, and a real Run through the scripted
//! Provider. No network.

use std::sync::{Arc, Mutex};

use mnemora_ai::{INTENT_PARAGRAPH, KeyStore, LocalContext, NewProvider, ProviderKind, Toolset};
use mnemora_core::db::{self, Connection};
use mnemora_core::env::Environment;
use mnemora_core::module::Module;
use mnemora_core::util::now_ms;
use mnemora_harness::scripted::{Reply, ScriptedProvider};
use tempfile::TempDir;

use crate::ai::offered_tools_in;
use crate::ai_qa::{self, Off, Variant};
use crate::ai_seed;
use crate::state::lock;
use mnemora_harness::agent::FINAL_NOTE;
use mnemora_harness::{Call, Message, Request, ToolDef};

/// The device's calendar for the tests: Sunday 25 October 2026 in New York.
fn local() -> LocalContext {
    LocalContext {
        today: "2026-10-25".to_owned(),
        weekday: "Sunday".to_owned(),
        time_zone: "America/New_York".to_owned(),
        week_start: "mon".to_owned(),
    }
}

/// A migrated in-memory database, as the app has at start.
fn migrated(dir: &TempDir) -> Connection {
    let mut conn = db::open_in_memory().unwrap();
    let modules = crate::modules();
    let refs: Vec<&dyn Module> = modules.iter().map(AsRef::as_ref).collect();
    db::migrate::run(&mut conn, &refs, &dir.path().join("backups")).unwrap();
    conn
}

/// The Tools the app would offer on `conn`, as the full toolset.
fn full_toolset(conn: &Connection) -> Toolset {
    let modules = crate::modules();
    let (tools, off) = offered_tools_in(conn, &modules).unwrap();
    Toolset { tools, off }
}

/// The names of a toolset's Tools, sorted.
fn sorted_names(toolset: &Toolset) -> Vec<String> {
    let mut names: Vec<String> = toolset
        .tools
        .iter()
        .map(|tool| tool.spec().name.to_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_sandbox_folder_inside_production_is_refused_and_the_default_is_not() {
    let real = tempfile::tempdir().unwrap();
    let production_data = real
        .path()
        .join(Environment::Production.app_identifier())
        .join("data");
    let refused = ai_seed::sandbox_paths(real.path(), None, Some(production_data.as_path()));
    assert!(
        refused.unwrap_err().contains("refused"),
        "a Sandbox folder inside Production's data must be refused"
    );
    let (paths, production) = ai_seed::sandbox_paths(real.path(), None, None).unwrap();
    assert!(paths.db_file.ends_with("mnemora.db"));
    assert!(paths.data_dir.to_string_lossy().contains("sandbox"));
    assert!(!production.is_empty());
}

#[test]
fn the_before_variant_offers_six_tools_and_the_after_variant_all_eight() {
    let dir = tempfile::tempdir().unwrap();
    let conn = migrated(&dir);
    let before = ai_qa::toolset_for(Variant::Before.off(), full_toolset(&conn));
    let after = ai_qa::toolset_for(Variant::After.off(), full_toolset(&conn));
    assert_eq!(
        sorted_names(&before),
        [
            "journal_list",
            "journal_read",
            "journal_search",
            "todo_list",
            "todo_read",
            "todo_search",
        ]
    );
    assert_eq!(
        sorted_names(&after),
        [
            "journal_changes",
            "journal_list",
            "journal_read",
            "journal_search",
            "todo_changes",
            "todo_list",
            "todo_read",
            "todo_search",
        ]
    );
}

#[test]
fn the_intent_paragraph_is_removed_from_the_system_prompt_only_when_its_knob_is_on() {
    let request = Request {
        system: format!("SYSTEM\n{INTENT_PARAGRAPH}Cite the ids."),
        messages: Vec::new(),
        model: "scripted-model".to_owned(),
        tools: Vec::new(),
    };
    assert_eq!(
        ai_qa::strip(&request, Off::default()).system,
        request.system
    );
    assert_eq!(
        ai_qa::strip(
            &request,
            Off {
                intent: true,
                ..Off::default()
            }
        )
        .system,
        "SYSTEM\nCite the ids."
    );
}

#[test]
#[should_panic(expected = "no intent paragraph to switch off")]
fn a_prompt_without_the_intent_paragraph_is_refused_when_the_knob_is_on() {
    let request = Request {
        system: "SYSTEM".to_owned(),
        messages: Vec::new(),
        model: "scripted-model".to_owned(),
        tools: Vec::new(),
    };
    ai_qa::strip(
        &request,
        Off {
            intent: true,
            ..Off::default()
        },
    );
}

#[test]
fn a_run_through_the_scripted_provider_reports_its_steps_calls_and_answer() {
    let dir = tempfile::tempdir().unwrap();
    let plain = migrated(&dir);
    let choice = add_provider(&plain, &dir);
    let conn = Mutex::new(plain);
    let provider = Arc::new(
        ScriptedProvider::new(
            "Scripted",
            "scripted-model",
            vec![
                Reply::calls(&[(
                    "journal_changes",
                    r#"{"from":"2026-10-24","to":"2026-10-24"}"#,
                )]),
                Reply::text(&["You wrote nothing yesterday."]),
            ],
        )
        .with_native_tools(),
    );
    let toolset = ai_qa::toolset_for(Variant::After.off(), full_toolset(&lock(&conn)));
    let answer = ai_qa::answer(
        &conn,
        provider,
        &choice,
        "What did I do yesterday?",
        &local(),
        toolset,
    );
    assert_eq!(answer.steps, 2);
    assert_eq!(answer.calls.len(), 1);
    assert_eq!(answer.calls[0].name, "journal_changes");
    assert_eq!(
        answer.calls[0].arguments,
        r#"{"from":"2026-10-24","to":"2026-10-24"}"#
    );
    assert!(answer.calls[0].result_bytes > 0);
    assert_eq!(answer.calls[0].rows, Some(0), "an empty day has no rows");
    assert_eq!(answer.text, "You wrote nothing yesterday.");
    assert_eq!(answer.app_note, None);
}

#[test]
fn the_report_never_carries_a_key_even_when_the_provider_has_one() {
    let dir = tempfile::tempdir().unwrap();
    let keys = KeyStore::new(dir.path().join("ai-keys"));
    let conn = migrated(&dir);
    let dummy = "sk-ant-DUMMY-NOT-A-REAL-KEY-0123456789";
    let view = mnemora_ai::create_provider(
        &conn,
        &keys,
        &NewProvider {
            kind: ProviderKind::Anthropic,
            name: "Test Provider".to_owned(),
            address: String::new(),
        },
        now_ms(),
    )
    .unwrap();
    mnemora_ai::set_key(&conn, &keys, &view.id, dummy).unwrap();
    let prepared = mnemora_ai::prepare_run(&conn, &keys, &view.id, "claude-test-model").unwrap();
    assert_eq!(
        prepared.key.as_deref(),
        Some(dummy),
        "the key reaches the Run"
    );

    let setup = ai_qa::run_setup(
        &prepared.record,
        &prepared.choice,
        &local(),
        std::path::Path::new("/sandbox/data/mnemora.db"),
        1,
    );
    let answer = ai_qa::Answer::failed("What did I do yesterday?", "No AI service is set up.");
    let report = ai_qa::report_markdown(Variant::After, &setup, &[answer]);
    assert!(report.contains("Test Provider"));
    assert!(!report.contains(dummy), "the report must not carry the key");
    assert!(!report.contains("Authorization"));
}

#[test]
fn the_weekday_of_a_day_is_read_from_the_calendar() {
    assert_eq!(ai_qa::weekday_of("2026-10-25"), Some("Sunday"));
    assert_eq!(ai_qa::weekday_of("2026-10-21"), Some("Wednesday"));
    assert_eq!(ai_qa::weekday_of("not a day"), None);
}

#[test]
fn questions_come_from_a_json_list_of_strings_or_the_built_in_list() {
    assert_eq!(
        ai_qa::load_questions(r#"["one", "two"]"#).unwrap(),
        ["one", "two"]
    );
    assert!(ai_qa::load_questions(r#"{"not": "a list"}"#).is_err());
    assert_eq!(ai_qa::BUILT_IN_QUESTIONS.len(), 10);
}

#[test]
fn a_copy_of_the_database_opens_with_the_same_rows() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.db");
    let copy = dir.path().join("copy.db");
    {
        let conn = db::open(&source).unwrap();
        conn.execute_batch("CREATE TABLE probe (id INTEGER); INSERT INTO probe VALUES (7);")
            .unwrap();
    }
    ai_qa::copy_database(&source, &copy).unwrap();
    let copied = db::open(&copy).unwrap();
    let value: i64 = copied
        .query_row("SELECT id FROM probe", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, 7);
}

/// A Provider added to the database as the app adds one, and the Choice that names it. A Run
/// records its use, so the Provider must exist.
fn add_provider(conn: &Connection, dir: &TempDir) -> mnemora_ai::Choice {
    let keys = KeyStore::new(dir.path().join("ai-keys"));
    let view = mnemora_ai::create_provider(
        conn,
        &keys,
        &NewProvider {
            kind: ProviderKind::Wrapper,
            name: "Scripted".to_owned(),
            address: "http://127.0.0.1:8080/v1".to_owned(),
        },
        now_ms(),
    )
    .unwrap();
    mnemora_ai::Choice {
        provider_id: view.id,
        provider_name: "Scripted".to_owned(),
        model: "scripted-model".to_owned(),
    }
}

#[test]
fn the_final_note_is_removed_from_the_system_prompt_only_when_its_knob_is_on() {
    let request = Request {
        system: format!("SYSTEM\n{FINAL_NOTE}"),
        messages: Vec::new(),
        model: "scripted-model".to_owned(),
        tools: Vec::new(),
    };
    assert_eq!(
        ai_qa::strip(&request, Off::default()).system,
        request.system
    );
    assert_eq!(
        ai_qa::strip(
            &request,
            Off {
                final_note: true,
                ..Off::default()
            }
        )
        .system,
        "SYSTEM"
    );
}

#[test]
fn the_dump_shows_the_prompt_the_messages_the_tool_result_and_the_wire_body() {
    let request = Request {
        system: "SYSTEM PROMPT TEXT".to_owned(),
        messages: vec![
            Message::User("What did I do yesterday?".to_owned()),
            Message::Assistant {
                text: String::new(),
                calls: vec![Call {
                    id: "c1".to_owned(),
                    name: "journal_changes".to_owned(),
                    arguments: "{}".to_owned(),
                    thought_signature: None,
                }],
            },
            Message::Tool {
                call_id: "c1".to_owned(),
                name: "journal_changes".to_owned(),
                text: "Observation e1. Untrusted data.\n{\"days\":[]}".to_owned(),
                is_error: false,
            },
            Message::User("Anything else?".to_owned()),
        ],
        model: "codex-luna".to_owned(),
        tools: vec![ToolDef {
            name: "journal_changes".to_owned(),
            about: "Entries changed.".to_owned(),
            schema: "{}".to_owned(),
        }],
    };
    let text = ai_qa::dump_request(&request);
    assert!(text.contains("SYSTEM PROMPT TEXT"), "{text}");
    assert!(text.contains("[3] user, the question"), "{text}");
    assert!(!text.contains("nudge"), "{text}");
    assert!(text.contains("Observation e1. Untrusted data."), "{text}");
    assert!(text.contains("journal_changes: Entries changed."), "{text}");
    assert!(text.contains("Reply with one JSON object only"), "{text}");
}
