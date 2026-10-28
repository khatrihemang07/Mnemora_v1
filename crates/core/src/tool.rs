//! The read-only Tool contract (ADR 0002 amendment, ADR 0018). A Module offers Tools through
//! `Module::tools`; the AI Module runs them. A Tool only reads: it gets a connection and returns
//! data, and every failure it meets is an error result, never a panic or an exception.

use rusqlite::Connection;
use serde::Serialize;
use serde_json::{Map, Number, Value, json};

/// Largest result a Tool may return, in bytes. Lists and reads page below it.
pub const RESULT_CAP: usize = 16 * 1024;

/// Headroom kept free for the envelope around a result (the observation label and separators).
const ENVELOPE_ALLOWANCE: usize = 512;

/// What a parameter holds. Each kind is checked before a Tool runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Text of at most `max_chars` characters.
    Text {
        /// Longest allowed, in characters.
        max_chars: usize,
    },
    /// A whole number from `min` to `max`, inclusive.
    Integer {
        /// Smallest allowed.
        min: i64,
        /// Largest allowed.
        max: i64,
    },
    /// A calendar date, `YYYY-MM-DD`.
    Date,
    /// True or false.
    Flag,
    /// One of these words.
    Choice(&'static [&'static str]),
}

/// One parameter of a Tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Param {
    /// The name the Model sends.
    pub name: &'static str,
    /// What it holds.
    pub kind: Kind,
    /// Whether the Tool refuses to run without it.
    pub required: bool,
    /// One line the Model reads.
    pub about: &'static str,
}

/// A Tool's name, what it does and its parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spec {
    /// `module_verb`, such as `journal_list`.
    pub name: &'static str,
    /// What it is for, one line.
    pub about: &'static str,
    /// Its parameters. Anything else the Model sends is refused.
    pub params: &'static [Param],
}

/// Today on the device, which some Tools need for "overdue" and the like, and the zone its local
/// days and times are read in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Calendar {
    /// Today, `YYYY-MM-DD`.
    pub today: String,
    /// The device's IANA time zone, such as "Europe/London". Local days and times are read in it,
    /// not in the machine's zone.
    pub time_zone: String,
}

/// Arguments that passed `check`: every key is a parameter and has the kind it declares.
#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    values: Map<String, Value>,
}

impl Args {
    /// The text of a parameter, if it was sent.
    pub fn text(&self, name: &str) -> Option<&str> {
        self.values.get(name).and_then(Value::as_str)
    }

    /// The whole number of a parameter, if it was sent.
    pub fn int(&self, name: &str) -> Option<i64> {
        self.values.get(name).and_then(Value::as_i64)
    }

    /// The flag of a parameter, if it was sent.
    pub fn flag(&self, name: &str) -> Option<bool> {
        self.values.get(name).and_then(Value::as_bool)
    }
}

/// Checks raw arguments against a Tool's parameters. Unknown keys, missing required ones and wrong
/// kinds are refused with the sentence the Model reads back.
pub fn check(spec: &Spec, raw: &Value) -> Result<Args, String> {
    let Value::Object(given) = raw else {
        return Err("The arguments must be a JSON object.".to_owned());
    };
    for key in given.keys() {
        if !spec.params.iter().any(|p| p.name == key) {
            return Err(format!(
                "Unknown parameter: {key}. Remove it and try again."
            ));
        }
    }
    let mut values = Map::new();
    for param in spec.params {
        match given.get(param.name) {
            None if param.required => {
                return Err(format!("Missing parameter: {}.", param.name));
            }
            None => {}
            Some(value) => {
                check_kind(param, value)?;
                values.insert(param.name.to_owned(), value.clone());
            }
        }
    }
    Ok(Args { values })
}

