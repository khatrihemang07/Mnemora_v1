//! The pairing exchange over one secured connection, from each end. It knows nothing about
//! sockets, windows or databases: the connection is a [`Channel`] and the rest comes through the
//! two port traits.
//!
//! 1. The typing device sends `Start` with its SPAKE2 message (so it speaks first) and the
//!    showing device answers `Reply`. Both now hold a key that is equal only if the codes were.
//! 2. Each proves it holds that key with an HMAC over the transcript, which includes both TLS
//!    public keys. That is what authenticates the keys: a device in the middle has a different
//!    key towards each end, so its proofs fail. A wrong code fails here, and counts as a try.
//! 3. After the proofs the channel is authenticated, and the devices swap who they are (ids,
//!    names, platform, Environment, data versions, the group's device list). Versions and
//!    Environments must match.
//! 4. The showing device asks its person Allow or Deny; the typing device waits.
//! 5. On Allow the showing device sends what it holds (`Allowed`). If both devices hold data the
//!    typing device shows the merge preview (`Reviewing`) and its person chooses Merge or Cancel
//!    while the showing device waits; if one is empty the merge goes straight ahead. Nothing is
//!    stored by either device until Merge.
//! 6. On Merge the typing device backs up its database and says `Go`; the showing device backs up
//!    its own, so both backups exist before either records the pairing (and so before any record
//!    is exchanged). A backup that fails refuses the pairing and nothing changes.
//! 7. The showing device records the pairing and says `Accepted`; the typing device records it
//!    and says `Done`.

use std::future::Future;
use std::time::Duration;

use mnemora_core::env::Environment;
use mnemora_core::syncable::Holdings;
use ring::hmac;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::timeout;

use super::Timing;
use super::host::{Paired, Stored};
use super::msg::{Msg, Reason, Who, hex, unhex};
use super::preview::needs_preview;
use super::view::Older;
use crate::SyncError;
use crate::net::link::HANDSHAKE_MAX_FRAME;
use crate::net::{Channel, PublicKey};

const ID_TYPER: &[u8] = b"mnemora pairing typer";
const ID_SHOWER: &[u8] = b"mnemora pairing shower";

/// Why an exchange ended without a pairing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Failure {
    /// The code did not match (typing side: the shower said so).
    WrongCode { tries_left: u8, name: String },
    /// One side said no, or the code ran out. Never `Reason::Environment` (see `Environment`).
    Refused {
        reason: Reason,
        name: Option<String>,
    },
    /// The other device runs the other Environment.
    Environment { theirs: Environment },
    /// Data versions differ.
    Version { name: String, older: Older },
    /// The connection failed, went quiet or spoke nonsense.
    Lost,
}

/// How the person answered the question on the showing device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decision {
    Allow,
    Deny,
    Expired,
    Cancelled,
    Busy,
}

/// What the typing device's person chose on the merge preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choice {
    Merge,
    Cancel,
}

/// What one proof from the typing device came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Attempt {
    /// The proof matched.
    Right,
    /// It did not; this many tries are left (0: the code is dead).
    Wrong(u8),
    /// The code died or was replaced meanwhile.
    Dead,
}

/// The typing device's side of what happens around the exchange.
pub(crate) trait TyperPort: Sync {
    /// The code was right: the shower is `shower` and its person is being asked.
    fn waiting(&self, shower: &Who);
    /// What this device holds.
    fn holdings(&self) -> Result<Holdings, SyncError>;
    /// Shows the merge preview and resolves when the person chooses (or the pairing is stopped,
    /// which counts as Cancel).
    fn review(
        &self,
        shower: &Who,
        mine: &Holdings,
        theirs: &Holdings,
    ) -> impl Future<Output = Choice> + Send;
    /// Backs up this device's database, before anything is merged.
    fn backup(&self, shower: &Who) -> Result<(), SyncError>;
    /// Records the pairing; the exchange finishes only if this succeeds.
    fn commit(&self, paired: &Paired) -> Result<Stored, SyncError>;
}

