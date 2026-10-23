//! Sync scenarios for the Activity log on a [`Cluster`]: every kind of event appears once on every
//! device, a session's numbers and breakdown match what moved, a quiet sync is left out, and a
//! long log pages newest first. Every step is a real command; every assertion reads back through
//! commands (ADR 0009).

use std::collections::{BTreeMap, BTreeSet};

use mnemora_sync::{Local, Why};
use serde_json::{Value, json};

use crate::test_support::{Cluster, TestApp};

const NAMES: [&str; 3] = ["Mac", "Pixel 8", "Tablet"];

/// `n` devices, each named and knowing the others as paired devices.
fn group(n: usize) -> Cluster {
    let c = Cluster::new(n);
    let ids: Vec<String> = (0..n).map(|i| c[i].device_id()).collect();
    for i in 0..n {
        c[i].invoke::<()>("sync_rename", json!({ "name": NAMES[i] }))
            .unwrap();
        c[i].with_db(|db| {
            db.execute(
                "INSERT INTO sync_group(one, id) VALUES (1, 'group') ON CONFLICT DO NOTHING",
                [],
            )
            .unwrap();
            for j in (0..n).filter(|j| *j != i) {
                db.execute(
                    "INSERT INTO sync_peers(id, name, paired_at) VALUES (?1, ?2, 1)",
                    [ids[j].as_str(), NAMES[j]],
                )
                .unwrap();
            }
        });
    }
    c
}

/// Indexes of the cluster's devices from the lowest device id up: the first of two leads.
fn by_id(c: &Cluster, n: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|i| c[*i].device_id());
    order
}

fn page(app: &TestApp, filter: Value, cursor: Option<&str>, limit: u32) -> Value {
    app.invoke(
        "sync_activity",
        json!({ "filter": filter, "cursor": cursor, "limit": limit }),
    )
    .unwrap()
}

fn all(app: &TestApp) -> Vec<Value> {
    let mut rows = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let p = page(app, json!({ "kind": "all" }), cursor.as_deref(), 100);
        rows.extend(p["rows"].as_array().unwrap().iter().cloned());
        match p["next"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => return rows,
        }
    }
}

fn of_kind(app: &TestApp, kind: &str) -> Vec<Value> {
    all(app).into_iter().filter(|r| r["kind"] == kind).collect()
}

/// A row as the screen reads it: "Synced with Pixel 8 · 3 in, 1 out · on Tablet".
fn line(row: &Value) -> String {
    let text: String = row["text"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["text"].as_str().unwrap())
        .collect();
    match row["on"].as_str() {
        Some(on) => format!("{text} · on {on}"),
        None => text,
    }
}

fn ids(rows: &[Value]) -> BTreeSet<String> {
    rows.iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect()
}

fn create_task(app: &TestApp, title: &str) {
    app.invoke::<Value>("todo_task_create", json!({ "title": title }))
        .unwrap();
}

fn create_entry(app: &TestApp, text: &str) -> Value {
    app.invoke(
        "journal_entry_create",
        json!({ "entry": {
            "page_date": "2026-10-21", "text": text, "written_tz": "UTC",
            "written_offset_min": 0, "session_id": "create",
        }}),
    )
    .unwrap()
}

fn numbers(text: &str) -> i64 {
    text.split(|c: char| !c.is_ascii_digit())
        .filter_map(|n| n.parse::<i64>().ok())
        .sum()
}

fn local(c: &Cluster, i: usize) -> (String, &'static str) {
    (c[i].device_id(), NAMES[i])
}

// ---------------------------------------------------------------- sessions