fn check_kind(param: &Param, value: &Value) -> Result<(), String> {
    let name = param.name;
    match param.kind {
        Kind::Text { max_chars } => match value.as_str() {
            Some(text) if text.chars().count() <= max_chars => Ok(()),
            Some(_) => Err(format!("{name} is longer than {max_chars} characters.")),
            None => Err(format!("{name} must be text.")),
        },
        Kind::Integer { min, max } => match value.as_i64() {
            Some(n) if (min..=max).contains(&n) => Ok(()),
            Some(_) => Err(format!("{name} must be from {min} to {max}.")),
            None => Err(format!("{name} must be a whole number.")),
        },
        Kind::Date => match value.as_str() {
            Some(date) if is_date(date) => Ok(()),
            _ => Err(format!("{name} must be a date, YYYY-MM-DD.")),
        },
        Kind::Flag => match value.as_bool() {
            Some(_) => Ok(()),
            None => Err(format!("{name} must be true or false.")),
        },
        Kind::Choice(words) => match value.as_str() {
            Some(word) if words.contains(&word) => Ok(()),
            _ => Err(format!("{name} must be one of: {}.", words.join(", "))),
        },
    }
}

/// `YYYY-MM-DD` with a month and day that exist in some year.
fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| {
        text.get(range)
            .filter(|s| s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse::<u32>().ok())
    };
    match (digits(0..4), digits(5..7), digits(8..10)) {
        (Some(_), Some(month), Some(day)) => (1..=12).contains(&month) && (1..=31).contains(&day),
        _ => false,
    }
}

/// The JSON Schema a Provider with native Tool calls is sent for this Tool.
pub fn json_schema(spec: &Spec) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for param in spec.params {
        properties.insert(param.name.to_owned(), schema_of(param));
        if param.required {
            required.push(Value::from(param.name));
        }
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn schema_of(param: &Param) -> Value {
    let mut node = match param.kind {
        Kind::Text { max_chars } => json!({ "type": "string", "maxLength": max_chars }),
        Kind::Integer { min, max } => {
            json!({ "type": "integer", "minimum": Number::from(min), "maximum": Number::from(max) })
        }
        Kind::Date => json!({ "type": "string", "pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}$" }),
        Kind::Flag => json!({ "type": "boolean" }),
        Kind::Choice(words) => json!({ "type": "string", "enum": words }),
    };
    node["description"] = Value::from(param.about);
    node
}

/// One thing a Tool returned that the answer may cite: an Entry or a Task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    /// The Entry's or Task's id.
    pub id: String,
    /// The Module it belongs to, `journal` or `todo`.
    pub module: &'static str,
    /// A short label the person can recognise it by.
    pub title: String,
    /// Its date, `YYYY-MM-DD`, when it has one (a Page date or a due date).
    pub date: Option<String>,
}

/// What a Tool hands back: the data, a plain sentence about what it did, and the items it returned.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    /// The data, as JSON. Capped by `fit_rows` and `RESULT_CAP` before it reaches the Model.
    pub body: Value,
    /// What the activity line says, in plain words: "Listed entries, 7 entries".
    pub summary: String,
    /// The items the result carries, so the Sources check can see them.
    pub items: Vec<Item>,
    /// Whether this is an error the Model should read as a refusal.
    pub is_error: bool,
}

impl Output {
    /// A result with data.
    pub fn ok(body: Value, summary: String, items: Vec<Item>) -> Self {
        Self {
            body,
            summary,
            items,
            is_error: false,
        }
    }

    /// A refusal: the sentence says what is wrong, and no item is returned.
    pub fn error(message: &str) -> Self {
        Self {
            body: json!({ "error": message }),
            summary: "Could not read that".to_owned(),
            items: Vec::new(),
            is_error: true,
        }
    }
}

/// A read-only capability a Module offers the AI (ADR 0002 amendment).
pub trait Tool: Send + Sync {
    /// Its name, description and parameters.
    fn spec(&self) -> Spec;

    /// Runs it on checked arguments. Never writes.
    fn run(&self, conn: &Connection, args: &Args, calendar: &Calendar) -> Output;
}

/// A count argument (an offset or a limit) as a `usize`, or `default` when it was not sent or is
/// negative.
pub fn count(value: Option<i64>, default: usize) -> usize {
    value
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(default)
}

/// A count as the whole number a result carries, saturating far beyond any real list.
pub fn number(count: usize) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}

