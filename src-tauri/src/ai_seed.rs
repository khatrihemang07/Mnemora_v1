//! The Sandbox seed: fills a Sandbox database with the recent-changes fixture, dated from today, so
//! that yesterday has data to read. Run through `examples/seed_sandbox.rs` (`make seed-sandbox`);
//! the app never calls it.
//!
//! Safety, in order, before anything is written: the target folders must not be inside Production's
//! data (its folder under the platform data root, or the folder its device file moved it to); the
//! database must not be in use by another connection; and it must be missing, empty, or hold exactly
//! what an earlier seed wrote. Anything else is refused unless `--reset` is given, which deletes the
//! Sandbox database files (never a Production file) and seeds again.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mnemora_core::db::{self, Connection};
use mnemora_core::device::{DeviceSettings, DeviceStore};
use mnemora_core::env::Environment;
use mnemora_core::local_time;
use mnemora_core::module::Module;
use mnemora_core::paths::AppPaths;
use mnemora_core::util::now_ms;
use mnemora_journal::NewEntry;
use mnemora_todo::{LocalNow, NewTaskDetails, NewWhen, RepeatRule};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::ai_fixture::{self, Driver};
use crate::state::lock;

/// The tables a seed writes rows to. A database whose rows in these tables are not exactly what an
/// earlier seed wrote holds the user's own data.
const DATA_TABLES: [&str; 7] = [
    "todo_lists",
    "todo_tasks",
    "todo_comments",
    "journal_entries",
    "ai_conversations",
    "ai_entries",
    "ai_saved_questions",
];
/// The table a finished seed records its row counts in; its presence marks a seeded database.
const MARKER_TABLE: &str = "sandbox_seed";
const MARKER_KEY: &str = "counts";

const USAGE: &str = "usage: make seed-sandbox ARGS=\"[--zone <IANA zone>] [--data-dir <folder>] [--base <folder>] [--reset]\"
  --zone      the zone the fixture is written in (default: this Mac's)
  --data-dir  the Sandbox data folder (default: the one the Sandbox app would use)
  --base      the platform data folder the Sandbox lives in (default: this user's)
  --reset     delete the Sandbox database and seed again, when it holds anything else";

/// What the seed found in the Sandbox database before it wrote anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbState {
    /// No database file yet.
    Missing,
    /// A database with no rows in the data tables.
    Empty,
    /// Holds exactly the rows an earlier seed wrote.
    Seeded,
    /// Holds rows that are not the fixture's.
    Other,
}

/// What `seed_at` does with a database in a given state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Write the fixture into the database as it is.
    Seed,
    /// Delete the Sandbox database files, then write the fixture.
    Reset,
    /// Write nothing: the fixture is already there.
    AlreadySeeded,
}

/// What a seed did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The fixture was written.
    Seeded {
        /// Fixture events written.
        events: usize,
        /// Keyed items written (Lists, Tasks and Entries).
        items: usize,
        /// Fixture events left out because they fall after the seed's `now`.
        skipped: usize,
        /// Rows per data table after the seed.
        counts: BTreeMap<String, i64>,
    },
    /// Nothing was written: the database already holds the fixture.
    AlreadySeeded,
}

/// The state the database was in, and what the seed did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub before: DbState,
    pub outcome: Outcome,
}

/// Command-line options.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Options {
    pub help: bool,
    pub base: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub zone: Option<String>,
    pub reset: bool,
}

/// Reads `--reset`, `--zone <zone>`, `--data-dir <folder>`, `--base <folder>` and `--help`.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => options.help = true,
            "--reset" => options.reset = true,
            "--base" => options.base = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--data-dir" => options.data_dir = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--zone" => options.zone = Some(value(&mut args, &arg)?),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(options)
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
}

/// This user's platform data folder, where the app's folders live: `~/Library/Application Support`
/// on macOS, `%APPDATA%` on Windows, `$XDG_DATA_HOME` or `~/.local/share` elsewhere.
pub fn platform_data_root() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        Ok(PathBuf::from(home).join("Library/Application Support"))
    }
    #[cfg(target_os = "windows")]
    {
        let roaming = std::env::var_os("APPDATA").ok_or("APPDATA is not set")?;
        Ok(PathBuf::from(roaming))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(data) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            return Ok(PathBuf::from(data));
        }
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        Ok(PathBuf::from(home).join(".local/share"))
    }
}

