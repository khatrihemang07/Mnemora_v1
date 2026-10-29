//! The run itself: the command line, the copy of the Sandbox the run works on, and each variant's
//! questions asked on that copy. The Sandbox is never written.

use std::path::Path;
use std::sync::{Arc, Mutex};

use mnemora_ai::{KeyStore, LocalContext, Prepared, Toolset};
use mnemora_core::db::{self, Connection};
use mnemora_core::local_time;
use mnemora_core::module::Module;
use mnemora_core::util::now_ms;

use super::answer::{Answer, answer};
use super::args::{Args, USAGE, parse_args};
use super::dump::dump_request;
use super::knobs::{Off, Variant, toolset_for};
use super::provider::{Ablated, default_model, pick_provider};
use super::questions::{BUILT_IN_QUESTIONS, load_questions};
use super::report::{run_setup, write_reports};
use crate::ai::offered_tools_in;
use crate::ai_seed::{self, DbState};
use crate::qa_cli::exit_code;
use crate::state::lock;

/// The copy of the Sandbox database a run works on, in `--out`. Removed when the run ends.
const COPY_NAME: &str = "qa-sandbox-copy.db";

/// The refusal when the copy's catch-up day does not match `--fixture-skip-catchup`, one for each way
/// it can be wrong.
const CATCHUP_NOT_LEFT_OUT: &str = "--fixture-skip-catchup needs a Sandbox seeded without the catch-up day: seed one with `make seed-sandbox ARGS=\"--skip-catchup ...\"` and run again.";
const CATCHUP_LEFT_OUT: &str = "The Sandbox was seeded without the catch-up day, so the run needs --fixture-skip-catchup, which says so in the report.";

/// The refusal when the Sandbox has no Provider, and what to do about it.
const NO_PROVIDER: &str = "The Sandbox has no AI Provider configured, so no Run can be made. Add one in the Sandbox app (AI, then Providers: a kind, a name and an address; Anthropic and OpenRouter also need a key, which the app saves), use it once so a Model is recorded, then run this again.";

/// Runs the harness from the command line and returns the process exit code: 0 when the reports
/// were written, 1 when it refused or failed, 2 for bad arguments.
pub fn run_cli<I: IntoIterator<Item = String>>(args: I) -> i32 {
    exit_code(
        "ai_qa",
        USAGE,
        parse_args(args),
        |options| options.help,
        execute,
    )
}

/// Asks the questions on a copy of the Sandbox and writes the reports. Returns the lines to print.
fn execute(options: &Args) -> Result<String, String> {
    let out = options
        .out
        .clone()
        .ok_or("--out <folder> is needed: the reports are written there")?;
    let questions: Vec<String> = match &options.questions {
        Some(file) => {
            let text =
                std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
            load_questions(&text)?
        }
        None => BUILT_IN_QUESTIONS.iter().map(|q| (*q).to_owned()).collect(),
    };
    let real_base = ai_seed::platform_data_root()?;
    let (paths, production) = ai_seed::sandbox_paths(
        &real_base,
        options.base.as_deref(),
        options.data_dir.as_deref(),
    )?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    ai_seed::refuse_production(&[out.as_path()], &production)?;
    match ai_seed::inspect(&paths.db_file)? {
        DbState::Seeded => {}
        other => {
            return Err(format!(
                "the Sandbox database is {other:?}, not the seeded fixture; run `make seed-sandbox` first"
            ));
        }
    }
    let copy = out.join(COPY_NAME);
    copy_database(&paths.db_file, &copy)?;
    let result = run_on_copy(
        options,
        &paths.config_dir,
        &paths.db_file,
        &copy,
        &questions,
        &out,
    );
    let removed = ai_seed::remove_database(&copy);
    let written = result?;
    removed?;
    Ok(written)
}