/// The showing device's side.
pub(crate) trait ShowerPort: Sync {
    /// The code that is valid right now, if any.
    fn code(&self) -> Option<String>;
    /// Checks one proof against the code on display, atomically with counting it: `proves` runs
    /// while the code's state is locked, so parallel connections cannot each get a look at the
    /// code after its last try was spent.
    fn attempt(&self, code: &str, proves: &dyn Fn() -> bool) -> Attempt;
    /// Asks the person; resolves when they answer, the code expires or pairing is cancelled.
    fn ask(&self, who: &Who) -> impl Future<Output = Decision> + Send;
    /// What this device holds.
    fn holdings(&self) -> Result<Holdings, SyncError>;
    /// The typing device's person is looking at the merge preview: shows that. Resolves when the
    /// person here cancels (or pairing is stopped).
    fn reviewing(&self) -> impl Future<Output = ()> + Send;
    /// Backs up this device's database, before anything is merged.
    fn backup(&self) -> Result<(), SyncError>;
    /// Records the pairing.
    fn commit(&self, paired: &Paired) -> Result<Stored, SyncError>;
}

fn lost(what: &str, error: impl std::fmt::Display) -> Failure {
    tracing::debug!(target: "sync", %error, "pairing {what} failed");
    Failure::Lost
}

async fn send<S>(ch: &mut Channel<S>, msg: &Msg) -> Result<(), Failure>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    ch.send(msg).await.map_err(|e| lost("send", e))
}

async fn recv<S>(ch: &mut Channel<S>, wait: Duration) -> Result<Msg, Failure>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    match timeout(wait, ch.recv_max::<Msg>(HANDSHAKE_MAX_FRAME)).await {
        Ok(Ok(msg)) => Ok(msg),
        Ok(Err(e)) => Err(lost("receive", e)),
        Err(_) => Err(lost("receive", "timed out")),
    }
}

/// The bytes both proofs are computed over: who is proving, both TLS keys and both SPAKE2 messages.
fn transcript(label: &str, typer: &PublicKey, shower: &PublicKey, a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(label.as_bytes());
    out.push(0);
    out.extend_from_slice(typer.as_bytes());
    out.extend_from_slice(shower.as_bytes());
    for part in [a, b] {
        out.extend_from_slice(&u32::try_from(part.len()).unwrap_or(u32::MAX).to_be_bytes());
        out.extend_from_slice(part);
    }
    out
}

fn prove(key: &[u8], data: &[u8]) -> String {
    hex(hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, key), data).as_ref())
}

fn proves(key: &[u8], data: &[u8], tag: &str) -> bool {
    unhex(tag)
        .is_some_and(|t| hmac::verify(&hmac::Key::new(hmac::HMAC_SHA256, key), data, &t).is_ok())
}

/// The Environment, data version and group rules. `Err` names what to tell the person.
pub(crate) fn compatible(me: &Who, them: &Who) -> Result<(), Failure> {
    if me.environment != them.environment {
        return Err(Failure::Environment {
            theirs: them.environment,
        });
    }
    let (mut mine_older, mut theirs_older) = (false, false);
    for module in me.data_versions.keys().chain(them.data_versions.keys()) {
        let (a, b) = (me.data_versions.get(module), them.data_versions.get(module));
        if a < b {
            mine_older = true;
        } else if b < a {
            theirs_older = true;
        }
    }
    if mine_older || theirs_older {
        return Err(Failure::Version {
            name: them.name.clone(),
            older: match (mine_older, theirs_older) {
                (true, true) => Older::Both,
                (true, false) => Older::Me,
                _ => Older::Them,
            },
        });
    }
    if let (Some(a), Some(b)) = (&me.group, &them.group)
        && a != b
    {
        return Err(Failure::Refused {
            reason: Reason::Groups,
            name: Some(them.name.clone()),
        });
    }
    Ok(())
}

