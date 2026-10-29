//! The last request of a question as text, for reading: the system prompt, every message, the Tool
//! specs and the Wrapper's wire body.

use std::fmt::Write as _;

use mnemora_ai::WrapperProvider;
use mnemora_harness::{Message, Request};

/// The request as the model is sent it: the system prompt, every message in order, the Tool specs,
/// and the Wrapper's wire body (the same function the adapter calls).
pub fn dump_request(request: &Request) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "### System prompt, as the harness sends it\n\n````text\n{}\n````\n",
        request.system
    );
    let _ = writeln!(out, "### Messages, oldest first\n");
    for (index, message) in request.messages.iter().enumerate() {
        let _ = match message {
            Message::User(text) => writeln!(
                out,
                "#### [{index}] user, the question\n\n````text\n{text}\n````\n"
            ),
            Message::Assistant { text, calls } => {
                let calls: Vec<String> = calls
                    .iter()
                    .map(|call| format!("{} {}", call.name, call.arguments))
                    .collect();
                writeln!(
                    out,
                    "#### [{index}] assistant, a Step that asked for Tools\n\ntext: {text:?}\ncalls: {}\n",
                    calls.join("; ")
                )
            }
            Message::Tool {
                call_id,
                name,
                text,
                is_error,
            } => writeln!(
                out,
                "#### [{index}] tool result for {name} (call {call_id}), refused: {is_error}, as the model reads it\n\n````text\n{text}\n````\n"
            ),
            Message::Correction(text) => {
                writeln!(out, "#### [{index}] correction\n\n````text\n{text}\n````\n")
            }
        };
    }
    let _ = writeln!(out, "### Tool specs offered this Step\n");
    for tool in &request.tools {
        let _ = writeln!(
            out,
            "- {}: {}\n  schema: {}",
            tool.name, tool.about, tool.schema
        );
    }
    let wire = WrapperProvider::body(request, request.tools.is_empty());
    let _ = writeln!(
        out,
        "\n### Wire body, as the Wrapper adapter builds it\n\n````json\n{}\n````",
        serde_json::to_string_pretty(&wire).unwrap_or_default()
    );
    out
}
