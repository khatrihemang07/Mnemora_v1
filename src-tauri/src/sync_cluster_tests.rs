//! Sync scenarios for Todo Tasks on a [`Cluster`]: several devices, each a `TestApp` with its own
//! database, synced pairwise through the sync crate's in-memory transport. Every step is a real
//! command on a device; every assertion reads back through commands (ADR 0009).

use serde_json::{Value, json};

use crate::test_support::{Cluster, TestApp, inbox_tasks_of};

const TODAY: &str = "2026-10-22";

fn local() -> Value {
    json!({ "today": TODAY, "now": "14:05", "offset_min": 0 })
}

fn create(app: &TestApp, title: &str) -> Value {
    app.invoke("todo_task_create", json!({ "title": title }))
        .unwrap()
}

fn create_repeating(app: &TestApp, title: &str, date: &str) -> Value {
    app.invoke(
        "todo_task_create",
        json!({
            "title": title,
            "when": { "date": date, "time": null, "local": local() },
            "details": { "repeat": { "freq": "day", "every": 1, "days": null, "mode": "fixed" } },
        }),
    )
    .unwrap()
}

fn task(app: &TestApp, id: &Value) -> Result<Value, Value> {
    app.invoke("todo_task", json!({ "id": id }))
}

fn priority(app: &TestApp, id: &Value, p: u32) {
    app.invoke::<Value>("todo_task_set_priority", json!({ "id": id, "priority": p }))
        .unwrap();
}

fn set_when(app: &TestApp, id: &Value, date: &str, time: Option<&str>, repeat: Option<Value>) {
    app.invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": id, "date": date, "time": time, "repeat": repeat, "local": local() }),
    )
    .unwrap();
}

fn complete(app: &TestApp, id: &Value) -> Value {
    app.invoke("todo_task_complete", json!({ "id": id, "local": local() }))
        .unwrap()
}

fn update(app: &TestApp, id: &Value, title: &str, notes: &str, session: &str) {
    app.invoke::<Value>(
        "todo_task_update",
        json!({ "id": id, "title": title, "notes": notes, "sessionId": session }),
    )
    .unwrap();
}

fn history(app: &TestApp, id: &Value) -> Vec<String> {
    let lines: Value = app
        .invoke("todo_task_history", json!({ "id": id }))
        .unwrap();
    lines
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            format!(
                "{} {}",
                l["kind"].as_str().unwrap(),
                l["text"].as_str().unwrap()
            )
        })
        .collect()
}

/// Titles of every Task Done shows, across all days.
fn done_titles(app: &TestApp) -> Vec<String> {
    let view: Value = app
        .invoke(
            "todo_done",
            json!({ "before": null, "days": 60, "local": local() }),
        )
        .unwrap();
    view["days"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|d| d["tasks"].as_array().unwrap().iter())
        .map(|t| t["title"].as_str().unwrap().to_owned())
        .collect()
}

fn inbox_titles(app: &TestApp) -> Vec<String> {
    let view: Value = app
        .invoke("todo_inbox", json!({ "local": local() }))
        .unwrap();
    let mut titles: Vec<String> = inbox_tasks_of(&view)
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_owned())
        .collect();
    titles.sort();
    titles
}

/// A Task as the user sees it, without the fields that are per device (`updated_at`).
fn seen(app: &TestApp, id: &Value) -> Value {
    match task(app, id) {
        Ok(mut t) => {
            t.as_object_mut().unwrap().remove("updated_at");
            t
        }
        Err(e) => json!({ "error": e["code"] }),
    }
}

/// Two devices that have synced one Task and then diverge.
fn pair_with_task(title: &str) -> (Cluster, Value) {
    let c = Cluster::new(2);
    let t = create(&c[0], title);
    c.sync(0, 1);
    (c, t["id"].clone())
}

// ---------------------------------------------------------------- merging fields

#[test]
fn a_new_task_reaches_the_other_device_as_it_was_made() {
    let c = Cluster::new(2);
    let made = c[0]
        .invoke::<Value>(
            "todo_task_create",
            json!({
                "title": "Plan weekend hike",
                "when": { "date": "2026-10-24", "time": "09:30", "local": local() },
                "details": { "notes": "Boots", "priority": 2 },
            }),
        )
        .unwrap();
    let (sent, got) = c.sync(0, 1);
    assert_eq!((sent.sent, got.received), (1, 1));
    let there = seen(&c[1], &made["id"]);
    assert_eq!(seen(&c[0], &made["id"]), there);
    assert_eq!(there["title"], "Plan weekend hike");
    assert_eq!(there["due_time"], "09:30");
    assert_eq!(there["notes"], "Boots");
}