#[test]
fn a_session_is_logged_once_with_counts_that_match_and_read_the_right_way_round() {
    let c = group(3);
    for t in ["a", "b", "c"] {
        create_task(&c[0], t);
    }
    create_entry(&c[1], "Morning page.\n");
    let (a, b) = c.sync(0, 1);
    let order = by_id(&c, 2);
    let (lead, follow) = (order[0], order[1]);
    let stats = if lead == 0 { &a } else { &b };
    assert!(stats.received + stats.sent > 0);

    c.sync_all();

    // The lead's line, in its own words.
    let rows = of_kind(&c[lead], "session");
    let first = rows
        .iter()
        .find(|r| line(r).starts_with(&format!("Synced with {}", NAMES[follow])))
        .unwrap_or_else(|| panic!("no line for the first session: {rows:?}"));
    assert_eq!(
        line(first),
        format!(
            "Synced with {} · {} in, {} out",
            NAMES[follow], stats.received, stats.sent
        )
    );
    // The breakdown names Modules and kinds, adds up to what moved, and holds no content.
    let breakdown = first["breakdown"].as_str().unwrap();
    assert_eq!(
        numbers(breakdown),
        i64::try_from(stats.received + stats.sent).unwrap()
    );
    assert!(
        breakdown.contains("Tasks") && breakdown.contains("Journal: 1 Entry"),
        "{breakdown}"
    );
    assert!(!breakdown.contains("Morning"), "no content");

    // The other end reads it the other way round.
    let mine = of_kind(&c[follow], "session");
    let theirs = mine
        .iter()
        .find(|r| r["id"] == first["id"])
        .expect("the follower has the lead's line");
    assert_eq!(
        line(theirs),
        format!(
            "Synced with {} · {} in, {} out",
            NAMES[lead], stats.sent, stats.received
        )
    );
    assert_eq!(theirs["breakdown"], first["breakdown"]);

    // The third device sees it as someone else's.
    let third = of_kind(&c[2], "session");
    let seen = third.iter().find(|r| r["id"] == first["id"]).unwrap();
    assert_eq!(
        line(seen),
        format!(
            "Synced with {} · {} in, {} out · on {}",
            NAMES[follow], stats.received, stats.sent, NAMES[lead]
        )
    );

    // Every device holds the same set of session lines, each once.
    let sets: Vec<BTreeSet<String>> = (0..3).map(|i| ids(&of_kind(&c[i], "session"))).collect();
    assert_eq!(sets[0], sets[1]);
    assert_eq!(sets[1], sets[2]);
    for i in 0..3 {
        assert_eq!(
            of_kind(&c[i], "session").len(),
            sets[i].len(),
            "no duplicates"
        );
    }
}

#[test]
fn every_session_that_moved_something_is_one_line_and_each_line_matches_its_session() {
    let c = group(2);
    let order = by_id(&c, 2);
    let lead = order[0];
    let mut expected = Vec::new();
    for round in 0..3 {
        for t in 0..=round {
            create_task(&c[1 - lead], &format!("task {round}.{t}"));
        }
        let (a, b) = c.sync(0, 1);
        let stats = if lead == 0 { a } else { b };
        expected.push((stats.received, stats.sent));
        c[0].clock().advance(60_000);
        c[1].clock().advance(60_000);
    }
    c.sync_all();
    let mut got: Vec<(i64, i64)> = of_kind(&c[lead], "session")
        .iter()
        .map(|r| {
            let text = line(r);
            let counts = text.split_once(" · ").unwrap().1;
            let nums: Vec<i64> = counts
                .split(|ch: char| !ch.is_ascii_digit())
                .filter_map(|n| n.parse().ok())
                .collect();
            (nums[0], nums[1])
        })
        .collect();
    got.reverse();
    let expected: Vec<(i64, i64)> = expected
        .into_iter()
        .map(|(i, o)| (i64::try_from(i).unwrap(), i64::try_from(o).unwrap()))
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn a_sync_that_moved_nothing_is_not_logged_and_neither_is_passing_activity_on() {
    let c = group(3);
    create_task(&c[0], "one");
    c.sync_all();
    let before: Vec<usize> = (0..3).map(|i| all(&c[i]).len()).collect();
    assert!(before.iter().all(|n| *n > 0));
    // Everything is where it should be: more exchanges (they move nothing) add no lines.
    c.sync_all();
    c.sync_all();
    let after: Vec<usize> = (0..3).map(|i| all(&c[i]).len()).collect();
    assert_eq!(before, after);

    // A first sync of two empty devices is quiet too.
    let d = group(2);
    d.sync(0, 1);
    d.sync_all();
    assert!(all(&d[0]).is_empty() && all(&d[1]).is_empty());
}

// ---------------------------------------------------------------- the other kinds

const BASE: &str = "Went for a run along the canal.\nCoffee with Sam after.\n";
const MINE: &str =
    "Went for a run along the canal, felt great after the rain.\nCoffee with Sam after.\n";
const YOURS: &str =
    "Went for a run along the canal, legs heavy but good.\nCoffee with Sam after.\n";

#[test]
fn a_conflict_raised_and_resolved_is_one_line_each_on_every_device() {
    let c = group(3);
    let entry = create_entry(&c[0], BASE)["id"].clone();
    c.sync_all();
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>(
        "journal_entry_update",
        json!({ "id": entry, "text": MINE, "sessionId": "a" }),
    )
    .unwrap();
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>(
        "journal_entry_update",
        json!({ "id": entry, "text": YOURS, "sessionId": "b" }),
    )
    .unwrap();
    c.sync_all();

    for i in 0..3 {
        let raised = of_kind(&c[i], "conflict_raised");
        assert_eq!(raised.len(), 1, "device {i}: {raised:?}");
        assert!(line(&raised[0]).starts_with("Conflict in an Entry's text needs you"));
        assert_eq!(raised[0]["link"], "conflicts");
    }
    assert_eq!(
        ids(&of_kind(&c[0], "conflict_raised")),
        ids(&of_kind(&c[2], "conflict_raised"))
    );

    let waiting: Value = c[2].invoke("sync_conflicts", json!({})).unwrap();
    let id = waiting["needs_you"][0]["id"].as_str().unwrap().to_owned();
    c[2].invoke::<()>(
        "sync_conflict_resolve",
        json!({ "id": id, "choice": { "kind": "keep_this" } }),
    )
    .unwrap();
    c.sync_all();
    for i in 0..3 {
        let resolved = of_kind(&c[i], "conflict_resolved");
        assert_eq!(resolved.len(), 1, "device {i}: {resolved:?}");
        assert!(line(&resolved[0]).starts_with("Conflict in an Entry's text resolved"));
    }
}

#[test]
fn a_list_merge_is_one_line_on_every_device() {
    let c = group(3);
    c[0].invoke::<Value>("todo_list_create", json!({ "name": "Groceries" }))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(3));
    c[1].invoke::<Value>("todo_list_create", json!({ "name": "groceries" }))
        .unwrap();
    c.sync_all();
    let counts: Vec<usize> = (0..3)
        .map(|i| of_kind(&c[i], "lists_merged").len())
        .collect();
    assert_eq!(counts, [1, 1, 1], "{counts:?}");
    let sets: Vec<BTreeSet<String>> = (0..3)
        .map(|i| ids(&of_kind(&c[i], "lists_merged")))
        .collect();
    assert_eq!(sets[0], sets[1]);
    assert_eq!(sets[1], sets[2]);
    assert!(line(&of_kind(&c[0], "lists_merged")[0]).contains(" merged into "));
}

