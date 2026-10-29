//! The seed's driver: the fixture's commands run through the Modules' services, with no Tauri
//! runtime. Each command takes the same arguments the app's command takes, under the names the
//! fixture loader sends.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use mnemora_core::db::{self, Connection};
use mnemora_journal::NewEntry;
use mnemora_todo::{LocalNow, NewTaskDetails, NewWhen, RepeatRule};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::err;
use crate::ai_fixture::Driver;
use crate::state::lock;

/// Drives the fixture's commands through the Modules' services, with no Tauri runtime.
pub(super) struct SeedDriver {
    conn: Mutex<Connection>,
    clock: Arc<AtomicI64>,
}

impl SeedDriver {
    /// A driver over `conn`, whose time is read from `clock` (the fixture sets it as it goes).
    pub(super) fn new(conn: Connection, clock: Arc<AtomicI64>) -> Result<Self, String> {
        let source = Arc::clone(&clock);
        db::set_time_source(&conn, Arc::new(move || source.load(Ordering::SeqCst))).map_err(err)?;
        Ok(Self {
            conn: Mutex::new(conn),
            clock,
        })
    }
}

impl Driver for SeedDriver {
    fn invoke(&self, command: &str, args: Value) -> Result<Value, Value> {
        let mut guard = lock(&self.conn);
        let db: &mut Connection = &mut guard;
        match command {
            "todo_list_create" => reply(mnemora_todo::create_list(
                db,
                &arg::<String>(&args, "name")?,
            )),
            "todo_task_create" => reply(mnemora_todo::create_task(
                db,
                &arg::<String>(&args, "title")?,
                arg::<Option<String>>(&args, "list_id")?.as_deref(),
                arg::<Option<NewWhen>>(&args, "when")?.as_ref(),
                arg::<Option<NewTaskDetails>>(&args, "details")?.as_ref(),
            )),
            "todo_task_set_when" => reply(mnemora_todo::set_when(
                db,
                &arg::<String>(&args, "id")?,
                arg::<Option<String>>(&args, "date")?.as_deref(),
                arg::<Option<String>>(&args, "time")?.as_deref(),
                arg::<Option<RepeatRule>>(&args, "repeat")?.as_ref(),
                &arg::<LocalNow>(&args, "local")?,
            )),
            "todo_task_set_priority" => reply(mnemora_todo::set_priority(
                db,
                &arg::<String>(&args, "id")?,
                arg::<u32>(&args, "priority")?,
            )),
            "todo_task_move" => reply(mnemora_todo::move_task(
                db,
                &arg::<String>(&args, "id")?,
                arg::<Option<String>>(&args, "listId")?.as_deref(),
            )),
            "todo_task_complete" => reply(mnemora_todo::complete_task(
                db,
                &arg::<String>(&args, "id")?,
                &arg::<LocalNow>(&args, "local")?,
            )),
            "todo_task_wont_do" => reply(mnemora_todo::wont_do(
                db,
                &arg::<String>(&args, "id")?,
                &arg::<LocalNow>(&args, "local")?,
            )),
            "todo_task_skip" => reply(mnemora_todo::skip_task(
                db,
                &arg::<String>(&args, "id")?,
                &arg::<LocalNow>(&args, "local")?,
            )),
            "todo_task_delete" => {
                reply(mnemora_todo::delete_task(db, &arg::<String>(&args, "id")?))
            }
            "todo_comment_add" => reply(mnemora_todo::add_comment(
                db,
                &arg::<String>(&args, "taskId")?,
                &arg::<String>(&args, "body")?,
            )),
            "journal_entry_create" => reply(mnemora_journal::create_entry(
                db,
                arg::<NewEntry>(&args, "entry")?,
            )),
            "journal_entry_update" => reply(mnemora_journal::update_entry(
                db,
                &arg::<String>(&args, "id")?,
                arg::<String>(&args, "text")?,
                &arg::<String>(&args, "sessionId")?,
            )),
            "journal_entry_delete" => reply(mnemora_journal::delete_entry(
                db,
                &arg::<String>(&args, "id")?,
            )),
            other => Err(json!({ "message": format!("the Sandbox seed has no command {other}") })),
        }
    }

    fn set_clock(&self, ms: i64) {
        self.clock.store(ms, Ordering::SeqCst);
    }

    fn with_db(&self, f: &mut dyn FnMut(&Connection)) {
        f(&lock(&self.conn));
    }
}

/// One named argument of a fixture command, or a refusal naming it.
fn arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<T, Value> {
    serde_json::from_value(args.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|e| json!({ "message": format!("{key}: {e}") }))
}

/// A service's result as the loader reads it: the serialised value, or the serialised error.
fn reply<T: Serialize, E: Serialize>(result: Result<T, E>) -> Result<Value, Value> {
    match result {
        Ok(value) => Ok(serde_json::to_value(value).unwrap_or(Value::Null)),
        Err(error) => {
            Err(serde_json::to_value(error).unwrap_or_else(|e| json!({ "message": e.to_string() })))
        }
    }
}