/// What to tell the other end when this end refuses.
fn reason_for(failure: &Failure, mine: Environment) -> Reason {
    match failure {
        Failure::Refused { reason, .. } => *reason,
        Failure::Environment { .. } => Reason::Environment { sender: mine },
        _ => Reason::Version,
    }
}

/// What the other end's `Refuse` means here.
fn refused_by_peer(reason: Reason, name: Option<String>) -> Failure {
    match reason {
        Reason::Environment { sender } => Failure::Environment { theirs: sender },
        reason => Failure::Refused { reason, name },
    }
}

/// The typing device's end. `peer` is the showing device's TLS key; `me.key` is our own.
pub(crate) async fn type_code<S, P>(
    ch: &mut Channel<S>,
    port: &P,
    me: &Who,
    peer: PublicKey,
    code: &str,
    timing: &Timing,
) -> Result<Stored, Failure>
where
    S: AsyncRead + AsyncWrite + Unpin,
    P: TyperPort,
{
    let own = PublicKey::from_hex(&me.key).ok_or(Failure::Lost)?;
    let (spake, a_msg) = Spake2::<Ed25519Group>::start_a(
        &Password::new(code),
        &Identity::new(ID_TYPER),
        &Identity::new(ID_SHOWER),
    );
    send(
        ch,
        &Msg::Start {
            environment: me.environment,
            spake: hex(&a_msg),
        },
    )
    .await?;
    let b_msg = match recv(ch, timing.step).await? {
        Msg::Reply { spake } => unhex(&spake).ok_or(Failure::Lost)?,
        Msg::Refuse { reason } => return Err(refused_by_peer(reason, None)),
        _ => return Err(Failure::Lost),
    };
    let key = spake
        .finish(&b_msg)
        .map_err(|e| lost("key", format!("{e:?}")))?;
    let mine = transcript("typer", &own, &peer, &a_msg, &b_msg);
    send(
        ch,
        &Msg::Confirm {
            tag: prove(&key, &mine),
        },
    )
    .await?;
    let shower = match recv(ch, timing.step).await? {
        Msg::Confirmed { tag, who } => {
            let theirs = transcript("shower", &own, &peer, &a_msg, &b_msg);
            if !proves(&key, &theirs, &tag) || who.key != hex(peer.as_bytes()) {
                return Err(Failure::Lost);
            }
            *who
        }
        Msg::Wrong { tries_left, name } => return Err(Failure::WrongCode { tries_left, name }),
        Msg::Refuse { reason } => return Err(refused_by_peer(reason, None)),
        _ => return Err(Failure::Lost),
    };
    port.waiting(&shower);
    send(
        ch,
        &Msg::Identity {
            who: Box::new(me.clone()),
        },
    )
    .await?;
    // Our own checks come after the identity went out: the shower runs the same rules on it and
    // closes with the same reason, and its person sees our name.
    compatible(me, &shower)?;
    let wait = timing.code_life + timing.grace;
    let theirs = match recv(ch, wait).await? {
        Msg::Allowed { holdings } => holdings,
        Msg::Refuse { reason } => return Err(refused_by_peer(reason, Some(shower.name))),
        _ => return Err(Failure::Lost),
    };
    agree_to_merge(ch, port, &shower, &theirs, timing).await?;
    send(ch, &Msg::Go).await?;
    let group = match recv(ch, wait).await? {
        Msg::Accepted { group } => group,
        Msg::Refuse { reason } => return Err(refused_by_peer(reason, Some(shower.name))),
        _ => return Err(Failure::Lost),
    };
    if me.group.as_ref().is_some_and(|g| *g != group) {
        return Err(Failure::Refused {
            reason: Reason::Groups,
            name: Some(shower.name),
        });
    }
    let stored = port
        .commit(&Paired { who: shower, group })
        .map_err(|e| lost("store", e))?;
    // The pairing is recorded here either way; this only tells the other end to record it too.
    let _ = send(ch, &Msg::Done).await;
    Ok(stored)
}

