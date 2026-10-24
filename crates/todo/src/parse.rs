//! The natural-language parser behind the capture box: reads dates, times, Repeats, `#List` and
//! `p1`-`p4` out of typed text. Pure, with no clock of its own: the caller says what today, now
//! and the Week start are. The browser mock carries a TypeScript copy (`src/lib/ipc/mock-todo-parse.ts`);
//! both run `src/lib/ipc/fixtures/parse-cases.json` (ADR 0009).
//!
//! Every candidate of every kind is collected first (text inside quotes or backticks is never
//! read, and neither are numeric dates or bare numbers), then one winner per kind is chosen: the
//! last one, unless the user picked another ("use instead"). The losers come back as alternatives
//! so the UI can offer them; words the user kept as text come back as `ignored` so the UI can
//! offer to read them again. Titles keep the typed words: nothing here rewrites the text.
//!
//! Offsets are UTF-16 code units, because the text lives in a JavaScript string.

use std::collections::HashMap;

use mnemora_core::text::fold;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::list::MAX_LIST_NAME_CHARS;
use crate::recurrence::{RepeatRule, Rule};
use crate::when::{add_days, epoch_day};

/// Weekday bits for "every weekday": Monday to Friday, Sunday being bit 0.
const WEEKDAYS: u32 = 0b011_1110;
/// What "tonight" means as a time.
const TONIGHT: &str = "20:00";
/// The order kinds are settled in.
const KINDS: [&str; 5] = ["repeat", "date", "time", "prio", "list"];

/// A stretch of the text, in UTF-16 code units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub struct TextRange {
    /// First unit.
    pub start: u32,
    /// One past the last unit.
    pub end: u32,
}

/// What to read, and what the user has already decided.
#[derive(Debug, Clone, Deserialize, Type)]
pub struct ParseRequest {
    /// The typed text.
    pub text: String,
    /// Keys of words the user keeps as text.
    #[serde(default)]
    pub ignored: Vec<String>,
    /// Per kind, the key of the word the user picked over the last one ("use instead").
    #[serde(default)]
    pub prefer: HashMap<String, String>,
    /// Stretches already read before (Quick edit's old title): candidates inside them are dropped.
    #[serde(default)]
    pub frozen: Vec<TextRange>,
    /// Today, `YYYY-MM-DD`.
    pub today: String,
    /// The time of day, `HH:MM`.
    pub now: String,
    /// `mon` or `sun`: the first day of the week, which "next week" and "next fri" count from.
    pub week_start: String,
}

/// A List the parser may match `#Name` against, in the List order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRef {
    /// The List.
    pub id: String,
    /// Its name.
    pub name: String,
}

/// A recognised word and what it means.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ParseSpan {
    /// Names the word across parses: `kind:word:n` for the nth such word in the text.
    pub key: String,
    /// `date`, `time`, `repeat`, `prio` or `list`.
    pub kind: String,
    /// Where it starts in the text.
    pub start: u32,
    /// Where it ends.
    pub end: u32,
    /// The words as typed.
    pub word: String,
    /// The meaning: a `YYYY-MM-DD` date, an `HH:MM` time, the Repeat as JSON, a Priority number
    /// or a List's name.
    pub value: String,
    /// A List that does not exist yet and is created when the Task is added.
    pub new_list: bool,
}

/// A word that was recognised but did not win.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ParseAlternative {
    /// The word.
    #[serde(flatten)]
    pub span: ParseSpan,
    /// Why it lost: `conflict` (a later one of the same kind won).
    pub reason: String,
}

/// What the text means.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct Parsed {
    /// The words that won, first to last.
    pub spans: Vec<ParseSpan>,
    /// The words that lost.
    pub alternatives: Vec<ParseAlternative>,
    /// The words the user keeps as text, which would have been read.
    pub ignored: Vec<ParseSpan>,
    /// The Due date: a date word, or today/tomorrow for a time alone.
    pub date: Option<String>,
    /// The Due time.
    pub time: Option<String>,
    /// The Repeat.
    pub repeat: Option<RepeatRule>,
    /// 1 to 4.
    pub priority: Option<u32>,
    /// The List a `#Name` matched.
    pub list_id: Option<String>,
    /// A `#Name` that matched nothing: the List to create.
    pub new_list: Option<String>,
}

/// What a candidate sets.
#[derive(Debug, Clone)]
enum Meaning {
    Date(String),
    Time(String),
    Repeat(RepeatRule),
    Prio(u32),
    List { id: Option<String>, name: String },
}

