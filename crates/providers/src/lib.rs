//! Adapters that turn a Step into an OpenAI-compatible request and the reply back into a Step.
//! Every failure becomes a `Failure`; the service's own error text is read, mapped and dropped.

#![forbid(unsafe_code)]

pub mod address;
pub mod client;
pub mod envelope;
pub mod openai;
pub mod wrapper;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use wrapper::WrapperProvider;
