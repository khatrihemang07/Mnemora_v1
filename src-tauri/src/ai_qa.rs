//! The headless QA harness: real AI Runs over a temporary copy of the Sandbox's seeded data, written
//! to markdown reports, with no app window and no browser. `examples/ai_qa.rs` runs it (`--features
//! seed`). A question goes through the path `ai_run_start` takes: the same Provider adapter, Tools,
//! system prompt, calendar and final-Step note. It differs in three ways: it runs on the
//! calling thread, it does not claim the Conversation, and it reads the answer from the saved
//! entries. The Sandbox itself is never written: each run works on a copy, so no Conversation is
//! left behind and the seed stays valid.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use mnemora_ai::{
    Ask, Choice, EntryPayload, Handles, KeyStore, LocalContext, ModelUsage, ProviderRecord,
    ProviderView, RunEvent, RunOutcome, Steering, Toolset, Wording, WrapperProvider,
};
use mnemora_core::db::{self, Connection};
use mnemora_core::local_time;
use mnemora_core::module::Module;
use mnemora_core::util::{new_id, now_ms};
use mnemora_harness::agent::FINAL_NOTE;
use mnemora_harness::{Control, Message, Provider, Request, StreamEvent, ToolProtocol};
use serde_json::Value;

use crate::ai::offered_tools_in;
use crate::ai_seed::{self, DbState};

/// The questions asked when `--questions` is not given.
pub const BUILT_IN_QUESTIONS: [&str; 10] = [
    "What did I do yesterday?",
    "What did I get done last week?",
    "What's overdue?",
    "What did I complete yesterday versus what did I only create?",
    "What was on my mind in my journal this week?",
    "What happened on Wednesday 21 October?",
    "Hi",
    "What exactly did I write in the Entry about the dog barking?",
    "Any updates on the passport?",
    "How many tasks did I finish this week?",
];

/// The change Tools the before variant leaves out. They are the only Tools that differ.
pub const BEFORE_ONLY_TOOLS: [&str; 2] = ["journal_changes", "todo_changes"];

/// The file the report of a variant is written to, by its label.
const SIDE_BY_SIDE: &str = "qa-side-by-side.md";
/// The copy of the Sandbox database a run works on, in `--out`. Removed when the run ends.
const COPY_NAME: &str = "qa-sandbox-copy.db";

const USAGE: &str =
    "usage: cargo run -p mnemora-app --features seed --example ai_qa -- --out <folder> [options]
  --out <folder>      where the reports are written (required)
  --variant <v>       before, after or both (default: both)
  --provider <name>   the Provider to ask (default: the Sandbox's only one, or the one used last)
  --model <id>        the Model id (default: the Model last used with that Provider)
  --zone <zone>       the calendar's IANA zone (default: this Mac's)
  --questions <file>  a JSON list of questions (default: the built-in ten)
  --data-dir <folder> the Sandbox data folder (default: the one the Sandbox app would use)
  --base <folder>     the platform data folder the Sandbox lives in (default: this user's)";

/// The reports' variant: which Tools and which prompt the Run is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// Without `journal_changes` and `todo_changes`, and without the intent paragraph.
    Before,
    /// The app's own Tools and prompt.
    After,
}

impl Variant {
    /// The name used in the report's file name and headings.
    pub fn label(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }

    /// What the variant changes, for the report's header.
    fn describe(self) -> &'static str {
        match self {
            Self::Before => {
                "Tools: every Tool the app offers except `journal_changes` and `todo_changes`. \
                 System prompt: without the intent paragraph (\"Work out what the person wants \
                 before you call a Tool\"). The final-Step note is kept."
            }
            Self::After => "The current code: every Tool the app offers, and the app's prompt.",
        }
    }
}

/// Makes the toolset of `variant` from the app's toolset. The before variant drops exactly the two
/// change Tools and switches the prompt to its wording without the intent paragraph; the after
/// variant is the app's own toolset.
pub fn variant_toolset(variant: Variant, toolset: Toolset) -> Toolset {
    toolset_for(variant_off(variant), toolset)
}

/// The knobs a variant turns off: the before variant has no change Tools and no intent paragraph.
fn variant_off(variant: Variant) -> Off {
    match variant {
        Variant::After => Off::default(),
        Variant::Before => Off {
            intent: true,
            changes: true,
            ..Off::default()
        },
    }
}