impl Meaning {
    fn kind(&self) -> &'static str {
        match self {
            Self::Date(_) => "date",
            Self::Time(_) => "time",
            Self::Repeat(_) => "repeat",
            Self::Prio(_) => "prio",
            Self::List { .. } => "list",
        }
    }

    fn value(&self) -> String {
        match self {
            Self::Date(v) | Self::Time(v) => v.clone(),
            Self::Repeat(r) => serde_json::to_string(r).unwrap_or_default(),
            Self::Prio(p) => p.to_string(),
            Self::List { name, .. } => name.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct Cand {
    /// UTF-16 range of the words.
    start: u32,
    end: u32,
    word: String,
    meaning: Meaning,
    key: String,
}

impl Cand {
    fn span(&self) -> ParseSpan {
        ParseSpan {
            key: self.key.clone(),
            kind: self.meaning.kind().into(),
            start: self.start,
            end: self.end,
            word: self.word.clone(),
            value: self.meaning.value(),
            new_list: matches!(self.meaning, Meaning::List { id: None, .. }),
        }
    }
}

/// A whitespace-separated word with the punctuation around it set aside.
struct Token {
    /// First and last (exclusive) char of the core, in chars.
    from: usize,
    to: usize,
    /// The core, lowercased.
    low: String,
    /// The core as typed.
    raw: String,
    /// A comma followed the core.
    comma: bool,
    /// Inside a quote or backticks.
    masked: bool,
}

fn mask_ranges(chars: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let close = match chars[i] {
            '"' => '"',
            '“' => '”',
            '`' => '`',
            _ => {
                i += 1;
                continue;
            }
        };
        match chars[i + 1..].iter().position(|&c| c == close) {
            Some(n) => {
                out.push((i, i + 1 + n + 1));
                i += n + 2;
            }
            None => i += 1,
        }
    }
    out
}

fn tokenize(chars: &[char]) -> Vec<Token> {
    let masks = mask_ranges(chars);
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let a = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        let (mut from, mut to) = (a, i);
        while from < to && matches!(chars[from], '(' | '[' | '{') {
            from += 1;
        }
        let mut comma = false;
        while to > from
            && matches!(
                chars[to - 1],
                '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}'
            )
        {
            comma |= chars[to - 1] == ',';
            to -= 1;
        }
        if from == to {
            continue;
        }
        let raw: String = chars[from..to].iter().collect();
        out.push(Token {
            from,
            to,
            low: raw.to_lowercase(),
            raw,
            comma,
            masked: masks.iter().any(|&(s, e)| a < e && i > s),
        });
    }
    out
}

fn weekday_of(date: &str) -> u32 {
    u32::try_from((epoch_day(date).unwrap_or(0) + 4).rem_euclid(7)).unwrap_or(0)
}

fn weekday_name(word: &str) -> Option<u32> {
    Some(match word {
        "sun" | "sunday" => 0,
        "mon" | "monday" => 1,
        "tue" | "tues" | "tuesday" => 2,
        "wed" | "wednesday" => 3,
        "thu" | "thur" | "thurs" | "thursday" => 4,
        "fri" | "friday" => 5,
        "sat" | "saturday" => 6,
        _ => return None,
    })
}

fn month_name(word: &str) -> Option<u32> {
    Some(match word {
        "jan" | "january" => 1,
        "feb" | "february" => 2,
        "mar" | "march" => 3,
        "apr" | "april" => 4,
        "may" => 5,
        "jun" | "june" => 6,
        "jul" | "july" => 7,
        "aug" | "august" => 8,
        "sep" | "sept" | "september" => 9,
        "oct" | "october" => 10,
        "nov" | "november" => 11,
        "dec" | "december" => 12,
        _ => return None,
    })
}

/// A number written with plain digits, at most `max_len` of them.
fn digits(word: &str, max_len: usize) -> Option<u32> {
    (!word.is_empty() && word.len() <= max_len && word.bytes().all(|b| b.is_ascii_digit()))
        .then(|| word.parse().ok())
        .flatten()
}

fn ymd(date: &str) -> (i64, u32, u32) {
    (
        date[0..4].parse().unwrap_or(0),
        date[5..7].parse().unwrap_or(0),
        date[8..10].parse().unwrap_or(0),
    )
}

/// The calendar and clock the words are read against.
struct Context<'a> {
    today: &'a str,
    /// 1 for Monday, 0 for Sunday.
    week_start: u32,
    lists: &'a [ListRef],
}

impl Context<'_> {
    fn start_of_week(&self, date: &str) -> String {
        let back = (weekday_of(date) + 7 - self.week_start) % 7;
        add_days(date, -i64::from(back)).unwrap_or_default()
    }

    /// A day and month, with a year or without: the next time that day comes round.
    fn named_date(&self, day: u32, month: u32, year: Option<i64>) -> Option<String> {
        let make = |y: i64| {
            let date = format!("{y:04}-{month:02}-{day:02}");
            epoch_day(&date).map(|_| date)
        };
        if let Some(year) = year {
            return make(year).filter(|d| d.as_str() >= self.today);
        }
        let (this_year, ..) = ymd(self.today);
        (this_year..this_year + 8)
            .filter_map(make)
            .find(|d| d.as_str() >= self.today)
    }

    fn next_month(&self) -> String {
        let (y, m, _) = ymd(self.today);
        let (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
        format!("{y:04}-{m:02}-01")
    }
}

/// How many tokens a match used, and what it set.
type Match = (usize, Vec<Meaning>);

fn rule(freq: &str, every: u32, days: u32, after: bool) -> Option<RepeatRule> {
    let rule = RepeatRule {
        freq: freq.into(),
        every,
        days: (days != 0).then_some(days),
        mode: if after { "after" } else { "fixed" }.into(),
    };
    rule.check().ok().map(Rule::to_wire)
}

fn unit_of(word: &str) -> Option<&'static str> {
    Some(match word.strip_suffix('s').unwrap_or(word) {
        "day" => "day",
        "week" => "week",
        "month" => "month",
        "year" => "year",
        _ => return None,
    })
}

/// The weekdays listed from token `from`: "mon", "mon and thu", "mon, wed and fri". Returns how many
/// tokens they take and their weekday bits.
fn weekday_list(t: &[Token], from: usize) -> Option<(usize, u32)> {
    let name_at = |n: usize| t.get(n).and_then(|x| weekday_name(&x.low));
    let mut mask = 1u32 << name_at(from)?;
    let mut last = from;
    loop {
        let mut next = last + 1;
        let mut separated = t[last].comma;
        if matches!(t.get(next).map(|x| x.low.as_str()), Some("and" | "&")) {
            next += 1;
            separated = true;
        }
        match name_at(next) {
            Some(day) if separated => {
                mask |= 1 << day;
                last = next;
            }
            _ => break,
        }
    }
    Some((last + 1 - from, mask))
}

