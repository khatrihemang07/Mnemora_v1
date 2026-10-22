//! The Todo widget's Quick add, in Rust: reading typed text for its sheet, and adding the Task. It
//! is a port of the app's Capture box rules (`capture.ts` `resolveFields`, `understood.ts`, and
//! `CaptureBox.svelte`'s add), so a Task added from the home screen is the Task the app would
//! have added. `src/lib/ipc/fixtures/capture-resolve.json` runs the same resolution cases here and
//! in Vitest. Task text must never reach a log line.

use std::collections::HashMap;

use mnemora_core::error::AppResult;
use mnemora_core::settings;
use rusqlite::Connection;
use serde::{Deserialize, Deserializer, Serialize};

use crate::capture::parse_text;
use crate::list::TodoList;
use crate::list_repo;
use crate::list_service::create_list;
use crate::parse::{ParseRequest, ParseSpan, Parsed};
use crate::recurrence::RepeatRule;
use crate::task::{Task, validate_priority};
use crate::when::{LocalNow, NewWhen, add_days};
use crate::widget::{WidgetList, WidgetView, list_chip, lists as widget_lists};
use crate::widget_text::{
    TimeFormat, WeekStart, landed_in, meaning_text, quick_added_message, when_chip_label,
};
use crate::{NewTaskDetails, create_task};

// --- What the Task gets --------------------------------------------------------------------------

/// What a Task has before the text says anything: where the box opened.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Base {
    /// The List, or none for the Inbox.
    pub list_id: Option<String>,
    /// Due date, `YYYY-MM-DD`.
    pub date: Option<String>,
    /// Due time, `HH:MM`.
    pub time: Option<String>,
    /// The Repeat.
    pub repeat: Option<RepeatRule>,
    /// 1 to 4.
    pub priority: u32,
}

impl Base {
    /// A new Task in the Inbox with no date.
    pub fn bare() -> Self {
        Self {
            list_id: None,
            date: None,
            time: None,
            repeat: None,
            priority: 4,
        }
    }
}

/// A property chip: not touched, cleared by hand, or set by hand.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Chip<T> {
    /// Not used: where the box opened stands.
    #[default]
    Untouched,
    /// Cleared by hand (the Inbox, no date): `null` in the request.
    Cleared,
    /// Set by hand.
    Set(T),
}

impl<T: Clone> Chip<T> {
    /// What the chip gives, or `base` when it was not touched.
    fn or(&self, base: Option<&T>) -> Option<T> {
        match self {
            Self::Untouched => base.cloned(),
            Self::Cleared => None,
            Self::Set(v) => Some(v.clone()),
        }
    }
}

/// A key that is present, null or not, is a chip that was touched.
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Chip<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(d)? {
            Some(v) => Self::Set(v),
            None => Self::Cleared,
        })
    }
}

/// What the property chips set by hand.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Hand {
    /// The List (cleared is the Inbox).
    #[serde(default)]
    pub list_id: Chip<String>,
    /// Due date.
    #[serde(default)]
    pub date: Chip<String>,
    /// Due time.
    #[serde(default)]
    pub time: Chip<String>,
    /// The Repeat.
    #[serde(default)]
    pub repeat: Chip<RepeatRule>,
    /// 1 to 4.
    pub priority: Option<u32>,
}

/// The fields the Task will get.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Fields {
    /// Due date.
    pub date: Option<String>,
    /// Due time.
    pub time: Option<String>,
    /// The Repeat.
    pub repeat: Option<RepeatRule>,
    /// 1 to 4.
    pub priority: u32,
    /// The List, or none for the Inbox.
    pub list_id: Option<String>,
    /// A List from `#Name` that does not exist yet: made when the Task is added.
    pub new_list: Option<String>,
    /// The kinds of field the typed words set, in the order they first appear.
    pub from_text: Vec<String>,
}

/// The fields the Task will get: the words win over a chip, a chip over where the box opened. A
/// time with no date keeps the day the box opened on, and only a box that opened with no day takes
/// the parser's today-or-tomorrow. Port of `resolveFields`.
pub fn resolve_fields(parsed: Option<&Parsed>, hand: &Hand, base: &Base) -> Fields {
    let spans: &[ParseSpan] = parsed.map_or(&[], |p| p.spans.as_slice());
    let has = |kind: &str| spans.iter().any(|s| s.kind == kind);
    let parsed_date = parsed.and_then(|p| p.date.clone());
    let inherited = hand.date.or(base.date.as_ref());
    let date = if has("date") {
        parsed_date
    } else {
        inherited.or(parsed_date)
    };
    let list_id = parsed.and_then(|p| p.list_id.clone()).or_else(|| {
        if parsed.is_some_and(|p| p.new_list.is_some()) {
            None
        } else {
            hand.list_id.or(base.list_id.as_ref())
        }
    });
    let mut from_text: Vec<String> = Vec::new();
    for s in spans {
        if !from_text.contains(&s.kind) {
            from_text.push(s.kind.clone());
        }
    }
    Fields {
        date,
        time: parsed
            .and_then(|p| p.time.clone())
            .or_else(|| hand.time.or(base.time.as_ref())),
        repeat: parsed
            .and_then(|p| p.repeat.clone())
            .or_else(|| hand.repeat.or(base.repeat.as_ref())),
        priority: parsed
            .and_then(|p| p.priority)
            .or(hand.priority)
            .unwrap_or(base.priority),
        list_id,
        new_list: parsed.and_then(|p| p.new_list.clone()),
        from_text,
    }
}

