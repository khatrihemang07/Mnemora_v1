//! The Todo widget's bridge (ADR 0014): one JSON-in, JSON-out function over the same Todo rules
//! as the commands. Kotlin calls it through JNI (`android.rs`); tests call [`handle`] directly.
//!
//! Replies are one of `{"ok": ...}`, `{"status": "not_set_up" | "needs_update"}` or
//! `{"err": {"code", "message"}}`. Nothing here may panic: release builds abort on panic, and the
//! caller is a broadcast receiver.

// Off Android only the tests drive the bridge.
#![cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]

#[cfg(target_os = "android")]
mod android;
pub mod signal;
pub mod source;

use mnemora_core::db::Connection;
use mnemora_core::error::{AppError, ErrorCode};
use mnemora_core::util::new_id;
use mnemora_todo::{LocalNow, WidgetView};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[cfg(target_os = "android")]
pub use android::register_app;
pub use source::Source;

/// What the widget can ask.
#[derive(Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
enum Request {
    /// A view as the widget draws it, as of the device's clock.
    Snapshot { view: String, local: LocalNow },
    /// The Lists the setup screen offers, with the app's theme.
    Lists,
    /// A tick. A Task with open steps is answered with `open_steps` and left alone unless
    /// `tick_steps` says what to do: `true` ticks every step first, `false` completes as it is.
    Complete {
        id: String,
        tick_steps: Option<bool>,
        local: LocalNow,
    },
    /// Undo for a tick: the `token` its reply gave.
    Undo { token: String },
}

/// What Undo needs to put a tick back, handed to the widget and returned by it. Opaque to Kotlin.
#[derive(Serialize, Deserialize)]
struct UndoToken {
    /// The Task, or for a repeating one the occurrence it recorded: what is reopened.
    reopen: String,
    /// The notes to put back, when "Tick all and complete" changed them.
    notes: Option<String>,
}

/// What a request came to.
pub enum Outcome {
    /// The reply's payload.
    Ok(Value),
    /// The database is not ready; only the app can fix that.
    Status(Status),
    /// It failed.
    Err(AppError),
}

/// Why the widget cannot show Tasks yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The app has never run here: there is no database.
    NotSetUp,
    /// The app was updated and its migrations have not run yet.
    NeedsUpdate,
}

fn reply(outcome: Outcome) -> String {
    let value = match outcome {
        Outcome::Ok(payload) => json!({ "ok": payload }),
        Outcome::Status(Status::NotSetUp) => json!({ "status": "not_set_up" }),
        Outcome::Status(Status::NeedsUpdate) => json!({ "status": "needs_update" }),
        Outcome::Err(e) => json!({ "err": { "code": e.code, "message": e.message } }),
    };
    value.to_string()
}

fn encode<T: serde::Serialize>(value: &T) -> Result<Value, AppError> {
    serde_json::to_value(value).map_err(|e| {
        AppError::new(ErrorCode::Internal, "Could not encode the reply").with_details(e)
    })
}

/// Answers one request. Never panics; a malformed request is an `err` reply.
pub fn handle(source: &dyn Source, request: &str) -> String {
    let request: Request = match serde_json::from_str(request) {
        Ok(request) => request,
        Err(e) => {
            // The message is the parser's, never the request text: titles must not reach logs.
            return reply(Outcome::Err(
                AppError::new(ErrorCode::Validation, "That request was not understood")
                    .with_details(e),
            ));
        }
    };
    match request {
        Request::Snapshot { view, local } => reply(source.run(&mut |conn| {
            let view = WidgetView::parse(&view)?;
            let snapshot = mnemora_todo::snapshot(conn, &view, &local)?;
            encode(&snapshot)
        })),
        Request::Lists => reply(source.run(&mut |conn| encode(&mnemora_todo::widget_lists(conn)?))),
        Request::Complete {
            id,
            tick_steps,
            local,
        } => complete(source, &id, tick_steps, &local),
        Request::Undo { token } => reply(source.write(&mut |conn| undo(conn, &token))),
    }
}

/// Ticks a Task through the app's own rules (`complete_task`: History, change log, Repeats). One
/// with open steps is asked about first, as in the app, unless `tick_steps` already answers.
fn complete(source: &dyn Source, id: &str, tick_steps: Option<bool>, local: &LocalNow) -> String {
    if tick_steps.is_none() {
        let asked = source.run(&mut |conn| {
            let task = mnemora_todo::task(conn, id)?;
            let count = mnemora_todo::open_steps(&task.notes);
            let open = task.state == mnemora_todo::TaskState::Open && count > 0;
            Ok(json!({ "open": open, "count": count, "title": mnemora_todo::plain_title(&task.title) }))
        });
        match asked {
            Outcome::Ok(v) if v["open"] == true => {
                return reply(Outcome::Ok(json!({
                    "kind": "open_steps", "task_id": id, "title": v["title"], "count": v["count"],
                })));
            }
            Outcome::Ok(_) => {}
            other => return reply(other),
        }
    }
    reply(source.write(&mut |conn| tick(conn, id, tick_steps == Some(true), local)))
}

fn tick(
    conn: &mut Connection,
    id: &str,
    tick_all: bool,
    local: &LocalNow,
) -> Result<Value, AppError> {
    let task = mnemora_todo::task(conn, id)?;
    // Like the app: the steps are ticked in their own save, then the Task is completed.
    let ticked = tick_all && mnemora_todo::open_steps(&task.notes) > 0;
    if ticked {
        let notes = mnemora_todo::tick_all_steps(&task.notes);
        mnemora_todo::update_task(conn, id, &task.title, &notes, &new_id())?;
    }
    let done = match mnemora_todo::complete_task(conn, id, local) {
        Ok(done) => done,
        Err(e) => {
            if ticked {
                let _ = mnemora_todo::update_task(conn, id, &task.title, &task.notes, &new_id());
            }
            return Err(e);
        }
    };
    let lead = mnemora_todo::completed_message(&task.title);
    // A repeating Task says where it went; the widget keeps that part whole and cuts the title.
    let suffix = match (&done.occurrence, &done.task.due_date) {
        (Some(_), Some(next)) => {
            let full = mnemora_todo::completed_repeat_message(&task.title, next, &local.today);
            full.strip_prefix(&lead)
                .map(|rest| rest.trim_start().to_owned())
        }
        _ => None,
    };
    let message = suffix
        .as_ref()
        .map_or_else(|| lead.clone(), |s| format!("{lead} {s}"));
    let token = UndoToken {
        reopen: done.occurrence.map_or_else(|| id.to_owned(), |o| o.id),
        notes: ticked.then_some(task.notes),
    };
    Ok(json!({
        "kind": "completed",
        "message": message,
        "lead": lead,
        "suffix": suffix,
        "token": encode(&token)?.to_string(),
    }))
}

/// Reopens what a tick completed and puts back the notes "Tick all" changed.
fn undo(conn: &mut Connection, token: &str) -> Result<Value, AppError> {
    let token: UndoToken = serde_json::from_str(token).map_err(|e| {
        AppError::new(ErrorCode::Validation, "That Undo is not valid").with_details(e)
    })?;
    let reopened = mnemora_todo::reopen_task(conn, &token.reopen)?;
    if let Some(notes) = token.notes.filter(|notes| *notes != reopened.notes) {
        mnemora_todo::update_task(conn, &reopened.id, &reopened.title, &notes, &new_id())?;
    }
    Ok(json!({ "undone": true }))
}
