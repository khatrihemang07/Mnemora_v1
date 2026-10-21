//! Due dates and the date views: setting a Task's When, moving Overdue Tasks, and the ready-made
//! groups behind Today and Upcoming. The UI says what today is (`LocalNow`); nothing here reads a
//! clock for dates. Task text must never reach a log line.

use std::cmp::Ordering;

use mnemora_core::changelog::ChangeOp;
use mnemora_core::error::AppResult;
use mnemora_core::util::now_ms;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::TodoError;
use crate::recurrence::{RepeatRule, Rule};
use crate::repeat::rule_of;
use crate::repo;
use crate::service::record_at;
use crate::task::{Task, TaskState};
use crate::when::{LocalNow, resolve_when, validate_date, validate_pickable, validate_time};

/// Today: what is late, what is due, and what is already done.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct TodayView {
    /// Open Tasks due before today, oldest date first.
    pub overdue: Vec<Task>,
    /// Open Tasks due today: timed ones by time, then the rest in manual order. A timed Task whose
    /// time has passed stays here.
    pub tasks: Vec<Task>,
    /// Tasks done today, earliest first. Won't do is not counted.
    pub done: Vec<Task>,
    /// Overdue, due and done today together: the "of M" in "N of M done".
    pub total: u32,
}

/// The Tasks due on one day of Upcoming.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct UpcomingDay {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Timed Tasks by time, then the rest in manual order.
    pub tasks: Vec<Task>,
}

/// Upcoming: Overdue on top, then only the days from tomorrow on that have Tasks.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct UpcomingView {
    /// Open Tasks due before today, oldest date first.
    pub overdue: Vec<Task>,
    /// Days with Tasks, earliest first.
    pub days: Vec<UpcomingDay>,
}

/// The number of open Tasks due on one date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct DayCount {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Open Tasks due that day.
    pub count: u32,
}

/// A Task's Due date, time and Repeat as they were, for Undo. A past date can only come back this
/// way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DueChange {
    /// The Task.
    pub id: String,
    /// Due date, `YYYY-MM-DD`.
    pub due_date: Option<String>,
    /// Due time, `HH:MM`.
    pub due_time: Option<String>,
    /// The Repeat, which goes with the date.
    pub repeat: Option<RepeatRule>,
}

/// What "Move all" did: hand `previous` to `todo_tasks_restore_when` to undo it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct OverdueMove {
    /// The date they were moved to.
    pub to_date: String,
    /// The Tasks moved, with the date and time each had.
    pub previous: Vec<DueChange>,
}

