//! The Provider a QA run asks, and the decorator that switches knobs off on each request it sends.
//! The app never wraps its Provider; only this harness does.

use std::sync::{Arc, Mutex, PoisonError};

use mnemora_ai::{INTENT_PARAGRAPH, ModelUsage, ProviderView};
use mnemora_harness::agent::FINAL_NOTE;
use mnemora_harness::{Provider, Request, StreamEvent, ToolProtocol};

use super::knobs::Off;

/// The Provider a run asks: the one `--provider` names, else the Sandbox's only one, else the one
/// used most recently. Refused when there is none, or when the choice is not clear.
pub fn pick_provider<'a>(
    providers: &'a [ProviderView],
    wanted: Option<&str>,
    usage: &ModelUsage,
) -> Result<&'a ProviderView, String> {
    if let Some(wanted) = wanted {
        return providers
            .iter()
            .find(|p| p.id == wanted || p.name.eq_ignore_ascii_case(wanted))
            .ok_or_else(|| format!("the Sandbox has no Provider called {wanted}"));
    }
    if let [only] = providers {
        return Ok(only);
    }
    usage
        .recent
        .iter()
        .find_map(|used| providers.iter().find(|p| p.id == used.provider_id))
        .ok_or_else(|| {
            "the Sandbox has several Providers and none was used recently; pass --provider"
                .to_owned()
        })
}

/// The Model a run asks when `--model` is not given: the one last used with the Provider.
pub fn default_model(usage: &ModelUsage, provider_id: &str) -> Option<String> {
    usage
        .recent
        .iter()
        .chain(&usage.most_used)
        .find(|used| used.provider_id == provider_id)
        .map(|used| used.model.clone())
}

/// The request with what `off` switches off removed: the final-Step note at the end of the system
/// prompt, and the intent paragraph. Everything else is kept.
///
/// Each removal is checked. A prompt without the paragraph it should lose panics, so a change to the
/// prompt cannot make the run quietly match the variant it is not.
pub fn strip(request: &Request, off: Off) -> Request {
    let mut out = request.clone();
    if off.final_note {
        let stripped = out
            .system
            .strip_suffix(&format!("\n{FINAL_NOTE}"))
            .map(str::to_owned);
        if let Some(system) = stripped {
            out.system = system;
        }
    }
    if off.intent {
        assert!(
            out.system.contains(INTENT_PARAGRAPH),
            "the system prompt has no intent paragraph to switch off: the prompt changed, so the run would not be the variant it names"
        );
        out.system = out.system.replace(INTENT_PARAGRAPH, "");
        assert!(
            !out.system.contains(INTENT_PARAGRAPH),
            "the intent paragraph is still in the system prompt after it was switched off"
        );
    }
    out
}

/// A Provider that removes what `off` switches off from each request before the real adapter sees
/// it, and keeps a copy of each request it sent.
pub struct Ablated {
    inner: Arc<dyn Provider>,
    off: Off,
    sent: Mutex<Vec<Request>>,
}

impl Ablated {
    /// Wraps `inner`, switching off what `off` names.
    pub fn new(inner: Arc<dyn Provider>, off: Off) -> Self {
        Self {
            inner,
            off,
            sent: Mutex::new(Vec::new()),
        }
    }

    /// The request this Provider sent last, with its knobs applied.
    pub fn last_sent(&self) -> Option<Request> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .last()
            .cloned()
    }
}

impl Provider for Ablated {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }

    fn tool_protocol(&self) -> ToolProtocol {
        self.inner.tool_protocol()
    }

    fn stream(
        &self,
        request: &Request,
        halt: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(StreamEvent),
    ) {
        let request = strip(request, self.off);
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        self.inner.stream(&request, halt, emit);
    }
}