#[test]
fn priority_changed_on_one_device_and_due_date_on_the_other_both_survive_on_both() {
    let (c, id) = pair_with_task("Water plants");
    c[0].clock().advance(1_000);
    priority(&c[0], &id, 1);
    c[1].clock().advance(2_000);
    set_when(&c[1], &id, "2026-10-30", None, None);
    c.sync(0, 1);
    for device in 0..2 {
        let t = task(&c[device], &id).unwrap();
        assert_eq!(t["priority"], 1, "device {device}");
        assert_eq!(t["due_date"], "2026-10-30", "device {device}");
    }
}

#[test]
fn the_same_field_changed_on_both_takes_the_newest_everywhere() {
    let (c, id) = pair_with_task("Water plants");
    c[0].clock().advance(5_000);
    priority(&c[0], &id, 2);
    c[1].clock().advance(1_000);
    priority(&c[1], &id, 3);
    c.sync(0, 1);
    assert_eq!(task(&c[0], &id).unwrap()["priority"], 2, "A was later");
    assert_eq!(task(&c[1], &id).unwrap()["priority"], 2);
}

#[test]
fn the_when_group_is_taken_whole_from_the_newest_never_mixed() {
    let (c, id) = pair_with_task("Stretch");
    // A: a date, a time and a weekly Repeat. B (newer): only a different date.
    c[0].clock().advance(1_000);
    set_when(
        &c[0],
        &id,
        "2026-10-26",
        Some("07:00"),
        Some(json!({ "freq": "week", "every": 1, "days": null, "mode": "fixed" })),
    );
    c[1].clock().advance(2_000);
    set_when(&c[1], &id, "2026-11-02", None, None);
    c.sync(0, 1);
    for device in 0..2 {
        let t = task(&c[device], &id).unwrap();
        assert_eq!(t["due_date"], "2026-11-02", "device {device}");
        assert_eq!(t["due_time"], Value::Null, "no time carried over from A");
        assert_eq!(
            t["repeat_freq"],
            Value::Null,
            "no Repeat carried over from A"
        );
    }

    // And the other way round: the older side's Repeat must not survive a newer date change
    // that only touches the date.
    let (c, id) = pair_with_task("Stretch");
    c[0].clock().advance(2_000);
    set_when(&c[0], &id, "2026-11-02", None, None);
    c[1].clock().advance(1_000);
    set_when(
        &c[1],
        &id,
        "2026-10-26",
        Some("07:00"),
        Some(json!({ "freq": "week", "every": 1, "days": null, "mode": "fixed" })),
    );
    c.sync(0, 1);
    for device in 0..2 {
        let t = task(&c[device], &id).unwrap();
        assert_eq!(t["due_date"], "2026-11-02", "device {device}");
        assert_eq!(t["repeat_freq"], Value::Null);
        assert_eq!(t["due_time"], Value::Null);
    }
}

