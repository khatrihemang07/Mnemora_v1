//! The recent-changes fixture: about three weeks of Journal Entries, Tasks and Comments, replayed
//! through the real commands so the change log and the change Tools read them as they would a real
//! install. The JSON (`fixtures/recent_changes.json`) gives each event's day and local time relative
//! to a now, so "yesterday" is always populated.

use std::collections::HashMap;

use mnemora_core::db::Connection;
use mnemora_core::local_time;
use mnemora_core::util::now_ms;
use serde::Deserialize;
use serde_json::{Value, json};

#[cfg(test)]
use crate::test_support::TestApp;

/// The fixture, with each Entry's source (book and original date) beside its text.
pub const FIXTURE: &str = include_str!("fixtures/recent_changes.json");

const MINUTE_MS: i64 = 60_000;
const DAY_MS: i64 = 24 * 60 * MINUTE_MS;

/// The fixture day that holds the catch-up: 139 events, Tuesday 6 October for a seed on Sunday 25
/// October (days from the day the seed ran). `load_except` leaves it out on request.
pub const CATCHUP_DAY: i64 = -19;

/// Where a load's events go: the commands, the clock they are stamped with, and the database for
/// the columns no command sets from the clock. The Sandbox seed (`ai_seed`) implements it over the
/// services; the tests implement it over a `TestApp`.
pub trait Driver {
    /// Runs a command, as the UI would, with `args` keyed by parameter name.
    fn invoke(&self, command: &str, args: Value) -> Result<Value, Value>;
    /// Stands the clock at `ms` for the commands that follow.
    fn set_clock(&self, ms: i64);
    /// Runs `f` on the database.
    fn with_db(&self, f: &mut dyn FnMut(&Connection));
}

#[cfg(test)]
impl Driver for TestApp {
    fn invoke(&self, command: &str, args: Value) -> Result<Value, Value> {
        TestApp::invoke::<Value>(self, command, args)
    }

    fn set_clock(&self, ms: i64) {
        self.clock().freeze(ms);
    }

    fn with_db(&self, f: &mut dyn FnMut(&Connection)) {
        TestApp::with_db(self, |conn| f(conn));
    }
}

/// The ids of what a load wrote, by the key the fixture gives it.
pub struct Loaded {
    ids: HashMap<String, String>,
    events: usize,
    skipped: usize,
    left_out: usize,
}

impl Loaded {
    /// How many keyed items were written (Lists, Tasks and Entries).
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// How many fixture events were written.
    pub fn events(&self) -> usize {
        self.events
    }

    /// How many fixture events were left out because they fall after `now` (not yet happened).
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    /// How many fixture events were left out because `load_except` was asked to leave their day out.
    pub fn left_out(&self) -> usize {
        self.left_out
    }

    /// Whether the fixture wrote an item with `key`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn contains(&self, key: &str) -> bool {
        self.ids.contains_key(key)
    }

    /// The id of the List, Task or Entry with `key`. Panics when the fixture did not write it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn id(&self, key: &str) -> &str {
        self.ids
            .get(key)
            .unwrap_or_else(|| panic!("the fixture wrote no item called {key}"))
    }
}

#[derive(Deserialize)]
struct Fixture {
    events: Vec<Event>,
}

/// One change, at `day` (relative to today) and `at` (local HH:MM). `do` names what it does; the
/// other fields are read by the kinds that use them.
#[derive(Deserialize)]
struct Event {
    day: i64,
    at: String,
    #[serde(rename = "do")]
    op: String,
    key: Option<String>,
    name: Option<String>,
    title: Option<String>,
    list: Option<String>,
    due: Option<i64>,
    page: Option<i64>,
    text: Option<String>,
    body: Option<String>,
    priority: Option<u32>,
    repeat: Option<Value>,
}

/// An event with its place on the device's calendar.
struct Placed<'a> {
    at: i64,
    index: usize,
    date: String,
    offset_min: i32,
    event: &'a Event,
}

/// Replays the fixture through `driver` in time order, each event at its own local time in `zone`,
/// on the day it names relative to `now`'s local day. Events after `now` are left out (they have not
/// happened yet). Leaves the clock at `now`. Returns the ids the commands gave, by key.
#[cfg_attr(not(test), allow(dead_code))]
pub fn load(driver: &impl Driver, now: i64, zone: &str) -> Result<Loaded, String> {
    load_except(driver, now, zone, None)
}