/// One recognised word in the record stored with the Task.
#[derive(Serialize)]
struct Understood<'a> {
    word: &'a str,
    kind: &'a str,
    value: String,
}

/// The record of what the typed words meant, stored as `Task.understood`: the words that won,
/// with a List's id in place of its name. None when nothing was understood. Port of
/// `understoodFromSpans` and `serializeUnderstood`.
fn understood_record(spans: &[ParseSpan], list_id: Option<&str>) -> Option<String> {
    let entries: Vec<Understood<'_>> = spans
        .iter()
        .filter_map(|s| {
            let value = if s.kind == "list" {
                list_id?.to_owned()
            } else {
                s.value.clone()
            };
            Some(Understood {
                word: &s.word,
                kind: &s.kind,
                value,
            })
        })
        .collect();
    if entries.is_empty() {
        None
    } else {
        serde_json::to_string(&entries).ok()
    }
}

// --- Where the sheet opened ----------------------------------------------------------------------

/// The place a Quick add opens in: a day, a List, or neither (the Inbox).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Place {
    /// The day a Task is due: today in Today, tomorrow in Upcoming.
    pub date: Option<String>,
    /// The List a Task goes to.
    pub list_id: Option<String>,
}

/// Where the widget's + adds for `view`, as the app's + does (`addPlaceFor`): the Task is due today
/// in Today, tomorrow in Upcoming, in the List in a List, and in the Inbox elsewhere. A List that
/// was deleted since the widget was set up is the Inbox, and the placeholder says so.
fn place_for(conn: &Connection, view: &WidgetView, local: &LocalNow) -> AppResult<(Place, String)> {
    let none = |date: Option<String>| Place {
        date,
        list_id: None,
    };
    Ok(match view {
        WidgetView::Today => (
            none(Some(local.today.clone())),
            "Quick add, due today".to_owned(),
        ),
        WidgetView::Upcoming => (
            none(add_days(&local.today, 1)),
            "Quick add, due tomorrow".to_owned(),
        ),
        WidgetView::Inbox | WidgetView::All => (none(None), "Quick add to Inbox".to_owned()),
        WidgetView::List(id) => match list_repo::get(conn, id)? {
            Some(list) => (
                Place {
                    date: None,
                    list_id: Some(list.id.clone()),
                },
                format!("Quick add to {}", list_chip(&list)),
            ),
            None => (none(None), "Quick add to Inbox".to_owned()),
        },
    })
}

/// Everything the sheet needs before the first key: the placeholder, where it opened, the Lists to
/// pick from and the Settings that shape the reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaptureContext {
    /// The field's hint: `Quick add, due today`, `Quick add to 🏠 Home`.
    pub placeholder: String,
    /// Where a Task goes unless the text or a chip says otherwise.
    pub place: Place,
    /// The Lists, in the sidebar's order.
    pub lists: Vec<WidgetList>,
    /// "Recognise dates in titles": whether typed words are read at all.
    pub parse_dates: bool,
    /// `mon` or `sun`.
    pub week_start: String,
    /// `24h` or `12h`.
    pub time_format: String,
}

/// The sheet's context for the widget showing `view`.
pub fn capture_context(
    conn: &Connection,
    view: &WidgetView,
    local: &LocalNow,
) -> AppResult<CaptureContext> {
    local.check()?;
    let (place, placeholder) = place_for(conn, view, local)?;
    Ok(CaptureContext {
        placeholder,
        place,
        lists: widget_lists(conn)?.lists,
        parse_dates: parse_dates(conn)?,
        week_start: WeekStart::read(conn)?.as_str().to_owned(),
        time_format: TimeFormat::read(conn)?.as_str().to_owned(),
    })
}

fn parse_dates(conn: &Connection) -> AppResult<bool> {
    Ok(settings::stored(conn, "todo", "parse_dates")?.and_then(|v| v.as_bool()) != Some(false))
}

// --- Reading and adding --------------------------------------------------------------------------