/// The typing device's part between `Allowed` and `Go`: the preview when both devices hold data
/// (the person may cancel), then this device's backup.
async fn agree_to_merge<S, P>(
    ch: &mut Channel<S>,
    port: &P,
    shower: &Who,
    theirs: &Holdings,
    timing: &Timing,
) -> Result<(), Failure>
where
    S: AsyncRead + AsyncWrite + Unpin,
    P: TyperPort,
{
    let mine = port.holdings().map_err(|e| lost("count", e))?;
    if needs_preview(&mine, theirs) {
        send(ch, &Msg::Reviewing).await?;
        let choice = tokio::select! {
            c = timeout(timing.confirm, port.review(shower, &mine, theirs)) => {
                c.unwrap_or(Choice::Cancel)
            }
            m = ch.recv_max::<Msg>(HANDSHAKE_MAX_FRAME) => {
                return Err(match m {
                    Ok(Msg::Refuse { reason }) => refused_by_peer(reason, Some(shower.name.clone())),
                    _ => Failure::Lost,
                });
            }
        };
        if choice == Choice::Cancel {
            send(
                ch,
                &Msg::Refuse {
                    reason: Reason::Cancelled,
                },
            )
            .await?;
            // `name: None`: this device's own person stopped it.
            return Err(Failure::Refused {
                reason: Reason::Cancelled,
                name: None,
            });
        }
    }
    if let Err(e) = port.backup(shower) {
        tracing::warn!(target: "sync", error = %e, "could not back up before the first merge");
        send(
            ch,
            &Msg::Refuse {
                reason: Reason::BackupFailed,
            },
        )
        .await?;
        return Err(Failure::Refused {
            reason: Reason::BackupFailed,
            name: None,
        });
    }
    Ok(())
}

/// The showing device's end. `peer` is the typing device's TLS key; `me.key` is our own.
pub(crate) async fn show_code<S, P>(
    ch: &mut Channel<S>,
    port: &P,
    me: &Who,
    peer: PublicKey,
    timing: &Timing,
) -> Result<Stored, Failure>
where
    S: AsyncRead + AsyncWrite + Unpin,
    P: ShowerPort,
{
    let typer = check_typer(ch, port, me, peer, timing).await?;
    // The typing device sends nothing more until it is answered, so a message or a close here
    // means it left.
    let decision = tokio::select! {
        d = port.ask(&typer) => d,
        _ = ch.recv_max::<Msg>(HANDSHAKE_MAX_FRAME) => return Err(Failure::Lost),
    };
    let refused = match decision {
        Decision::Allow => None,
        Decision::Deny => Some(Reason::Declined),
        Decision::Expired => Some(Reason::Expired),
        Decision::Cancelled => Some(Reason::Cancelled),
        Decision::Busy => Some(Reason::Busy),
    };
    if let Some(reason) = refused {
        send(ch, &Msg::Refuse { reason }).await?;
        return Err(Failure::Refused {
            reason,
            name: Some(typer.name),
        });
    }
    send(
        ch,
        &Msg::Allowed {
            holdings: port.holdings().map_err(|e| lost("count", e))?,
        },
    )
    .await?;
    // The typing device either shows the preview or says Go; its person may take a while.
    let mut answer = recv(ch, timing.step).await?;
    if answer == Msg::Reviewing {
        let cancelled = port.reviewing();
        tokio::select! {
            m = recv(ch, timing.confirm + timing.grace) => answer = m?,
            () = cancelled => {
                send(ch, &Msg::Refuse { reason: Reason::Cancelled }).await?;
                return Err(Failure::Refused {
                    reason: Reason::Cancelled,
                    name: Some(typer.name),
                });
            }
        }
    }
    match answer {
        Msg::Go => {}
        Msg::Refuse { reason } => return Err(refused_by_peer(reason, Some(typer.name))),
        _ => return Err(Failure::Lost),
    }
    if let Err(e) = port.backup() {
        tracing::warn!(target: "sync", error = %e, "could not back up before the first merge");
        send(
            ch,
            &Msg::Refuse {
                reason: Reason::BackupFailed,
            },
        )
        .await?;
        return Err(Failure::Refused {
            reason: Reason::BackupFailed,
            name: None,
        });
    }
    let group = me
        .group
        .clone()
        .or_else(|| typer.group.clone())
        .unwrap_or_else(mnemora_core::util::new_id);
    let stored = port
        .commit(&Paired {
            who: typer,
            group: group.clone(),
        })
        .map_err(|e| lost("store", e))?;
    send(ch, &Msg::Accepted { group }).await?;
    // Whether the other end got it is its business; this end is paired.
    let _ = recv(ch, timing.step).await;
    Ok(stored)
}

