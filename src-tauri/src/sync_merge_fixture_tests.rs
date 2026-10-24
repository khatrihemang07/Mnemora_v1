//! The merge cases of `fixtures/sync-merge-rules.json` on a [`Cluster`] of two real devices. The
//! browser mock runs the same file through its TypeScript copy of the merge rules
//! (`mock-sync-merge.test.ts`), so the copy cannot drift from Rust (ADR 0009). Every step is a
//! real command; the worlds are read back from the databases, and the Conflicts and Resolved
//! lists through commands.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::test_support::{Cluster, TestApp};

const RULES: &str = include_str!("../../src/lib/ipc/fixtures/sync-merge-rules.json");
/// Look and feel: per device, never synced.
const LOOK: [&str; 6] = [
    "core.theme",
    "core.density",
    "core.text_size",
    "core.reduce_motion",
    "core.phone_tabs",
    "core.phone_tabs_edited",
];

fn local() -> Value {
    json!({ "today": "2026-10-22", "now": "14:05", "offset_min": 0 })
}

/// Label to id, shared by both devices (ids travel with the records).
#[derive(Default)]
struct Ids(BTreeMap<String, Value>);

impl Ids {
    fn get(&self, label: &Value) -> Value {
        label.as_str().map_or(Value::Null, |l| {
            self.0
                .get(l)
                .unwrap_or_else(|| panic!("no such label {l}"))
                .clone()
        })
    }

    fn label_of(&self, id: &Value) -> Value {
        self.0
            .iter()
            .find(|(_, v)| *v == id)
            .map_or(Value::Null, |(l, _)| json!(l))
    }
}

fn str_of<'a>(step: &'a Value, key: &str) -> &'a str {
    step[key]
        .as_str()
        .unwrap_or_else(|| panic!("step needs {key}"))
}

