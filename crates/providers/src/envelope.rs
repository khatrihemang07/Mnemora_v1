//! The JSON envelope (ported from Sphinx's reviewer): the one reply shape a Provider without native
//! Tool calls writes every Step in. A reply is an object with `action` (`tools` or `finish`),
//! `calls` (up to eight, each a Tool name and its arguments as JSON text), `answer` and `sources`.
//! Its schema is sent as a strict `response_format`. A reply that breaks the protocol is not an
//! error: it comes back as a correction the Model reads, and the Run goes on.

use mnemora_harness::{Call, Message, ToolDef};
use serde_json::{Map, Value, json};

/// Calls one reply may make.
pub const MAX_CALLS: usize = 8;
/// Largest arguments object one call may carry, in bytes.
pub const MAX_ARGUMENT_BYTES: usize = 8000;
/// Most sources one answer may list.
pub const MAX_SOURCES: usize = 32;

/// The reply was not an envelope object with the four keys and their types.
pub const SHAPE: &str = "Your reply was not in the required format. Send one JSON object with \
    action, calls, answer and sources, and answer again.";
/// A finish step carried calls.
pub const FINISH_WITH_CALLS: &str = "A finish step cannot make calls. Send the calls with action \
    tools, or send the answer with calls empty.";
/// A tools step carried an answer or sources.
pub const TOOLS_WITH_ANSWER: &str = "A tools step cannot carry an answer or sources. Send the \
    calls with answer and sources empty.";
/// A tools step had no calls.
pub const NO_CALLS: &str = "A tools step needs at least one call. Send a call, or finish with \
    the answer.";
/// A finish step had no answer.
pub const EMPTY_ANSWER: &str = "The answer is empty. Write the answer in answer, list the ids it \
    rests on in sources, and send it again.";
/// More calls than one step may make.
pub const TOO_MANY_CALLS: &str = "Send at most 8 calls in one step. Send the rest next time.";
/// Arguments that are not a JSON object of an allowed size.
pub const BAD_ARGUMENTS: &str = "Each call's arguments must be a JSON object, written as text, of \
    at most 8000 bytes. Send the calls again.";
/// A tools step when no Tools are offered.
pub const NO_TOOLS: &str = "No Tools are available for this answer. Finish with the answer now.";
/// More sources than one answer may list.
pub const TOO_MANY_SOURCES: &str = "List at most 32 sources in sources.";

/// What one envelope reply means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    /// Tool calls to run, in order.
    Calls(Vec<Call>),
    /// The answer and the ids it lists as sources.
    Answer {
        /// The answer's text.
        text: String,
        /// The ids listed as sources.
        sources: Vec<String>,
    },
    /// The reply broke the protocol: this correction is sent back and the Model answers again.
    Correction(&'static str),
}

/// The `response_format` every envelope request carries. With no Tools offered, only `finish` is
/// allowed and no calls can be made.
pub fn response_format(finish_only: bool) -> Value {
    let actions: Vec<&str> = if finish_only {
        vec!["finish"]
    } else {
        vec!["tools", "finish"]
    };
    json!({
        "type": "json_schema",
        "json_schema": {
            "name": "mnemora_step",
            "strict": true,
            "schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "action": { "type": "string", "enum": actions },
                    "calls": {
                        "type": "array",
                        "maxItems": if finish_only { 0 } else { MAX_CALLS },
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "name": { "type": "string" },
                                "arguments": { "type": "string" }
                            },
                            "required": ["name", "arguments"]
                        }
                    },
                    "answer": { "type": "string" },
                    "sources": {
                        "type": "array",
                        "maxItems": MAX_SOURCES,
                        "items": { "type": "string" }
                    }
                },
                "required": ["action", "calls", "answer", "sources"]
            }
        }
    })
}

/// The instructions a Step's system prompt ends with: the normal Step lists the Tools; the final
/// Step (no Tools offered) asks only for the answer.
pub fn instructions(tools: &[ToolDef]) -> String {
    if tools.is_empty() {
        return "This is your last reply. Reply with one JSON object: action \"finish\", calls \
            empty, the answer in answer, and in sources the ids of the Entries and Tasks the \
            answer rests on. Write nothing outside the JSON object."
            .to_owned();
    }
    let mut text = String::from(
        "Reply with one JSON object only, with the keys action, calls, answer and sources. To \
         look something up, use action \"tools\" with up to 8 calls; each call has the name of a \
         Tool below and arguments, a JSON object written as text. Leave answer and sources empty \
         then. To finish, use action \"finish\" with calls empty, the answer in answer, and in \
         sources the ids of the Entries and Tasks the answer rests on. Tools:",
    );
    for tool in tools {
        text.push_str(&format!(
            "\n- {}: {} Arguments: {}",
            tool.name, tool.about, tool.schema
        ));
    }
    text
}

