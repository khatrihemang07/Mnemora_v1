//! Typed in-process event bus. Modules talk to each other only through this, never by import.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

use serde::{Deserialize, Serialize};
use specta::Type;

type Handler = Arc<dyn Fn(&dyn Any) + Send + Sync>;

/// Publish/subscribe keyed by the event's Rust type.
#[derive(Clone, Default)]
pub struct EventBus {
    handlers: Arc<RwLock<HashMap<TypeId, Vec<Handler>>>>,
}

impl EventBus {
    /// An empty bus.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `handler` for every future `E`.
    pub fn subscribe<E: Any>(&self, handler: impl Fn(&E) + Send + Sync + 'static) {
        let erased: Handler = Arc::new(move |any: &dyn Any| {
            if let Some(event) = any.downcast_ref::<E>() {
                handler(event);
            }
        });
        self.handlers
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(TypeId::of::<E>())
            .or_default()
            .push(erased);
    }

    /// Delivers `event` synchronously to every subscriber of `E`.
    pub fn publish<E: Any>(&self, event: &E) {
        // Clone the handler list so a handler may subscribe without deadlocking.
        let handlers: Vec<Handler> = self
            .handlers
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&TypeId::of::<E>())
            .cloned()
            .unwrap_or_default();
        for handler in handlers {
            handler(event);
        }
    }
}

/// Some Settings keys of one Module were saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SettingsChanged {
    /// Owning Module id, e.g. `core`.
    pub module: String,
    /// Keys within that Module (without the module prefix).
    pub keys: Vec<String>,
}

/// Sync applied changes from another device to a Module's data, so whatever that Module shows
/// is stale. The app shell turns this into the Module's own change event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteChanged {
    /// Id of the Module whose data changed, e.g. `todo`.
    pub module: String,
}

/// A sync session with another device ended, having changed these Modules' data. Sent once per
/// session, not per batch, for things that only need to know at the end (the Todo widget).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSessionEnded {
    /// Ids of the Modules whose data the session changed.
    pub modules: Vec<String>,
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Debug, PartialEq)]
    struct Ping(u32);
    struct Other;

    #[test]
    fn subscribers_receive_matching_events_only() {
        let bus = EventBus::new();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        bus.subscribe(move |p: &Ping| sink.lock().unwrap().push(p.0));
        bus.publish(&Ping(1));
        bus.publish(&Other);
        bus.publish(&Ping(2));
        assert_eq!(*seen.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn every_subscriber_gets_the_event() {
        let bus = EventBus::new();
        let count = Arc::new(Mutex::new(0));
        for _ in 0..3 {
            let c = count.clone();
            bus.subscribe(move |_: &Ping| *c.lock().unwrap() += 1);
        }
        bus.publish(&Ping(0));
        assert_eq!(*count.lock().unwrap(), 3);
    }

    #[test]
    fn publish_without_subscribers_is_a_no_op() {
        EventBus::new().publish(&Ping(9));
    }

    #[test]
    fn clones_share_subscriptions() {
        let bus = EventBus::new();
        let other = bus.clone();
        let hits = Arc::new(Mutex::new(0));
        let h = hits.clone();
        other.subscribe(move |_: &Ping| *h.lock().unwrap() += 1);
        bus.publish(&Ping(0));
        assert_eq!(*hits.lock().unwrap(), 1);
    }

    #[test]
    fn settings_changed_wire_shape() {
        let e = SettingsChanged {
            module: "core".into(),
            keys: vec!["theme".into()],
        };
        assert_eq!(
            serde_json::to_value(e).unwrap(),
            serde_json::json!({"module": "core", "keys": ["theme"]})
        );
    }
}