/// The open Tasks of `task`'s List (or the Inbox) in the order the List shows them.
fn open_order(app: &TestApp, list: &Value) -> Vec<String> {
    app.with_db(|db| {
        let mut stmt = db
            .prepare(
                "SELECT id FROM todo_tasks WHERE list_id IS ?1 AND deleted_at IS NULL
                 AND state = 'open' ORDER BY sort_key, id",
            )
            .unwrap();
        stmt.query_map([list.as_str()], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    })
}

/// The instant of step `n`: one second after the one before, on a clock both devices share.
fn at(n: usize) -> i64 {
    Cluster::START + (i64::try_from(n).unwrap() + 1) * 1_000
}

#[allow(clippy::too_many_lines)] // one arm per op of the fixture: reads best together
fn step(c: &Cluster, ids: &mut Ids, n: usize, s: &Value) {
    let clock = at(n);
    for d in 0..2 {
        c[d].clock().freeze(clock);
    }
    if s["op"] == "sync" {
        c.sync(0, 1);
        c.sync(0, 1);
        return;
    }
    let app = &c[usize::from(s["d"] == "b")];
    let session = format!("s{n}");
    let task = s["task"]
        .as_str()
        .and_then(|l| ids.0.get(l))
        .cloned()
        .unwrap_or(Value::Null);
    match str_of(s, "op") {
        "list_create" => {
            // List ids are UUIDv7 from the real clock: a pause makes "older" unambiguous.
            std::thread::sleep(Duration::from_millis(3));
            let list: Value = app
                .invoke("todo_list_create", json!({ "name": s["name"] }))
                .unwrap();
            ids.0.insert(str_of(s, "list").into(), list["id"].clone());
        }
        "list_rename" => {
            app.invoke::<Value>(
                "todo_list_rename",
                json!({ "id": ids.get(&s["list"]), "name": s["name"] }),
            )
            .unwrap();
        }
        "list_sort" => {
            app.invoke::<Value>(
                "todo_list_set_sort",
                json!({ "id": ids.get(&s["list"]), "sort": s["sort"] }),
            )
            .unwrap();
        }
        "list_group" => {
            app.invoke::<Value>(
                "todo_list_set_group",
                json!({ "id": ids.get(&s["list"]), "groupBy": s["group"] }),
            )
            .unwrap();
        }
        "list_delete" => {
            app.invoke::<Value>(
                "todo_list_delete",
                json!({ "id": ids.get(&s["list"]), "choice": { "kind": "move_to_inbox" } }),
            )
            .unwrap();
        }
        "task_create" => {
            std::thread::sleep(Duration::from_millis(3));
            let made: Value = app
                .invoke(
                    "todo_task_create",
                    json!({
                        "title": s["title"], "listId": ids.get(&s["list"]),
                        "details": { "understood": s.get("understood") },
                    }),
                )
                .unwrap();
            ids.0.insert(str_of(s, "task").into(), made["id"].clone());
            if let Some(notes) = s.get("notes") {
                app.invoke::<Value>(
                    "todo_task_update",
                    json!({ "id": made["id"], "title": s["title"], "notes": notes, "sessionId": session }),
                )
                .unwrap();
            }
        }
        "task_edit" => {
            let now: Value = app.invoke("todo_task", json!({ "id": task })).unwrap();
            app.invoke::<Value>(
                "todo_task_update",
                json!({
                    "id": task,
                    "title": s.get("title").unwrap_or(&now["title"]),
                    "notes": s.get("notes").unwrap_or(&now["notes"]),
                    "sessionId": session,
                }),
            )
            .unwrap();
        }
        "task_title" => {
            let now: Value = app.invoke("todo_task", json!({ "id": task })).unwrap();
            let repeat = (!now["repeat_freq"].is_null()).then(|| {
                json!({
                    "freq": now["repeat_freq"], "every": now["repeat_every"],
                    "days": now["repeat_days"], "mode": now["repeat_mode"],
                })
            });
            app.invoke::<Value>(
                "todo_task_quick_edit",
                json!({
                    "id": task,
                    "edit": {
                        "title": s["title"], "understood": s.get("understood"),
                        "due_date": now["due_date"], "due_time": now["due_time"],
                        "repeat": repeat, "priority": now["priority"], "list_id": now["list_id"],
                    },
                    "local": local(),
                }),
            )
            .unwrap();
        }
        "task_priority" => {
            app.invoke::<Value>(
                "todo_task_set_priority",
                json!({ "id": task, "priority": s["priority"] }),
            )
            .unwrap();
        }
        "task_when" => {
            app.invoke::<Value>(
                "todo_task_set_when",
                json!({
                    "id": task, "date": s["date"], "time": s["time"],
                    "repeat": s.get("repeat"), "local": local(),
                }),
            )
            .unwrap();
        }
        "task_skip" => {
            app.invoke::<Value>("todo_task_skip", json!({ "id": task, "local": local() }))
                .unwrap();
        }
        "task_done" => {
            app.invoke::<Value>(
                "todo_task_complete",
                json!({ "id": task, "local": local() }),
            )
            .unwrap();
        }
        "task_move" => {
            app.invoke::<Value>(
                "todo_task_move",
                json!({ "id": task, "listId": ids.get(&s["list"]) }),
            )
            .unwrap();
        }
        "task_delete" => {
            app.invoke::<()>("todo_task_delete", json!({ "id": task }))
                .unwrap();
        }
        "task_place" => {
            let list = app.with_db(|db| {
                db.query_row(
                    "SELECT list_id FROM todo_tasks WHERE id = ?1",
                    [task.as_str()],
                    |r| r.get::<_, Option<String>>(0),
                )
                .unwrap()
            });
            let others: Vec<String> = open_order(app, &json!(list))
                .into_iter()
                .filter(|t| Some(t.as_str()) != task.as_str())
                .collect();
            let (above, below) = match s["before"].as_str() {
                Some(label) => {
                    let target = ids.get(&json!(label));
                    let at = others
                        .iter()
                        .position(|t| Some(t.as_str()) == target.as_str());
                    let at = at.unwrap();
                    (
                        at.checked_sub(1).map(|i| json!(others[i])),
                        Some(json!(others[at])),
                    )
                }
                None => (others.last().map(|t| json!(t)), None),
            };
            app.invoke::<Value>(
                "todo_task_reorder",
                json!({
                    "id": task, "before": above, "after": below, "target": [], "local": local()
                }),
            )
            .unwrap();
        }
        "comment_add" => {
            let made: Value = app
                .invoke(
                    "todo_comment_add",
                    json!({ "taskId": task, "body": s["body"] }),
                )
                .unwrap();
            ids.0
                .insert(str_of(s, "comment").into(), made["id"].clone());
        }
        "comment_edit" => {
            app.invoke::<Value>(
                "todo_comment_edit",
                json!({ "id": ids.get(&s["comment"]), "body": s["body"] }),
            )
            .unwrap();
        }
        "entry_create" => {
            std::thread::sleep(Duration::from_millis(3));
            let made: Value = app
                .invoke(
                    "journal_entry_create",
                    json!({ "entry": {
                        "page_date": "2026-10-21", "text": s["body"], "written_tz": "UTC",
                        "written_offset_min": 0, "session_id": session,
                    }}),
                )
                .unwrap();
            ids.0.insert(str_of(s, "entry").into(), made["id"].clone());
        }
        "entry_delete" => {
            app.invoke::<()>(
                "journal_entry_delete",
                json!({ "id": ids.get(&s["entry"]) }),
            )
            .unwrap();
        }
        "entry_edit" => {
            app.invoke::<Value>(
                "journal_entry_update",
                json!({ "id": ids.get(&s["entry"]), "text": s["body"], "sessionId": session }),
            )
            .unwrap();
        }
        "pref" => app.set_pref(str_of(s, "key"), s["value"].clone()),
        other => panic!("unknown op {other}"),
    }
}

/// One device as plain data, in the shape of the fixture's `expect`.
#[allow(clippy::too_many_lines)] // one entry per part of `expect`: reads best together
fn world(app: &TestApp, ids: &Ids) -> Value {
    let (lists, tasks, comments, entries, occurrences) = app.with_db(|db| {
        let rows = |columns: &str, from: &str| -> Vec<Value> {
            let mut stmt = db
                .prepare(&format!("SELECT json_array({columns}) FROM {from}"))
                .unwrap();
            stmt.query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .map(|row| serde_json::from_str(&row.unwrap()).unwrap())
                .collect()
        };
        (
            rows(
                "id, name, task_sort, COALESCE(group_by, 'none')",
                "todo_lists WHERE deleted_at IS NULL",
            ),
            rows(
                "id, title, notes, priority, due_date, due_time, state, list_id,
                 deleted_at IS NOT NULL, understood, repeat_freq, repeat_every, repeat_days,
                 repeat_mode",
                "todo_tasks",
            ),
            rows("id, task_id, body, edited", "todo_comments WHERE deleted_at IS NULL"),
            rows("id, body", "journal_entries WHERE deleted_at IS NULL"),
            db.query_row(
                "SELECT COUNT(*) FROM todo_tasks WHERE repeat_of IS NOT NULL AND deleted_at IS NULL",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap(),
        )
    });
    let mut out = Map::new();
    let labelled = |rows: &[Value], pick: &dyn Fn(&Value) -> Value| -> Value {
        Value::Object(
            rows.iter()
                .map(|r| {
                    (
                        ids.label_of(&r[0]).as_str().unwrap_or("?").to_owned(),
                        pick(r),
                    )
                })
                .collect(),
        )
    };
    out.insert("lists".into(), labelled(&lists, &|r| r[1].clone()));
    out.insert(
        "arrange".into(),
        labelled(&lists, &|r| json!({ "sort": r[2], "group": r[3] })),
    );
    out.insert("occurrences".into(), json!(occurrences));
    out.insert(
        "tasks".into(),
        labelled(&tasks, &|r| {
            let gone = r[8] == json!(1);
            json!({
                "title": r[1], "understood": r[9], "notes": r[2],
                "repeat_freq": r[10], "repeat_every": r[11], "repeat_days": r[12],
                "repeat_mode": r[13], "priority": r[3], "due_date": r[4], "due_time": r[5],
                "state": r[6], "deleted": gone,
                // A deleted Task's List is hidden state that depends on who went first.
                "list": if gone { Value::Null } else { ids.label_of(&r[7]) },
            })
        }),
    );
    out.insert(
        "comments".into(),
        labelled(
            &comments,
            &|r| json!({ "task": ids.label_of(&r[1]), "body": r[2], "edited": r[3] == json!(1) }),
        ),
    );
    out.insert("entries".into(), labelled(&entries, &|r| r[1].clone()));
    let mut order = Map::new();
    let in_order = |list: &Value| -> Value {
        json!(
            open_order(app, list)
                .iter()
                .map(|t| ids.label_of(&json!(t)))
                .collect::<Vec<_>>()
        )
    };
    order.insert("inbox".into(), in_order(&Value::Null));
    for (label, id) in &ids.0 {
        if lists.iter().any(|l| l[0] == *id) {
            order.insert(label.clone(), in_order(id));
        }
    }
    out.insert("order".into(), Value::Object(order));
    let all: Value = app.invoke("settings_get_all", json!({})).unwrap();
    out.insert("settings".into(), all["values"].clone());
    let list: Value = app.invoke("sync_conflicts", json!({})).unwrap();
    out.insert(
        "conflicts".into(),
        json!(list["needs_you"].as_array().unwrap().len()),
    );
    let mut kinds: Vec<&str> = list["resolved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["kind"].as_str().unwrap())
        .filter(|k| *k != "order_prefs")
        .collect();
    kinds.sort_unstable();
    out.insert("resolved".into(), json!(kinds));
    Value::Object(out)
}

/// The world without what stays on each device.
fn shared(mut w: Value) -> Value {
    if let Some(settings) = w["settings"].as_object_mut() {
        settings.retain(|k, _| !LOOK.contains(&k.as_str()));
    }
    w
}

/// Every part of `expect` holds in `world`.
fn check(name: &str, expect: &Value, worlds: &[Value; 2]) {
    for (key, want) in expect.as_object().unwrap() {
        match key.as_str() {
            "device" => {
                for (setting, by) in want.as_object().unwrap() {
                    for (d, w) in ["a", "b"].iter().zip(worlds) {
                        assert_eq!(&w["settings"][setting], &by[*d], "{name}: {setting} on {d}");
                    }
                }
            }
            "settings" | "tasks" | "comments" => {
                for (item, fields) in want.as_object().unwrap() {
                    for (d, w) in ["a", "b"].iter().zip(worlds) {
                        if key == "settings" {
                            assert_eq!(&w[key][item], fields, "{name}: {item} on {d}");
                            continue;
                        }
                        for (f, v) in fields.as_object().unwrap() {
                            assert_eq!(&w[key][item][f], v, "{name}: {key} {item}.{f} on {d}");
                        }
                    }
                }
            }
            "order" => {
                for (list, tasks) in want.as_object().unwrap() {
                    for (d, w) in ["a", "b"].iter().zip(worlds) {
                        assert_eq!(&w["order"][list], tasks, "{name}: order of {list} on {d}");
                    }
                }
            }
            _ => {
                for (d, w) in ["a", "b"].iter().zip(worlds) {
                    assert_eq!(&w[key], want, "{name}: {key} on {d}");
                }
            }
        }
    }
}

#[test]
fn every_merge_case_of_the_fixture_holds_on_two_real_devices() {
    let rules: Value = serde_json::from_str(RULES).unwrap();
    for case in rules["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let c = Cluster::new(2);
        let mut ids = Ids::default();
        for (n, s) in case["steps"].as_array().unwrap().iter().enumerate() {
            step(&c, &mut ids, n, s);
        }
        let end = case["steps"].as_array().unwrap().len();
        for d in 0..2 {
            c[d].clock().freeze(at(end));
        }
        c.sync(0, 1);
        c.sync(0, 1);
        let worlds = [world(&c[0], &ids), world(&c[1], &ids)];
        assert_eq!(
            shared(worlds[0].clone()),
            shared(worlds[1].clone()),
            "{name}: the devices differ"
        );
        check(name, &case["expect"], &worlds);
    }
}
