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

use mnemora_core::error::{AppError, ErrorCode};
use mnemora_todo::{LocalNow, WidgetView};
use serde::Deserialize;
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
    }
}