/// The toolset with the parts `off` switches off: the change Tools (`changes`) and the intent
/// paragraph (`intent`, through the wording). Every other part is kept as it is.
pub fn toolset_for(off: Off, toolset: Toolset) -> Toolset {
    let tools = if off.changes {
        toolset
            .tools
            .into_iter()
            .filter(|tool| !BEFORE_ONLY_TOOLS.contains(&tool.spec().name))
            .collect()
    } else {
        toolset.tools
    };
    Toolset {
        tools,
        off: toolset.off,
        wording: if off.intent {
            Wording::WithoutIntent
        } else {
            Wording::Full
        },
    }
}

/// What a QA run switches off, one knob each. All false is the app's own behaviour. `final_note` is
/// the final-Step note; `intent` is the intent paragraph; `changes` is `journal_changes` and
/// `todo_changes`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Off {
    pub final_note: bool,
    pub intent: bool,
    pub changes: bool,
}

impl Off {
    /// Reads a comma-separated list of knob names: `final`, `intent`, `changes`. Empty is
    /// the app's behaviour.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut off = Self::default();
        for name in text
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            match name {
                "final" => off.final_note = true,
                "intent" => off.intent = true,
                "changes" => off.changes = true,
                other => {
                    return Err(format!("--off takes final, intent or changes, not {other}"));
                }
            }
        }
        Ok(off)
    }

    /// The knobs on, for a report's header.
    pub fn describe(self) -> String {
        let on: Vec<&str> = [
            (self.final_note, "final-Step note"),
            (self.intent, "intent paragraph"),
            (self.changes, "journal_changes and todo_changes Tools"),
        ]
        .iter()
        .filter(|(set, _)| *set)
        .map(|(_, name)| *name)
        .collect();
        if on.is_empty() {
            "nothing (the app's own behaviour)".to_owned()
        } else {
            on.join(", ")
        }
    }

    /// Both sets of knobs together.
    fn with(self, other: Self) -> Self {
        Self {
            final_note: self.final_note || other.final_note,
            intent: self.intent || other.intent,
            changes: self.changes || other.changes,
        }
    }
}

/// The request with what `off` switches off removed: the final-Step note at the end of the system
/// prompt. Everything else is kept.
pub fn strip(request: &Request, off: Off) -> Request {
    let mut out = request.clone();
    if off.final_note {
        let stripped = out
            .system
            .strip_suffix(&format!("\n{FINAL_NOTE}"))
            .map(str::to_owned);
        if let Some(system) = stripped {
            out.system = system;
        }
    }
    out
}

/// A Provider that removes what `off` switches off from each request before the real adapter sees
/// it, and keeps a copy of each request it sent. QA only: the app never wraps its Provider.
pub struct Ablated {
    inner: Arc<dyn Provider>,
    off: Off,
    sent: Mutex<Vec<Request>>,
}

impl Ablated {
    pub fn new(inner: Arc<dyn Provider>, off: Off) -> Self {
        Self {
            inner,
            off,
            sent: Mutex::new(Vec::new()),
        }
    }

    /// The request this Provider sent last, with its knobs applied.
    pub fn last_sent(&self) -> Option<Request> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .last()
            .cloned()
    }
}

impl Provider for Ablated {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }

    fn tool_protocol(&self) -> ToolProtocol {
        self.inner.tool_protocol()
    }

    fn stream(
        &self,
        request: &Request,
        halt: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(StreamEvent),
    ) {
        let request = strip(request, self.off);
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        self.inner.stream(&request, halt, emit);
    }
}