/// "daily", "every 2 weeks", "every mon and thu", "every weekday", each optionally "after done".
fn read_repeat(t: &[Token], i: usize) -> Option<Match> {
    let at = |n: usize| t.get(n).map(|x| x.low.as_str());
    let (mut used, parts): (usize, (&str, u32, u32)) = match at(i)? {
        "daily" => (1, ("day", 1, 0)),
        "weekly" => (1, ("week", 1, 0)),
        "monthly" => (1, ("month", 1, 0)),
        "yearly" => (1, ("year", 1, 0)),
        "every" => match (at(i + 1)?, at(i + 2)) {
            ("weekday" | "weekdays", _) => (2, ("week", 1, WEEKDAYS)),
            ("other", Some(unit)) => (3, (unit_of(unit)?, 2, 0)),
            (n, Some(unit)) if digits(n, 2).is_some() => (3, (unit_of(unit)?, digits(n, 2)?, 0)),
            (unit, _) if unit_of(unit).is_some() && !unit.ends_with('s') => {
                (2, (unit_of(unit)?, 1, 0))
            }
            (_, _) => {
                let (words, mask) = weekday_list(t, i + 1)?;
                (1 + words, ("week", 1, mask))
            }
        },
        _ => return None,
    };
    let after = at(i + used) == Some("after") && at(i + used + 1) == Some("done");
    // "After done" counts from an interval, never from chosen weekdays.
    let after = after && parts.2 == 0;
    if after {
        used += 2;
    }
    Some((
        used,
        vec![Meaning::Repeat(rule(parts.0, parts.1, parts.2, after)?)],
    ))
}

fn read_date(t: &[Token], i: usize, cx: &Context) -> Option<Match> {
    let at = |n: usize| t.get(n).map(|x| x.low.as_str());
    let today = cx.today;
    let date = |used: usize, d: String| Some((used, vec![Meaning::Date(d)]));
    let first = at(i)?;
    match first {
        "today" | "tod" => return date(1, today.into()),
        "tomorrow" | "tmr" | "tmrw" | "tom" => return date(1, add_days(today, 1)?),
        "tonight" => {
            return Some((
                1,
                vec![Meaning::Date(today.into()), Meaning::Time(TONIGHT.into())],
            ));
        }
        "on" => {
            let w = weekday_name(at(i + 1)?)?;
            return date(
                2,
                add_days(today, i64::from((w + 7 - weekday_of(today)) % 7))?,
            );
        }
        "next" => {
            let next_week = add_days(&cx.start_of_week(today), 7)?;
            return match at(i + 1)? {
                "week" => date(2, next_week),
                "month" => date(2, cx.next_month()),
                word => {
                    let w = weekday_name(word)?;
                    date(
                        2,
                        add_days(&next_week, i64::from((w + 7 - cx.week_start) % 7))?,
                    )
                }
            };
        }
        "this" if at(i + 1) == Some("weekend") => {
            let wd = weekday_of(today);
            let to_sat = if wd == 0 || wd == 6 {
                0
            } else {
                i64::from(6 - wd)
            };
            return date(2, add_days(today, to_sat)?);
        }
        "in" => {
            let n = i64::from(digits(at(i + 1)?, 3).filter(|n| *n > 0)?);
            let per = match unit_of(at(i + 2)?)? {
                "day" => 1,
                "week" => 7,
                _ => return None,
            };
            return date(3, add_days(today, n * per)?);
        }
        _ => {}
    }
    if let Some(w) = weekday_name(first) {
        return date(
            1,
            add_days(today, i64::from((w + 7 - weekday_of(today)) % 7))?,
        );
    }
    // "21 oct", "21st october 2027" and "oct 21", "oct 21st 2027".
    let day_of = |word: &str| {
        let n = word.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        let suffix = &word[n.len()..];
        matches!(suffix, "" | "st" | "nd" | "rd" | "th")
            .then(|| digits(n, 2))
            .flatten()
            .filter(|d| (1..=31).contains(d))
    };
    let (day, month) = match (day_of(first), month_name(first)) {
        (Some(d), _) => (d, month_name(at(i + 1)?)?),
        (None, Some(m)) => (day_of(at(i + 1)?)?, m),
        _ => return None,
    };
    let year = at(i + 2).and_then(|y| digits(y, 4).filter(|_| y.len() == 4));
    match year {
        Some(y) => date(3, cx.named_date(day, month, Some(i64::from(y)))?),
        None => date(2, cx.named_date(day, month, None)?),
    }
}

/// `9am`, `9:30pm`: the hour on a 12-hour clock, then am or pm.
fn am_pm(word: &str) -> Option<(u32, u32, bool)> {
    let (body, pm) = match (word.strip_suffix("am"), word.strip_suffix("pm")) {
        (Some(b), _) => (b, false),
        (_, Some(b)) => (b, true),
        _ => return None,
    };
    hour_minute(body).map(|(h, m)| (h, m, pm))
}

/// `9` or `9:30`.
fn hour_minute(word: &str) -> Option<(u32, u32)> {
    match word.split_once(':') {
        Some((h, m)) if m.len() == 2 => Some((digits(h, 2)?, digits(m, 2)?)),
        Some(_) => None,
        None => Some((digits(word, 2)?, 0)),
    }
}