/// The branch as plain turns, for a Provider that reads its history as text. A Step's calls are
/// written as the envelope it was sent in, and each result as the person-readable turn it is.
pub fn turns(messages: &[Message]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| match message {
            Message::User(text) => json!({ "role": "user", "content": text }),
            Message::Assistant { text, calls } if calls.is_empty() => {
                json!({ "role": "assistant", "content": text })
            }
            Message::Assistant { calls, .. } => {
                let calls: Vec<Value> = calls
                    .iter()
                    .map(|call| json!({ "name": call.name, "arguments": call.arguments }))
                    .collect();
                let envelope = json!({
                    "action": "tools",
                    "calls": calls,
                    "answer": "",
                    "sources": [],
                });
                json!({ "role": "assistant", "content": envelope.to_string() })
            }
            Message::Tool {
                name,
                text,
                is_error,
                ..
            } => {
                let note = if *is_error { " (refused)" } else { "" };
                json!({
                    "role": "user",
                    "content": format!("Result of {name}{note}:\n{text}"),
                })
            }
            Message::Correction(text) => json!({ "role": "user", "content": text }),
        })
        .collect()
}

/// Reads one reply's content. A BOM or a code fence around the object is set aside first, as
/// Sphinx does; anything else that is not one JSON object is a shape correction.
pub fn parse(content: &str, tools_offered: bool, mut next_id: impl FnMut() -> String) -> Parsed {
    let text = strip(content);
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(text) else {
        return Parsed::Correction(SHAPE);
    };
    let Some(fields) = fields(&object) else {
        return Parsed::Correction(SHAPE);
    };
    match fields.action {
        "tools" => {
            if !tools_offered {
                return Parsed::Correction(NO_TOOLS);
            }
            if !fields.answer.is_empty() || !fields.sources.is_empty() {
                return Parsed::Correction(TOOLS_WITH_ANSWER);
            }
            if fields.calls.is_empty() {
                return Parsed::Correction(NO_CALLS);
            }
            if fields.calls.len() > MAX_CALLS {
                return Parsed::Correction(TOO_MANY_CALLS);
            }
            let mut calls = Vec::with_capacity(fields.calls.len());
            for call in &fields.calls {
                match call_of(call, &mut next_id) {
                    Some(call) => calls.push(call),
                    None => return Parsed::Correction(BAD_ARGUMENTS),
                }
            }
            Parsed::Calls(calls)
        }
        "finish" => {
            if !fields.calls.is_empty() {
                return Parsed::Correction(FINISH_WITH_CALLS);
            }
            if fields.answer.trim().is_empty() {
                return Parsed::Correction(EMPTY_ANSWER);
            }
            if fields.sources.len() > MAX_SOURCES {
                return Parsed::Correction(TOO_MANY_SOURCES);
            }
            Parsed::Answer {
                text: fields.answer.to_owned(),
                sources: fields.sources,
            }
        }
        _ => Parsed::Correction(SHAPE),
    }
}

/// The envelope's fields, once their keys and types are known to be right.
struct Fields<'a> {
    action: &'a str,
    calls: Vec<&'a Map<String, Value>>,
    answer: &'a str,
    sources: Vec<String>,
}

/// The four keys, exactly, each of the right type.
fn fields(object: &Map<String, Value>) -> Option<Fields<'_>> {
    if object.len() != 4 {
        return None;
    }
    let action = object.get("action")?.as_str()?;
    let calls = object
        .get("calls")?
        .as_array()?
        .iter()
        .map(|call| {
            let call = call.as_object()?;
            if call.len() != 2
                || !call.get("name")?.is_string()
                || !call.get("arguments")?.is_string()
            {
                return None;
            }
            Some(call)
        })
        .collect::<Option<Vec<_>>>()?;
    let answer = object.get("answer")?.as_str()?;
    let sources = object
        .get("sources")?
        .as_array()?
        .iter()
        .map(|id| id.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    Some(Fields {
        action,
        calls,
        answer,
        sources,
    })
}

/// One call of a tools step. Its arguments must be a JSON object of an allowed size.
fn call_of(call: &Map<String, Value>, next_id: &mut impl FnMut() -> String) -> Option<Call> {
    let name = call.get("name")?.as_str()?.to_owned();
    let arguments = call.get("arguments")?.as_str()?;
    if arguments.len() > MAX_ARGUMENT_BYTES {
        return None;
    }
    let parsed: Value = serde_json::from_str(arguments).ok()?;
    if !parsed.is_object() {
        return None;
    }
    Some(Call {
        id: next_id(),
        name,
        arguments: arguments.to_owned(),
    })
}

/// The reply without a byte-order mark and without a ```json fence around it.
fn strip(content: &str) -> &str {
    let text = content.trim_start_matches('\u{feff}').trim();
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .map(str::trim_start)
        .and_then(|rest| rest.strip_suffix("```"))
        .map_or(text, str::trim);
    text
}