#[test]
fn renaming_forgetting_leaving_pauses_resumes_and_failures_are_each_one_attributed_line() {
    let c = group(3);
    let order = by_id(&c, 3);
    let (a, b) = (order[0], order[1]);

    // Renaming: through the command, which logs it.
    c[a].invoke::<()>("sync_rename", json!({ "name": "Studio Mac" }))
        .unwrap();
    // The rest are what #69 and #72 call; the lower id of two devices logs what they share.
    let (a_id, a_name) = local(&c, a);
    let (b_id, b_name) = local(&c, b);
    c[a].with_db(|db| {
        let me = Local {
            device: &a_id,
            name: a_name,
        };
        assert!(mnemora_sync::log_paused(db, me, &b_id, Why::Update).unwrap());
        assert!(mnemora_sync::log_resumed(db, me, &b_id).unwrap());
        assert!(mnemora_sync::log_failed(db, me, &b_id, Why::Lost).unwrap());
        assert!(
            !mnemora_sync::log_failed(db, me, &b_id, Why::Lost).unwrap(),
            "a flaky link is one line"
        );
        assert!(mnemora_sync::log_forgot(db, me, &b_id, b_name).unwrap());
        assert!(mnemora_sync::log_left(db, me).unwrap());
    });
    // The higher id logs none of the shared ones.
    c[b].with_db(|db| {
        let me = Local {
            device: &b_id,
            name: b_name,
        };
        assert!(!mnemora_sync::log_paused(db, me, &a_id, Why::Update).unwrap());
        assert!(!mnemora_sync::log_resumed(db, me, &a_id).unwrap());
        assert!(!mnemora_sync::log_failed(db, me, &a_id, Why::Lost).unwrap());
    });
    c.sync_all();

    let lines: Vec<BTreeMap<String, usize>> = (0..3)
        .map(|i| {
            let mut kinds = BTreeMap::new();
            for r in all(&c[i]) {
                *kinds
                    .entry(r["kind"].as_str().unwrap().to_owned())
                    .or_default() += 1;
            }
            kinds
        })
        .collect();
    for (i, kinds) in lines.iter().enumerate() {
        for kind in ["renamed", "paused", "resumed", "failed", "forgot", "left"] {
            assert_eq!(kinds.get(kind), Some(&1), "device {i}, {kind}: {kinds:?}");
        }
    }

    // Attributed: the device that did it reads it plainly, the others "· on <name>".
    let paused_on = |i: usize| line(&of_kind(&c[i], "paused")[0]);
    assert_eq!(
        paused_on(a),
        format!("Paused with {b_name}: one of you needs an update")
    );
    // The rename reached the others (#69), so they call it by its new name.
    assert_eq!(
        paused_on(b),
        "Paused with Studio Mac: one of you needs an update"
    );
    let third = order[2];
    assert_eq!(
        paused_on(third),
        format!("Paused with {b_name}: one of you needs an update · on Studio Mac")
    );
    assert_eq!(
        line(&of_kind(&c[third], "renamed")[0]),
        format!("{a_name} renamed to Studio Mac")
    );
    assert_eq!(
        line(&of_kind(&c[third], "failed")[0]),
        format!("Connection to {b_name} lost; retrying · on Studio Mac")
    );
}

