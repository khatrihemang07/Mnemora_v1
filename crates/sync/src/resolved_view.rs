//! What the Conflicts tab's "Resolved automatically" shows, ready to draw: the title, what was
//! kept and what was replaced, and the "Use ‹other› instead" button, all as text. Computed here
//! from the stored decision and the records' latest state, with dates and times written the way
//! the person's format Preferences say; the window only lays them out.
//!
//! The stored rows keep raw values (they travel between devices); words are made on read, so a
//! changed format Preference or a renamed device applies to the whole log at once.

use mnemora_core::error::AppResult;
use mnemora_core::settings;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{Map, Value};
use specta::Type;

use crate::conflict_view::{ConflictSubject, subject_of};
use crate::peers;
use crate::resolutions::{self, Folded, Kind, Row};
use crate::texts;

/// What a row is about, for its icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedKind {
    /// A value two devices changed; the newest took.
    Value,
    /// A record deleted on one device and edited on another.
    DeleteEdit,
    /// A Task put in the Inbox because its List is gone.
    TaskInbox,
    /// A text clash on a record that is deleted.
    ConflictDeleted,
    /// Order and Preference changes folded into one line.
    OrderPrefs,
}

/// The icon a row wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedIcon {
    /// Priority.
    Flag,
    /// A date or time.
    Calendar,
    /// A List.
    List,
    /// A state.
    Check,
    /// Something deleted.
    Trash,
    /// A text.
    Text,
    /// Order and Preferences.
    Order,
    /// The Inbox.
    Inbox,
    /// Anything else.
    Other,
}

/// One line of "Resolved automatically".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ResolvedRow {
    /// The row's id.
    pub id: String,
    /// What kind of decision it is.
    pub kind: ResolvedKind,
    /// The Module it belongs to for the row's tint: `journal`, `todo` or `sync`.
    pub module: String,
    /// Its icon.
    pub icon: ResolvedIcon,
    /// "‘Water plants’ · Priority".
    pub title: String,
    /// "P1 (Mac) kept · P2 (Pixel 8) replaced".
    pub detail: String,
    /// When it was decided: UTC milliseconds. Rows group by this day.
    #[specta(type = u32)]
    pub at: i64,
    /// This device learned of it since the person last looked.
    pub fresh: bool,
    /// The button: "Use P2 instead". None when nothing can be changed back or it was done.
    pub action: Option<String>,
    /// After the button was used: "Changed to P2".
    pub used: Option<String>,
    /// For the folded line, what "Show" lists.
    pub changes: Vec<String>,
}

// ---------------------------------------------------------------- dates and times

/// How a date is written (the `core.date_format` Preference).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateStyle {
    Long,
    Short,
    Iso,
}

/// How a time of day is written (the `core.time_format` Preference).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeStyle {
    H24,
    H12,
}

#[derive(Debug, Clone, Copy)]
struct Formats {
    date: DateStyle,
    time: TimeStyle,
}

impl Formats {
    fn read(conn: &Connection) -> AppResult<Self> {
        let text = |key: &str| {
            settings::stored(conn, "core", key).map(|v| {
                v.and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default()
            })
        };
        Ok(Self {
            date: match text("date_format")?.as_str() {
                "short" => DateStyle::Short,
                "iso" => DateStyle::Iso,
                _ => DateStyle::Long,
            },
            time: if text("time_format")? == "12h" {
                TimeStyle::H12
            } else {
                TimeStyle::H24
            },
        })
    }
}

const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day count since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn date_parts(date: &str) -> Option<(i64, i64, i64)> {
    let mut it = date.splitn(3, '-');
    let y = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d = it.next()?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}

fn date_words(y: i64, m: i64, d: i64, style: DateStyle) -> String {
    let weekday = DAYS[usize::try_from((days_from_civil(y, m, d) + 4).rem_euclid(7)).unwrap_or(0)];
    let month = MONTHS[usize::try_from(m - 1).unwrap_or(0)];
    match style {
        DateStyle::Long => format!("{weekday} {d} {month}"),
        DateStyle::Short => format!("{month} {d}, {y}"),
        DateStyle::Iso => format!("{y:04}-{m:02}-{d:02}"),
    }
}

