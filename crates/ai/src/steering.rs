//! The notes a person types while a Run answers. They wait here, in order, until the Run reaches a
//! Step boundary and sends them to the Model; "Send now" moves one to the front. Closed when the
//! Run ends, and whatever is left is handed back so the person can still send it.

use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use mnemora_core::util::new_id;
use serde::{Deserialize, Serialize};
use specta::Type;

/// A note waiting to be sent to the Model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct QueuedNote {
    /// The note's id, for removing or sending it now.
    pub id: String,
    /// The note as typed.
    pub text: String,
}

#[derive(Debug, Default)]
struct Queue {
    notes: VecDeque<QueuedNote>,
    open: bool,
}

/// The notes of one Run. Shared by the Run's thread and the commands that add, remove or promote
/// them.
#[derive(Debug)]
pub struct Steering {
    queue: Mutex<Queue>,
}

impl Default for Steering {
    fn default() -> Self {
        Self {
            queue: Mutex::new(Queue {
                notes: VecDeque::new(),
                open: true,
            }),
        }
    }
}

impl Steering {
    /// An open queue with no notes.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Queues a note at the end. `None` once the Run has ended, so nothing is added to a Run that
    /// will never send it.
    pub fn add(&self, text: &str) -> Option<QueuedNote> {
        let mut queue = self.lock();
        if !queue.open {
            return None;
        }
        let note = QueuedNote {
            id: new_id(),
            text: text.to_owned(),
        };
        queue.notes.push_back(note.clone());
        Some(note)
    }

    /// Removes a note that has not been sent. Returns whether it was there.
    pub fn remove(&self, id: &str) -> bool {
        let mut queue = self.lock();
        let before = queue.notes.len();
        queue.notes.retain(|note| note.id != id);
        queue.notes.len() != before
    }

    /// Moves a note to the front, so it is the next one sent. Returns whether it was there.
    pub fn promote(&self, id: &str) -> bool {
        let mut queue = self.lock();
        let Some(index) = queue.notes.iter().position(|note| note.id == id) else {
            return false;
        };
        if let Some(note) = queue.notes.remove(index) {
            queue.notes.push_front(note);
        }
        true
    }

    /// Takes the next note to send, if any.
    pub fn next(&self) -> Option<String> {
        self.lock().notes.pop_front().map(|note| note.text)
    }

    /// The notes still waiting, in order.
    pub fn snapshot(&self) -> Vec<QueuedNote> {
        self.lock().notes.iter().cloned().collect()
    }

    /// Ends the queue: no more notes are accepted. Returns the notes that were never sent.
    pub fn close(&self) -> Vec<QueuedNote> {
        let mut queue = self.lock();
        queue.open = false;
        queue.notes.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_sent_in_order_and_can_be_removed_or_promoted() {
        let queue = Steering::new();
        let first = queue.add("first").unwrap();
        let second = queue.add("second").unwrap();
        let third = queue.add("third").unwrap();
        assert!(queue.promote(&third.id));
        assert!(queue.remove(&second.id));
        assert!(!queue.remove(&second.id));
        assert_eq!(queue.snapshot(), vec![third.clone(), first.clone()]);
        assert_eq!(queue.next().as_deref(), Some("third"));
        assert_eq!(queue.next().as_deref(), Some("first"));
        assert_eq!(queue.next(), None);
    }

    #[test]
    fn a_closed_queue_takes_no_more_and_hands_back_what_is_left() {
        let queue = Steering::new();
        queue.add("left").unwrap();
        let left = queue.close();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].text, "left");
        assert!(queue.add("late").is_none());
    }
}
