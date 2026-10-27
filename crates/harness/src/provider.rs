//! The contract every AI service adapter implements. A Provider never throws: it ends each Step
//! with exactly one `Done` or `Failed` event, and its raw error text stays inside it.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::message::Message;

/// Why a Step failed. The AI Module words it for the person; the raw text is never kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    /// No Provider is set up on this device.
    NoProvider,
    /// The address could not be reached.
    Unreachable,
    /// The service did not answer in time.
    Timeout,
    /// The service refused the key.
    KeyRejected,
    /// The service is busy or has a limit in force.
    RateLimited,
    /// The chosen Model is not offered by this service.
    ModelUnavailable,
    /// The chosen Model cannot make Tool calls.
    ModelCannotUseTools,
    /// The reply was not in a shape Mnemora can read.
    BadResponse,
    /// The request is too large for the Model.
    TooLong,
    /// The Run was stopped by the app (Android background, an abort).
    Interrupted,
    /// The device has no connection.
    Offline,
    /// Anything else.
    Unexpected,
}

/// Why a Step's reply ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// The Model finished its reply.
    Stop,
    /// The reply hit the Model's output limit.
    Length,
}

/// Everything one Step sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The system prompt for this Run.
    pub system: String,
    /// The branch, oldest first, ending with the question being answered.
    pub messages: Vec<Message>,
    /// The Model id the Provider sends.
    pub model: String,
}

/// One event of a Step's stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// A piece of the reply's text, in order.
    TextDelta(String),
    /// The reply is complete.
    Done(StopReason),
    /// The Step failed; nothing after this is expected.
    Failed(Failure),
}

/// An AI service the Run can ask. Implementations are adapters: the wire format lives in them,
/// the loop does not.
pub trait Provider: Send + Sync {
    /// The name the person gave this Provider (kept on each answer, so other devices can show it).
    fn name(&self) -> &str;

    /// The Model id this Provider sends.
    fn model(&self) -> &str;

    /// Streams one Step, calling `emit` for each event in order. Must end with `Done` or `Failed`.
    fn stream(&self, request: &Request, emit: &mut dyn FnMut(StreamEvent));
}