/// The request as the model is sent it, for reading: the system prompt, every message in order, the
/// Tool specs, and the Wrapper's wire body (the same function the adapter calls).
pub fn dump_request(request: &Request) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "### System prompt, as the harness sends it\n\n````text\n{}\n````\n",
        request.system
    );
    let _ = writeln!(out, "### Messages, oldest first\n");
    for (index, message) in request.messages.iter().enumerate() {
        let _ = match message {
            Message::User(text) => writeln!(
                out,
                "#### [{index}] user, the question\n\n````text\n{text}\n````\n"
            ),
            Message::Assistant { text, calls } => {
                let calls: Vec<String> = calls
                    .iter()
                    .map(|call| format!("{} {}", call.name, call.arguments))
                    .collect();
                writeln!(
                    out,
                    "#### [{index}] assistant, a Step that asked for Tools\n\ntext: {text:?}\ncalls: {}\n",
                    calls.join("; ")
                )
            }
            Message::Tool {
                call_id,
                name,
                text,
                is_error,
            } => writeln!(
                out,
                "#### [{index}] tool result for {name} (call {call_id}), refused: {is_error}, as the model reads it\n\n````text\n{text}\n````\n"
            ),
            Message::Correction(text) => {
                writeln!(out, "#### [{index}] correction\n\n````text\n{text}\n````\n")
            }
        };
    }
    let _ = writeln!(out, "### Tool specs offered this Step\n");
    for tool in &request.tools {
        let _ = writeln!(
            out,
            "- {}: {}\n  schema: {}",
            tool.name, tool.about, tool.schema
        );
    }
    let wire = WrapperProvider::body(request, request.tools.is_empty());
    let _ = writeln!(
        out,
        "\n### Wire body, as the Wrapper adapter builds it\n\n````json\n{}\n````",
        serde_json::to_string_pretty(&wire).unwrap_or_default()
    );
    out
}

/// The questions in a JSON list of strings, which `--questions` reads.
pub fn load_questions(text: &str) -> Result<Vec<String>, String> {
    serde_json::from_str::<Vec<String>>(text)
        .map_err(|e| format!("the questions file must be a JSON list of strings: {e}"))
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

/// One Tool call of a Run, as the report shows it: its arguments and the size of its result, with
/// the row count for the change Tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallLine {
    /// The Tool's name.
    pub name: String,
    /// The arguments the Model sent (JSON text).
    pub arguments: String,
    /// The result's size in bytes, observation label included.
    pub result_bytes: usize,
    /// Whether the Tool refused the call.
    pub is_error: bool,
    /// The rows a change Tool returned across its days; none for other Tools or a refusal.
    pub rows: Option<usize>,
}

/// One question's Run: its Steps, Tool calls, final answer, the app's note and how long it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The question as asked.
    pub question: String,
    /// Model requests (Steps) the Run made.
    pub steps: usize,
    /// The Tool calls in order.
    pub calls: Vec<CallLine>,
    /// The final answer's Markdown; empty when the Run gave none.
    pub text: String,
    /// What the app would tell the person beside the answer, if anything.
    pub app_note: Option<String>,
    /// Wall-clock time of the Run.
    pub elapsed: Duration,
    /// The last request the Provider was sent in this Run, for the dump. None when it sent none.
    pub last_request: Option<Request>,
}

impl Answer {
    /// A question that could not be run at all: no Steps, and the reason as the app note.
    pub fn failed(question: &str, note: &str) -> Self {
        Self {
            question: question.to_owned(),
            steps: 0,
            calls: Vec::new(),
            text: String::new(),
            app_note: Some(note.to_owned()),
            elapsed: Duration::ZERO,
            last_request: None,
        }
    }
}

/// What a report says about the run it covers: the Provider and Model (names only, never a key),
/// the calendar and the database the copy came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The Provider's name and kind.
    pub provider: String,
    /// The Model id.
    pub model: String,
    /// The calendar's zone.
    pub zone: String,
    /// The day and weekday the calendar says is today.
    pub today: String,
    /// The first day of the week the calendar uses.
    pub week_start: String,
    /// The Sandbox database the copy was made from.
    pub sandbox_db: PathBuf,
    /// How many questions each variant asks.
    pub questions: usize,
}

/// The facts of a run, from the Provider the Run uses and its calendar. The key is not among them.
pub fn facts(
    record: &ProviderRecord,
    choice: &Choice,
    local: &LocalContext,
    sandbox_db: &Path,
    questions: usize,
) -> Facts {
    Facts {
        provider: format!("{} ({})", record.name, record.kind.label()),
        model: choice.model.clone(),
        zone: local.time_zone.clone(),
        today: format!("{} {}", local.weekday, local.today),
        week_start: local.week_start.clone(),
        sandbox_db: sandbox_db.to_path_buf(),
        questions,
    }
}

fn lock(db: &Mutex<Connection>) -> MutexGuard<'_, Connection> {
    db.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The rows a change Tool returned across its days, from the JSON after the observation line.
