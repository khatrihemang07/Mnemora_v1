//! The agent harness: typed messages, the Provider contract and the agent loop. It knows nothing
//! about Modules or storage; the AI Module drives it (ADR 0018).

#![forbid(unsafe_code)]

pub mod agent;
mod message;
mod provider;
pub mod scripted;
mod step;

pub use agent::{
    Cited, Event, MAX_CALLS_PER_STEP, MAX_INVALID_STEPS, MAX_PROMPT_BYTES, MAX_REPEATS,
    MAX_REQUESTS, Outcome, Reply, StepRecord, ToolHost,
};
pub use message::{Call, Message};
pub use provider::{Failure, Provider, Request, StopReason, StreamEvent, ToolDef, ToolProtocol};
pub use step::{Answer, run_step};