/// Typed text, and the decisions made about it.
#[derive(Debug, Clone, Deserialize)]
pub struct QuickRequest {
    /// The title as typed.
    pub text: String,
    /// The notes ("Notes" chip, Shift+Enter, a multi-line paste).
    #[serde(default)]
    pub notes: String,
    /// Keys of words the user keeps as text.
    #[serde(default)]
    pub ignored: Vec<String>,
    /// Per kind, the key of the word the user picked over the last one.
    #[serde(default)]
    pub prefer: HashMap<String, String>,
    /// The chips used by hand.
    #[serde(default)]
    pub hand: Hand,
    /// The widget's view: where the sheet opened.
    pub view: String,
    /// The device's clock.
    pub local: LocalNow,
}

/// A recognised word with what it means, ready for a chip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Marked {
    /// Names the word across parses.
    pub key: String,
    /// `date`, `time`, `repeat`, `prio` or `list`.
    pub kind: String,
    /// Where it starts in the text, in UTF-16 units (a Kotlin `String` index).
    pub start: u32,
    /// Where it ends.
    pub end: u32,
    /// The words as typed.
    pub word: String,
    /// The meaning in words: `Fri 23 Oct`, `9:30 PM`, `P1`, a List's name.
    pub meaning: String,
    /// A List that does not exist yet.
    pub new_list: bool,
    /// For an alternative, why it lost.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// What the sheet shows for the text so far.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reading {
    /// The words that won.
    pub spans: Vec<Marked>,
    /// The words that lost ("use instead").
    pub alternatives: Vec<Marked>,
    /// The words kept as text ("kept as text").
    pub ignored: Vec<Marked>,
    /// The fields the Task will get.
    pub fields: Fields,
    /// The List chip: the List's emoji and name, `Inbox`, or the name of a new List.
    pub list_label: String,
    /// The chosen List is one `#Name` will make.
    pub list_is_new: bool,
    /// The When chip: `Today · 09:00`, or `When`.
    pub when_label: String,
}

fn marked(span: &ParseSpan, reason: Option<&str>, today: &str, format: TimeFormat) -> Marked {
    let meaning = if span.kind == "list" {
        if span.new_list {
            format!("New list “{}”", span.value)
        } else {
            span.value.clone()
        }
    } else {
        meaning_text(&span.kind, &span.value, today, format)
    };
    Marked {
        key: span.key.clone(),
        kind: span.kind.clone(),
        start: span.start,
        end: span.end,
        word: span.word.clone(),
        meaning,
        new_list: span.new_list,
        reason: reason.map(str::to_owned),
    }
}

/// What the text means, and the place a Task goes. With recognition turned off, or no text, the
/// words are not read.
fn read_text(
    conn: &Connection,
    req: &QuickRequest,
    base: &Base,
) -> AppResult<(Option<Parsed>, Fields)> {
    let parsed = if parse_dates(conn)? && !req.text.trim().is_empty() {
        Some(parse_text(
            conn,
            &ParseRequest {
                text: req.text.clone(),
                ignored: req.ignored.clone(),
                prefer: req.prefer.clone(),
                frozen: Vec::new(),
                today: req.local.today.clone(),
                now: req.local.now.clone(),
                week_start: WeekStart::read(conn)?.as_str().to_owned(),
            },
        )?)
    } else {
        None
    };
    let fields = resolve_fields(parsed.as_ref(), &req.hand, base);
    Ok((parsed, fields))
}

fn base_of(conn: &Connection, req: &QuickRequest) -> AppResult<Base> {
    req.local.check()?;
    let view = WidgetView::parse(&req.view)?;
    let (place, _) = place_for(conn, &view, &req.local)?;
    Ok(Base {
        list_id: place.list_id,
        date: place.date,
        ..Base::bare()
    })
}

/// Reads the typed text for the sheet: the highlighted words, their chips, and what the Task would
/// get. Writes nothing.
pub fn read_quick(conn: &Connection, req: &QuickRequest) -> AppResult<Reading> {
    let base = base_of(conn, req)?;
    let (parsed, fields) = read_text(conn, req, &base)?;
    let format = TimeFormat::read(conn)?;
    let today = req.local.today.as_str();
    let all = |spans: &[ParseSpan]| -> Vec<Marked> {
        spans
            .iter()
            .map(|s| marked(s, None, today, format))
            .collect()
    };
    let (spans, alternatives, ignored) = match &parsed {
        Some(p) => (
            all(&p.spans),
            p.alternatives
                .iter()
                .map(|a| marked(&a.span, Some(&a.reason), today, format))
                .collect(),
            all(&p.ignored),
        ),
        None => (Vec::new(), Vec::new(), Vec::new()),
    };
    let chosen: Option<TodoList> = match fields.list_id.as_deref() {
        Some(id) => list_repo::get(conn, id)?,
        None => None,
    };
    let (list_label, list_is_new) = match (&fields.new_list, &chosen) {
        (Some(name), _) => (name.clone(), true),
        (None, Some(list)) => (list_chip(list), false),
        (None, None) => ("Inbox".to_owned(), false),
    };
    let when_label = when_chip_label(
        fields.date.as_deref(),
        fields.time.as_deref(),
        fields.repeat.as_ref(),
        today,
        format,
    );
    Ok(Reading {
        spans,
        alternatives,
        ignored,
        fields,
        list_label,
        list_is_new,
        when_label,
    })
}