fn clock(hour: u32, minute: u32) -> Option<String> {
    (hour < 24 && minute < 60).then(|| format!("{hour:02}:{minute:02}"))
}

fn twelve(hour: u32, minute: u32, pm: bool) -> Option<String> {
    if !(1..=12).contains(&hour) {
        return None;
    }
    clock(hour % 12 + if pm { 12 } else { 0 }, minute)
}

/// "9am", "9:30pm", "9 pm", "21:30", "noon", "midnight", "at 9".
fn read_time(t: &[Token], i: usize) -> Option<Match> {
    let at = |n: usize| t.get(n).map(|x| x.low.as_str());
    let at_prefix = at(i) == Some("at");
    let k = i + usize::from(at_prefix);
    let found = |used: usize, time: String| Some((used, vec![Meaning::Time(time)]));
    let word = at(k)?;
    let used = |n: usize| n + usize::from(at_prefix);
    match word {
        "noon" => return found(used(1), "12:00".into()),
        "midnight" => return found(used(1), "00:00".into()),
        _ => {}
    }
    if let Some((h, m, pm)) = am_pm(word) {
        return found(used(1), twelve(h, m, pm)?);
    }
    // "9 pm": the hour, then am or pm on its own.
    if let (Some((h, m)), Some(mer @ ("am" | "pm"))) = (hour_minute(word), at(k + 1)) {
        return found(used(2), twelve(h, m, mer == "pm")?);
    }
    if let Some((h, m)) = word.split_once(':').and_then(|_| hour_minute(word)) {
        return found(used(1), clock(h, m)?);
    }
    // A bare hour is a time only after "at", and not when a month follows ("at 21 oct").
    if at_prefix && at(k + 1).and_then(month_name).is_none() {
        return found(used(1), clock(digits(word, 2)?, 0)?);
    }
    None
}

fn read_prio(t: &[Token], i: usize) -> Option<Match> {
    match t[i].low.as_str() {
        "p1" => Some(1),
        "p2" => Some(2),
        "p3" => Some(3),
        "p4" => Some(4),
        _ => None,
    }
    .map(|p| (1, vec![Meaning::Prio(p)]))
}

/// The part of a List name a `#tag` can spell: letters, digits, `_` and `-`, folded.
fn tag_key(name: &str) -> String {
    fold(name)
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '_' | '-'))
        .collect()
}

fn read_list(t: &[Token], i: usize, cx: &Context) -> Option<Match> {
    let name = t[i].raw.strip_prefix('#')?;
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-'))
    {
        return None;
    }
    let typed = tag_key(name);
    let hit = cx
        .lists
        .iter()
        .find(|l| tag_key(&l.name) == typed)
        .or_else(|| {
            cx.lists
                .iter()
                .find(|l| tag_key(&l.name).starts_with(&typed))
        });
    let meaning = match hit {
        Some(l) => Meaning::List {
            id: Some(l.id.clone()),
            name: l.name.clone(),
        },
        // A tag of digits only ("issue #123") is a reference, not a List to create.
        None if name.chars().all(|c| c.is_ascii_digit()) => return None,
        None if name.chars().count() > MAX_LIST_NAME_CHARS => return None,
        None => Meaning::List {
            id: None,
            name: name.to_owned(),
        },
    };
    Some((1, vec![meaning]))
}

/// Every candidate in the text, left to right, none overlapping.
fn candidates(chars: &[char], cx: &Context) -> Vec<Cand> {
    let units = {
        let mut at = Vec::with_capacity(chars.len() + 1);
        let mut n = 0u32;
        for c in chars {
            at.push(n);
            n += u32::try_from(c.len_utf16()).unwrap_or(1);
        }
        at.push(n);
        at
    };
    let tokens = tokenize(chars);
    let mut out: Vec<Cand> = Vec::new();
    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut i = 0;
    while i < tokens.len() {
        let found = if tokens[i].masked {
            None
        } else {
            read_repeat(&tokens, i)
                .or_else(|| read_date(&tokens, i, cx))
                .or_else(|| read_time(&tokens, i))
                .or_else(|| read_prio(&tokens, i))
                .or_else(|| read_list(&tokens, i, cx))
                .filter(|(used, _)| tokens[i..i + used].iter().all(|x| !x.masked))
        };
        let Some((used, meanings)) = found else {
            i += 1;
            continue;
        };
        let (from, to) = (tokens[i].from, tokens[i + used - 1].to);
        let word: String = chars[from..to].iter().collect();
        for meaning in meanings {
            let tally = seen
                .entry(format!("{}:{}", meaning.kind(), word.to_lowercase()))
                .or_insert(0);
            *tally += 1;
            out.push(Cand {
                start: units[from],
                end: units[to],
                key: format!("{}:{}:{tally}", meaning.kind(), word.to_lowercase()),
                word: word.clone(),
                meaning,
            });
        }
        i += used;
    }
    out
}

