//! The headless QA harness: real AI Runs over a temporary copy of the Sandbox's seeded data, written
//! to markdown reports, with no app window and no browser. `examples/ai_qa.rs` runs it (`--features
//! seed`). A question goes through the path `ai_run_start` takes: the same Provider adapter, Tools,
//! system prompt, calendar and final-Step note. It differs in three ways: it runs on the calling
//! thread, it does not claim the Conversation, and it reads the answer from the saved entries. The
//! Sandbox itself is never written: each run works on a copy, so no Conversation is left behind and
//! the seed stays valid.
//!
//! The parts, one concern each, in `ai_qa/`: `args` (the flags), `knobs` (the variants and what
//! they switch off), `provider` (the Provider and Model a run asks, and the decorator that switches
//! knobs off on the wire), `questions`, `answer` (one question's Run), `report` (the markdown),
//! `dump` (a request as text) and `run` (the run on the copy). The shared command-line shape is in
//! `qa_cli`.

mod answer;
mod args;
mod dump;
mod knobs;
mod provider;
mod questions;
mod report;
mod run;

pub use answer::{Answer, answer};
pub use dump::dump_request;
pub use knobs::{BEFORE_ONLY_TOOLS, Off, Variant, toolset_for};
pub use provider::strip;
pub use questions::{BUILT_IN_QUESTIONS, load_questions};
pub use report::{RunSetup, report_markdown, run_setup};
pub use run::{copy_database, run_cli, weekday_of};