/// The folders that hold Production's data, read from the real platform data folder: Production's
/// own folder, and the data folder its device file moved it to, if it did. Reads only.
pub fn production_roots(real_base: &Path) -> Vec<PathBuf> {
    let production = AppPaths::resolve(Environment::Production, real_base, None);
    let mut roots = vec![real_base.join(Environment::Production.app_identifier())];
    if let Some(moved) = DeviceSettings::load(&production.device_file()).data_dir_override() {
        roots.push(moved);
    }
    roots
}

/// Refuses when any of `targets` lies inside one of `production`'s folders, after symlinks are
/// resolved, so a path that leads into Production by a link is refused too.
pub fn refuse_production(targets: &[&Path], production: &[PathBuf]) -> Result<(), String> {
    let roots: Vec<PathBuf> = production.iter().map(|root| resolved(root)).collect();
    for target in targets {
        let real = resolved(target);
        for (root, real_root) in production.iter().zip(&roots) {
            if real.starts_with(real_root) {
                return Err(format!(
                    "refused: {} is inside Production's data ({}). The seed never writes Production data.",
                    target.display(),
                    root.display()
                ));
            }
        }
    }
    Ok(())
}

/// `path` with symlinks resolved as far as it exists: the longest existing ancestor is made real and
/// the rest appended, so a folder that does not exist yet can still be compared with Production's.
fn resolved(path: &Path) -> PathBuf {
    let mut rest: Vec<&OsStr> = Vec::new();
    let mut at = path;
    loop {
        if let Ok(real) = fs::canonicalize(at) {
            return rest.iter().rev().fold(real, |acc, part| acc.join(part));
        }
        match (at.parent(), at.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name);
                at = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Looks at the database at `db_file` without changing it. Refuses (with a clear message) when another
/// connection is writing to it, since the seed would then compete with the app for the same rows.
pub fn inspect(db_file: &Path) -> Result<DbState, String> {
    if !db_file.is_file() {
        return Ok(DbState::Missing);
    }
    let Some(conn) = db::open_existing(db_file).map_err(err)? else {
        return Ok(DbState::Missing);
    };
    conn.busy_timeout(Duration::ZERO).map_err(err)?;
    if let Err(e) = conn.execute_batch("BEGIN IMMEDIATE; ROLLBACK;") {
        return Err(format!(
            "the Sandbox database is in use ({e}); quit the Sandbox app and try again"
        ));
    }
    let counts = count_rows(&conn)?;
    Ok(match marker(&conn)? {
        Some(recorded) if recorded == counts => DbState::Seeded,
        None if counts.values().all(|n| *n == 0) => DbState::Empty,
        _ => DbState::Other,
    })
}

/// What to do with a database in `state`. Refuses when it holds rows that are not the fixture's
/// unless `reset` is given.
pub fn plan(state: DbState, reset: bool) -> Result<Step, String> {
    match (state, reset) {
        (DbState::Missing | DbState::Empty, _) => Ok(Step::Seed),
        (DbState::Seeded, false) => Ok(Step::AlreadySeeded),
        (DbState::Seeded | DbState::Other, true) => Ok(Step::Reset),
        (DbState::Other, false) => Err(
            "the Sandbox database holds rows that are not the fixture, so nothing was written. \
             Pass --reset to delete the Sandbox database and seed it again."
                .to_owned(),
        ),
    }
}

/// Seeds the database of `paths` with the fixture, as of `now` in `zone`. `production` are the
/// folders of Production's data (see `production_roots`); a target inside any of them is refused
/// before anything else is looked at.
pub fn seed_at(
    paths: &AppPaths,
    production: &[PathBuf],
    now: i64,
    zone: &str,
    reset: bool,
) -> Result<Report, String> {
    refuse_production(
        &[
            paths.data_dir.as_path(),
            paths.config_dir.as_path(),
            paths.db_file.as_path(),
        ],
        production,
    )?;
    if local_time::day(now, zone).is_none() {
        return Err(format!("{zone} is not an IANA time zone; pass --zone"));
    }
    let before = inspect(&paths.db_file)?;
    let outcome = match plan(before, reset)? {
        Step::AlreadySeeded => Outcome::AlreadySeeded,
        step => {
            if step == Step::Reset {
                remove_database(&paths.db_file)?;
            }
            write_fixture(paths, now, zone)?
        }
    };
    Ok(Report { before, outcome })
}

/// Opens (creating) the database, migrates it with every Module as the app does at start, then
/// replays the fixture through the Modules' services and records the row counts.
fn write_fixture(paths: &AppPaths, now: i64, zone: &str) -> Result<Outcome, String> {
    let mut conn = db::open(&paths.db_file).map_err(err)?;
    let modules = crate::modules();
    let refs: Vec<&dyn Module> = modules.iter().map(AsRef::as_ref).collect();
    let report = db::migrate::run(&mut conn, &refs, &paths.backup_dir).map_err(err)?;
    let device = DeviceStore::new(
        paths.device_file(),
        DeviceSettings::load(&paths.device_file()),
    );
    for module in &modules {
        module
            .on_startup(&mut conn, &device, &report)
            .map_err(err)?;
    }

    let driver = SeedDriver::new(conn, Arc::new(AtomicI64::new(now)))?;
    let loaded = ai_fixture::load(&driver, now, zone)?;
    let mut recorded = Err("the database was not reached".to_owned());
    driver.with_db(&mut |conn| recorded = record_counts(conn));
    let counts = recorded?;
    Ok(Outcome::Seeded {
        events: loaded.events(),
        items: loaded.len(),
        skipped: loaded.skipped(),
        counts,
    })
}

/// Counts the data tables and writes the counts to the marker table, which marks the database as
/// seeded. Only `plan` and `inspect` read it.
fn record_counts(conn: &Connection) -> Result<BTreeMap<String, i64>, String> {
    let counts = count_rows(conn)?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {MARKER_TABLE} (key TEXT PRIMARY KEY, value TEXT NOT NULL)"
    ))
    .map_err(err)?;
    let text = serde_json::to_string(&counts).map_err(err)?;
    conn.execute(
        &format!("INSERT OR REPLACE INTO {MARKER_TABLE} (key, value) VALUES (?1, ?2)"),
        (MARKER_KEY, text),
    )
    .map_err(err)?;
    Ok(counts)
}

