//! The Sync Module: keeps a person's own devices in step, directly between them (ADR 0017). Off by
//! default and inert while off: its startup returns at once and nothing listens, advertises or
//! runs. It never imports the Journal or Todo.

#![forbid(unsafe_code)]

mod addresses;
mod apply;
mod attention;
mod clocks;
mod conflict_view;
mod conflicts;
mod devices;
pub mod discovery;
mod error;
mod held;
mod live;
mod merge;
mod migrations;
mod name;
pub mod net;
mod network;
pub mod pair;
mod peer_state;
mod peers;
mod registry;
mod resolutions;
mod resolved_view;
mod runtime;
mod session;
mod state;
mod stream;
mod textmerge;
mod texts;
pub mod transport;
mod vector;
mod wire;

use mnemora_core::error::{AppError, AppResult, ErrorCode};
use mnemora_core::module::Module;
use mnemora_core::settings::SettingsRegistry;
use mnemora_core::syncable::{Applied, RemoteChange, SyncEntity};
use rusqlite::Connection;
use rusqlite_migration::Migrations;

pub use addresses::{is_tailscale, local as local_addresses, shareable as shareable_address};
pub use attention::{Attention, LastSynced, UNSEEN_DAYS, UnseenDevice, attention};
pub use conflict_view::{
    ClashChoice, ClashPiece, ConflictChoice, ConflictList, ConflictRow, ConflictSide,
    ConflictSubject, ConflictView, ConflictWrite, TaskText, draft as conflict_draft,
    finish as conflict_finish, list as conflict_list, plan as conflict_plan, view as conflict_view,
};
pub use conflicts::{Resolution as ConflictResolution, open_count as conflict_count};
pub use devices::{DevicesView, ThisDevice, view as devices_view};
pub use error::SyncError;
pub use held::{Released, release_ready as release_held};
pub use live::{Host, Pace, PeerLive, Progress, Schedule};
pub use name::{MAX_NAME_CHARS, default_device_name, validate_name};
pub use network::{Network, NetworkOptions};
pub use pair::msg::{Member, Who};
pub use pair::{
    Older, Paired, Pairing, PairingError, PairingHost, PairingView, Platform, Stored, Timing,
};
pub use peer_state::Route;
pub use peers::{
    PeerRow, forget_all, group, list as list_peers, members, paired_count, pinned, store_paired,
};
pub use registry::{Modules, Registry};
pub use resolutions::{
    UsePlan, finish_use as resolution_finish_use, mark_visited as resolutions_visited,
    plan_use as resolution_plan_use,
};
pub use resolved_view::{ResolvedIcon, ResolvedKind, ResolvedRow};
pub use runtime::{RuntimeProbe, SyncRuntime};
pub use session::{Endpoint, Session, SessionStats, pump};
pub use state::{SyncOption, SyncState, current, rename, set_enabled, set_option};
pub use stream::{BATCH_BYTES, BATCH_RECORDS};
pub use textmerge::DiffSpan;
pub use tokio::sync::Notify;
pub use transport::Transport;
pub use wire::{Message, Vector, WireRecord};

/// The Sync Module.
pub struct SyncModule;

/// Module id: also the migration namespace and the change-log module name.
pub const MODULE_ID: &str = "sync";

impl Module for SyncModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    /// Off until the user turns it on, on each device separately.
    fn default_enabled(&self) -> bool {
        false
    }

    fn migrations(&self) -> Migrations<'static> {
        migrations::all()
    }

    /// What Sync syncs about itself: Conflicts and Resolved automatically.
    fn sync_entities(&self) -> &'static [SyncEntity] {
        &[conflicts::CONFLICT, resolutions::RESOLUTION]
    }

    fn apply_remote(&self, conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
        match change.entity {
            conflicts::ENTITY => conflicts::apply(conn, change),
            resolutions::ENTITY => resolutions::apply(conn, change),
            _ => Err(AppError::new(
                ErrorCode::Internal,
                "Sync does not sync that kind of record",
            )),
        }
    }

    /// Sync has no Preferences: its choices are Device settings (`DeviceSettings::sync`).
    fn register_settings(&self, _reg: &mut SettingsRegistry) {}

    // `on_startup` stays the trait's no-op: with Sync off there is nothing to start, and the app
    // shell starts `SyncRuntime` only when the Device setting says it is on.
}
