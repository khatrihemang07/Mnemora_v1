//! One sync exchange between two devices, as a state machine that does no I/O of its own.
//!
//! Both sides say Hello with their version vector, then each streams what the other lacks in
//! batches, waiting for an Ack after every batch (so neither side buffers a whole database).
//! Each received batch is applied in one transaction that also advances the vector, so a
//! dropped connection costs at most the batch in flight and the next session resumes from the
//! vector. A driver ([`Endpoint`]) moves messages between a session and a [`Transport`].

use std::collections::BTreeSet;

use std::collections::BTreeMap;

use mnemora_core::events::{
    EventBus, ListsMerged, RemoteChanged, RemoteSessionEnded, SettingsChanged,
};
use mnemora_core::syncable::SyncEvent;
use rusqlite::Connection;

use crate::apply::apply_batch;
use crate::error::SyncError;
use crate::peers;
use crate::registry::Registry;
use crate::stream::{Cursor, next_batch, pending};
use crate::transport::Transport;
use crate::vector;
use crate::wire::{Message, Vector};

/// What a session moved.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SessionStats {
    /// Records sent to the other device.
    pub sent: usize,
    /// Records received and stored.
    pub received: usize,
    /// Modules whose data the session changed here.
    pub changed: BTreeSet<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum Outbound {
    /// Waiting for the other device's Hello.
    Idle,
    /// A batch is out; `last` says whether it ends the stream.
    AwaitingAck { last: bool },
    /// Everything was sent and acknowledged.
    Finished,
}

/// One exchange with one other device. Created through `SyncRuntime::session`, which refuses
/// while Sync is off.
pub struct Session {
    me: String,
    registry: Registry,
    bus: EventBus,
    peer: Option<(String, Vector)>,
    cursor: Cursor,
    outbound: Outbound,
    inbound_done: bool,
    stats: SessionStats,
    /// Records the other side said it will send, and this side will send.
    in_total: usize,
    out_total: usize,
    /// Records of the batch in flight, counted once acknowledged.
    in_flight: usize,
    acked: usize,
    announced: bool,
    first_sent: bool,
}

impl Session {
    pub(crate) fn new(me: String, registry: Registry, bus: EventBus) -> Self {
        Self {
            me,
            registry,
            bus,
            peer: None,
            cursor: Cursor::default(),
            outbound: Outbound::Idle,
            inbound_done: false,
            stats: SessionStats::default(),
            in_total: 0,
            out_total: 0,
            in_flight: 0,
            acked: 0,
            announced: false,
            first_sent: false,
        }
    }

    /// The message that opens the exchange.
    pub fn hello(&self, conn: &Connection) -> Result<Message, SyncError> {
        Ok(Message::Hello {
            device: self.me.clone(),
            vector: vector::all(conn, &self.registry, &self.me)?,
        })
    }

    /// The device id of the other side, once it has said Hello.
    pub fn peer(&self) -> Option<&str> {
        self.peer.as_ref().map(|p| p.0.as_str())
    }

    /// Everything was sent, acknowledged and received.
    pub fn is_done(&self) -> bool {
        self.inbound_done && self.outbound == Outbound::Finished
    }

    /// How far the exchange has got: records moved (either way) of the records it will move.
    pub fn progress(&self) -> (usize, usize) {
        (
            self.stats.received + self.acked,
            self.in_total + self.out_total,
        )
    }

    /// What has moved so far.
    pub fn stats(&self) -> &SessionStats {
        &self.stats
    }

    /// Reacts to a message from the other device; returns what to send back. An error leaves
    /// the database as it was before that message.
    pub fn handle(
        &mut self,
        conn: &Connection,
        message: Message,
    ) -> Result<Vec<Message>, SyncError> {
        let replies = self.step(conn, message)?;
        self.announce_end();
        Ok(replies)
    }

    /// Tells the bus once, when the exchange is over, which Modules it changed.
    fn announce_end(&mut self) {
        if self.is_done() && !self.announced {
            self.announced = true;
            if !self.stats.changed.is_empty() {
                self.bus.publish(&RemoteSessionEnded {
                    modules: self.stats.changed.iter().cloned().collect(),
                });
            }
        }
    }

    fn step(&mut self, conn: &Connection, message: Message) -> Result<Vec<Message>, SyncError> {
        match message {
            Message::Hello { device, vector } => {
                if self.peer.is_some() || self.outbound != Outbound::Idle {
                    return Err(SyncError::Protocol("a second Hello".into()));
                }
                if device == self.me {
                    return Err(SyncError::Protocol(
                        "this device is talking to itself".into(),
                    ));
                }
                self.out_total = pending(conn, &self.registry, &self.me, &vector)?;
                self.peer = Some((device, vector));
                Ok(vec![self.next_outbound(conn)?])
            }
            Message::Batch {
                records,
                last,
                total,
            } => {
                if self.peer.is_none() || self.inbound_done {
                    return Err(SyncError::Protocol("a batch out of turn".into()));
                }
                if let Some(total) = total {
                    self.in_total = total;
                }
                let applied = apply_batch(conn, &self.registry, &self.me, records)?;
                self.stats.received += applied.stored;
                // First, so a screen on a List that merged away is moved before it refreshes and
                // finds the List gone.
                self.announce(conn, applied.events);
                for module in &applied.changed {
                    self.bus.publish(&RemoteChanged {
                        module: module.clone(),
                    });
                }
                self.stats.changed.extend(applied.changed);
                self.inbound_done = last;
                Ok(vec![Message::Ack])
            }
            Message::Ack => match self.outbound {
                Outbound::AwaitingAck { last: true } => {
                    self.acked += self.in_flight;
                    self.in_flight = 0;
                    self.outbound = Outbound::Finished;
                    Ok(Vec::new())
                }
                Outbound::AwaitingAck { last: false } => {
                    self.acked += self.in_flight;
                    Ok(vec![self.next_outbound(conn)?])
                }
                _ => Err(SyncError::Protocol("an Ack out of turn".into())),
            },
        }
    }