fn rows_of(name: &str, result: &str) -> Option<usize> {
    if !BEFORE_ONLY_TOOLS.contains(&name) {
        return None;
    }
    let (_, body) = result.split_once('\n')?;
    let value: Value = serde_json::from_str(body).ok()?;
    let days = value.get("days")?.as_array()?;
    Some(
        days.iter()
            .filter_map(|day| day.get("rows")?.as_array().map(Vec::len))
            .sum(),
    )
}

/// Asks one question as a new Conversation, through the same steps `ai_run_start` takes, and reads
/// the answer back from the saved entries. The Run is on this thread; `db` is locked only while a
/// row is read or written, as in the app.
pub fn answer(
    db: &Mutex<Connection>,
    provider: Arc<dyn Provider>,
    choice: &Choice,
    question: &str,
    local: &LocalContext,
    toolset: Toolset,
) -> Answer {
    let ask = Ask {
        conversation_id: new_id(),
        continues: false,
        text: question.to_owned(),
        local: local.clone(),
        choice: choice.clone(),
        branch_of: None,
    };
    let started = Instant::now();
    let question_id = match mnemora_ai::begin(db, &ask) {
        Ok(id) => id,
        Err(error) => return Answer::failed(question, &error.message),
    };
    let control = Control::new();
    let steering = Steering::new();
    let mut events = Vec::new();
    mnemora_ai::execute(
        db,
        provider,
        &ask,
        &question_id,
        toolset,
        Handles {
            control: &control,
            steering: &steering,
        },
        &mut |event| events.push(event),
    );
    let elapsed = started.elapsed();
    let steps = events
        .iter()
        .filter(|event| matches!(event, RunEvent::StepStarted))
        .count();
    let outcome = events.iter().rev().find_map(|event| match event {
        RunEvent::RunEnded { outcome } => Some(outcome.clone()),
        _ => None,
    });
    let view = match mnemora_ai::conversation(&lock(db), &ask.conversation_id, false) {
        Ok(view) => view,
        Err(error) => return Answer::failed(question, &error.message),
    };
    let mut calls = Vec::new();
    let mut text = String::new();
    let mut budget = None;
    for entry in &view.entries {
        match &entry.payload {
            EntryPayload::ToolResult {
                name,
                arguments,
                result,
                is_error,
                ..
            } => calls.push(CallLine {
                name: name.clone(),
                arguments: arguments.clone(),
                result_bytes: result.len(),
                is_error: *is_error,
                rows: rows_of(name, result),
            }),
            EntryPayload::Assistant {
                text: reply,
                budget_note,
                ..
            } => {
                text.clone_from(reply);
                budget.clone_from(budget_note);
            }
            _ => {}
        }
    }
    let app_note = match outcome {
        Some(RunOutcome::Failed { message, .. }) => Some(message),
        Some(RunOutcome::Stopped) => Some("The Run was stopped.".to_owned()),
        _ => budget,
    };
    Answer {
        question: question.to_owned(),
        steps,
        calls,
        text,
        app_note,
        elapsed,
        last_request: None,
    }
}

/// The report of one variant: its header, then each question's Steps, Tool calls, answer and note.
pub fn report_markdown(variant: Variant, facts: &Facts, answers: &[Answer]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# AI QA report: {} variant\n", variant.label());
    let _ = writeln!(out, "{}\n", variant.describe());
    let _ = writeln!(out, "{}\n", header_lines(facts));
    for (index, answer) in answers.iter().enumerate() {
        let _ = writeln!(out, "## {}. {}\n", index + 1, answer.question);
        let _ = writeln!(out, "- Steps (Model requests): {}", answer.steps);
        let _ = writeln!(out, "- Elapsed: {}", seconds(answer.elapsed));
        let note = answer.app_note.as_deref().unwrap_or("none");
        let _ = writeln!(out, "- App note: {note}\n");
        if answer.calls.is_empty() {
            let _ = writeln!(out, "Tool calls: none\n");
        } else {
            let _ = writeln!(out, "Tool calls:\n");
            for call in &answer.calls {
                let _ = writeln!(out, "- `{}`", call.name);
                let _ = writeln!(out, "  - Arguments: `{}`", call.arguments);
                let _ = writeln!(
                    out,
                    "  - Result: {} bytes{}",
                    call.result_bytes,
                    status(call)
                );
            }
            out.push('\n');
        }
        let _ = writeln!(out, "Answer:\n");
        let body = if answer.text.trim().is_empty() {
            "(no answer)"
        } else {
            answer.text.trim()
        };
        let _ = writeln!(out, "{body}\n");
    }
    out
}