/// As `load`, but no event of fixture day `leave_out` (days from the day `now` is on) is written.
/// Its events are counted in `left_out`. Every other day is written as `load` writes it.
pub fn load_except(
    driver: &impl Driver,
    now: i64,
    zone: &str,
    leave_out: Option<i64>,
) -> Result<Loaded, String> {
    let fixture: Fixture = serde_json::from_str(FIXTURE).map_err(|e| e.to_string())?;
    let today = local_time::day(now, zone).ok_or_else(|| format!("unknown time zone {zone}"))?;
    let midnight = local_time::day_start(&today, zone).ok_or_else(|| format!("bad day {today}"))?;

    let mut placed = Vec::with_capacity(fixture.events.len());
    for (index, event) in fixture.events.iter().enumerate() {
        let date = date_at(midnight, event.day, zone)?;
        let at = instant(&date, &event.at, zone)?;
        let offset_min = offset_min(&date, zone)?;
        placed.push(Placed {
            at,
            index,
            date,
            offset_min,
            event,
        });
    }
    placed.sort_by_key(|p| (p.at, p.index));
    let skipped = placed.iter().filter(|p| p.at > now).count();
    placed.retain(|p| p.at <= now);
    let before_leave_out = placed.len();
    if let Some(day) = leave_out {
        placed.retain(|p| p.event.day != day);
    }
    let left_out = before_leave_out - placed.len();
    // `restamp` tells this event's writes from earlier ones by comparing with the system clock, so
    // every event must lie before it.
    if placed.last().is_some_and(|p| p.at >= now_ms()) {
        return Err("the fixture has an event after the system clock".to_owned());
    }

    let mut loaded = Loaded {
        ids: HashMap::new(),
        events: placed.len(),
        skipped,
        left_out,
    };
    for p in &placed {
        driver.set_clock(p.at);
        let real_before = now_ms();
        run(driver, p, midnight, zone, &mut loaded.ids)?;
        restamp(driver, p.at, real_before)?;
    }
    driver.set_clock(now);
    Ok(loaded)
}

/// Runs one event through its command, recording the id a create returns under the event's key.
fn run(
    driver: &impl Driver,
    p: &Placed,
    midnight: i64,
    zone: &str,
    ids: &mut HashMap<String, String>,
) -> Result<(), String> {
    let e = p.event;
    let key = e.key.clone().unwrap_or_default();
    let local = json!({ "today": p.date, "now": e.at, "offset_min": p.offset_min });
    let call = |command: &str, args: Value| {
        driver
            .invoke(command, args)
            .map_err(|err| format!("{} ({key}): {err}", e.op))
    };
    let list = e
        .list
        .as_deref()
        .map(|list_key| lookup(ids, list_key))
        .transpose()?;
    let due = e.due.map(|day| date_at(midnight, day, zone)).transpose()?;
    match e.op.as_str() {
        "list" => {
            let made = call("todo_list_create", json!({ "name": e.name }))?;
            ids.insert(key.clone(), id_of(&made)?);
        }
        "task_add" => {
            let when = due.map(|date| json!({ "date": date, "time": null, "local": local }));
            let made = call(
                "todo_task_create",
                json!({ "title": e.title, "listId": list, "when": when, "details": null }),
            )?;
            ids.insert(key.clone(), id_of(&made)?);
        }
        "task_when" => {
            let id = lookup(ids, &key)?;
            call(
                "todo_task_set_when",
                json!({ "id": id, "date": due, "time": null, "repeat": e.repeat, "local": local }),
            )?;
        }
        "task_priority" => {
            let id = lookup(ids, &key)?;
            call(
                "todo_task_set_priority",
                json!({ "id": id, "priority": e.priority }),
            )?;
        }
        "task_move" => {
            let id = lookup(ids, &key)?;
            call("todo_task_move", json!({ "id": id, "listId": list }))?;
        }
        "task_complete" => {
            let id = lookup(ids, &key)?;
            call("todo_task_complete", json!({ "id": id, "local": local }))?;
        }
        "task_wont_do" => {
            let id = lookup(ids, &key)?;
            call("todo_task_wont_do", json!({ "id": id, "local": local }))?;
        }
        "task_skip" => {
            let id = lookup(ids, &key)?;
            call("todo_task_skip", json!({ "id": id, "local": local }))?;
        }
        "task_delete" => {
            let id = lookup(ids, &key)?;
            call("todo_task_delete", json!({ "id": id }))?;
        }
        "comment" => {
            let id = lookup(ids, &key)?;
            let body = e.body.as_deref().unwrap_or_default();
            call("todo_comment_add", json!({ "taskId": id, "body": body }))?;
        }
        "entry" => {
            let page = date_at(midnight, e.page.unwrap_or(e.day), zone)?;
            let entry = json!({
                "page_date": page,
                "text": e.text,
                "written_tz": zone,
                "written_offset_min": p.offset_min,
                "session_id": key,
            });
            let made = call("journal_entry_create", json!({ "entry": entry }))?;
            ids.insert(key.clone(), id_of(&made)?);
        }
        "entry_edit" => {
            let id = lookup(ids, &key)?;
            let session = format!("edit-{key}");
            call(
                "journal_entry_update",
                json!({ "id": id, "text": e.text, "sessionId": session }),
            )?;
        }
        "entry_delete" => {
            let id = lookup(ids, &key)?;
            call("journal_entry_delete", json!({ "id": id }))?;
        }
        other => return Err(format!("the fixture has an unknown event: {other}")),
    }
    Ok(())
}