#[test]
fn the_state_and_its_closed_time_are_taken_together() {
    let (c, id) = pair_with_task("Call the bank");
    c[0].clock().advance(1_000);
    complete(&c[0], &id);
    // The two close at different moments, so mixing one's state with the other's time shows.
    std::thread::sleep(std::time::Duration::from_millis(3));
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>("todo_task_wont_do", json!({ "id": id, "local": local() }))
        .unwrap();
    c.sync(0, 1);
    let (a, b) = (task(&c[0], &id).unwrap(), task(&c[1], &id).unwrap());
    assert_eq!(a["state"], "wont", "B was later");
    assert_eq!(
        (&a["state"], &a["closed_at"]),
        (&b["state"], &b["closed_at"])
    );
    let reopened = c[1].with_db(|db| {
        db.query_row(
            "SELECT closed_at FROM todo_tasks WHERE id = ?1",
            [id.as_str()],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
    });
    assert_eq!(a["closed_at"], reopened);
}

#[test]
fn recognised_words_follow_the_newest_title() {
    let (c, id) = pair_with_task("Call mum tomorrow");
    let words = json!([{ "word": "tomorrow", "kind": "date", "value": "2026-10-23" }]).to_string();
    let quick = |id: &Value, title: &str, understood: Option<&str>| {
        json!({
            "id": id,
            "edit": {
                "title": title, "understood": understood, "due_date": null, "due_time": null,
                "repeat": null, "priority": 4, "list_id": null
            },
            "local": local(),
        })
    };
    // A's Quick edit sets the title and its recognised words together.
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>(
        "todo_task_quick_edit",
        quick(&id, "Call mum tomorrow!", Some(&words)),
    )
    .unwrap();
    // B, later, retitles from the Task sheet and so brings the words it had (none).
    c[1].clock().advance(2_000);
    update(&c[1], &id, "Call mum", "", "sheet");
    c.sync(0, 1);
    let (a, b) = (task(&c[0], &id).unwrap(), task(&c[1], &id).unwrap());
    assert_eq!(a["title"], "Call mum");
    assert_eq!(
        a["understood"],
        Value::Null,
        "the words of the older title are gone"
    );
    assert_eq!(
        (&a["title"], &a["understood"]),
        (&b["title"], &b["understood"])
    );

    // And when A's edit is the newer one, its words come with its title.
    let (c, id) = pair_with_task("Call mum tomorrow");
    c[1].clock().advance(1_000);
    update(&c[1], &id, "Call mum", "", "sheet");
    c[0].clock().advance(2_000);
    c[0].invoke::<Value>(
        "todo_task_quick_edit",
        quick(&id, "Call mum tomorrow!", Some(&words)),
    )
    .unwrap();
    c.sync(0, 1);
    for device in 0..2 {
        let t = task(&c[device], &id).unwrap();
        assert_eq!(t["title"], "Call mum tomorrow!");
        assert_eq!(t["understood"], json!(words));
    }
}

#[test]
fn manual_order_takes_the_newest_key() {
    let c = Cluster::new(2);
    let (a, b, d) = (create(&c[0], "a"), create(&c[0], "b"), create(&c[0], "d"));
    c.sync(0, 1);
    let reorder = |app: &TestApp, before: Option<&Value>, after: Option<&Value>| -> Value {
        app.invoke(
            "todo_task_reorder",
            json!({
                "id": d["id"], "before": before.map(|t| &t["id"]), "after": after.map(|t| &t["id"]),
                "target": [], "local": local(),
            }),
        )
        .unwrap()
    };
    // A puts "d" first; B, later, puts it between "a" and "b".
    c[0].clock().advance(1_000);
    reorder(&c[0], None, Some(&a));
    c[1].clock().advance(2_000);
    let wanted = reorder(&c[1], Some(&a), Some(&b));
    c.sync(0, 1);
    for device in 0..2 {
        assert_eq!(
            inbox_in_order(&c[device]),
            vec!["a", "d", "b"],
            "device {device}"
        );
        assert_eq!(
            task(&c[device], &d["id"]).unwrap()["sort_key"],
            wanted["sort_key"]
        );
    }
}

fn inbox_in_order(app: &TestApp) -> Vec<String> {
    let view: Value = app
        .invoke("todo_inbox", json!({ "local": local() }))
        .unwrap();
    inbox_tasks_of(&view)
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_owned())
        .collect()
}

// ---------------------------------------------------------------- deletes and repeats

#[test]
fn delete_on_one_device_and_notes_on_the_other_leaves_it_deleted_with_the_new_notes() {
    let (c, id) = pair_with_task("Plan weekend hike");
    c[0].clock().advance(1_000);
    c[0].invoke::<()>("todo_task_delete", json!({ "id": id }))
        .unwrap();
    c[1].clock().advance(2_000);
    update(&c[1], &id, "Plan weekend hike", "Take the long trail", "s1");
    c.sync(0, 1);
    for device in 0..2 {
        assert_eq!(
            task(&c[device], &id).unwrap_err()["code"],
            "not_found",
            "deleted on device {device}"
        );
        // Restoring brings back what was inside: the new notes.
        let back: Value = c[device]
            .invoke("todo_task_restore", json!({ "id": id }))
            .unwrap();
        assert_eq!(back["notes"], "Take the long trail", "device {device}");
    }
}

#[test]
fn a_deleted_task_stays_deleted_when_an_old_device_syncs_later() {
    let (c, id) = pair_with_task("Old");
    c[0].invoke::<()>("todo_task_delete", json!({ "id": id }))
        .unwrap();
    c.sync(0, 1);
    c.sync(0, 1);
    assert!(task(&c[1], &id).is_err());
}

