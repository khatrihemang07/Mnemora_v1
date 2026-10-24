//! The seam between the sync engine and whatever carries its messages. The engine never opens
//! a socket: it is handed a [`Transport`]. [`memory`] is the in-process one the tests use; the
//! network one (TLS) implements the same two methods.

use crate::error::SyncError;
use crate::wire::Message;

/// One end of a connection between two devices.
pub trait Transport {
    /// Hands a message to the other end. Fails with [`SyncError::Disconnected`] once the
    /// connection is gone.
    fn send(&mut self, message: Message) -> Result<(), SyncError>;

    /// The next message that has arrived, in order, or `None` when there is none yet. Never
    /// blocks: a driver polls it (a socket driver reads whatever is buffered). Fails with
    /// [`SyncError::Disconnected`] once the connection is gone.
    fn try_recv(&mut self) -> Result<Option<Message>, SyncError>;
}

pub mod memory {
    //! An in-process connection: two ends sharing a pair of queues, with a handle that can cut
    //! the line, for the fault tests.

    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex, PoisonError};

    use super::Transport;
    use crate::error::SyncError;
    use crate::wire::Message;

    #[derive(Default)]
    struct Line {
        to_a: VecDeque<Message>,
        to_b: VecDeque<Message>,
        /// Messages that may still be received before the line drops.
        deliveries_left: Option<usize>,
        cut: bool,
    }

    /// One end. Cloning is not offered: an end belongs to one driver.
    pub struct MemoryTransport {
        line: Arc<Mutex<Line>>,
        is_a: bool,
    }

    /// Controls a connection from outside, e.g. to drop it mid-exchange.
    #[derive(Clone)]
    pub struct Link {
        line: Arc<Mutex<Line>>,
    }

    impl Link {
        /// Drops the connection now; whatever was in flight is lost.
        pub fn cut(&self) {
            let mut line = self.line.lock().unwrap_or_else(PoisonError::into_inner);
            line.cut = true;
            line.to_a.clear();
            line.to_b.clear();
        }

        /// Drops the connection once `n` more messages have been received (by either end).
        pub fn cut_after(&self, n: usize) {
            let mut line = self.line.lock().unwrap_or_else(PoisonError::into_inner);
            line.deliveries_left = Some(n);
        }
    }

    /// Two connected ends and the handle that controls the line.
    pub fn pair() -> (MemoryTransport, MemoryTransport, Link) {
        let line = Arc::new(Mutex::new(Line::default()));
        (
            MemoryTransport {
                line: Arc::clone(&line),
                is_a: true,
            },
            MemoryTransport {
                line: Arc::clone(&line),
                is_a: false,
            },
            Link { line },
        )
    }

    impl Transport for MemoryTransport {
        fn send(&mut self, message: Message) -> Result<(), SyncError> {
            let mut line = self.line.lock().unwrap_or_else(PoisonError::into_inner);
            if line.cut {
                return Err(SyncError::Disconnected);
            }
            let queue = if self.is_a {
                &mut line.to_b
            } else {
                &mut line.to_a
            };
            queue.push_back(message);
            Ok(())
        }

        fn try_recv(&mut self) -> Result<Option<Message>, SyncError> {
            let mut line = self.line.lock().unwrap_or_else(PoisonError::into_inner);
            if line.cut {
                return Err(SyncError::Disconnected);
            }
            let next = if self.is_a {
                line.to_a.pop_front()
            } else {
                line.to_b.pop_front()
            };
            if next.is_some()
                && let Some(left) = line.deliveries_left.as_mut()
            {
                *left = left.saturating_sub(1);
                if *left == 0 {
                    // The message just taken is the last one the line carries.
                    line.cut = true;
                    line.to_a.clear();
                    line.to_b.clear();
                }
            }
            Ok(next)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn messages_arrive_in_order_at_the_other_end_only() {
            let (mut a, mut b, _) = pair();
            a.send(Message::Ack).unwrap();
            assert_eq!(a.try_recv().unwrap(), None, "not echoed back");
            assert_eq!(b.try_recv().unwrap(), Some(Message::Ack));
            assert_eq!(b.try_recv().unwrap(), None);
        }

        #[test]
        fn a_cut_line_fails_both_ends_and_loses_what_was_in_flight() {
            let (mut a, mut b, link) = pair();
            a.send(Message::Ack).unwrap();
            link.cut();
            assert!(matches!(b.try_recv(), Err(SyncError::Disconnected)));
            assert!(matches!(a.send(Message::Ack), Err(SyncError::Disconnected)));
        }

        #[test]
        fn cut_after_lets_a_number_of_messages_through() {
            let (mut a, mut b, link) = pair();
            link.cut_after(2);
            for _ in 0..3 {
                a.send(Message::Ack).unwrap();
            }
            assert!(b.try_recv().unwrap().is_some());
            assert!(b.try_recv().unwrap().is_some());
            assert!(matches!(b.try_recv(), Err(SyncError::Disconnected)));
        }
    }
}
