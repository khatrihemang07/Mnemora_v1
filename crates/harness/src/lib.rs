//! The agent harness: typed messages, the Provider contract and one Step of the loop. It knows
//! nothing about Modules or storage; the AI Module drives it (ADR 0018).

#![forbid(unsafe_code)]

mod message;
mod provider;
pub mod scripted;
mod step;

pub use message::Message;
pub use provider::{Failure, Provider, Request, StopReason, StreamEvent};
pub use step::{Answer, run_step};