#[test]
fn completing_the_same_repeating_task_offline_on_two_devices_gives_one_occurrence() {
    let c = Cluster::new(2);
    let t = create_repeating(&c[0], "Take vitamins", "2026-10-22");
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    complete(&c[0], &t["id"]);
    c[1].clock().advance(2_000);
    complete(&c[1], &t["id"]);
    c.sync(0, 1);
    for device in 0..2 {
        assert_eq!(
            done_titles(&c[device]),
            vec!["Take vitamins"],
            "one occurrence on device {device}"
        );
        let live = task(&c[device], &t["id"]).unwrap();
        assert_eq!(live["due_date"], "2026-10-23", "moved on once, not twice");
        assert_eq!(live["state"], "open");
    }
}

// ---------------------------------------------------------------- the engine

#[test]
fn a_session_rewritten_after_a_sync_is_sent_again_and_not_duplicated() {
    let (c, id) = pair_with_task("Draft");
    c[0].clock().advance(1_000);
    update(&c[0], &id, "Draft v2", "first thoughts", "sheet-1");
    c.sync(0, 1);
    assert_eq!(task(&c[1], &id).unwrap()["title"], "Draft v2");

    // The same opening of the Task sheet goes on: its record is rewritten at the origin.
    c[0].clock().advance(1_000);
    update(&c[0], &id, "Draft v3", "more thoughts", "sheet-1");
    let (sent, got) = c.sync(0, 1);
    assert_eq!(
        (sent.sent, got.received),
        (1, 1),
        "the rewrite is sent again"
    );
    let there = task(&c[1], &id).unwrap();
    assert_eq!(
        (&there["title"], &there["notes"]),
        (&json!("Draft v3"), &json!("more thoughts"))
    );

    // The receiver holds one record for the session, so History reads the same on both.
    assert_eq!(history(&c[1], &id), history(&c[0], &id));
    let (again, _) = c.sync(0, 1);
    assert_eq!(again.sent, 0, "nothing left to send");
}

#[test]
fn an_exchange_cut_mid_way_resumes_exactly_without_duplicates_or_gaps() {
    let c = Cluster::new(2);
    let mut titles = Vec::new();
    for n in 0..1_200 {
        let title = format!("Task {n:04}");
        create(&c[0], &title);
        titles.push(title);
    }
    // Hello each way, then the first batch of 500 reaches B and is applied; the line drops
    // before the second.
    let cut = c.try_sync(0, 1, Some(4));
    assert!(cut.is_err(), "the cut line fails the exchange");
    let held = inbox_titles(&c[1]).len();
    assert_eq!(held, 500, "exactly the batches that were acknowledged");

    let (sent, got) = c.sync(0, 1);
    assert_eq!(
        (sent.sent, got.received),
        (700, 700),
        "only what was missing"
    );
    assert_eq!(inbox_titles(&c[1]), titles, "every Task once, none missing");
    let (nothing, _) = c.sync(0, 1);
    assert_eq!(nothing.sent, 0);
}

#[test]
fn a_change_travels_through_a_middle_device() {
    let c = Cluster::new(3);
    let t = create(&c[0], "Relayed");
    c.sync(0, 1);
    c.sync(1, 2);
    assert_eq!(seen(&c[2], &t["id"]), seen(&c[0], &t["id"]));
    // Later edits from C reach A through B too.
    c[2].clock().advance(1_000);
    priority(&c[2], &t["id"], 1);
    c.sync(2, 1);
    c.sync(1, 0);
    assert_eq!(task(&c[0], &t["id"]).unwrap()["priority"], 1);
}

#[test]
fn remote_changes_tell_todo_to_refresh_and_the_widget_to_redraw() {
    let c = Cluster::new(2);
    create(&c[0], "Fresh");
    let before = c[1].widget_signals();
    c.sync(0, 1);
    assert_eq!(c[1].widget_signals(), before + 1, "once for the batch");
    c.sync(0, 1);
    assert_eq!(
        c[1].widget_signals(),
        before + 1,
        "nothing new, nothing signalled"
    );
}

#[test]
fn received_changes_are_remote_not_new_local_ones() {
    let (c, id) = pair_with_task("Original");
    c[0].clock().advance(1_000);
    priority(&c[0], &id, 1);
    c.sync(0, 1);
    // B did nothing of its own, so it has nothing to send back.
    let (a, b) = c.sync(0, 1);
    assert_eq!((a.sent, b.sent), (0, 0));
    assert_eq!(history(&c[1], &id), history(&c[0], &id));
}