/// The recorded counts, when the database carries the marker.
fn marker(conn: &Connection) -> Result<Option<BTreeMap<String, i64>>, String> {
    if !table_exists(conn, MARKER_TABLE)? {
        return Ok(None);
    }
    let mut statement = conn
        .prepare(&format!("SELECT value FROM {MARKER_TABLE} WHERE key = ?1"))
        .map_err(err)?;
    let mut found = statement
        .query_map([MARKER_KEY], |row| row.get::<_, String>(0))
        .map_err(err)?;
    match found.next() {
        None => Ok(None),
        Some(text) => {
            let text = text.map_err(err)?;
            serde_json::from_str(&text).map(Some).map_err(err)
        }
    }
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .map_err(err)
}

/// Rows per data table; a table that does not exist yet counts as empty.
fn count_rows(conn: &Connection) -> Result<BTreeMap<String, i64>, String> {
    let mut counts = BTreeMap::new();
    for table in DATA_TABLES {
        let n: i64 = if table_exists(conn, table)? {
            conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .map_err(err)?
        } else {
            0
        };
        counts.insert(table.to_owned(), n);
    }
    Ok(counts)
}

/// Deletes the database file and its WAL and shared-memory files. Only ever called with the Sandbox
/// path, after `refuse_production` has passed.
fn remove_database(db_file: &Path) -> Result<(), String> {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = db_file.as_os_str().to_owned();
        name.push(suffix);
        match fs::remove_file(&name) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!(
                    "could not remove {}: {e}",
                    Path::new(&name).display()
                ));
            }
        }
    }
    Ok(())
}

