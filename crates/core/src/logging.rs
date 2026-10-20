//! Tracing setup: daily rolling files, a console layer (which is logcat on Android), 7-day pruning.

use std::path::Path;
use std::time::{Duration, SystemTime};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry, fmt, reload};

use crate::error::{AppError, AppResult, ErrorCode};

/// File-name prefix of rolling logs; pruning only ever touches files with it.
pub const LOG_PREFIX: &str = "mnemora";
/// Log files older than this are deleted at startup.
pub const RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);

const LEVELS: [&str; 5] = ["error", "warn", "info", "debug", "trace"];

/// Keeps the file writer alive and allows changing the level at runtime.
/// Dropping it flushes pending log lines.
pub struct LogHandle {
    _guard: WorkerGuard,
    filter: reload::Handle<EnvFilter, Registry>,
}

impl LogHandle {
    /// Applies a new level (`error` to `trace`) immediately, without a restart.
    pub fn set_level(&self, level: &str) -> AppResult<()> {
        let level = normalize_level(level)?;
        self.filter.reload(EnvFilter::new(level)).map_err(|e| {
            AppError::new(ErrorCode::Internal, "Could not change the log level").with_details(e)
        })
    }
}

impl LogHandle {
    /// A handle connected to no subscriber, for tests that need an `AppState` without installing
    /// the process-global subscriber. `set_level` on it fails.
    pub fn detached() -> Self {
        let (_writer, guard) = tracing_appender::non_blocking(std::io::sink());
        let (_layer, filter) = reload::Layer::new(EnvFilter::new("info"));
        Self {
            _guard: guard,
            filter,
        }
    }
}

/// Lower-cases and checks a level name.
pub fn normalize_level(level: &str) -> AppResult<String> {
    let lower = level.trim().to_ascii_lowercase();
    if LEVELS.contains(&lower.as_str()) {
        Ok(lower)
    } else {
        Err(AppError::new(ErrorCode::Validation, "Unknown log level").with_details(level))
    }
}

/// Prunes old files, then installs the global subscriber. `RUST_LOG` overrides `level`
/// for developers. Call once per process.
pub fn init(log_dir: &Path, level: &str) -> AppResult<LogHandle> {
    std::fs::create_dir_all(log_dir)?;
    if let Err(e) = prune_old_logs(log_dir, SystemTime::now(), RETENTION) {
        // Pruning is housekeeping; never block startup on it. eprintln because the subscriber
        // is only installed further down in this function.
        eprintln!("log pruning failed: {e}");
    }

    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix(LOG_PREFIX)
        .filename_suffix("log")
        .build(log_dir)
        .map_err(|e| AppError::new(ErrorCode::Io, "Could not open log file").with_details(e))?;
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let level = normalize_level(level).unwrap_or_else(|_| "info".to_owned());
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let (filter, filter_handle) = reload::Layer::new(env_filter);

    // Human-readable: timestamp, level, target (module path), message.
    let file_layer = fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true);
    // On Android, stderr is already forwarded to logcat (tag RustStdoutStderr); a second logcat
    // layer would duplicate every line, and ANSI colours show up as garbage there.
    let console_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(!cfg!(target_os = "android"))
        .with_target(true);

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(console_layer);

    subscriber.try_init().map_err(|e| {
        AppError::new(ErrorCode::Internal, "Logging was already initialised").with_details(e)
    })?;
    Ok(LogHandle {
        _guard: guard,
        filter: filter_handle,
    })
}

/// Deletes log files in `dir` last modified more than `max_age` before `now`.
/// Returns how many were removed. Files without the log prefix are never touched.
pub fn prune_old_logs(dir: &Path, now: SystemTime, max_age: Duration) -> AppResult<usize> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.into()),
    };
    let mut removed = 0;
    for entry in entries.filter_map(Result::ok) {
        if !entry.file_name().to_string_lossy().starts_with(LOG_PREFIX) {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        // A clock set backwards yields Err; keep the file rather than guess.
        if now.duration_since(modified).is_ok_and(|age| age > max_age)
            && std::fs::remove_file(entry.path()).is_ok()
        {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;

    fn touch(dir: &Path, name: &str, age: Duration, now: SystemTime) {
        let f = File::create(dir.join(name)).unwrap();
        f.set_modified(now - age).unwrap();
    }

    const DAY: Duration = Duration::from_secs(86_400);

    #[test]
    fn prunes_only_old_log_files() {
        let dir = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        touch(dir.path(), "mnemora.2026-01-01.log", 10 * DAY, now);
        touch(dir.path(), "mnemora.2026-01-05.log", 8 * DAY, now);
        touch(dir.path(), "mnemora.2026-01-09.log", 6 * DAY, now);
        touch(dir.path(), "mnemora.2026-01-14.log", Duration::ZERO, now);
        touch(dir.path(), "unrelated.txt", 30 * DAY, now);

        assert_eq!(prune_old_logs(dir.path(), now, RETENTION).unwrap(), 2);

        let mut left: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            vec![
                "mnemora.2026-01-09.log",
                "mnemora.2026-01-14.log",
                "unrelated.txt"
            ]
        );
    }

    #[test]
    fn prune_tolerates_a_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            prune_old_logs(&dir.path().join("none"), SystemTime::now(), RETENTION).unwrap(),
            0
        );
    }

    #[test]
    fn level_names_are_normalised_and_checked() {
        assert_eq!(normalize_level(" DEBUG ").unwrap(), "debug");
        assert_eq!(
            normalize_level("nope").unwrap_err().code,
            ErrorCode::Validation
        );
    }

    // The only test that installs the process-global subscriber.
    #[test]
    fn init_writes_readable_lines_and_level_can_change() {
        let dir = tempfile::tempdir().unwrap();
        let handle = init(dir.path(), "info").unwrap();
        tracing::info!(target: "mnemora_core::test", "hello from test");
        tracing::debug!("hidden at info");
        handle.set_level("debug").unwrap();
        tracing::debug!("visible at debug");
        assert!(handle.set_level("loud").is_err());
        drop(handle); // flushes the non-blocking writer

        let file = std::fs::read_dir(dir.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let text = std::fs::read_to_string(file).unwrap();
        assert!(
            text.contains("INFO mnemora_core::test: hello from test"),
            "{text}"
        );
        assert!(!text.contains("hidden at info"));
        assert!(text.contains("visible at debug"));
    }
}