#[test]
fn a_pause_and_a_resume_are_logged_once_however_often_they_are_noticed() {
    let c = group(2);
    let order = by_id(&c, 2);
    let (a, b) = (order[0], order[1]);
    let (a_id, a_name) = local(&c, a);
    let (b_id, _) = local(&c, b);
    c[a].with_db(|db| {
        let me = Local {
            device: &a_id,
            name: a_name,
        };
        assert!(
            !mnemora_sync::log_resumed(db, me, &b_id).unwrap(),
            "nothing was paused"
        );
        assert!(mnemora_sync::log_paused(db, me, &b_id, Why::Update).unwrap());
        // After a restart the device meets the same pause again.
        assert!(!mnemora_sync::log_paused(db, me, &b_id, Why::Update).unwrap());
        assert!(mnemora_sync::log_resumed(db, me, &b_id).unwrap());
        assert!(!mnemora_sync::log_resumed(db, me, &b_id).unwrap());
        assert!(mnemora_sync::log_paused(db, me, &b_id, Why::Update).unwrap());
    });
    assert_eq!(of_kind(&c[a], "paused").len(), 2);
    assert_eq!(of_kind(&c[a], "resumed").len(), 1);
}

// ---------------------------------------------------------------- paging and filters

#[test]
fn a_long_log_pages_newest_first_without_gaps_or_repeats_and_filters_by_device() {
    let c = group(3);
    let (a_id, a_name) = local(&c, 0);
    let (b_id, _) = local(&c, 1);
    let total = 450_usize;
    for n in 0..total {
        c[0].clock().advance(1_000);
        c[0].with_db(|db| {
            let name = format!("Mac {n}");
            mnemora_sync::log_renamed(db, &a_id, a_name, &name).unwrap();
        });
        if n % 3 == 0 {
            // Every third line also involves device 1 (a forgotten peer).
            c[0].with_db(|db| {
                mnemora_sync::log_forgot(
                    db,
                    Local {
                        device: &a_id,
                        name: a_name,
                    },
                    &b_id,
                    "Pixel 8",
                )
                .unwrap();
            });
        }
    }
    let expected = total + total.div_ceil(3);

    let mut seen: Vec<Value> = Vec::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    loop {
        let p = page(&c[0], json!({ "kind": "all" }), cursor.as_deref(), 100);
        pages += 1;
        let rows = p["rows"].as_array().unwrap();
        assert!(rows.len() <= 100);
        seen.extend(rows.iter().cloned());
        match p["next"].as_str() {
            Some(n) => {
                assert_eq!(rows.len(), 100, "a page is full unless it is the last");
                cursor = Some(n.to_owned());
            }
            None => break,
        }
    }
    assert_eq!(seen.len(), expected);
    assert_eq!(pages, expected.div_ceil(100));
    assert_eq!(ids(&seen).len(), expected, "no repeats");
    let times: Vec<i64> = seen.iter().map(|r| r["at"].as_i64().unwrap()).collect();
    assert!(times.windows(2).all(|w| w[0] >= w[1]), "newest first");

    // Device 0 took part in everything; device 1 only in the forgotten-peer lines; device 2 in none.
    let count = |filter: Value| {
        let mut n = 0;
        let mut cursor: Option<String> = None;
        loop {
            let p = page(&c[0], filter.clone(), cursor.as_deref(), 200);
            n += p["rows"].as_array().unwrap().len();
            match p["next"].as_str() {
                Some(next) => cursor = Some(next.to_owned()),
                None => return n,
            }
        }
    };
    assert_eq!(count(json!({ "kind": "this_device" })), expected);
    assert_eq!(
        count(json!({ "kind": "device", "id": b_id })),
        total.div_ceil(3)
    );
    assert_eq!(
        count(json!({ "kind": "device", "id": c[2].device_id() })),
        0
    );
}

#[test]
fn a_page_asks_for_at_least_one_row_and_at_most_the_limit() {
    let c = group(2);
    let (a_id, a_name) = local(&c, 0);
    for n in 0..5 {
        c[0].clock().advance(1_000);
        c[0].with_db(|db| {
            mnemora_sync::log_renamed(db, &a_id, a_name, &format!("Mac {n}")).unwrap();
        });
    }
    assert_eq!(
        page(&c[0], json!({ "kind": "all" }), None, 0)["rows"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        page(&c[0], json!({ "kind": "all" }), None, 100_000)["rows"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
}