/// Runs the seed from the command line and returns the process exit code: 0 when it wrote or found
/// the fixture already there, 1 when it refused or failed, 2 for bad arguments.
pub fn run_cli<I: IntoIterator<Item = String>>(args: I) -> i32 {
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("seed-sandbox: {message}\n{USAGE}");
            return 2;
        }
    };
    if options.help {
        println!("{USAGE}");
        return 0;
    }
    match run(&options) {
        Ok(text) => {
            println!("{text}");
            0
        }
        Err(message) => {
            eprintln!("seed-sandbox: {message}");
            1
        }
    }
}

fn run(options: &Options) -> Result<String, String> {
    let real_base = platform_data_root()?;
    let base = options.base.clone().unwrap_or_else(|| real_base.clone());
    let boot = AppPaths::resolve(Environment::Sandbox, &base, None);
    let data_dir = options
        .data_dir
        .clone()
        .or_else(|| DeviceSettings::load(&boot.device_file()).data_dir_override());
    let paths = AppPaths::resolve(Environment::Sandbox, &base, data_dir.as_deref());
    let zone = options
        .zone
        .clone()
        .or_else(system_zone)
        .ok_or("this Mac's time zone is not an IANA name; pass --zone")?;
    let now = now_ms();
    let production = production_roots(&real_base);
    let report = seed_at(&paths, &production, now, &zone, options.reset)?;
    Ok(describe(&paths, &production, now, &zone, &report))
}

/// This machine's IANA zone: `TZ` when it names one, else what `/etc/localtime` points at.
fn system_zone() -> Option<String> {
    if let Ok(tz) = std::env::var("TZ") {
        let tz = tz.trim_start_matches(':');
        if !tz.is_empty() {
            return Some(tz.to_owned());
        }
    }
    let target = fs::read_link("/etc/localtime").ok()?;
    let text = target.to_string_lossy().into_owned();
    text.split_once("zoneinfo/")
        .map(|(_, zone)| zone.to_owned())
}

/// The lines `run_cli` prints: where the seed wrote, what it found and what it did.
fn describe(
    paths: &AppPaths,
    production: &[PathBuf],
    now: i64,
    zone: &str,
    report: &Report,
) -> String {
    let when = local_time::minute(now, zone).unwrap_or_else(|| "unknown".to_owned());
    let checked: Vec<String> = production.iter().map(|p| p.display().to_string()).collect();
    let mut lines = vec![
        format!("Sandbox database: {}", paths.db_file.display()),
        format!(
            "  production folders checked, none touched: {}",
            checked.join(", ")
        ),
        format!("  zone {zone}, written as of {when}"),
        format!("  database before: {}", describe_state(report.before)),
    ];
    match &report.outcome {
        Outcome::Seeded {
            events,
            items,
            skipped,
            counts,
        } => {
            lines.push(format!(
                "  wrote {events} events as {items} items; {skipped} events after now were left out"
            ));
            let rows: Vec<String> = counts.iter().map(|(t, n)| format!("{t} {n}")).collect();
            lines.push(format!("  rows now: {}", rows.join(", ")));
        }
        Outcome::AlreadySeeded => lines.push(
            "  nothing written: the fixture is already there (pass --reset to seed it again)"
                .to_owned(),
        ),
    }
    lines.join("\n")
}

fn describe_state(state: DbState) -> &'static str {
    match state {
        DbState::Missing => "none (created)",
        DbState::Empty => "empty",
        DbState::Seeded => "the fixture",
        DbState::Other => "rows that are not the fixture (replaced: --reset)",
    }
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Drives the fixture's commands through the Modules' services, with no Tauri runtime. Each command
/// takes the same arguments the app's command takes, under the names the fixture loader sends.
struct SeedDriver {
    conn: Mutex<Connection>,
    clock: Arc<AtomicI64>,
}

impl SeedDriver {
    fn new(conn: Connection, clock: Arc<AtomicI64>) -> Result<Self, String> {
        let source = Arc::clone(&clock);
        db::set_time_source(&conn, Arc::new(move || source.load(Ordering::SeqCst))).map_err(err)?;
        Ok(Self {
            conn: Mutex::new(conn),
            clock,
        })
    }
}

