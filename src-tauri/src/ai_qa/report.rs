//! The markdown a QA run writes: one report per variant, and the side-by-side summary of both.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use mnemora_ai::{Choice, LocalContext, ProviderRecord};

use super::answer::{Answer, CallLine};
use super::dump::dump_request;
use super::knobs::{Off, Variant};

/// The side-by-side summary's file name, in `--out`.
const SIDE_BY_SIDE: &str = "qa-side-by-side.md";

/// What a report says about the run it covers: the Provider and Model (names only, never a key),
/// the calendar and the database the copy came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSetup {
    /// The Provider's name and kind, such as `Local AI Wrapper (Wrapper)`.
    pub provider: String,
    /// The Model id.
    pub model: String,
    /// The calendar's IANA zone.
    pub zone: String,
    /// The day and weekday the calendar says is today, such as `Sunday 2026-10-25`.
    pub today: String,
    /// The first day of the week the calendar uses: `mon` or `sun`.
    pub week_start: String,
    /// The Sandbox database the copy was made from.
    pub sandbox_db: PathBuf,
    /// How many questions each variant asks.
    pub questions: usize,
}

/// The setup of a run, from the Provider the Run uses and its calendar. The key is not among them.
pub fn run_setup(
    record: &ProviderRecord,
    choice: &Choice,
    local: &LocalContext,
    sandbox_db: &Path,
    questions: usize,
) -> RunSetup {
    RunSetup {
        provider: format!("{} ({})", record.name, record.kind.label()),
        model: choice.model.clone(),
        zone: local.time_zone.clone(),
        today: format!("{} {}", local.weekday, local.today),
        week_start: local.week_start.clone(),
        sandbox_db: sandbox_db.to_path_buf(),
        questions,
    }
}

/// The report of one variant: its header, then each question's Steps, Tool calls, answer and note.
pub fn report_markdown(variant: Variant, setup: &RunSetup, answers: &[Answer]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# AI QA report: {} variant\n", variant.label());
    let _ = writeln!(out, "{}\n", variant.describe());
    let _ = writeln!(out, "{}\n", header_lines(setup));
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
        let _ = writeln!(out, "{}\n", answer_or_none(answer));
        if let Some(request) = &answer.first_request {
            let _ = writeln!(
                out,
                "### First request of this Run (no Tool call on its first Step)\n\n{}",
                dump_request(request)
            );
        }
    }
    out
}

/// The header lines every report starts with: the Provider and Model by name, the calendar, and
/// where the copy came from.
fn header_lines(setup: &RunSetup) -> String {
    [
        format!("- Provider: {}", setup.provider),
        format!("- Model: {}", setup.model),
        format!(
            "- Calendar: today is {}, zone {}, weeks start {}",
            setup.today, setup.zone, setup.week_start
        ),
        format!("- Questions per variant: {}", setup.questions),
        format!(
            "- Sandbox database (copied, the copy was run): {}",
            setup.sandbox_db.display()
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
pub fn summary_markdown(setup: &RunSetup, before: &[Answer], after: &[Answer]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# AI QA: before and after, side by side\n");
    let _ = writeln!(
        out,
        "Before: every Tool except `journal_changes` and `todo_changes`, and the prompt without the intent paragraph. After: the current code. Both run the same questions on the same copy.\n"
    );
    let _ = writeln!(out, "{}\n", header_lines(setup));
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

/// Writes one report per variant, and the side-by-side summary when both ran. Returns the lines
/// that say where they went.
pub fn write_reports(
    out: &Path,
    setup: &RunSetup,
    per_variant: &[(Variant, Vec<Answer>)],
    off: Off,
) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let switched = format!("Switched off for this run: {}\n\n", off.describe());
    for (variant, answers) in per_variant {
        let path = out.join(format!("qa-{}.md", variant.label()));
        std::fs::write(
            &path,
            format!("{switched}{}", report_markdown(*variant, setup, answers)),
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
        lines.push(format!("Wrote {}", path.display()));
    }
    if let [(_, before), (_, after)] = per_variant {
        let path = out.join(SIDE_BY_SIDE);
        std::fs::write(&path, summary_markdown(setup, before, after))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        lines.push(format!("Wrote {}", path.display()));
    }
    Ok(lines)
}
