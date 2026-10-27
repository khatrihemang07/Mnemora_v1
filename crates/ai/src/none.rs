//! The Provider a device has before any is set up: every Step fails as "no Provider".

use mnemora_harness::{Failure, Provider, Request, StreamEvent};

/// Stands in for a Provider until one is configured (the Providers ticket adds them).
pub struct NoProvider;

impl Provider for NoProvider {
    fn name(&self) -> &'static str {
        ""
    }

    fn model(&self) -> &'static str {
        ""
    }

    fn stream(&self, _request: &Request, emit: &mut dyn FnMut(StreamEvent)) {
        emit(StreamEvent::Failed(Failure::NoProvider));
    }
}