/// A `YYYY-MM-DD` date written the way the person chose.
fn date_text(date: &str, style: DateStyle) -> String {
    date_parts(date).map_or_else(|| date.to_owned(), |(y, m, d)| date_words(y, m, d, style))
}

fn clock_words(minute_of_day: i64, style: TimeStyle) -> String {
    let (h, m) = (minute_of_day / 60, minute_of_day % 60);
    match style {
        TimeStyle::H24 => format!("{h:02}:{m:02}"),
        TimeStyle::H12 => format!(
            "{}:{m:02} {}",
            if h % 12 == 0 { 12 } else { h % 12 },
            if h < 12 { "AM" } else { "PM" }
        ),
    }
}

/// A `HH:MM` time written the way the person chose.
fn time_text(time: &str, style: TimeStyle) -> String {
    let mut it = time.splitn(2, ':');
    match (
        it.next().and_then(|h| h.parse::<i64>().ok()),
        it.next().and_then(|m| m.parse::<i64>().ok()),
    ) {
        (Some(h), Some(m)) if (0..24).contains(&h) && (0..60).contains(&m) => {
            clock_words(h * 60 + m, style)
        }
        _ => time.to_owned(),
    }
}

// ---------------------------------------------------------------- words for values

fn text_of(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// "daily", "weekly", "every 2 weeks".
fn repeat_words(values: &Map<String, Value>) -> Option<String> {
    let freq = text_of(values.get("repeat_freq"))?;
    let every = values
        .get("repeat_every")
        .and_then(Value::as_i64)
        .unwrap_or(1);
    let (once, many) = match freq {
        "day" => ("daily", "days"),
        "week" => ("weekly", "weeks"),
        "month" => ("monthly", "months"),
        "year" => ("yearly", "years"),
        _ => return None,
    };
    Some(if every <= 1 {
        once.to_owned()
    } else {
        format!("every {every} {many}")
    })
}

/// Words for a record's name in a title: a Task's title in quotes, a List's, an Entry's moment.
fn target_words(
    conn: &Connection,
    target: &str,
    id: &str,
    f: Formats,
) -> Result<String, crate::SyncError> {
    let quoted = |s: &str| {
        if s.is_empty() {
            "Untitled".to_owned()
        } else {
            format!("‘{s}’")
        }
    };
    let state = texts::current(conn, target, id)?;
    let text = |key: &str| {
        state
            .as_ref()
            .and_then(|s| s.get(key))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    Ok(match target {
        "todo_task" => quoted(&text("title")),
        "todo_list" => format!("List {}", quoted(&text("name"))),
        "todo_comment" => {
            let task = texts::current(conn, "todo_task", &text("task_id"))?;
            let title = task
                .as_ref()
                .and_then(|t| t.get("title"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            format!("Comment on {}", quoted(title))
        }
        "journal_entry" => match subject_of(conn, target, id, "body")? {
            (
                ConflictSubject::Entry {
                    page_date,
                    written_time,
                },
                _,
            ) => format!(
                "{} · {} Entry",
                date_text(&page_date, f.date),
                time_text(&written_time, f.time)
            ),
            _ => "Entry".to_owned(),
        },
        _ => "Something".to_owned(),
    })
}

/// The unit's name as a person says it.
fn unit_words(unit: &str) -> String {
    match unit {
        "priority" => "Priority",
        "when" => "Due date",
        "state" => "Status",
        "list_id" => "List",
        "deleted_at" => "Deleted",
        "name" => "Name",
        "emoji" => "Emoji",
        "task_sort" => "Sort",
        "group_by" => "Grouping",
        "page_date" => "Page",
        "written" => "Time written",
        "task_id" => "Task",
        "title" => "Title",
        "notes" => "Notes",
        "body" => "Text",
        other => return humanize(other),
    }
    .to_owned()
}

/// `week_start` -> "Week start".
fn humanize(key: &str) -> String {
    let spaced = key.rsplit('.').next().unwrap_or(key).replace('_', " ");
    let mut chars = spaced.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn sort_words(v: &str) -> &str {
    match v {
        "manual" => "Manual",
        "due" => "Due date",
        "priority" => "Priority",
        "title" => "Title",
        "added" => "Date added",
        "none" => "None",
        "list" => "List",
        "day" => "Day",
        other => other,
    }
}

/// One value (or group of values) as a person reads it.
fn value_words(
    conn: &Connection,
    target: &str,
    unit: &str,
    values: &Map<String, Value>,
    f: Formats,
) -> Result<String, crate::SyncError> {
    let get = |k: &str| values.get(k);
    Ok(match (target, unit) {
        (_, "deleted_at") => {
            if get("deleted_at").is_some_and(|v| !v.is_null()) {
                "Deleted".to_owned()
            } else {
                "Restored".to_owned()
            }
        }
        ("todo_task", "priority") => {
            format!("P{}", get("priority").and_then(Value::as_i64).unwrap_or(4))
        }
        ("todo_task", "when") => {
            let mut parts: Vec<String> = Vec::new();
            match text_of(get("due_date")) {
                Some(d) => parts.push(date_text(d, f.date)),
                None => parts.push("No date".to_owned()),
            }
            if let Some(t) = text_of(get("due_time")) {
                parts.push(time_text(t, f.time));
            }
            let when = parts.join(" ");
            match repeat_words(values) {
                Some(r) => format!("{when} · repeats {r}"),
                None => when,
            }
        }
        ("todo_task", "state") => match text_of(get("state")) {
            Some("done") => "Done",
            Some("wont") => "Won’t do",
            _ => "Open",
        }
        .to_owned(),
        ("todo_task", "list_id") => match text_of(get("list_id")) {
            None => "Inbox".to_owned(),
            Some(id) => list_name(conn, id)?,
        },
        ("todo_list", "name") => text_of(get("name")).unwrap_or("Untitled").to_owned(),
        ("todo_list", "emoji") => text_of(get("emoji")).unwrap_or("No emoji").to_owned(),
        ("todo_list", "task_sort" | "group_by") => text_of(get(unit))
            .map(|v| sort_words(v).to_owned())
            .unwrap_or_default(),
        ("journal_entry", "page_date") => text_of(get("page_date"))
            .map(|d| date_text(d, f.date))
            .unwrap_or_default(),
        ("journal_entry", "written") => {
            let at = get("written_at")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            let offset = get("written_offset_min")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            let local = at / 1000 / 60 + offset;
            let (y, m, d) = civil_from_days(local.div_euclid(1440));
            format!(
                "{} {}",
                date_words(y, m, d, f.date),
                clock_words(local.rem_euclid(1440), f.time)
            )
        }
        ("todo_comment", "task_id") => {
            let id = text_of(get("task_id")).unwrap_or_default();
            texts::current(conn, "todo_task", id)?
                .and_then(|t| t.get("title").and_then(Value::as_str).map(str::to_owned))
                .unwrap_or_default()
        }
        _ => values
            .values()
            .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned))
            .collect::<Vec<_>>()
            .join(" "),
    })
}

fn list_name(conn: &Connection, id: &str) -> Result<String, crate::SyncError> {
    Ok(texts::current(conn, "todo_list", id)?
        .and_then(|l| l.get("name").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| "A List".to_owned()))
}

/// Whether "Use ‹other› instead" can write this unit back through its Module. A Task's status
/// can, unless the Task repeats (closing one is about its next date, so it has its own
/// commands). A deletion never is: two devices deleting is not a clash, and a delete met by an
/// edit is its own kind of row.
fn can_write_back(
    conn: &Connection,
    target: &str,
    id: &str,
    unit: &str,
) -> Result<bool, crate::SyncError> {
    Ok(match (target, unit) {
        ("todo_task", "state") => texts::current(conn, target, id)?.is_some_and(|t| {
            ["repeat_freq", "repeat_of"]
                .iter()
                .all(|k| t.get(k).is_none_or(Value::is_null))
        }),
        ("todo_task", "priority" | "when" | "list_id")
        | ("todo_list", "name" | "emoji" | "task_sort" | "group_by")
        | ("journal_entry", "page_date") => true,
        _ => false,
    })
}

// ---------------------------------------------------------------- rows

/// Who made a value: this device's name, a paired device's name, or a device no longer paired.
fn device_name(conn: &Connection, device: &str, me: &str, my_name: &str) -> String {
    if device == me {
        return my_name.to_owned();
    }
    peers::name_of(conn, device)
        .ok()
        .flatten()
        .unwrap_or_else(|| "Another device".to_owned())
}

struct Ctx<'a> {
    conn: &'a Connection,
    me: &'a str,
    my_name: &'a str,
    formats: Formats,
    visited: i64,
}

impl Ctx<'_> {
    fn who(&self, device: &str) -> String {
        device_name(self.conn, device, self.me, self.my_name)
    }
}

fn change_words(ctx: &Ctx<'_>, c: &Folded) -> Result<String, crate::SyncError> {
    Ok(if c.kind == "preference" {
        humanize(&c.key)
    } else {
        format!(
            "{} moved",
            target_words(ctx.conn, &c.entity, &c.entity_id, ctx.formats)?
        )
    })
}

fn module_of(target: &str) -> &'static str {
    match target {
        "journal_entry" => "journal",
        "todo_task" | "todo_list" | "todo_comment" => "todo",
        _ => "sync",
    }
}