/// The header lines every report starts with: the Provider and Model by name, the calendar, and
/// where the copy came from.
fn header_lines(facts: &Facts) -> String {
    [
        format!("- Provider: {}", facts.provider),
        format!("- Model: {}", facts.model),
        format!(
            "- Calendar: today is {}, zone {}, weeks start {}",
            facts.today, facts.zone, facts.week_start
        ),
        format!("- Questions per variant: {}", facts.questions),
        format!(
            "- Sandbox database (copied, the copy was run): {}",
            facts.sandbox_db.display()
        ),
    ]
    .join("\n")
}

fn status(call: &CallLine) -> String {
    let refused = if call.is_error { ", refused" } else { "" };
    match call.rows {
        Some(rows) => format!("{refused}, {rows} rows"),
        None => refused.to_owned(),
    }
}

fn seconds(elapsed: Duration) -> String {
    format!("{:.1} s", elapsed.as_secs_f64())
}

/// The variants side by side: each question with both variants' Steps, Tool calls, elapsed time and
/// note, then both answers.
pub fn summary_markdown(facts: &Facts, before: &[Answer], after: &[Answer]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# AI QA: before and after, side by side\n");
    let _ = writeln!(
        out,
        "Before: every Tool except `journal_changes` and `todo_changes`, and the prompt without the intent paragraph. After: the current code. Both run the same questions on the same copy.\n"
    );
    let _ = writeln!(out, "{}\n", header_lines(facts));
    for (index, (left, right)) in before.iter().zip(after).enumerate() {
        let _ = writeln!(out, "## {}. {}\n", index + 1, left.question);
        let _ = writeln!(out, "| | Before | After |");
        let _ = writeln!(out, "|---|---|---|");
        let _ = writeln!(out, "| Steps | {} | {} |", left.steps, right.steps);
        let _ = writeln!(
            out,
            "| Tool calls | {} | {} |",
            calls_cell(left),
            calls_cell(right)
        );
        let _ = writeln!(
            out,
            "| Elapsed | {} | {} |",
            seconds(left.elapsed),
            seconds(right.elapsed)
        );
        let _ = writeln!(
            out,
            "| App note | {} | {} |\n",
            left.app_note.as_deref().unwrap_or("none"),
            right.app_note.as_deref().unwrap_or("none")
        );
        let _ = writeln!(out, "### Before answer\n\n{}\n", answer_or_none(left));
        let _ = writeln!(out, "### After answer\n\n{}\n", answer_or_none(right));
    }
    out
}