    /// Turns what the Modules reported into events for the app: Preferences that changed, and
    /// Lists that merged (named after the other device when it is a paired one).
    fn announce(&self, conn: &Connection, events: Vec<SyncEvent>) {
        let mut settings: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for event in events {
            match event {
                SyncEvent::SettingChanged { module, key } => {
                    settings.entry(module).or_default().push(key);
                }
                SyncEvent::ListsMerged {
                    from_id,
                    from_name,
                    from_device,
                    into_id,
                    into_name,
                    into_device,
                } => {
                    // The device that is not this one: the survivor's, else the other List's.
                    let other = [into_device, from_device]
                        .into_iter()
                        .flatten()
                        .find(|d| *d != self.me);
                    let device = other.and_then(|d| peers::name_of(conn, &d).ok().flatten());
                    tracing::info!(target: "sync", "two Lists with the same name merged");
                    self.bus.publish(&ListsMerged {
                        from_id,
                        from_name,
                        into_id,
                        into_name,
                        device,
                    });
                }
                // Recorded by Activity and Resolved automatically (#67/#68); the Conflicts tab
                // reads the table, and `RemoteChanged` for Sync refreshes it.
                SyncEvent::ConflictRaised { .. } | SyncEvent::ConflictAutoResolved { .. } => {
                    tracing::debug!(target: "sync", "a text clash was recorded");
                }
                SyncEvent::TaskToInbox { .. } => {
                    tracing::debug!(target: "sync", "a Task went to the Inbox: its List is gone");
                }
            }
        }
        for (module, keys) in settings {
            self.bus.publish(&SettingsChanged { module, keys });
        }
    }

    fn next_outbound(&mut self, conn: &Connection) -> Result<Message, SyncError> {
        let Some((_, peer_vector)) = &self.peer else {
            return Err(SyncError::Protocol("no Hello yet".into()));
        };
        let (records, more) = next_batch(
            conn,
            &self.registry,
            &self.me,
            peer_vector,
            &mut self.cursor,
        )?;
        self.stats.sent += records.len();
        self.in_flight = records.len();
        let first = !self.first_sent;
        self.first_sent = true;
        self.outbound = Outbound::AwaitingAck { last: !more };
        Ok(Message::Batch {
            records,
            last: !more,
            total: first.then_some(self.out_total),
        })
    }
}

/// A [`Session`] joined to a [`Transport`]: poll it until it is done.
pub struct Endpoint<T: Transport> {
    session: Session,
    transport: T,
    opened: bool,
}

impl<T: Transport> Endpoint<T> {
    /// Joins `session` to `transport`.
    pub fn new(session: Session, transport: T) -> Self {
        Self {
            session,
            transport,
            opened: false,
        }
    }

    /// Does whatever can be done now: opens the exchange, or takes one received message and
    /// answers it. Returns whether anything happened.
    pub fn poll(&mut self, conn: &Connection) -> Result<bool, SyncError> {
        if !self.opened {
            self.opened = true;
            let hello = self.session.hello(conn)?;
            self.transport.send(hello)?;
            return Ok(true);
        }
        let Some(message) = self.transport.try_recv()? else {
            return Ok(false);
        };
        for reply in self.session.handle(conn, message)? {
            self.transport.send(reply)?;
        }
        Ok(true)
    }

    /// The session finished.
    pub fn is_done(&self) -> bool {
        self.session.is_done()
    }

    /// The session, for its stats.
    pub fn session(&self) -> &Session {
        &self.session
    }
}

/// Runs two endpoints, each over its own database, until both are done: the in-process driver
/// the tests use. Fails with the first error either side meets (a cut line fails both).
pub fn pump<A: Transport, B: Transport>(
    a: (&mut Endpoint<A>, &Connection),
    b: (&mut Endpoint<B>, &Connection),
) -> Result<(), SyncError> {
    let (ea, ca) = a;
    let (eb, cb) = b;
    while !(ea.is_done() && eb.is_done()) {
        let moved_a = ea.poll(ca)?;
        let moved_b = eb.poll(cb)?;
        if !moved_a && !moved_b {
            return Err(SyncError::Protocol("the exchange stalled".into()));
        }
    }
    Ok(())
}