#[test]
fn a_skip_reads_as_skipped_in_history_on_the_other_device_too() {
    let c = Cluster::new(2);
    let t = create_repeating(&c[0], "Water plants", "2026-10-22");
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>("todo_task_skip", json!({ "id": t["id"], "local": local() }))
        .unwrap();
    c.sync(0, 1);
    let mine = history(&c[0], &t["id"]);
    assert!(mine[0].starts_with("skip Skipped"), "{mine:?}");
    assert_eq!(history(&c[1], &t["id"]), mine, "History reads the same");
    // The marker is only on the skip: a later ordinary edit is not a skip.
    c[1].clock().advance(1_000);
    priority(&c[1], &t["id"], 1);
    c.sync(0, 1);
    assert_eq!(history(&c[1], &t["id"]), history(&c[0], &t["id"]));
}

#[test]
fn a_session_does_not_claim_a_value_another_device_changed_since() {
    let c = Cluster::new(3);
    let t = create(&c[0], "Report");
    let id = t["id"].clone();
    c.sync_all();
    // B opens the sheet and saves a title.
    c[1].clock().advance(1_000);
    update(&c[1], &id, "B title", "", "sheet-b");
    // A sets a newer title; B receives it.
    c[0].clock().advance(2_000);
    update(&c[0], &id, "A title", "", "sheet-a");
    c.sync(0, 1);
    assert_eq!(task(&c[1], &id).unwrap()["title"], "A title");
    // C, who has seen nobody, sets a title between A's and B's next save.
    c[2].clock().advance(3_000);
    update(&c[2], &id, "C title", "", "sheet-c");
    // B's sheet goes on (it shows A's title now) and only the notes change.
    c[1].clock().advance(4_000);
    update(&c[1], &id, "A title", "some notes", "sheet-b");
    c.sync_all();
    for d in 0..3 {
        assert_eq!(
            task(&c[d], &id).unwrap()["title"],
            "C title",
            "the newest title edit wins on device {d}; B only edited notes"
        );
        assert_eq!(task(&c[d], &id).unwrap()["notes"], "some notes");
    }
}

#[test]
fn a_local_edit_made_between_two_exchanges_is_judged_by_its_own_clock() {
    let c = Cluster::new(3);
    let t = create(&c[0], "Clocks");
    let id = t["id"].clone();
    c.sync_all();
    // C edits early, but its change only reaches B after B has synced once and edited since.
    c[2].clock().advance(1_000);
    priority(&c[2], &id, 3);
    c[0].clock().advance(2_000);
    priority(&c[0], &id, 2);
    c.sync(0, 1);
    assert_eq!(task(&c[1], &id).unwrap()["priority"], 2);
    c[1].clock().advance(5_000);
    priority(&c[1], &id, 1);
    c.sync(2, 1);
    assert_eq!(
        task(&c[1], &id).unwrap()["priority"],
        1,
        "B's later edit is not beaten by C's older one"
    );
    c.sync_all();
    for d in 0..3 {
        assert_eq!(task(&c[d], &id).unwrap()["priority"], 1, "device {d}");
    }
}

#[test]
fn a_record_that_arrives_by_two_paths_is_stored_once() {
    let c = Cluster::new(3);
    let t = create(&c[0], "Twice");
    c[0].clock().advance(10);
    priority(&c[0], &t["id"], 2);
    c.sync(0, 2);
    c.sync(0, 1);
    let (sent, got) = c.sync(2, 1);
    assert_eq!((sent.sent, got.received), (0, 0), "B already holds both");
    let rows: i64 = c[1].with_db(|db| {
        db.query_row("SELECT COUNT(*) FROM change_log", [], |r| r.get(0))
            .unwrap()
    });
    assert_eq!(rows, 2);
}

// ---------------------------------------------------------------- sync off