/// Keeps as many of `rows` as fit under the result cap, in order. Returns them and whether any
/// were left out, so the Model can ask for the next page.
pub fn fit_rows(rows: Vec<Value>) -> (Vec<Value>, bool) {
    let budget = RESULT_CAP - ENVELOPE_ALLOWANCE;
    let mut used = 2;
    let mut kept = Vec::new();
    for row in rows {
        let size = row.to_string().len() + 1;
        if used + size > budget {
            return (kept, true);
        }
        used += size;
        kept.push(row);
    }
    (kept, false)
}

/// Cuts `text` to at most `max_bytes`, on a character boundary.
pub fn cut(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Whether a Module is on for this device, from the two Preferences that list its exceptions.
pub fn is_enabled(
    module: &dyn crate::module::Module,
    disabled: &[String],
    opted_in: &[String],
) -> bool {
    let id = module.id();
    if module.default_enabled() {
        !disabled.iter().any(|d| d == id)
    } else {
        opted_in.iter().any(|o| o == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORDS: &[&str] = &["open", "done"];
    const PARAMS: &[Param] = &[
        Param {
            name: "query",
            kind: Kind::Text { max_chars: 5 },
            required: true,
            about: "words",
        },
        Param {
            name: "limit",
            kind: Kind::Integer { min: 1, max: 25 },
            required: false,
            about: "rows",
        },
        Param {
            name: "state",
            kind: Kind::Choice(WORDS),
            required: false,
            about: "state",
        },
        Param {
            name: "from",
            kind: Kind::Date,
            required: false,
            about: "first day",
        },
    ];
    const SPEC: Spec = Spec {
        name: "demo_search",
        about: "demo",
        params: PARAMS,
    };

    #[test]
    fn a_valid_call_passes_and_its_values_are_readable() {
        let args = check(
            &SPEC,
            &json!({ "query": "walk", "limit": 10, "from": "2026-10-01" }),
        )
        .unwrap();
        assert_eq!(args.text("query"), Some("walk"));
        assert_eq!(args.int("limit"), Some(10));
        assert_eq!(args.text("from"), Some("2026-10-01"));
        assert_eq!(args.text("state"), None);
    }

    #[test]
    fn unknown_keys_missing_required_and_wrong_kinds_are_refused() {
        assert!(
            check(&SPEC, &json!({ "query": "a", "sort": "x" }))
                .unwrap_err()
                .contains("Unknown parameter: sort")
        );
        assert_eq!(
            check(&SPEC, &json!({ "limit": 3 })).unwrap_err(),
            "Missing parameter: query."
        );
        assert!(
            check(&SPEC, &json!({ "query": "walk", "limit": 26 }))
                .unwrap_err()
                .contains("from 1 to 25")
        );
        assert!(
            check(&SPEC, &json!({ "query": "walk", "state": "dead" }))
                .unwrap_err()
                .contains("open, done")
        );
        assert!(
            check(&SPEC, &json!({ "query": "walk", "from": "2026-13-01" }))
                .unwrap_err()
                .contains("YYYY-MM-DD")
        );
        assert!(
            check(&SPEC, &json!({ "query": "toolongword" }))
                .unwrap_err()
                .contains("5 characters")
        );
        assert!(check(&SPEC, &json!([1])).is_err());
    }

    #[test]
    fn the_schema_forbids_extra_keys_and_lists_the_required_ones() {
        let schema = json_schema(&SPEC);
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], json!(["query"]));
        assert_eq!(
            schema["properties"]["state"]["enum"],
            json!(["open", "done"])
        );
    }

    #[test]
    fn fit_rows_keeps_a_prefix_under_the_cap_and_says_when_it_cut() {
        let big = json!({ "text": "x".repeat(4_000) });
        let (kept, more) = fit_rows(vec![
            big.clone(),
            big.clone(),
            big.clone(),
            big.clone(),
            big,
        ]);
        assert!(kept.len() < 5 && more);
        let (all, none) = fit_rows(vec![json!(1), json!(2)]);
        assert_eq!(all.len(), 2);
        assert!(!none);
    }

    #[test]
    fn cut_never_splits_a_character() {
        assert_eq!(cut("héllo", 2), "h");
        assert_eq!(cut("abc", 10), "abc");
    }
}