fn icon_of(target: &str, unit: &str) -> ResolvedIcon {
    match (target, unit) {
        (_, "deleted_at") => ResolvedIcon::Trash,
        (_, "priority") => ResolvedIcon::Flag,
        (_, "when" | "page_date" | "written") => ResolvedIcon::Calendar,
        (_, "state") => ResolvedIcon::Check,
        (_, "list_id" | "task_id" | "name" | "emoji" | "task_sort" | "group_by") => {
            ResolvedIcon::List
        }
        _ => ResolvedIcon::Other,
    }
}

#[allow(clippy::too_many_lines)] // one arm per kind of row: reads best together
fn row_of(ctx: &Ctx<'_>, r: &Row) -> Result<Option<ResolvedRow>, crate::SyncError> {
    let Some(kind) = Kind::parse(&r.kind) else {
        return Ok(None);
    };
    let name = |t: &str, id: &str| target_words(ctx.conn, t, id, ctx.formats);
    let fresh = r.learned_at > ctx.visited;
    let mut row = ResolvedRow {
        id: r.id.clone(),
        kind: ResolvedKind::Value,
        module: module_of(&r.target).to_owned(),
        icon: ResolvedIcon::Other,
        title: String::new(),
        detail: String::new(),
        at: r.at,
        fresh,
        action: None,
        used: None,
        changes: Vec::new(),
    };
    match kind {
        Kind::Value => {
            let kept = value_words(ctx.conn, &r.target, &r.field, &r.kept, ctx.formats)?;
            let replaced = value_words(ctx.conn, &r.target, &r.field, &r.replaced, ctx.formats)?;
            row.kind = ResolvedKind::Value;
            row.icon = icon_of(&r.target, &r.field);
            row.title = format!(
                "{} · {}",
                name(&r.target, &r.target_id)?,
                unit_words(&r.field)
            );
            row.detail = format!(
                "{kept} ({}) kept · {replaced} ({}) replaced",
                ctx.who(&r.kept_device),
                ctx.who(&r.replaced_device)
            );
            if r.used_at.is_some() {
                row.used = Some(format!("Changed to {replaced}"));
            } else if can_write_back(ctx.conn, &r.target, &r.target_id, &r.field)? {
                row.action = Some(format!("Use {replaced} instead"));
            }
        }
        Kind::DeleteEdit => {
            let edited: Vec<String> = r
                .replaced
                .keys()
                .map(|k| unit_words(k).to_lowercase())
                .collect();
            row.kind = ResolvedKind::DeleteEdit;
            row.icon = ResolvedIcon::Trash;
            row.title = format!("{} · Deleted", name(&r.target, &r.target_id)?);
            row.detail = format!(
                "Deleted ({}) kept · {} edited ({}) kept inside it",
                ctx.who(&r.kept_device),
                if edited.is_empty() {
                    "changes".to_owned()
                } else {
                    edited.join(", ")
                },
                ctx.who(&r.replaced_device)
            );
        }
        Kind::TaskInbox => {
            row.kind = ResolvedKind::TaskInbox;
            row.icon = ResolvedIcon::Inbox;
            row.title = format!("{} · List", name(&r.target, &r.target_id)?);
            row.detail = format!(
                "Moved to the Inbox · ‘{}’ is gone · {} had moved it there",
                list_name(ctx.conn, &r.extra)?,
                ctx.who(&r.replaced_device)
            );
        }
        Kind::ConflictDeleted => {
            row.kind = ResolvedKind::ConflictDeleted;
            row.icon = ResolvedIcon::Text;
            row.title = format!(
                "{} · {}",
                name(&r.target, &r.target_id)?,
                unit_words(&r.field).to_lowercase()
            );
            row.detail = format!(
                "Deleted, so nothing was asked · {} and {} both rewrote it",
                ctx.who(&r.kept_device),
                ctx.who(&r.replaced_device)
            );
        }
        Kind::OrderPrefs => {
            row.kind = ResolvedKind::OrderPrefs;
            "sync".clone_into(&mut row.module);
            row.icon = ResolvedIcon::Order;
            row.title = format!(
                "Order & preferences · {} {}",
                r.count,
                if r.count == 1 { "change" } else { "changes" }
            );
            row.detail = ctx.who(&r.replaced_device);
            row.changes = r
                .detail
                .iter()
                .map(|c| change_words(ctx, c))
                .collect::<Result<_, _>>()?;
        }
    }
    Ok(Some(row))
}