/// The Task services stamp `created_at`, `updated_at`, `closed_at` and Comment times from the system
/// clock, not the database clock the change log uses. Moves what this event just stamped to its
/// instant. Only values at or after `real_before` were stamped by this event; earlier events' values
/// were moved already, to instants before the system clock.
fn restamp(driver: &impl Driver, at: i64, real_before: i64) -> Result<(), String> {
    let mut outcome = Ok(());
    driver.with_db(&mut |conn| {
        outcome = conn
            .execute(
                "UPDATE todo_tasks SET
                   created_at = CASE WHEN created_at >= ?2 THEN ?1 ELSE created_at END,
                   updated_at = CASE WHEN updated_at >= ?2 THEN ?1 ELSE updated_at END,
                   closed_at = CASE WHEN closed_at >= ?2 THEN ?1 ELSE closed_at END",
                (at, real_before),
            )
            .and_then(|_| {
                conn.execute(
                    "UPDATE todo_comments SET created_at = CASE WHEN created_at >= ?2 THEN ?1 ELSE created_at END",
                    (at, real_before),
                )
            })
            .map(|_| ())
            .map_err(|e| e.to_string());
    });
    outcome
}

/// The id a create command returned.
fn id_of(made: &Value) -> Result<String, String> {
    made["id"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("no id in {made}"))
}

fn lookup(ids: &HashMap<String, String>, key: &str) -> Result<String, String> {
    ids.get(key)
        .cloned()
        .ok_or_else(|| format!("the fixture refers to {key} before writing it"))
}

/// The day `day` days from the day that starts at `midnight`, as `YYYY-MM-DD` in `zone`. Noon of
/// the day is used so that a daylight-saving change cannot move the date.
fn date_at(midnight: i64, day: i64, zone: &str) -> Result<String, String> {
    local_time::day(midnight + day * DAY_MS + DAY_MS / 2, zone)
        .ok_or_else(|| format!("no day {day} from {midnight} in {zone}"))
}

/// The instant of `HH:MM` on `date` in `zone`.
fn instant(date: &str, at: &str, zone: &str) -> Result<i64, String> {
    let midnight = local_time::day_start(date, zone).ok_or_else(|| format!("bad day {date}"))?;
    let (hours, minutes) = at
        .split_once(':')
        .and_then(|(h, m)| Some((h.parse::<i64>().ok()?, m.parse::<i64>().ok()?)))
        .ok_or_else(|| format!("bad time {at}"))?;
    Ok(midnight + (hours * 60 + minutes) * MINUTE_MS)
}

/// Minutes `zone` is ahead of UTC on `date`, as the todo commands take it (negative behind).
fn offset_min(date: &str, zone: &str) -> Result<i32, String> {
    let local = local_time::day_start(date, zone).ok_or_else(|| format!("bad day {date}"))?;
    let utc = local_time::day_start(date, "UTC").ok_or_else(|| "UTC unknown".to_owned())?;
    i32::try_from(-((local - utc) / MINUTE_MS)).map_err(|e| e.to_string())
}
