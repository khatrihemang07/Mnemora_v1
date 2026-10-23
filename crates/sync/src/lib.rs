//! The Sync Module: keeps a person's own devices in step, directly between them (ADR 0017). Off by
//! default and inert while off: its startup returns at once and nothing listens, advertises or
//! runs. It never imports the Journal or Todo.

#![forbid(unsafe_code)]

mod activity;
mod activity_view;
mod address;
mod addresses;
mod apply;
mod attention;
mod check;
mod clocks;
mod conflict_view;
mod conflicts;
mod detail;
mod devices;
pub mod discovery;
mod error;
mod held;
mod introduce;
mod live;
mod merge;
mod migrations;
mod name;
pub mod net;
mod network;
pub mod pair;
mod pause;
mod peer_state;
mod peers;
mod registry;
mod rename;
mod resolutions;
mod resolved_view;
mod revoke;
mod runtime;
mod session;
mod state;
mod stream;
mod textmerge;
mod texts;
pub mod transport;
mod vector;
mod wire;

use mnemora_core::db::migrate::MigrationReport;
use mnemora_core::device::DeviceStore;
use mnemora_core::error::{AppError, AppResult, ErrorCode};
use mnemora_core::module::Module;
use mnemora_core::settings::SettingsRegistry;
use mnemora_core::syncable::{Applied, RemoteChange, SyncEntity};
use rusqlite::Connection;
use rusqlite_migration::Migrations;

pub use activity::{
    Kind as ActivityKind, Local, Why, log_failed, log_forgot, log_left, log_paused, log_renamed,
    log_resumed,
};
pub use activity_view::{
    ActivityFilter, ActivityLink, ActivityPage, ActivityRow, Piece, Viewer as ActivityViewer,
    page as activity_page,
};
pub use address::{Address, AddressError};
pub use addresses::{is_tailscale, local as local_addresses, shareable as shareable_address};
pub use attention::{Attention, LastSynced, UNSEEN_DAYS, UnseenDevice, attention};
pub use check::{CheckKind, CheckResult, Expect as CheckExpect, check as check_address};
pub use conflict_view::{
    ClashChoice, ClashPiece, ConflictChoice, ConflictList, ConflictRow, ConflictSide,
    ConflictSubject, ConflictView, ConflictWrite, TaskText, draft as conflict_draft,
    finish as conflict_finish, list as conflict_list, plan as conflict_plan, view as conflict_view,
};
pub use conflicts::{Resolution as ConflictResolution, open_count as conflict_count};
pub use detail::{
    DeviceDetail, KnownAddress, ModuleVersion, detail as device_detail, set_typed_address,
    typed_for_dial,
};
pub use devices::{DevicesView, ThisDevice, view as devices_view};
pub use error::SyncError;
pub use held::{Released, release_ready as release_held};
pub use introduce::introduce as introduce_members;
pub use live::{Host, Pace, PeerLive, Progress, Schedule};
pub use name::{MAX_NAME_CHARS, default_device_name, validate_name};
pub use network::{Network, NetworkOptions};
pub use pair::msg::{Member, Who};
pub use pair::{
    Older, Paired, Pairing, PairingError, PairingHost, PairingView, Platform, Stored, Timing,
};
pub use pause::{MAX_AHEAD_MS, Pause};
pub use peer_state::{Route, worked as remember_worked};
pub use peers::{
    PeerRow, forget_all, group, list as list_peers, members, name_of as peer_name, paired_count,
    pinned, store_paired,
};
pub use registry::{Modules, Registry};
pub use rename::{
    adopt_local_addresses, check_own_name, record_own as record_own_name, rename_peer,
};
pub use resolutions::{
    UsePlan, finish_use as resolution_finish_use, mark_visited as resolutions_visited,
    plan_use as resolution_plan_use,
};
pub use resolved_view::{ResolvedIcon, ResolvedKind, ResolvedRow};
pub use revoke::{
    Forgotten, forget as forget_peer, leave as announce_leaving, lift as lift_revocation,
    revoked_keys,
};
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

    /// What Sync syncs about itself: Conflicts, Resolved automatically, Activity, who was forgotten
    /// and what the devices are called.
    fn sync_entities(&self) -> &'static [SyncEntity] {
        &[
            conflicts::CONFLICT,
            resolutions::RESOLUTION,
            activity::ACTIVITY,
            revoke::REVOCATION,
            rename::DEVICE,
            introduce::INTRODUCTION,
        ]
    }

    fn apply_remote(&self, conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
        match change.entity {
            conflicts::ENTITY => conflicts::apply(conn, change),
            resolutions::ENTITY => resolutions::apply(conn, change),
            activity::ENTITY => activity::apply(conn, change),
            revoke::ENTITY => revoke::apply(conn, change),
            rename::ENTITY => rename::apply(conn, change),
            introduce::ENTITY => introduce::apply(conn, change),
            _ => Err(AppError::new(
                ErrorCode::Internal,
                "Sync does not sync that kind of record",
            )),
        }
    }

    /// Sync has no Preferences: its choices are Device settings (`DeviceSettings::sync`).
    fn register_settings(&self, _reg: &mut SettingsRegistry) {}

    /// Addresses typed before they synced join the log (once). The network itself is started by
    /// the app shell, only when the Device setting says Sync is on.
    fn on_startup(
        &self,
        conn: &mut Connection,
        _device: &DeviceStore,
        _report: &MigrationReport,
    ) -> AppResult<()> {
        rename::adopt_local_addresses(conn)
            .map_err(|e| AppError::new(ErrorCode::Internal, e.to_string()))
    }
}