impl Driver for SeedDriver {
    fn invoke(&self, command: &str, args: Value) -> Result<Value, Value> {
        let mut guard = lock(&self.conn);
        let db: &mut Connection = &mut guard;
        match command {
            "todo_list_create" => reply(mnemora_todo::create_list(
                db,
                &arg::<String>(&args, "name")?,
            )),
            "todo_task_create" => reply(mnemora_todo::create_task(
                db,
                &arg::<String>(&args, "title")?,
                arg::<Option<String>>(&args, "list_id")?.as_deref(),
                arg::<Option<NewWhen>>(&args, "when")?.as_ref(),
                arg::<Option<NewTaskDetails>>(&args, "details")?.as_ref(),
            )),
            "todo_task_set_when" => reply(mnemora_todo::set_when(
                db,
                &arg::<String>(&args, "id")?,
                arg::<Option<String>>(&args, "date")?.as_deref(),
                arg::<Option<String>>(&args, "time")?.as_deref(),
                arg::<Option<RepeatRule>>(&args, "repeat")?.as_ref(),
                &arg::<LocalNow>(&args, "local")?,
            )),
            "todo_task_set_priority" => reply(mnemora_todo::set_priority(
                db,
                &arg::<String>(&args, "id")?,
                arg::<u32>(&args, "priority")?,
            )),
            "todo_task_move" => reply(mnemora_todo::move_task(
                db,
                &arg::<String>(&args, "id")?,
                arg::<Option<String>>(&args, "listId")?.as_deref(),
            )),
            "todo_task_complete" => reply(mnemora_todo::complete_task(
                db,
                &arg::<String>(&args, "id")?,
                &arg::<LocalNow>(&args, "local")?,
            )),
            "todo_task_wont_do" => reply(mnemora_todo::wont_do(
                db,
                &arg::<String>(&args, "id")?,
                &arg::<LocalNow>(&args, "local")?,
            )),
            "todo_task_skip" => reply(mnemora_todo::skip_task(
                db,
                &arg::<String>(&args, "id")?,
                &arg::<LocalNow>(&args, "local")?,
            )),
            "todo_task_delete" => {
                reply(mnemora_todo::delete_task(db, &arg::<String>(&args, "id")?))
            }
            "todo_comment_add" => reply(mnemora_todo::add_comment(
                db,
                &arg::<String>(&args, "taskId")?,
                &arg::<String>(&args, "body")?,
            )),
            "journal_entry_create" => reply(mnemora_journal::create_entry(
                db,
                arg::<NewEntry>(&args, "entry")?,
            )),
            "journal_entry_update" => reply(mnemora_journal::update_entry(
                db,
                &arg::<String>(&args, "id")?,
                arg::<String>(&args, "text")?,
                &arg::<String>(&args, "sessionId")?,
            )),
            "journal_entry_delete" => reply(mnemora_journal::delete_entry(
                db,
                &arg::<String>(&args, "id")?,
            )),
            other => Err(json!({ "message": format!("the Sandbox seed has no command {other}") })),
        }
    }

    fn set_clock(&self, ms: i64) {
        self.clock.store(ms, Ordering::SeqCst);
    }

    fn with_db(&self, f: &mut dyn FnMut(&Connection)) {
        f(&lock(&self.conn));
    }
}

/// One named argument of a fixture command, or a refusal naming it.
fn arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<T, Value> {
    serde_json::from_value(args.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|e| json!({ "message": format!("{key}: {e}") }))
}

/// A service's result as the loader reads it: the serialised value, or the serialised error.
fn reply<T: Serialize, E: Serialize>(result: Result<T, E>) -> Result<Value, Value> {
    match result {
        Ok(value) => Ok(serde_json::to_value(value).unwrap_or(Value::Null)),
        Err(error) => {
            Err(serde_json::to_value(error).unwrap_or_else(|e| json!({ "message": e.to_string() })))
        }
    }
}
