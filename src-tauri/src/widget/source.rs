//! Which database connection the bridge uses: the running app's own, or (when the widget starts
//! the process before the app does) one opened here that never creates a database and never
//! migrates (ADR 0014).

use std::path::PathBuf;
use std::sync::Mutex;

use mnemora_core::db::{self, Connection};
use mnemora_core::device::DeviceSettings;
use mnemora_core::env::Environment;
use mnemora_core::error::{AppError, ErrorCode};
use mnemora_core::module::Module;
use mnemora_core::paths::AppPaths;
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event;

use super::{Outcome, Status};
use crate::TodoChangedEvent;
use crate::env_guard;
use crate::state::{AppState, lock};

/// Something that can run a job on the Todo database.
pub trait Source {
    /// Runs `job` with the connection, or says why there is none.
    fn run(&self, job: &mut dyn FnMut(&mut Connection) -> Result<Value, AppError>) -> Outcome;

    /// Like [`run`](Source::run), for a job that changes Todo data. A running app is told, so its
    /// screens update at once.
    fn write(&self, job: &mut dyn FnMut(&mut Connection) -> Result<Value, AppError>) -> Outcome {
        self.run(job)
    }

    /// Settings ▸ Theme, which lives in the device file: `system`, `light`, `dark` or `oled`.
    fn app_theme(&self) -> String;
}

/// The saved theme if it is one the widget knows, else `system`.
fn known_theme(saved: Option<String>) -> String {
    match saved.as_deref() {
        Some(t @ ("system" | "light" | "dark" | "oled")) => t.to_owned(),
        _ => "system".to_owned(),
    }
}

/// The running app: its connection, shared with the commands.
pub struct Live<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Live<R> {
    /// The bridge over `app`'s own connection.
    pub fn new(app: AppHandle<R>) -> Self {
        Self(app)
    }

    /// Whether the app has finished starting (its state exists), so its connection can be used.
    pub fn is_ready(&self) -> bool {
        self.0.try_state::<AppState>().is_some()
    }
}

impl<R: Runtime> Source for Live<R> {
    fn run(&self, job: &mut dyn FnMut(&mut Connection) -> Result<Value, AppError>) -> Outcome {
        let Some(state) = self.0.try_state::<AppState>() else {
            return Outcome::Status(Status::NotSetUp);
        };
        let mut conn = lock(&state.db);
        into_outcome(job(&mut conn))
    }

    fn app_theme(&self) -> String {
        let saved = self
            .0
            .try_state::<AppState>()
            .and_then(|state| state.device.get().look.theme);
        known_theme(saved)
    }

    fn write(&self, job: &mut dyn FnMut(&mut Connection) -> Result<Value, AppError>) -> Outcome {
        let outcome = self.run(job);
        if matches!(outcome, Outcome::Ok(_))
            && let Err(e) = TodoChangedEvent.emit(&self.0)
        {
            tracing::warn!(error = %e, "could not emit the Todo changed event");
        }
        outcome
    }
}

fn into_outcome(result: Result<Value, AppError>) -> Outcome {
    match result {
        Ok(value) => Outcome::Ok(value),
        Err(e) => Outcome::Err(e),
    }
}

/// No app in this process: the database is found the way app start-up finds it, opened only if
/// it exists and only used if every Module's migrations are current.
pub struct Standalone {
    env: Environment,
    /// The Android data dir (`context.dataDir`), the equivalent of Tauri's `data_dir`.
    base: PathBuf,
    /// Kept once it has been checked, so the next call skips the checks.
    conn: Mutex<Option<Connection>>,
}

impl Standalone {
    /// A bridge for `env` over the app data directory `base`.
    pub fn new(env: Environment, base: PathBuf) -> Self {
        Self {
            env,
            base,
            conn: Mutex::new(None),
        }
    }

    /// Opens and checks the database: a connection, or the reason there is none.
    fn open(&self) -> Result<Connection, Outcome> {
        env_guard::check(self.env, "", &self.base.to_string_lossy(), true)
            .map_err(|e| Outcome::Err(AppError::new(ErrorCode::Internal, e)))?;
        // The same two steps as app start-up: the device file first (it can move the data).
        let boot = AppPaths::resolve(self.env, &self.base, None);
        let device = DeviceSettings::load(&boot.device_file());
        let paths = AppPaths::resolve(self.env, &self.base, device.data_dir_override().as_deref());
        let Some(conn) = db::open_existing(&paths.db_file).map_err(Outcome::Err)? else {
            return Err(Outcome::Status(Status::NotSetUp));
        };
        let modules = crate::modules();
        let refs: Vec<&dyn Module> = modules.iter().map(AsRef::as_ref).collect();
        if db::migrate::is_current(&conn, &refs).map_err(Outcome::Err)? {
            Ok(conn)
        } else {
            Err(Outcome::Status(Status::NeedsUpdate))
        }
    }
}

impl Source for Standalone {
    fn run(&self, job: &mut dyn FnMut(&mut Connection) -> Result<Value, AppError>) -> Outcome {
        let mut cached = lock(&self.conn);
        if cached.is_none() {
            match self.open() {
                Ok(conn) => *cached = Some(conn),
                Err(outcome) => return outcome,
            }
        }
        let Some(conn) = cached.as_mut() else {
            return Outcome::Err(AppError::new(ErrorCode::Internal, "No connection"));
        };
        let result = job(conn);
        // A broken connection (file replaced, disk error) is reopened on the next call.
        if result
            .as_ref()
            .is_err_and(|e| matches!(e.code, ErrorCode::Database | ErrorCode::Io))
        {
            *cached = None;
        }
        into_outcome(result)
    }

    fn app_theme(&self) -> String {
        let boot = AppPaths::resolve(self.env, &self.base, None);
        known_theme(DeviceSettings::load(&boot.device_file()).look.theme)
    }
}