/// Reads the text. `lists` are the Lists `#Name` may match, in their order.
pub fn parse(req: &ParseRequest, lists: &[ListRef]) -> Parsed {
    let chars: Vec<char> = req.text.chars().collect();
    let cx = Context {
        today: &req.today,
        week_start: u32::from(req.week_start != "sun"),
        lists,
    };
    let cands: Vec<Cand> = candidates(&chars, &cx)
        .into_iter()
        .filter(|c| {
            !req.frozen
                .iter()
                .any(|r| c.start >= r.start && c.end <= r.end)
        })
        .collect();
    let (ignored, live): (Vec<Cand>, Vec<Cand>) = cands
        .into_iter()
        .partition(|c| req.ignored.contains(&c.key));

    let mut won: Vec<Cand> = Vec::new();
    let mut lost: Vec<Cand> = Vec::new();
    // One winner per kind: the user's pick, else the last; earlier ones are overridden.
    for kind in KINDS {
        let of_kind: Vec<&Cand> = live.iter().filter(|c| c.meaning.kind() == kind).collect();
        let Some(winner) = req
            .prefer
            .get(kind)
            .and_then(|key| of_kind.iter().find(|c| &c.key == key))
            .or(of_kind.last())
        else {
            continue;
        };
        won.push((*winner).clone());
        lost.extend(
            of_kind
                .iter()
                .filter(|c| c.key != winner.key)
                .map(|c| (*c).clone()),
        );
    }

    let mut out = Parsed {
        spans: Vec::new(),
        alternatives: Vec::new(),
        ignored: Vec::new(),
        date: None,
        time: None,
        repeat: None,
        priority: None,
        list_id: None,
        new_list: None,
    };
    for c in &won {
        match &c.meaning {
            Meaning::Date(d) => out.date = Some(d.clone()),
            Meaning::Time(t) => out.time = Some(t.clone()),
            Meaning::Repeat(r) => out.repeat = Some(r.clone()),
            Meaning::Prio(p) => out.priority = Some(*p),
            Meaning::List { id: Some(id), .. } => out.list_id = Some(id.clone()),
            Meaning::List { name, .. } => out.new_list = Some(name.clone()),
        }
    }
    // A time alone means today if it is still ahead, otherwise tomorrow. With a Repeat the Task
    // lands on the Repeat's first day instead, which the command works out.
    if let (Some(time), None, None) = (&out.time, &out.date, &out.repeat) {
        out.date = if time.as_str() >= req.now.as_str() {
            Some(req.today.clone())
        } else {
            add_days(&req.today, 1)
        };
    }
    won.sort_by_key(|c| (c.start, c.end));
    lost.sort_by_key(|c| (c.start, c.end));
    out.spans = won.iter().map(Cand::span).collect();
    out.alternatives = lost
        .iter()
        .map(|c| ParseAlternative {
            span: c.span(),
            reason: "conflict".into(),
        })
        .collect();
    out.ignored = ignored.iter().map(Cand::span).collect();
    out
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    // 21 October 2026 is a Wednesday.
    fn request(text: &str) -> ParseRequest {
        ParseRequest {
            text: text.into(),
            ignored: vec![],
            prefer: HashMap::new(),
            frozen: vec![],
            today: "2026-10-21".into(),
            now: "14:05".into(),
            week_start: "mon".into(),
        }
    }

    fn lists() -> Vec<ListRef> {
        [
            ("l-home", "Home"),
            ("l-work", "Work"),
            ("l-side", "Side projects"),
        ]
        .map(|(id, name)| ListRef {
            id: id.into(),
            name: name.into(),
        })
        .to_vec()
    }

    fn read(text: &str) -> Parsed {
        parse(&request(text), &lists())
    }

    fn words(p: &Parsed) -> Vec<(String, String, String)> {
        p.spans
            .iter()
            .map(|s| (s.word.clone(), s.kind.clone(), s.value.clone()))
            .collect()
    }

    fn date(text: &str) -> Option<String> {
        read(text).date
    }

    #[test]
    fn relative_days() {
        assert_eq!(date("Call mum today").as_deref(), Some("2026-10-21"));
        assert_eq!(date("Call mum tomorrow").as_deref(), Some("2026-10-22"));
        assert_eq!(date("Call mum tmr").as_deref(), Some("2026-10-22"));
        let tonight = read("Dinner tonight");
        assert_eq!(tonight.date.as_deref(), Some("2026-10-21"));
        assert_eq!(tonight.time.as_deref(), Some("20:00"));
    }

    #[test]
    fn weekdays_and_the_same_weekday_meaning_today() {
        assert_eq!(date("Gym fri").as_deref(), Some("2026-10-23"));
        assert_eq!(date("Gym on friday").as_deref(), Some("2026-10-23"));
        assert_eq!(date("Gym wed").as_deref(), Some("2026-10-21"));
        assert_eq!(date("Gym on mon").as_deref(), Some("2026-10-26"));
    }

    #[test]
    fn next_weekday_is_that_day_of_next_week() {
        assert_eq!(date("Gym next fri").as_deref(), Some("2026-10-30"));
        // With weeks starting on Monday, next Sunday ends next week.
        assert_eq!(date("Gym next sun").as_deref(), Some("2026-11-01"));
        assert_eq!(date("Gym next wed").as_deref(), Some("2026-10-28"));
        let mut sun = request("Gym next sun");
        sun.week_start = "sun".into();
        assert_eq!(parse(&sun, &[]).date.as_deref(), Some("2026-10-25"));
    }

    #[test]
    fn named_dates_roll_to_next_year_when_past() {
        assert_eq!(date("Pay 21 oct").as_deref(), Some("2026-10-21"));
        assert_eq!(date("Pay oct 25").as_deref(), Some("2026-10-25"));
        assert_eq!(date("Pay 3 nov").as_deref(), Some("2026-11-03"));
        assert_eq!(date("Pay 20 oct").as_deref(), Some("2027-10-20"));
        assert_eq!(date("Pay 1st january").as_deref(), Some("2027-01-01"));
        assert_eq!(date("Pay march 5th").as_deref(), Some("2027-03-05"));
        assert_eq!(date("Pay 3 nov 2028").as_deref(), Some("2028-11-03"));
        // A year already gone cannot be picked, and "31 feb" is no date.
        assert_eq!(date("Pay 3 nov 2020"), None);
        assert_eq!(date("Pay 31 feb"), None);
        assert_eq!(date("Pay 29 feb").as_deref(), Some("2028-02-29"));
    }

    #[test]
    fn in_n_days_and_weeks() {
        assert_eq!(date("Pay in 3 days").as_deref(), Some("2026-10-24"));
        assert_eq!(date("Pay in 1 day").as_deref(), Some("2026-10-22"));
        assert_eq!(date("Pay in 2 weeks").as_deref(), Some("2026-11-04"));
        assert_eq!(date("Pay in 0 days"), None);
    }

    #[test]
    fn next_week_this_weekend_next_month() {
        assert_eq!(date("Plan next week").as_deref(), Some("2026-10-26"));
        let mut sun = request("Plan next week");
        sun.week_start = "sun".into();
        assert_eq!(parse(&sun, &[]).date.as_deref(), Some("2026-10-25"));
        assert_eq!(date("Hike this weekend").as_deref(), Some("2026-10-24"));
        assert_eq!(date("Review next month").as_deref(), Some("2026-11-01"));
        let mut dec = request("Review next month");
        dec.today = "2026-12-15".into();
        assert_eq!(parse(&dec, &[]).date.as_deref(), Some("2027-01-01"));
    }

    #[test]
    fn this_weekend_on_a_weekend_is_today() {
        let mut sat = request("Hike this weekend");
        sat.today = "2026-10-24".into();
        assert_eq!(parse(&sat, &[]).date.as_deref(), Some("2026-10-24"));
    }

    #[test]
    fn times() {
        for (text, time) in [
            ("Call 9am", "09:00"),
            ("Call 9:30pm", "21:30"),
            ("Call 12am", "00:00"),
            ("Call 12pm", "12:00"),
            ("Call 9 pm", "21:00"),
            ("Call 21:30", "21:30"),
            ("Call 9:30", "09:30"),
            ("Call noon", "12:00"),
            ("Call midnight", "00:00"),
            ("Call at 9", "09:00"),
            ("Call at 9:15", "09:15"),
            ("Call at 17", "17:00"),
            ("Call at 5pm", "17:00"),
            ("Call at noon.", "12:00"),
        ] {
            assert_eq!(read(text).time.as_deref(), Some(time), "{text}");
        }
    }

    #[test]
    fn a_time_alone_means_today_if_ahead_else_tomorrow() {
        let ahead = read("Call 3pm");
        assert_eq!(ahead.date.as_deref(), Some("2026-10-21"));
        assert!(ahead.spans.iter().all(|s| s.kind == "time"));
        assert_eq!(read("Call 9am").date.as_deref(), Some("2026-10-22"));
        // The minute we are in still counts as ahead.
        assert_eq!(read("Call 14:05").date.as_deref(), Some("2026-10-21"));
    }

    #[test]
    fn a_date_word_keeps_its_own_date_with_a_time() {
        let p = read("Pay rent tomorrow 9am");
        assert_eq!(p.date.as_deref(), Some("2026-10-22"));
        assert_eq!(p.time.as_deref(), Some("09:00"));
        assert_eq!(p.spans.len(), 2);
    }

    #[test]
    fn repeats() {
        let rule = |freq: &str, every: u32, days: Option<u32>, mode: &str| RepeatRule {
            freq: freq.into(),
            every,
            days,
            mode: mode.into(),
        };
        let cases = [
            ("Water plants daily", rule("day", 1, None, "fixed")),
            ("Report weekly", rule("week", 1, None, "fixed")),
            ("Rent monthly", rule("month", 1, None, "fixed")),
            ("Tax yearly", rule("year", 1, None, "fixed")),
            ("Rent every month", rule("month", 1, None, "fixed")),
            ("Gym every 2 weeks", rule("week", 2, None, "fixed")),
            ("Gym every other week", rule("week", 2, None, "fixed")),
            ("Bins every 3 days", rule("day", 3, None, "fixed")),
            (
                "Gym every weekday",
                rule("week", 1, Some(0b011_1110), "fixed"),
            ),
            ("Gym every mon", rule("week", 1, Some(0b000_0010), "fixed")),
            (
                "Gym every mon and thu",
                rule("week", 1, Some(0b001_0010), "fixed"),
            ),
            (
                "Gym every mon, wed and fri",
                rule("week", 1, Some(0b010_1010), "fixed"),
            ),
            (
                "Gym every monday, thursday",
                rule("week", 1, Some(0b001_0010), "fixed"),
            ),
            (
                "Bins every 2 weeks after done",
                rule("week", 2, None, "after"),
            ),
            ("Bins every day after done", rule("day", 1, None, "after")),
        ];
        for (text, expect) in cases {
            let p = read(text);
            assert_eq!(p.repeat, Some(expect), "{text}");
            assert_eq!(p.spans.len(), 1, "{text}: {:?}", words(&p));
        }
    }

    #[test]
    fn after_done_is_left_as_text_after_chosen_weekdays() {
        let p = read("Gym every mon after done");
        assert_eq!(p.repeat.unwrap().mode, "fixed");
        assert_eq!(p.spans[0].word, "every mon");
    }

    #[test]
    fn a_repeat_swallows_its_weekdays_so_they_are_not_dates() {
        let p = read("Gym every mon and thu");
        assert_eq!(p.date, None);
        assert_eq!(p.spans.len(), 1);
        assert_eq!(p.spans[0].word, "every mon and thu");
    }

    #[test]
    fn a_repeat_with_a_time_and_no_date_leaves_the_date_to_the_repeat() {
        let p = read("Stand-up daily 9am");
        assert_eq!(p.date, None);
        assert_eq!(p.time.as_deref(), Some("09:00"));
    }

    #[test]
    fn lists_match_a_folded_prefix() {
        let p = read("Fix tap #ho");
        assert_eq!(p.list_id.as_deref(), Some("l-home"));
        assert_eq!(p.spans[0].value, "Home");
        assert!(!p.spans[0].new_list);
        assert_eq!(read("Fix tap #WORK").list_id.as_deref(), Some("l-work"));
        assert_eq!(read("Fix tap #side").list_id.as_deref(), Some("l-side"));
        assert_eq!(
            read("Fix tap #sideprojects").list_id.as_deref(),
            Some("l-side")
        );
        let accent = parse(
            &request("Fix tap #cafe"),
            &[ListRef {
                id: "l-cafe".into(),
                name: "Café".into(),
            }],
        );
        assert_eq!(accent.list_id.as_deref(), Some("l-cafe"));
    }

    #[test]
    fn an_exact_name_beats_an_earlier_prefix() {
        let two = [("l-1", "Homework"), ("l-2", "Home")].map(|(id, name)| ListRef {
            id: id.into(),
            name: name.into(),
        });
        let p = parse(&request("x #home"), &two);
        assert_eq!(p.list_id.as_deref(), Some("l-2"));
    }

    #[test]
    fn an_unknown_list_is_offered_as_new() {
        let p = read("Buy paint #Garage");
        assert_eq!(p.list_id, None);
        assert_eq!(p.new_list.as_deref(), Some("Garage"));
        assert!(p.spans[0].new_list);
        // A bare "#", and issue numbers, are not Lists.
        assert!(read("Fix # bug").spans.is_empty());
        assert!(read("Fix bug #123").spans.is_empty());
    }

    #[test]
    fn priorities() {
        assert_eq!(read("Pay rent p1").priority, Some(1));
        assert_eq!(read("Pay rent P3").priority, Some(3));
        assert_eq!(read("Pay rent p4").priority, Some(4));
        assert_eq!(read("Pay rent p5").priority, None);
        assert_eq!(read("Pay rent p0").priority, None);
        assert_eq!(read("Pay rent p10").priority, None);
    }

    #[test]
    fn everything_in_one_line() {
        let p = read("Pay rent tomorrow 9am every month #Home p1");
        assert_eq!(p.date.as_deref(), Some("2026-10-22"));
        assert_eq!(p.time.as_deref(), Some("09:00"));
        assert_eq!(p.repeat.unwrap().freq, "month");
        assert_eq!(p.list_id.as_deref(), Some("l-home"));
        assert_eq!(p.priority, Some(1));
        let order: Vec<_> = p.spans.iter().map(|s| s.word.as_str()).collect();
        assert_eq!(order, ["tomorrow", "9am", "every month", "#Home", "p1"]);
    }

    #[test]
    fn numeric_dates_and_bare_numbers_are_text() {
        for text in [
            "Add 2/3 cups of flour",
            "Buy 2 tickets",
            "Buy 9",
            "Call 3.30",
            "Meet 03/11",
            "Pay 2026-11-03",
            "Room 12",
            "Chapter 9 summary",
            "I have 9 apples at home",
        ] {
            assert!(
                read(text).spans.is_empty(),
                "{text}: {:?}",
                words(&read(text))
            );
        }
    }

    #[test]
    fn at_before_a_month_is_not_a_time() {
        let p = read("Meet at 21 oct");
        assert_eq!(p.time, None);
        assert_eq!(p.date.as_deref(), Some("2026-10-21"));
    }

    #[test]
    fn quoted_and_backticked_text_is_never_read() {
        for text in [
            "Read \"Friday tomorrow\" article",
            "Read “tomorrow” article",
            "Run `tomorrow` script",
            "Write \"today\"",
        ] {
            assert!(read(text).spans.is_empty(), "{text}");
        }
        // Outside the quotes still counts; an unclosed quote masks nothing.
        assert_eq!(
            date("Read \"today\" tomorrow").as_deref(),
            Some("2026-10-22")
        );
        assert_eq!(
            date("Measure 5\" pipe tomorrow").as_deref(),
            Some("2026-10-22")
        );
    }

    #[test]
    fn punctuation_around_a_word_is_not_part_of_it() {
        let p = read("Call (tomorrow), then email.");
        assert_eq!(p.spans[0].word, "tomorrow");
        assert_eq!(read("Call tomorrow.").spans[0].word, "tomorrow");
    }

    #[test]
    fn the_last_of_a_kind_wins_and_the_rest_are_alternatives() {
        let p = read("Pay rent tomorrow to tell today");
        assert_eq!(p.date.as_deref(), Some("2026-10-21"));
        assert_eq!(p.spans.len(), 1);
        assert_eq!(p.spans[0].word, "today");
        assert_eq!(p.alternatives.len(), 1);
        let alt = &p.alternatives[0];
        assert_eq!(
            (alt.span.word.as_str(), alt.reason.as_str()),
            ("tomorrow", "conflict")
        );
        assert_eq!(alt.span.value, "2026-10-22");
    }

    #[test]
    fn a_use_instead_pick_beats_the_last() {
        let first = read("Pay rent tomorrow to tell today");
        let key = first.alternatives[0].span.key.clone();
        let mut req = request("Pay rent tomorrow to tell today");
        req.prefer.insert("date".into(), key);
        let p = parse(&req, &[]);
        assert_eq!(p.date.as_deref(), Some("2026-10-22"));
        assert_eq!(p.alternatives[0].span.word, "today");
    }

    #[test]
    fn a_pick_for_a_word_that_is_gone_falls_back_to_the_last() {
        let mut req = request("Pay rent tomorrow to tell today");
        req.prefer.insert("date".into(), "date:friday:1".into());
        assert_eq!(parse(&req, &[]).date.as_deref(), Some("2026-10-21"));
    }

    #[test]
    fn a_word_kept_as_text_has_no_effect_and_comes_back_as_ignored() {
        let mut req = request("Pay rent tomorrow");
        req.ignored.push("date:tomorrow:1".into());
        let p = parse(&req, &[]);
        assert!(p.spans.is_empty());
        assert_eq!(p.date, None);
        assert_eq!(p.ignored.len(), 1);
        assert_eq!(p.ignored[0].word, "tomorrow");
    }

    #[test]
    fn ignoring_the_last_word_lets_the_earlier_one_win() {
        let mut req = request("Pay rent tomorrow to tell today");
        req.ignored.push("date:today:1".into());
        let p = parse(&req, &[]);
        assert_eq!(p.date.as_deref(), Some("2026-10-22"));
        assert!(p.alternatives.is_empty());
    }

    #[test]
    fn repeated_words_get_their_own_keys() {
        let p = read("tomorrow and tomorrow");
        assert_eq!(p.alternatives[0].span.key, "date:tomorrow:1");
        assert_eq!(p.spans[0].key, "date:tomorrow:2");
        let mut req = request("tomorrow and tomorrow");
        req.ignored.push("date:tomorrow:2".into());
        let p = parse(&req, &[]);
        assert_eq!(p.spans[0].key, "date:tomorrow:1");
    }

    #[test]
    fn candidates_inside_a_frozen_stretch_are_dropped() {
        // Quick edit: "tomorrow" was in the old title, the new word is "fri".
        let mut req = request("Pay rent tomorrow fri");
        req.frozen.push(TextRange { start: 0, end: 17 });
        let p = parse(&req, &[]);
        assert_eq!(p.date.as_deref(), Some("2026-10-23"));
        assert!(p.alternatives.is_empty());
        assert!(p.ignored.is_empty());
        // A candidate only partly inside stays.
        let mut req = request("Pay rent tomorrow");
        req.frozen.push(TextRange { start: 0, end: 12 });
        assert_eq!(parse(&req, &[]).date.as_deref(), Some("2026-10-22"));
    }

    #[test]
    fn tonight_loses_its_time_to_an_explicit_one() {
        let p = read("Dinner tonight at 9pm");
        assert_eq!(p.date.as_deref(), Some("2026-10-21"));
        assert_eq!(p.time.as_deref(), Some("21:00"));
        assert_eq!(p.alternatives.len(), 1);
        assert_eq!(p.alternatives[0].span.kind, "time");
        assert_eq!(p.alternatives[0].span.value, "20:00");
    }

    #[test]
    fn offsets_count_utf16_units() {
        // The emoji is two units.
        let p = read("Party \u{1F389} tomorrow");
        assert_eq!((p.spans[0].start, p.spans[0].end), (9, 17));
    }

    #[test]
    fn spans_are_ordered_and_never_overlap() {
        let p = read("p2 every mon and thu 9am #work next fri");
        let mut at = 0;
        for s in &p.spans {
            assert!(s.start >= at && s.end > s.start);
            at = s.end;
        }
    }

    #[test]
    fn empty_text_reads_nothing() {
        assert!(read("").spans.is_empty());
        assert!(read("   ").spans.is_empty());
    }

    // Shared fixtures: src/lib/ipc/fixtures/parse-cases.json runs here and in
    // src/lib/ipc/mock-todo-parse.test.ts against the TypeScript copy of this parser.
    const FIXTURES: &str = include_str!("../../../src/lib/ipc/fixtures/parse-cases.json");

    fn brief(spans: &[ParseSpan]) -> Value {
        spans
            .iter()
            .map(|s| json!([s.word, s.kind, s.value]))
            .collect()
    }

    #[test]
    fn the_shared_parse_fixtures_hold() {
        let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
        let cases = fixtures["cases"].as_array().unwrap();
        assert!(cases.len() > 40);
        for c in cases {
            let name = c["name"].as_str().unwrap();
            let mut merged = fixtures["defaults"].clone();
            for (k, v) in c.as_object().unwrap() {
                merged[k] = v.clone();
            }
            let req: ParseRequest = serde_json::from_value(merged.clone()).unwrap();
            let lists: Vec<ListRef> = merged["lists"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| ListRef {
                    id: l["id"].as_str().unwrap().into(),
                    name: l["name"].as_str().unwrap().into(),
                })
                .collect();
            let got = parse(&req, &lists);
            let wire = serde_json::to_value(&got).unwrap();
            for (key, want) in c["expect"].as_object().unwrap() {
                let have = match key.as_str() {
                    "spans" => brief(&got.spans),
                    "ignored" => brief(&got.ignored),
                    "alternatives" => got
                        .alternatives
                        .iter()
                        .map(|a| json!([a.span.word, a.span.kind, a.span.value, a.reason]))
                        .collect(),
                    other => wire[other].clone(),
                };
                assert_eq!(&have, want, "{name}: {key}");
            }
        }
    }
}
