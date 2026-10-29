//! The note for batches a Run left unread (2026-10-29 follow-up, decision 6). The Run's Tool results
//! are grouped by Tool and filters (every argument but `offset` and `limit`), and a group is
//! unresolved when its latest batch still has a next offset. Nothing asks the Model for this: it is
//! read from the results the Run saved.

use serde_json::{Map, Value};

/// Unresolved groups the note names; the rest are counted as "and N more".
const NAMED: usize = 3;

/// One paged result: the rows it shows, the total the query matches, and where the next batch starts.
struct Paged {
    shown: u64,
    total: u64,
    next: Option<u64>,
}

/// The latest batch a group has had: where it started, the paged result it was, and whether more follows.
struct Latest {
    offset: u64,
    paged: Paged,
}

/// Every call of one Tool with the same filters, in the order the Run made them.
struct Group {
    tool: String,
    filters: Value,
    latest: Latest,
}

/// The paged Tool results of one Run, grouped as the note needs them.
#[derive(Default)]
pub struct Unread {
    groups: Vec<Group>,
}

impl Unread {
    /// Records one Tool result. A result that is not paged (a refusal, or a Tool with no batches) is
    /// not kept, so a repeated call refused by the loop does not hide the batch it repeats.
    pub fn record(&mut self, tool: &str, arguments: &str, result: &str) {
        let Some(paged) = paged(result) else {
            return;
        };
        let (offset, filters) = filters_of(arguments);
        let latest = Latest { offset, paged };
        match self
            .groups
            .iter_mut()
            .find(|group| group.tool == tool && group.filters == filters)
        {
            Some(group) => group.latest = latest,
            None => self.groups.push(Group {
                tool: tool.to_owned(),
                filters,
                latest,
            }),
        }
    }

    /// One line per unresolved group, the first three named and the rest counted. None when every
    /// group was read to its end.
    pub fn note(&self) -> Option<String> {
        let lines: Vec<String> = self.groups.iter().filter_map(Group::line).collect();
        let mut named: Vec<String> = lines.iter().take(NAMED).cloned().collect();
        if lines.len() > NAMED {
            named.push(format!("and {} more", lines.len() - NAMED));
        }
        (!named.is_empty()).then(|| named.join("\n"))
    }
}

impl Group {
    /// The line for this group, when its latest batch still has a next offset.
    fn line(&self) -> Option<String> {
        // Only a group whose latest batch still has a next offset is unresolved.
        self.latest.paged.next?;
        let (noun, scope) = noun_of(&self.tool)?;
        let noun = match scope {
            Some(scope) => format!("{noun} ({scope})"),
            None => noun.to_owned(),
        };
        let first = self.latest.offset + self.latest.paged.shown;
        let total = self.latest.paged.total;
        Some(format!(
            "Based on the first {first} of {total} {noun}; the rest was not read."
        ))
    }
}

/// The paged fields of a Tool result: its body is the JSON after the observation line. None for a
/// result that is not paged, which has no `next_offset`.
fn paged(result: &str) -> Option<Paged> {
    let json = &result[result.find('{')?..];
    let body: Value = serde_json::from_str(json).ok()?;
    let next = body.get("next_offset")?.as_u64();
    Some(Paged {
        shown: body.get("shown")?.as_u64()?,
        total: body.get("total")?.as_u64()?,
        next,
    })
}

/// The offset a call asks for, and its filters: its arguments without `offset` and `limit`.
fn filters_of(arguments: &str) -> (u64, Value) {
    let mut args: Map<String, Value> = serde_json::from_str::<Value>(arguments)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let offset = args.remove("offset").and_then(|v| v.as_u64()).unwrap_or(0);
    args.remove("limit");
    (offset, Value::Object(args))
}

/// The noun a paged Tool counts its batch in, and the Module a changes Tool's rows belong to. These
/// names are the ones the Tools use in their notes (crates/todo/src/tools.rs and
/// crates/journal/src/tools.rs). A Tool not listed here adds no line.
fn noun_of(tool: &str) -> Option<(&'static str, Option<&'static str>)> {
    Some(match tool {
        "journal_changes" => ("changes", Some("Journal")),
        "todo_changes" => ("changes", Some("Tasks")),
        "journal_list" => ("entries", None),
        "todo_list" => ("tasks", None),
        "journal_search" | "todo_search" => ("matches", None),
        "journal_read" => ("bytes of the Entry", None),
        "todo_read" => ("bytes of the notes", None),
        _ => return None,
    })
}