/// The run itself, on the copy: the Provider and Model, the calendar, then each variant's
/// questions. Writes the reports and returns the lines to print.
fn run_on_copy(
    options: &Args,
    config_dir: &Path,
    sandbox_db: &Path,
    copy: &Path,
    questions: &[String],
    out: &Path,
) -> Result<String, String> {
    let left_out = ai_seed::leaves_out_catchup(copy)?;
    if left_out != options.off.catchup {
        return Err(if options.off.catchup {
            CATCHUP_NOT_LEFT_OUT.to_owned()
        } else {
            CATCHUP_LEFT_OUT.to_owned()
        });
    }
    let conn = db::open(copy).map_err(|e| e.to_string())?;
    let keys = KeyStore::new(config_dir.join(crate::AI_KEYS_DIR));
    let providers = mnemora_ai::list_providers(&conn, &keys).map_err(|e| e.message)?;
    if providers.is_empty() {
        return Err(NO_PROVIDER.to_owned());
    }
    let usage = mnemora_ai::model_usage(&conn).map_err(|e| e.message)?;
    let chosen = pick_provider(&providers, options.provider.as_deref(), &usage)?;
    let model = match &options.model {
        Some(model) => model.clone(),
        None => default_model(&usage, &chosen.id).ok_or_else(|| {
            format!(
                "the Sandbox has no Model used with {}; pass --model",
                chosen.name
            )
        })?,
    };
    let prepared =
        mnemora_ai::prepare_run(&conn, &keys, &chosen.id, &model).map_err(|e| e.message)?;
    let zone = options
        .zone
        .clone()
        .or_else(ai_seed::system_zone)
        .ok_or("this Mac's time zone is not an IANA name; pass --zone")?;
    let now = now_ms();
    let today =
        local_time::day(now, &zone).ok_or_else(|| format!("{zone} is not an IANA time zone"))?;
    let weekday = weekday_of(&today).ok_or("today could not be read as a day")?;
    let week_start = mnemora_core::settings::stored(&conn, "core", "week_start")
        .map_err(|e| e.message)?
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "mon".to_owned());
    let local = LocalContext {
        today,
        weekday: weekday.to_owned(),
        time_zone: zone,
        week_start,
    };
    let setup = run_setup(
        &prepared.record,
        &prepared.choice,
        &local,
        sandbox_db,
        questions.len(),
    );
    let modules = crate::modules();
    let db = Mutex::new(conn);
    let mut lines = vec![
        format!(
            "Provider: {} ({}), Model: {}",
            prepared.record.name,
            prepared.record.kind.label(),
            model
        ),
        format!(
            "Calendar: {} {}, zone {}",
            setup.today, local.time_zone, local.week_start
        ),
    ];
    let mut per_variant: Vec<(Variant, Vec<Answer>)> = Vec::new();
    for &variant in &options.variants {
        let off = variant.off().with(options.off);
        let answers = ask_questions(
            &db,
            &prepared,
            &modules,
            off,
            questions,
            &local,
            options.dump_first_on_zero_calls,
        )?;
        per_variant.push((variant, answers));
    }
    drop(db);
    if options.dump {
        let first = per_variant
            .first()
            .and_then(|(_, answers)| answers.first())
            .and_then(|answer| answer.last_request.as_ref());
        if let Some(request) = first {
            let path = out.join("dump-last-request.md");
            let text = format!(
                "Switched off for this run: {}\n\nThe last request of the first question of the first variant.\n\n{}",
                options.off.describe(),
                dump_request(request)
            );
            std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
            lines.push(format!("Wrote {}", path.display()));
        }
    }
    lines.extend(write_reports(out, &setup, &per_variant, options.off)?);
    Ok(lines.join("\n"))
}

/// Asks each question as a new Conversation with the Tools and knobs `off` names. With `dump_first`,
/// a Run that made no Tool call on its first Step keeps that first request for its report.
fn ask_questions(
    db: &Mutex<Connection>,
    prepared: &Prepared,
    modules: &[Box<dyn Module>],
    off: Off,
    questions: &[String],
    local: &LocalContext,
    dump_first: bool,
) -> Result<Vec<Answer>, String> {
    let mut answers = Vec::new();
    for question in questions {
        let toolset = {
            let conn = lock(db);
            let (tools, modules_off) = offered_tools_in(&conn, modules).map_err(|e| e.message)?;
            toolset_for(
                off,
                Toolset {
                    tools,
                    off: modules_off,
                },
            )
        };
        let ablated = Arc::new(Ablated::new(
            mnemora_ai::build(
                &prepared.record,
                &prepared.choice.model,
                prepared.key.clone(),
            ),
            off,
        ));
        let mut reply = answer(
            db,
            ablated.clone(),
            &prepared.choice,
            question,
            local,
            toolset,
        );
        reply.last_request = ablated.last_sent();
        if dump_first && reply.first_step_calls == 0 {
            reply.first_request = ablated.first_sent();
        }
        answers.push(reply);
    }
    Ok(answers)
}

/// Copies the database at `source` to `target` with `VACUUM INTO`, which reads the source and writes
/// only the new file. Any earlier copy at `target` is removed first.
pub fn copy_database(source: &Path, target: &Path) -> Result<(), String> {
    ai_seed::remove_database(target)?;
    let conn = db::open_existing(source)
        .map_err(|e| e.to_string())?
        .ok_or("the Sandbox database is missing")?;
    let quoted = target.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{quoted}'"))
        .map_err(|e| e.to_string())
}

/// The English weekday of a local day (`YYYY-MM-DD`), or none when the text is not a day.
pub fn weekday_of(day: &str) -> Option<&'static str> {
    const NAMES: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    // Sakamoto's method: 0 is Sunday.
    const OFFSET: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut parts = day.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: usize = parts.next()?.parse().ok()?;
    let date: i32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&date) {
        return None;
    }
    let year = if month < 3 { year - 1 } else { year };
    let index =
        (year + year / 4 - year / 100 + year / 400 + OFFSET[month - 1] + date).rem_euclid(7);
    NAMES.get(usize::try_from(index).ok()?).copied()
}