#[test]
fn with_sync_off_no_engine_work_runs_and_changes_wait_for_it_to_be_switched_on() {
    let c = Cluster::from_devices(vec![TestApp::syncing(), TestApp::new()]);
    let t = create(&c[0], "Made before pairing");
    let off = &c[1];
    assert!(
        c.try_sync(0, 1, None).is_err(),
        "the engine refuses while off"
    );
    assert_eq!(
        off.sync_probe().sessions,
        0,
        "no session ever started on the off device"
    );
    for table in ["sync_seen", "sync_field_clocks", "sync_clock_marks"] {
        let rows: i64 = off.with_db(|db| {
            db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap()
        });
        assert_eq!(rows, 0, "{table} stays empty");
    }
    assert!(task(off, &t["id"]).is_err());

    // Switch it on: the changes made meanwhile sync.
    off.invoke::<()>("sync_set_enabled", json!({ "enabled": true }))
        .unwrap();
    c[1].clock().freeze(Cluster::START);
    c.sync(0, 1);
    assert_eq!(task(off, &t["id"]).unwrap()["title"], "Made before pairing");
}

// ---------------------------------------------------------------- convergence

/// A tiny seeded generator (no dependency): xorshift64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).unwrap()).unwrap()
    }
}

fn converge(seed: u64) {
    let c = Cluster::new(3);
    let mut rng = Rng(seed);
    let mut ids: Vec<Value> = Vec::new();
    for step in 0..120 {
        let d = rng.below(3);
        c[d].clock()
            .advance(1 + i64::try_from(rng.below(40)).unwrap());
        let known: Vec<Value> = ids.clone();
        let pick = |rng: &mut Rng| known.get(rng.below(known.len().max(1))).cloned();
        match (rng.below(12), pick(&mut rng)) {
            (0, _) | (_, None) => {
                let t = create(&c[d], &format!("t{step}"));
                ids.push(t["id"].clone());
            }
            (1, Some(id)) => {
                let _ = c[d].invoke::<Value>(
                    "todo_task_set_priority",
                    json!({ "id": id, "priority": 1 + rng.below(4) }),
                );
            }
            (2, Some(id)) => {
                let _ = c[d].invoke::<Value>(
                    "todo_task_update",
                    json!({ "id": id, "title": format!("e{step}"), "notes": format!("n{}", rng.below(5)), "sessionId": format!("s{step}") }),
                );
            }
            (3, Some(id)) => {
                let _ = c[d].invoke::<Value>(
                    "todo_task_set_when",
                    json!({ "id": id, "date": format!("2026-11-{:02}", 1 + rng.below(27)), "time": null, "repeat": null, "local": local() }),
                );
            }
            (4, Some(id)) => {
                let _ = c[d].invoke::<()>("todo_task_delete", json!({ "id": id }));
            }
            (5, Some(id)) => {
                let _ = c[d].invoke::<Value>("todo_task_restore", json!({ "id": id }));
            }
            (6, Some(id)) => {
                let _ = c[d]
                    .invoke::<Value>("todo_task_complete", json!({ "id": id, "local": local() }));
            }
            (7, Some(id)) => {
                let _ = c[d].invoke::<Value>("todo_task_reopen", json!({ "id": id }));
            }
            (8, Some(id)) => {
                let _ = c[d]
                    .invoke::<Value>("todo_task_wont_do", json!({ "id": id, "local": local() }));
            }
            // An editing session that goes on across syncs: its record is rewritten in place.
            (9, Some(id)) => {
                let _ = c[d].invoke::<Value>(
                    "todo_task_update",
                    json!({ "id": id, "title": format!("r{}", rng.below(4)), "notes": format!("m{}", rng.below(3)), "sessionId": format!("d{d}-{}", rng.below(2)) }),
                );
            }
            (10, _) => {
                let t = create_repeating(&c[d], &format!("rep{step}"), "2026-10-22");
                ids.push(t["id"].clone());
            }
            (_, Some(id)) => {
                let _ =
                    c[d].invoke::<Value>("todo_task_skip", json!({ "id": id, "local": local() }));
            }
        }
        if rng.below(4) == 0 {
            let (a, b) = (rng.below(3), rng.below(3));
            if a != b {
                // Some exchanges drop part way: the next one must pick up exactly.
                let cut = (rng.below(3) == 0).then(|| 2 + rng.below(4));
                let _ = c.try_sync(a, b, cut);
            }
        }
    }
    c.sync_all();
    for id in &ids {
        let first = seen(&c[0], id);
        for d in 1..3 {
            assert_eq!(
                seen(&c[d], id),
                first,
                "seed {seed}: device {d} differs on {id}"
            );
        }
    }
}

#[test]
fn three_devices_with_random_edits_end_identical() {
    for seed in [1, 7, 42, 2026, 99_991, 31_337] {
        converge(seed);
    }
}