/// The showing device's first half: SPAKE2, the proofs (a wrong code is counted here), and the
/// typing device's identity once it has proved itself. Returns that identity if it may pair.
async fn check_typer<S, P>(
    ch: &mut Channel<S>,
    port: &P,
    me: &Who,
    peer: PublicKey,
    timing: &Timing,
) -> Result<Who, Failure>
where
    S: AsyncRead + AsyncWrite + Unpin,
    P: ShowerPort,
{
    let own = PublicKey::from_hex(&me.key).ok_or(Failure::Lost)?;
    let (environment, a_msg) = match recv(ch, timing.step).await? {
        Msg::Start { environment, spake } => (environment, unhex(&spake).ok_or(Failure::Lost)?),
        _ => return Err(Failure::Lost),
    };
    if environment != me.environment {
        let reason = Reason::Environment {
            sender: me.environment,
        };
        send(ch, &Msg::Refuse { reason }).await?;
        return Err(Failure::Environment {
            theirs: environment,
        });
    }
    let Some(code) = port.code() else {
        return refuse_expired(ch).await;
    };
    let (spake, b_msg) = Spake2::<Ed25519Group>::start_b(
        &Password::new(&code),
        &Identity::new(ID_TYPER),
        &Identity::new(ID_SHOWER),
    );
    send(ch, &Msg::Reply { spake: hex(&b_msg) }).await?;
    let key = spake
        .finish(&a_msg)
        .map_err(|e| lost("key", format!("{e:?}")))?;
    let Msg::Confirm { tag } = recv(ch, timing.step).await? else {
        return Err(Failure::Lost);
    };
    // From here the attempt costs a try, and the code may have died while we waited.
    let data = transcript("typer", &peer, &own, &a_msg, &b_msg);
    match port.attempt(&code, &|| proves(&key, &data, &tag)) {
        Attempt::Dead => return refuse_expired(ch).await,
        Attempt::Wrong(tries_left) => {
            let name = me.name.clone();
            send(
                ch,
                &Msg::Wrong {
                    tries_left,
                    name: name.clone(),
                },
            )
            .await?;
            return Err(Failure::WrongCode { tries_left, name });
        }
        Attempt::Right => {}
    }
    send(
        ch,
        &Msg::Confirmed {
            tag: prove(&key, &transcript("shower", &peer, &own, &a_msg, &b_msg)),
            who: Box::new(me.clone()),
        },
    )
    .await?;
    let typer = match recv(ch, timing.step).await? {
        Msg::Identity { who } if who.key == hex(peer.as_bytes()) => *who,
        _ => return Err(Failure::Lost),
    };
    if let Err(failure) = compatible(me, &typer) {
        send(
            ch,
            &Msg::Refuse {
                reason: reason_for(&failure, me.environment),
            },
        )
        .await?;
        return Err(failure);
    }
    Ok(typer)
}

async fn refuse_expired<S, T>(ch: &mut Channel<S>) -> Result<T, Failure>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    send(
        ch,
        &Msg::Refuse {
            reason: Reason::Expired,
        },
    )
    .await?;
    Err(Failure::Refused {
        reason: Reason::Expired,
        name: None,
    })
}