/// Every row of "Resolved automatically", newest first.
pub fn list(conn: &Connection, me: &str, my_name: &str) -> AppResult<Vec<ResolvedRow>> {
    let ctx = Ctx {
        conn,
        me,
        my_name,
        formats: Formats::read(conn)?,
        visited: resolutions::visited_at(conn)?,
    };
    let mut out = Vec::new();
    for r in resolutions::all(conn)? {
        if let Some(row) = row_of(&ctx, &r)? {
            out.push(row);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_round_trip_and_weekdays_are_right() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(
            civil_from_days(days_from_civil(2026, 10, 23)),
            (2026, 10, 23)
        );
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        // 23 October 2026 is a Friday.
        assert_eq!(date_text("2026-10-23", DateStyle::Long), "Fri 23 Oct");
        assert_eq!(date_text("2026-10-23", DateStyle::Short), "Oct 23, 2026");
        assert_eq!(date_text("2026-10-23", DateStyle::Iso), "2026-10-23");
        assert_eq!(date_text("nonsense", DateStyle::Long), "nonsense");
    }

    #[test]
    fn times_follow_the_clock_preference() {
        assert_eq!(time_text("19:05", TimeStyle::H24), "19:05");
        assert_eq!(time_text("19:05", TimeStyle::H12), "7:05 PM");
        assert_eq!(time_text("00:30", TimeStyle::H12), "12:30 AM");
        assert_eq!(time_text("12:00", TimeStyle::H12), "12:00 PM");
    }

    #[test]
    fn repeats_read_like_a_person_would_say_them() {
        let mut v = Map::new();
        v.insert("repeat_freq".into(), "week".into());
        v.insert("repeat_every".into(), 1.into());
        assert_eq!(repeat_words(&v).as_deref(), Some("weekly"));
        v.insert("repeat_every".into(), 2.into());
        assert_eq!(repeat_words(&v).as_deref(), Some("every 2 weeks"));
        assert_eq!(repeat_words(&Map::new()), None);
    }

    #[test]
    fn units_are_named_and_unknown_ones_are_made_readable() {
        assert_eq!(unit_words("priority"), "Priority");
        assert_eq!(unit_words("when"), "Due date");
        assert_eq!(unit_words("week_start"), "Week start");
        assert_eq!(humanize("core.week_start"), "Week start");
    }
}