/// A Task that was added, and the words for the toast.
#[derive(Debug, Clone, Serialize)]
pub struct Added {
    /// The Task as stored.
    pub task: Task,
    /// The toast: `Added “Call plumber” to Today`.
    pub message: String,
}

/// Adds the Task the sheet describes, as the app's Capture box does: the words are read, a `#Name`
/// that matches no List makes one, and the Task goes through `create_task` with the record of what
/// was understood. The List and the Task are two writes, as in the app.
pub fn add_quick(conn: &mut Connection, req: &QuickRequest) -> AppResult<Added> {
    let base = base_of(conn, req)?;
    let (parsed, fields) = read_text(conn, req, &base)?;
    let title = req.text.trim();
    let list_id = match &fields.new_list {
        Some(name) => Some(create_list(conn, name)?.id),
        None => fields.list_id.clone(),
    };
    let spans = parsed.as_ref().map_or(&[][..], |p| p.spans.as_slice());
    let when =
        (fields.date.is_some() || fields.time.is_some() || fields.repeat.is_some()).then(|| {
            NewWhen {
                date: fields.date.clone(),
                time: fields.time.clone(),
                local: req.local.clone(),
            }
        });
    let details = NewTaskDetails {
        notes: req.notes.clone(),
        priority: validate_priority(fields.priority).ok().filter(|p| *p < 4),
        repeat: fields.repeat.clone(),
        understood: understood_record(spans, list_id.as_deref()),
    };
    let task = create_task(
        conn,
        title,
        list_id.as_deref(),
        when.as_ref(),
        Some(&details),
    )?;
    let list_name = match (&fields.new_list, list_id.as_deref()) {
        (Some(name), _) => Some(name.clone()),
        (None, Some(id)) => list_repo::get(conn, id)?.map(|l| l.name),
        (None, None) => None,
    };
    let place = landed_in(
        task.due_date.as_deref(),
        list_name.as_deref(),
        &req.local.today,
    );
    Ok(Added {
        message: quick_added_message(&task.title, &place),
        task,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::parse::Parsed;

    const FIXTURE: &str = include_str!("../../../src/lib/ipc/fixtures/capture-resolve.json");

    fn parsed_of(case: &Value) -> Option<Parsed> {
        let p = case.as_object()?;
        let kinds = p["kinds"].as_array()?;
        let text = |k: &str| p[k].as_str().map(str::to_owned);
        Some(Parsed {
            spans: kinds
                .iter()
                .enumerate()
                .map(|(i, kind)| ParseSpan {
                    key: format!("{}:{i}", kind.as_str().unwrap()),
                    kind: kind.as_str().unwrap().to_owned(),
                    start: 0,
                    end: 0,
                    word: String::new(),
                    value: String::new(),
                    new_list: false,
                })
                .collect(),
            alternatives: Vec::new(),
            ignored: Vec::new(),
            date: text("date"),
            time: text("time"),
            repeat: serde_json::from_value(p["repeat"].clone()).unwrap(),
            priority: p["priority"].as_u64().and_then(|n| u32::try_from(n).ok()),
            list_id: text("list_id"),
            new_list: text("new_list"),
        })
    }

    #[test]
    fn the_fields_match_the_apps_resolve_fields() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        for case in fixture["resolve"].as_array().unwrap() {
            let parsed = parsed_of(&case["parsed"]);
            let hand: Hand = serde_json::from_value(case["hand"].clone()).unwrap();
            let base: Base = serde_json::from_value(case["base"].clone()).unwrap();
            let got = resolve_fields(parsed.as_ref(), &hand, &base);
            assert_eq!(
                serde_json::to_value(&got).unwrap(),
                case["fields"],
                "{}",
                case["name"]
            );
        }
    }

    #[test]
    fn a_hand_key_set_to_null_is_a_cleared_chip_and_a_missing_one_is_untouched() {
        let cleared: Hand = serde_json::from_value(json!({ "list_id": null })).unwrap();
        assert_eq!(cleared.list_id, Chip::Cleared);
        let set: Hand = serde_json::from_value(json!({ "list_id": "L1" })).unwrap();
        assert_eq!(set.list_id, Chip::Set("L1".to_owned()));
        let untouched: Hand = serde_json::from_value(json!({})).unwrap();
        assert_eq!(untouched.list_id, Chip::Untouched);
    }
}
