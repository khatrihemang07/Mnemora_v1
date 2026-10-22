//! Links that open the app (`mnemora://todo?...`), from the Todo widget's taps.
//!
//! The Android runtime reports them as "opened URL" events, and on a cold start the first one can
//! arrive before setup is done and long before the UI is listening. So they are queued here and the
//! UI drains the queue with `app_take_links`; when the app is already running, `LinkOpenedEvent`
//! tells the UI there is something to drain. The event carries nothing: draining is the only way to
//! read a link, so a link is never handled twice.

use std::sync::Mutex;

use tauri::{AppHandle, Runtime, State};
use tauri_specta::Event;

use crate::LinkOpenedEvent;
use crate::state::lock;

/// The only scheme the app takes links in.
const SCHEME: &str = "mnemora://";
/// Most links kept waiting; the oldest go first. Nothing legitimate queues more than a couple.
const MAX_PENDING: usize = 16;
/// Longest link kept, so a hostile intent cannot fill memory.
const MAX_LEN: usize = 2_048;

/// Links waiting for the UI, oldest first.
#[derive(Default)]
pub struct PendingLinks(Mutex<Vec<String>>);

impl PendingLinks {
    /// Queues the links that are ours and returns how many were kept.
    fn push(&self, urls: impl IntoIterator<Item = String>) -> usize {
        let mut queue = lock(&self.0);
        let mut kept = 0;
        for url in urls {
            if url.starts_with(SCHEME) && url.len() <= MAX_LEN {
                if queue.len() == MAX_PENDING {
                    queue.remove(0);
                }
                queue.push(url);
                kept += 1;
            }
        }
        kept
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *lock(&self.0))
    }
}

/// Queues `urls` the runtime says were opened with the app and tells a running UI to look.
pub fn accept<R: Runtime>(app: &AppHandle<R>, urls: impl IntoIterator<Item = String>) {
    use tauri::Manager;
    let Some(pending) = app.try_state::<PendingLinks>() else {
        return;
    };
    if pending.push(urls) > 0
        && let Err(e) = LinkOpenedEvent.emit(app)
    {
        // Before the UI exists this is expected; the queue is what carries the link.
        tracing::debug!(error = %e, "no one to tell about a link");
    }
}

/// The links that opened the app since the last call, oldest first.
#[tauri::command]
#[specta::specta]
pub fn app_take_links(links: State<'_, PendingLinks>) -> Vec<String> {
    links.take()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::TestApp;

    fn take(app: &TestApp) -> Vec<String> {
        app.invoke("app_take_links", json!({})).unwrap()
    }

    #[test]
    fn a_link_that_arrived_early_is_handed_over_once() {
        let app = TestApp::new();
        accept(&app.handle(), ["mnemora://todo?view=today".to_owned()]);
        assert_eq!(take(&app), ["mnemora://todo?view=today"]);
        assert!(take(&app).is_empty(), "taken links do not come back");
    }

    #[test]
    fn links_come_out_in_the_order_they_came_in() {
        let app = TestApp::new();
        accept(&app.handle(), ["mnemora://todo?view=inbox".to_owned()]);
        accept(
            &app.handle(),
            ["mnemora://todo?view=all&task=t1".to_owned()],
        );
        assert_eq!(
            take(&app),
            [
                "mnemora://todo?view=inbox",
                "mnemora://todo?view=all&task=t1"
            ]
        );
    }

    #[test]
    fn only_links_in_the_apps_own_scheme_are_kept() {
        let app = TestApp::new();
        accept(
            &app.handle(),
            [
                "https://example.com/todo".to_owned(),
                "content://media/external/file/1".to_owned(),
                "mnemora://todo".to_owned(),
            ],
        );
        assert_eq!(take(&app), ["mnemora://todo"]);
    }

    #[test]
    fn a_flood_keeps_the_newest_few() {
        let app = TestApp::new();
        let urls = (0..40).map(|i| format!("mnemora://todo?task=t{i}"));
        accept(&app.handle(), urls);
        let got = take(&app);
        assert_eq!(got.len(), MAX_PENDING);
        assert_eq!(
            got.last().map(String::as_str),
            Some("mnemora://todo?task=t39")
        );
    }

    #[test]
    fn an_overlong_link_is_dropped() {
        let app = TestApp::new();
        accept(
            &app.handle(),
            [format!("mnemora://todo?task={}", "x".repeat(MAX_LEN))],
        );
        assert!(take(&app).is_empty());
    }

    #[test]
    fn a_running_app_is_told_a_link_arrived() {
        let app = TestApp::new();
        let told = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let seen = std::sync::Arc::clone(&told);
        LinkOpenedEvent::listen(&app.handle(), move |_| {
            seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        accept(&app.handle(), ["https://nope".to_owned()]);
        accept(&app.handle(), ["mnemora://todo".to_owned()]);
        assert_eq!(told.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