fn calls_cell(answer: &Answer) -> String {
    if answer.calls.is_empty() {
        return "none".to_owned();
    }
    answer
        .calls
        .iter()
        .map(|call| {
            let rows = call.rows.map(|r| format!(", {r} rows")).unwrap_or_default();
            format!("{} ({} B{rows})", call.name, call.result_bytes)
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn answer_or_none(answer: &Answer) -> &str {
    let text = answer.text.trim();
    if text.is_empty() { "(no answer)" } else { text }
}

/// Command-line options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub help: bool,
    pub base: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub zone: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub variants: Vec<Variant>,
    pub questions: Option<PathBuf>,
    pub out: Option<PathBuf>,
    /// The knobs switched off for every variant (`--off`).
    pub off: Off,
    /// Whether to write the last request of the first question to a dump file (`--dump-last`).
    pub dump: bool,
}

/// Reads the flags. `--variant` is `before`, `after` or `both` (the default).
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Args, String> {
    let mut options = Args {
        help: false,
        base: None,
        data_dir: None,
        zone: None,
        provider: None,
        model: None,
        variants: vec![Variant::Before, Variant::After],
        questions: None,
        out: None,
        off: Off::default(),
        dump: false,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => options.help = true,
            "--base" => options.base = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--data-dir" => options.data_dir = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--zone" => options.zone = Some(value(&mut args, &arg)?),
            "--provider" => options.provider = Some(value(&mut args, &arg)?),
            "--model" => options.model = Some(value(&mut args, &arg)?),
            "--questions" => options.questions = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--out" => options.out = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--off" => options.off = Off::parse(&value(&mut args, &arg)?)?,
            "--dump-last" => options.dump = true,
            "--variant" => {
                options.variants = match value(&mut args, &arg)?.as_str() {
                    "before" => vec![Variant::Before],
                    "after" => vec![Variant::After],
                    "both" => vec![Variant::Before, Variant::After],
                    other => {
                        return Err(format!(
                            "--variant takes before, after or both, not {other}"
                        ));
                    }
                };
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(options)
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
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

/// The Provider a run asks: the one `--provider` names, else the Sandbox's only one, else the one
/// used most recently. Refused when there is none, or when the choice is not clear.
fn pick_provider<'a>(
    providers: &'a [ProviderView],
    wanted: Option<&str>,
    usage: &ModelUsage,
) -> Result<&'a ProviderView, String> {
    if let Some(wanted) = wanted {
        return providers
            .iter()
            .find(|p| p.id == wanted || p.name.eq_ignore_ascii_case(wanted))
            .ok_or_else(|| format!("the Sandbox has no Provider called {wanted}"));
    }
    if let [only] = providers {
        return Ok(only);
    }
    usage
        .recent
        .iter()
        .find_map(|used| providers.iter().find(|p| p.id == used.provider_id))
        .ok_or_else(|| {
            "the Sandbox has several Providers and none was used recently; pass --provider"
                .to_owned()
        })
}

/// The Model a run asks when `--model` is not given: the one last used with the Provider.
fn default_model(usage: &ModelUsage, provider_id: &str) -> Option<String> {
    usage
        .recent
        .iter()
        .chain(&usage.most_used)
        .find(|used| used.provider_id == provider_id)
        .map(|used| used.model.clone())
}

const NO_PROVIDER: &str = "The Sandbox has no AI Provider configured, so no Run can be made. Add one in the Sandbox app (AI, then Providers: a kind, a name and an address; Anthropic and OpenRouter also need a key, which the app saves), use it once so a Model is recorded, then run this again.";

/// Runs the harness from the command line and returns the process exit code: 0 when the reports
/// were written, 1 when it refused or failed, 2 for bad arguments.
pub fn run_cli<I: IntoIterator<Item = String>>(args: I) -> i32 {
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("ai_qa: {message}\n{USAGE}");
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
            eprintln!("ai_qa: {message}");
            1
        }
    }
}

/// Asks the questions on a copy of the Sandbox and writes the reports. Returns the lines to print.
fn run(options: &Args) -> Result<String, String> {
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

/// Asks each question as a new Conversation in `variant`, with the Tools that variant gives.
fn ask_questions(
    db: &Mutex<Connection>,
    prepared: &mnemora_ai::Prepared,
    modules: &[Box<dyn Module>],
    off: Off,
    questions: &[String],
    local: &LocalContext,
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
                    wording: Wording::Full,
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
        answers.push(reply);
    }
    Ok(answers)
}

/// Writes one report per variant, and the side-by-side summary when both ran. Returns the lines
/// that say where they went.
fn write_reports(
    out: &Path,
    facts: &Facts,
    per_variant: &[(Variant, Vec<Answer>)],
    off: Off,
) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let switched = format!("Switched off for this run: {}\n\n", off.describe());
    for (variant, answers) in per_variant {
        let path = out.join(format!("qa-{}.md", variant.label()));
        std::fs::write(
            &path,
            format!("{switched}{}", report_markdown(*variant, facts, answers)),
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
        lines.push(format!("Wrote {}", path.display()));
    }
    if let [(_, before), (_, after)] = per_variant {
        let path = out.join(SIDE_BY_SIDE);
        std::fs::write(&path, summary_markdown(facts, before, after))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        lines.push(format!("Wrote {}", path.display()));
    }
    Ok(lines)
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
    let facts = facts(
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
            facts.today, local.time_zone, local.week_start
        ),
    ];
    let mut per_variant: Vec<(Variant, Vec<Answer>)> = Vec::new();
    for &variant in &options.variants {
        let off = variant_off(variant).with(options.off);
        let answers = ask_questions(&db, &prepared, &modules, off, questions, &local)?;
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
    lines.extend(write_reports(out, &facts, &per_variant, options.off)?);
    Ok(lines.join("\n"))
}