/// Timed Tasks first, by time; then the rest. Within one time block (the same time, or no time)
/// the more urgent Priority goes first; ties keep the manual order.
fn in_day(a: &Task, b: &Task) -> Ordering {
    let by_time = match (&a.due_time, &b.due_time) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    by_time
        .then_with(|| a.priority.cmp(&b.priority))
        .then_with(|| a.sort_key.cmp(&b.sort_key))
        .then_with(|| a.id.cmp(&b.id))
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// A Task's Repeat as it is stored, settled.
fn stored_rule(task: &Task) -> AppResult<Option<Rule>> {
    rule_of(task)
}

/// Puts a new When and Repeat on a Task in memory and says whether anything changed. Without a
/// date there is no Repeat. A month or year Repeat keeps to the day of the month of its date, so
/// a new date is the new anchor; `keep_day` leaves the day it has.
pub(crate) fn assign_when(
    task: &mut Task,
    date: Option<String>,
    time: Option<String>,
    rule: Option<Rule>,
    keep_day: bool,
) -> AppResult<bool> {
    let rule = match (&date, rule) {
        (Some(date), Some(rule)) if !(keep_day && rule.has_day()) => Some(rule.anchored_on(date)),
        (Some(_), rule) => rule,
        (None, _) => None,
    };
    let wire = rule.map(Rule::to_wire);
    let before = stored_rule(task)?.map(Rule::to_wire);
    if task.due_date == date && task.due_time == time && before == wire {
        return Ok(false);
    }
    task.due_date = date;
    task.due_time = time;
    task.repeat_freq = wire.as_ref().map(|r| r.freq.clone());
    task.repeat_every = wire.as_ref().map(|r| r.every);
    task.repeat_days = wire.as_ref().and_then(|r| r.days);
    task.repeat_mode = wire.map(|r| r.mode);
    Ok(true)
}

/// Writes a new When and Repeat onto an open Task; logs it unless nothing changes. Returns
/// whether it did.
fn apply(
    conn: &Connection,
    task: &mut Task,
    date: Option<String>,
    time: Option<String>,
    rule: Option<Rule>,
    keep_day: bool,
) -> AppResult<bool> {
    if !assign_when(task, date, time, rule, keep_day)? {
        return Ok(false);
    }
    task.updated_at = now_ms();
    repo::update_when(conn, task)?;
    record_at(conn, task, ChangeOp::Update, None)?;
    Ok(true)
}

/// Turns a pick into what a Task stores: a time without a date means today if still ahead,
/// otherwise tomorrow; a Repeat without a date gives the first day it lands on; past dates are
/// refused.
pub(crate) fn settle_when(
    date: Option<&str>,
    time: Option<&str>,
    repeat: Option<&RepeatRule>,
    local: &LocalNow,
) -> AppResult<(Option<String>, Option<String>, Option<Rule>)> {
    let rule = repeat.map(RepeatRule::check).transpose()?;
    let (date, time) = match (date, rule) {
        (None, Some(rule)) => {
            local.check()?;
            let time = time.map(validate_time).transpose()?.map(str::to_owned);
            let first = rule
                .first_match(&local.today)
                .ok_or(TodoError::Validation("Not a valid date"))?;
            (Some(first), time)
        }
        _ => resolve_when(date, time, local)?,
    };
    Ok((date, time, rule))
}

/// Sets a Task's Due date, time and Repeat, or clears them (no date, no time, no Repeat). A time
/// without a date means today if still ahead, otherwise tomorrow; a Repeat without a date gives
/// the first day it lands on. Past dates are refused. Setting what is already there changes and
/// logs nothing. A date changed by hand is the new anchor of its Repeat; removing the date
/// removes the Repeat.
pub fn set_when(
    conn: &mut Connection,
    id: &str,
    date: Option<&str>,
    time: Option<&str>,
    repeat: Option<&RepeatRule>,
    local: &LocalNow,
) -> AppResult<Task> {
    let (date, time, rule) = settle_when(date, time, repeat, local)?;
    let tx = conn.transaction()?;
    let mut task = repo::get(&tx, id)?.ok_or(TodoError::TaskNotFound)?;
    if task.state != TaskState::Open {
        return Err(TodoError::Validation("That Task is already closed").into());
    }
    let keep_day = task.due_date == date;
    apply(&tx, &mut task, date, time, rule, keep_day)?;
    tx.commit()?;
    Ok(task)
}

/// Puts Due dates and times back as they were (Undo), past dates included. Tasks that are gone
/// are skipped. Each change is logged.
pub fn restore_when(conn: &mut Connection, changes: &[DueChange]) -> AppResult<Vec<Task>> {
    let mut rules = Vec::with_capacity(changes.len());
    for c in changes {
        c.due_date.as_deref().map(validate_date).transpose()?;
        c.due_time.as_deref().map(validate_time).transpose()?;
        if c.due_time.is_some() && c.due_date.is_none() {
            return Err(TodoError::Validation("A time needs a date").into());
        }
        rules.push(c.repeat.as_ref().map(RepeatRule::check).transpose()?);
    }
    let tx = conn.transaction()?;
    let mut restored = Vec::with_capacity(changes.len());
    for (c, rule) in changes.iter().zip(rules) {
        let Some(mut task) = repo::get(&tx, &c.id)? else {
            continue;
        };
        apply(
            &tx,
            &mut task,
            c.due_date.clone(),
            c.due_time.clone(),
            rule,
            true,
        )?;
        restored.push(task);
    }
    tx.commit()?;
    Ok(restored)
}

/// Moves every Overdue Task to `date` (today or later), keeping each one's time. Nothing overdue
/// moves nothing. Each change is logged.
pub fn move_overdue(conn: &mut Connection, date: &str, local: &LocalNow) -> AppResult<OverdueMove> {
    local.check()?;
    validate_pickable(date, &local.today)?;
    let tx = conn.transaction()?;
    let mut previous = Vec::new();
    for mut task in repo::overdue(&tx, &local.today)? {
        let rule = stored_rule(&task)?;
        previous.push(DueChange {
            id: task.id.clone(),
            due_date: task.due_date.clone(),
            due_time: task.due_time.clone(),
            repeat: rule.map(Rule::to_wire),
        });
        let time = task.due_time.clone();
        apply(&tx, &mut task, Some(date.to_owned()), time, rule, false)?;
    }
    tx.commit()?;
    Ok(OverdueMove {
        to_date: date.to_owned(),
        previous,
    })
}

/// Today's groups. Overdue is only ever by date: a Task due today stays in Today when its time
/// has passed.
pub fn today(conn: &Connection, local: &LocalNow) -> AppResult<TodayView> {
    local.check()?;
    let (start, end) = local.today_range_ms()?;
    let (mut overdue, mut tasks) = (Vec::new(), Vec::new());
    for task in repo::open_dated(conn)? {
        match task
            .due_date
            .as_deref()
            .map(|d| d.cmp(local.today.as_str()))
        {
            Some(Ordering::Less) => overdue.push(task),
            Some(Ordering::Equal) => tasks.push(task),
            _ => {}
        }
    }
    overdue.sort_by(by_date_then_day);
    tasks.sort_by(in_day);
    let done = repo::done_between(conn, start, end)?;
    Ok(TodayView {
        total: count(overdue.len() + tasks.len() + done.len()),
        overdue,
        tasks,
        done,
    })
}

fn by_date_then_day(a: &Task, b: &Task) -> Ordering {
    a.due_date.cmp(&b.due_date).then_with(|| in_day(a, b))
}

/// Upcoming's groups: Overdue, then each day after today that has Tasks.
pub fn upcoming(conn: &Connection, local: &LocalNow) -> AppResult<UpcomingView> {
    local.check()?;
    let mut overdue = Vec::new();
    let mut future = Vec::new();
    for task in repo::open_dated(conn)? {
        match task
            .due_date
            .as_deref()
            .map(|d| d.cmp(local.today.as_str()))
        {
            Some(Ordering::Less) => overdue.push(task),
            Some(Ordering::Greater) => future.push(task),
            _ => {}
        }
    }
    overdue.sort_by(by_date_then_day);
    future.sort_by(by_date_then_day);
    let mut days: Vec<UpcomingDay> = Vec::new();
    for task in future {
        let date = task.due_date.clone().unwrap_or_default();
        match days.last_mut() {
            Some(day) if day.date == date => day.tasks.push(task),
            _ => days.push(UpcomingDay {
                date,
                tasks: vec![task],
            }),
        }
    }
    Ok(UpcomingView { overdue, days })
}

/// Open Tasks per date in `[from, to]` (only dates that have any), leaving out one Task: the When
/// picker's dots, which should not count the Task being dated.
pub fn day_counts(
    conn: &Connection,
    from: &str,
    to: &str,
    exclude: Option<&str>,
) -> AppResult<Vec<DayCount>> {
    validate_date(from)?;
    validate_date(to)?;
    if from > to {
        return Err(TodoError::Validation("The range ends before it starts").into());
    }
    Ok(repo::day_counts(conn, from, to, exclude)?
        .into_iter()
        .map(|(date, count)| DayCount { date, count })
        .collect())
}
