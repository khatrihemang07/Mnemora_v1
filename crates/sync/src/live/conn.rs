//! One open connection to a paired device. It starts a session when it opens, answers the other
//! device's sessions, and starts another when told to (new local changes, "Sync now"). Between
//! sessions it just waits, which is what keeps the connection "open and pushing".
//!
//! Each session is a fresh [`Session`]: a side that has something to say sends its Hello, and a
//! side that is idle and receives a Hello answers with its own. When both start at once the two
//! Hellos simply cross. Database work runs on blocking threads, one batch per transaction.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use super::shared::Shared;
use super::status::Progress;
use super::{Host, blocking};
use crate::error::SyncError;
use crate::net::NetTransport;
use crate::peer_state;
use crate::session::{Session, SessionStats};
use crate::transport::Transport;
use crate::wire::Message;

/// A session that hears nothing for this long is given up on (the connection is closed).
const SILENCE: Duration = Duration::from_secs(60);

/// What the supervisor can ask of a connection.
#[derive(Debug)]
pub(super) enum ConnCmd {
    /// Start a session now (or right after the running one).
    Round,
    /// Close the connection.
    Close,
}

/// What a connection tells the supervisor.
#[derive(Debug)]
pub(super) struct RoundDone {
    pub peer: String,
    /// Records the session stored, the line it wrote in Activity or Activity it took in: news
    /// for the other devices (see `news`).
    pub received: usize,
}

/// How much of what a session did is news to the other devices. Nothing hooks the sync
/// connection's own writes, so the supervisor is told: stored records, the line this device wrote
/// in Activity and Activity that merely arrived (a device the sender cannot reach may be reached
/// from here). A round that brought nothing new ends the chain; Activity is written once, so
/// passing it on cannot go round for ever.
fn news(stats: &SessionStats) -> usize {
    stats.received + stats.activity + usize::from(stats.logged)
}

pub(super) struct Conn {
    pub shared: Arc<Shared>,
    pub peer: String,
    pub transport: NetTransport,
    pub cmds: UnboundedReceiver<ConnCmd>,
    pub done: UnboundedSender<RoundDone>,
    /// Stay open after a session; off for a "Sync now" with Sync automatically off.
    pub keep_open: bool,
    /// This connection already took the first-merge backup (or none was due).
    pub backed_up: bool,
}

impl Conn {
    fn host(&self) -> &Arc<dyn Host> {
        &self.shared.host
    }

    fn new_session(&self) -> Session {
        self.shared.sessions.fetch_add(1, Ordering::SeqCst);
        let host = self.host();
        Session::new(self.shared.me.clone(), host.registry(), host.bus()).named(&host.device_name())
    }

    async fn start_round(&mut self) -> Result<Session, SyncError> {
        // A device forgotten while this connection was open gets nothing more from it.
        let peer = self.peer.clone();
        let paired = blocking(self.host(), move |conn| {
            Ok(crate::peers::is_paired(conn, &peer)?)
        })
        .await?;
        if !paired {
            return Err(SyncError::Disconnected);
        }
        let session = self.new_session();
        let (session, hello) = blocking(self.host(), move |conn| {
            let hello = session.hello(conn)?;
            Ok((session, hello))
        })
        .await?;
        self.transport.send(hello)?;
        self.shared.update(&self.peer, true, |l| {
            l.syncing = true;
            l.progress = None;
        });
        Ok(session)
    }

    /// A device introduced by another member has never been merged with this one: its first
    /// records are preceded by a backup (the pairing backup covers devices paired directly). A
    /// failed backup ends the session before anything is applied; the next one tries again.
    async fn back_up_before_first_records(&mut self) -> Result<(), SyncError> {
        let (me, peer) = (self.shared.me.clone(), self.peer.clone());
        let due = blocking(self.host(), move |conn| {
            Ok(peer_state::needs_first_backup(conn, &me, &peer)?)
        })
        .await?;
        if due {
            let host = Arc::clone(self.host());
            tokio::task::spawn_blocking(move || host.backup())
                .await
                .map_err(|e| SyncError::Protocol(format!("a backup task failed: {e}")))??;
            tracing::info!(target: "sync", "backed up before the first records of an introduced device");
        }
        self.backed_up = true;
        Ok(())
    }

    /// Feeds `message` to the session, sends what it answers and reports progress. Returns the
    /// session while it still has work to do.
    async fn feed(
        &mut self,
        session: Session,
        message: Message,
    ) -> Result<Option<Session>, SyncError> {
        if !self.backed_up
            && matches!(&message, Message::Batch { records, .. } if !records.is_empty())
        {
            self.back_up_before_first_records().await?;
        }
        let (session, replies) = blocking(self.host(), move |conn| {
            let mut session = session;
            let replies = session.handle(conn, message)?;
            Ok((session, replies))
        })
        .await?;
        for reply in replies {
            self.transport.send(reply)?;
        }
        if session.is_done() {
            self.finish(session.stats().clone()).await?;
            return Ok(None);
        }
        let (done, total) = session.progress();
        self.shared.update(&self.peer, false, |l| {
            l.progress = Some(Progress {
                done: u32::try_from(done).unwrap_or(u32::MAX),
                total: u32::try_from(total).unwrap_or(u32::MAX),
            });
        });
        Ok(Some(session))
    }

    async fn finish(&mut self, stats: SessionStats) -> Result<(), SyncError> {
        let peer = self.peer.clone();
        let (received, sent) = (stats.received, stats.sent);
        blocking(self.host(), move |conn| {
            Ok(peer_state::finished(conn, &peer, received, sent)?)
        })
        .await?;
        tracing::info!(target: "sync", received, sent, "session finished");
        self.shared.update(&self.peer, true, |l| {
            l.syncing = false;
            l.progress = None;
        });
        let _ = self.done.send(RoundDone {
            peer: self.peer.clone(),
            received: news(&stats),
        });
        Ok(())
    }
}

/// Runs the connection until it ends. `Ok` means it was closed on purpose; an error means the
/// line was lost or the other device sent something unusable.
pub(super) async fn run(mut conn: Conn) -> Result<(), SyncError> {
    let mut round = Some(conn.start_round().await?);
    let mut again = false;
    loop {
        tokio::select! {
            message = conn.transport.recv() => {
                let message = message?;
                let session = match round.take() {
                    Some(session) => session,
                    // Idle and spoken to: answer with our own Hello, then deal with theirs.
                    None if matches!(message, Message::Hello { .. }) => conn.start_round().await?,
                    None => {
                        return Err(SyncError::Protocol("a message out of turn".into()));
                    }
                };
                round = conn.feed(session, message).await?;
            }
            cmd = conn.cmds.recv() => match cmd {
                Some(ConnCmd::Round) => again = true,
                Some(ConnCmd::Close) | None => return Ok(()),
            },
            () = tokio::time::sleep(SILENCE), if round.is_some() => {
                return Err(SyncError::Disconnected);
            }
        }
        if round.is_none() {
            if !conn.keep_open {
                return Ok(());
            }
            if again {
                again = false;
                round = Some(conn.start_round().await?);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_that_merely_arrived_is_news_and_an_empty_round_is_not() {
        assert_eq!(news(&SessionStats::default()), 0);
        let only_activity = SessionStats {
            activity: 2,
            ..SessionStats::default()
        };
        assert!(news(&only_activity) > 0);
        let logged = SessionStats {
            logged: true,
            ..SessionStats::default()
        };
        assert!(news(&logged) > 0);
    }
}
