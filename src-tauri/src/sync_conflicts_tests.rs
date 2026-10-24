//! Sync scenarios for text merging and Conflicts on a [`Cluster`]: edits to the same text on
//! several devices merge when they touch different words and raise one Conflict, the same on every
//! device, when they touch the same ones. Every step is a real command; every assertion reads back
//! through commands (ADR 0009).

use serde_json::{Value, json};

use crate::test_support::{Cluster, TestApp};

const ENTRY: &str = "journal_entry";

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

fn edit(app: &TestApp, id: &Value, text: &str, session: &str) {
    app.invoke::<Value>(
        "journal_entry_update",
        json!({ "id": id, "text": text, "sessionId": session }),
    )
    .unwrap();
}

pub(crate) fn body(app: &TestApp, id: &Value) -> String {
    let pages: Vec<Value> = app
        .invoke(
            "journal_pages",
            json!({ "cursor": null, "limit": 50, "direction": "older" }),
        )
        .unwrap();
    pages
        .iter()
        .flat_map(|p| p["entries"].as_array().unwrap().iter())
        .find(|e| e["id"] == *id)
        .map(|e| e["body"].as_str().unwrap().to_owned())
        .unwrap_or_default()
}

fn delete_entry(app: &TestApp, id: &Value) {
    app.invoke::<()>("journal_entry_delete", json!({ "id": id }))
        .unwrap();
}

/// "Needs you" as a device lists it.
pub(crate) fn waiting(app: &TestApp) -> Vec<Value> {
    let list: Value = app.invoke("sync_conflicts", json!({})).unwrap();
    list["needs_you"].as_array().unwrap().clone()
}

pub(crate) fn ids(app: &TestApp) -> Vec<String> {
    let mut ids: Vec<String> = waiting(app)
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect();
    ids.sort();
    ids
}

fn compare(app: &TestApp, id: &str) -> Value {
    app.invoke::<Option<Value>>("sync_conflict", json!({ "id": id }))
        .unwrap()
        .unwrap()
}

/// The text of a side of Compare: its stretches put together.
fn text_of(spans: &Value) -> String {
    spans
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["text"].as_str().unwrap())
        .collect()
}

pub(crate) fn resolve(app: &TestApp, id: &str, choice: Value) {
    app.invoke::<()>(
        "sync_conflict_resolve",
        json!({ "id": id, "choice": choice }),
    )
    .unwrap();
}

const BASE: &str = "Went for a run along the canal.\nCoffee with Sam after.\n";
const MINE: &str =
    "Went for a run along the canal, felt great after the rain.\nCoffee with Sam after.\n";
const YOURS: &str =
    "Went for a run along the canal, legs heavy but good.\nCoffee with Sam after.\n";

/// Two devices with one Entry, where A (older) and B (newer) then rewrite the same sentence.
fn clashing() -> (Cluster, Value) {
    let c = Cluster::new(2);
    let id = create_entry(&c[0], BASE)["id"].clone();
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    edit(&c[0], &id, MINE, "a");
    c[1].clock().advance(2_000);
    edit(&c[1], &id, YOURS, "b");
    c.sync(0, 1);
    (c, id)
}

// ---------------------------------------------------------------- silent merges

#[test]
fn edits_to_different_parts_of_an_entry_merge_silently_to_the_same_text() {
    let c = Cluster::new(2);
    let base = "Morning page.\nMiddle of the day.\nEvening page.\n";
    let id = create_entry(&c[0], base)["id"].clone();
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    edit(
        &c[0],
        &id,
        "Morning, very early page.\nMiddle of the day.\nEvening page.\n",
        "a",
    );
    c[1].clock().advance(2_000);
    edit(
        &c[1],
        &id,
        "Morning page.\nMiddle of the day.\nEvening, tired page.\n",
        "b",
    );
    c.sync(0, 1);
    let merged = "Morning, very early page.\nMiddle of the day.\nEvening, tired page.\n";
    for d in 0..2 {
        assert_eq!(body(&c[d], &id), merged, "device {d}");
        assert!(waiting(&c[d]).is_empty(), "device {d} was not asked");
    }
    // Nothing is left to send: the merge was not a new change of either device.
    let (a, b) = c.sync(0, 1);
    assert_eq!((a.sent, b.sent), (0, 0));
    assert_eq!(body(&c[0], &id), body(&c[1], &id));
}

#[test]
fn different_words_of_one_line_merge_and_a_late_record_is_replayed_in_its_place() {
    let c = Cluster::new(3);
    let id = create_entry(&c[0], "The quick brown fox jumps over the lazy dog.")["id"].clone();
    c.sync_all();
    c[0].clock().advance(1_000);
    edit(
        &c[0],
        &id,
        "The quick red fox jumps over the lazy dog.",
        "a",
    );
    c[1].clock().advance(2_000);
    edit(
        &c[1],
        &id,
        "The quick brown fox leaps over the lazy dog.",
        "b",
    );
    c[2].clock().advance(3_000);
    edit(
        &c[2],
        &id,
        "The quick brown fox jumps over the lazy cat.",
        "s-c",
    );
    // The newest edit reaches the middle device first, the oldest last.
    c.sync(1, 2);
    c.sync(0, 1);
    c.sync_all();
    for d in 0..3 {
        assert_eq!(
            body(&c[d], &id),
            "The quick red fox leaps over the lazy cat.",
            "device {d}"
        );
        assert!(waiting(&c[d]).is_empty());
    }
}

// ---------------------------------------------------------------- conflicts

#[test]
fn overlapping_edits_raise_one_conflict_on_both_devices_and_the_newest_shows() {
    let (c, id) = clashing();
    for d in 0..2 {
        assert_eq!(body(&c[d], &id), YOURS, "device {d} shows the newest");
        assert_eq!(waiting(&c[d]).len(), 1, "device {d}");
    }
    assert_eq!(ids(&c[0]), ids(&c[1]), "the same conflict on both");
    let row = &waiting(&c[0])[0];
    assert_eq!(row["module"], "journal");
    assert_eq!(row["subject"]["kind"], "entry");
    assert_eq!(row["subject"]["page_date"], "2026-10-21");
    // The time is sent as a timestamp and zone, so the screen can follow the Time format.
    assert!(row["subject"]["written_at"].as_i64().unwrap() > 0);
    assert_eq!(row["subject"]["written_offset_min"], 0);
    assert!(row["subject"].get("written_time").is_none());
    assert!(row["raised_at"].as_i64().unwrap() > 0);
    // More exchanges raise nothing new.
    c.sync_all();
    assert_eq!(ids(&c[0]), ids(&c[1]));
    assert_eq!(waiting(&c[1]).len(), 1);
}

#[test]
fn compare_shows_both_versions_with_the_differing_words_marked_and_the_original() {
    let (c, _) = clashing();
    let id = ids(&c[1])[0].clone();
    let view = compare(&c[1], &id);
    assert_eq!(view["open"], true);
    assert_eq!(text_of(&view["showing"]), YOURS);
    assert_eq!(text_of(&view["other"]), MINE);
    assert_eq!(view["original"], BASE);
    let marked = |side: &Value| -> Vec<String> {
        side.as_array()
            .unwrap()
            .iter()
            .filter(|s| s["changed"] == true)
            .map(|s| s["text"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(marked(&view["showing"]), ["legs heavy but good."]);
    assert_eq!(marked(&view["other"]), ["felt great after the rain."]);
    let clashes: Vec<&Value> = view["pieces"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["kind"] == "clash")
        .collect();
    assert_eq!(clashes.len(), 1);
    assert_eq!(clashes[0]["showing"], "legs heavy but good.");
    assert_eq!(clashes[0]["other"], "felt great after the rain.");
}

#[test]
fn resolving_on_one_device_clears_it_on_both() {
    let (c, id) = clashing();
    let conflict = ids(&c[0])[0].clone();
    // The older device keeps the other version: its own, which the merge had set aside.
    resolve(&c[0], &conflict, json!({ "kind": "keep_other" }));
    assert_eq!(body(&c[0], &id), MINE);
    assert!(waiting(&c[0]).is_empty());
    assert_eq!(waiting(&c[1]).len(), 1, "not yet told");
    c.sync(0, 1);
    assert!(waiting(&c[1]).is_empty(), "resolved there too");
    assert_eq!(
        body(&c[1], &id),
        MINE,
        "the choice is an ordinary edit that travelled"
    );
    let view = compare(&c[1], &conflict);
    assert_eq!(
        (view["open"].clone(), view["resolution"].clone()),
        (json!(false), json!("keep_other"))
    );
    // And it stays resolved, with no new conflict, however the devices go on syncing.
    c.sync_all();
    for d in 0..2 {
        assert!(waiting(&c[d]).is_empty(), "device {d}");
        assert_eq!(body(&c[d], &id), MINE);
    }
}

#[test]
fn keep_this_ends_the_conflict_and_changes_no_text() {
    let (c, id) = clashing();
    let conflict = ids(&c[1])[0].clone();
    resolve(&c[1], &conflict, json!({ "kind": "keep_this" }));
    assert_eq!(body(&c[1], &id), YOURS);
    let (a, b) = c.sync(0, 1);
    assert!(a.sent + b.sent > 0, "the decision travels");
    assert!(waiting(&c[0]).is_empty());
    assert_eq!(body(&c[0], &id), YOURS);
}

#[test]
fn edited_merge_writes_the_chosen_text_and_resolves_everywhere() {
    let (c, id) = clashing();
    let conflict = ids(&c[1])[0].clone();
    let draft: String = c[1]
        .invoke(
            "sync_conflict_draft",
            json!({ "id": conflict, "choices": ["both"] }),
        )
        .unwrap();
    assert_eq!(
        draft,
        "Went for a run along the canal, legs heavy but good. felt great after the rain.\nCoffee with Sam after.\n"
    );
    let first: String = c[1]
        .invoke(
            "sync_conflict_draft",
            json!({ "id": conflict, "choices": ["other"] }),
        )
        .unwrap();
    assert_eq!(first, MINE);
    let polished = "Went for a run along the canal; legs heavy, but it felt great after the rain.\nCoffee with Sam after.\n";
    resolve(
        &c[1],
        &conflict,
        json!({ "kind": "merged", "text": polished }),
    );
    c.sync(0, 1);
    for d in 0..2 {
        assert_eq!(body(&c[d], &id), polished, "device {d}");
        assert!(waiting(&c[d]).is_empty(), "device {d}");
    }
}

#[test]
fn a_conflict_follows_the_latest_text_when_it_is_edited_before_review() {
    let (c, id) = clashing();
    let conflict = ids(&c[1])[0].clone();
    c[1].clock().advance(1_000);
    let newer = format!("{YOURS}Then a long walk home.\n");
    edit(&c[1], &id, &newer, "later");
    // Still waiting, the same one, and Showing is the text as it is now.
    assert_eq!(ids(&c[1]), std::slice::from_ref(&conflict));
    let view = compare(&c[1], &conflict);
    assert_eq!(text_of(&view["showing"]), newer);
    assert_eq!(
        text_of(&view["other"]),
        MINE,
        "the other version stays as it was"
    );
    // The other device takes the new text and still has the conflict, now showing it too.
    c.sync(0, 1);
    assert_eq!(body(&c[0], &id), newer);
    assert_eq!(ids(&c[0]), std::slice::from_ref(&conflict));
    assert_eq!(text_of(&compare(&c[0], &conflict)["showing"]), newer);
    assert_eq!(waiting(&c[0]).len(), 1, "no second conflict from the edit");
}

fn draft(app: &TestApp, conflict: &str, choices: Value) -> String {
    app.invoke(
        "sync_conflict_draft",
        json!({ "id": conflict, "choices": choices }),
    )
    .unwrap()
}

#[test]
fn edit_merged_keeps_an_edit_made_outside_the_clash_after_the_conflict_was_raised() {
    let (c, id) = clashing();
    let conflict = ids(&c[1])[0].clone();
    c[1].clock().advance(1_000);
    let later = "Went for a run along the canal, legs heavy but good.
Coffee with Sam after.
Then a long walk home.
";
    edit(&c[1], &id, later, "later");
    // The later words are agreed text, not part of a clash.
    let view = compare(&c[1], &conflict);
    let pieces = view["pieces"].as_array().unwrap();
    assert_eq!(pieces.iter().filter(|p| p["kind"] == "clash").count(), 1);
    assert!(
        pieces
            .iter()
            .any(|p| p["kind"] == "agreed" && p["text"].as_str().unwrap().contains("long walk"))
    );
    assert_eq!(draft(&c[1], &conflict, json!(["showing"])), later);
    let other = draft(&c[1], &conflict, json!(["other"]));
    assert_eq!(other, format!("{MINE}Then a long walk home.\n"));
    let both = draft(&c[1], &conflict, json!(["both"]));
    assert!(both.contains("legs heavy but good. felt great after the rain."));
    assert!(both.ends_with("Then a long walk home.\n"));
    resolve(&c[1], &conflict, json!({ "kind": "merged", "text": both }));
    c.sync(0, 1);
    for d in 0..2 {
        assert_eq!(body(&c[d], &id), both, "device {d}");
        assert!(body(&c[d], &id).contains("Then a long walk home."));
        assert!(waiting(&c[d]).is_empty(), "device {d}");
    }
}

#[test]
fn edit_merged_keeps_an_edit_made_inside_the_clash_after_the_conflict_was_raised() {
    let (c, id) = clashing();
    let conflict = ids(&c[1])[0].clone();
    c[1].clock().advance(1_000);
    let later =
        "Went for a run along the canal, legs heavy but really good.\nCoffee with Sam after.\n";
    edit(&c[1], &id, later, "later");
    assert_eq!(ids(&c[1]), std::slice::from_ref(&conflict));
    // The clash now holds the words as they are, not as they were when it was raised.
    let view = compare(&c[1], &conflict);
    let clash = view["pieces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["kind"] == "clash")
        .unwrap()
        .clone();
    assert!(clash["showing"].as_str().unwrap().contains("really"));
    assert_eq!(clash["other"], "felt great after the rain.");
    assert_eq!(draft(&c[1], &conflict, json!(["showing"])), later);
    assert_eq!(draft(&c[1], &conflict, json!(["other"])), MINE);
    let both = draft(&c[1], &conflict, json!(["both"]));
    assert!(both.contains("really good."));
    assert!(both.contains("felt great after the rain."));
    resolve(&c[1], &conflict, json!({ "kind": "merged", "text": both }));
    c.sync(0, 1);
    for d in 0..2 {
        assert_eq!(body(&c[d], &id), both, "device {d}");
    }
}

#[test]
fn edit_merged_without_choices_keeps_the_text_as_it_shows_now() {
    let (c, id) = clashing();
    let conflict = ids(&c[1])[0].clone();
    c[1].clock().advance(1_000);
    let later = format!("Morning.\n{YOURS}");
    edit(&c[1], &id, &later, "later");
    assert_eq!(draft(&c[1], &conflict, json!([])), later);
}

#[test]
fn a_clash_on_a_deleted_entry_is_resolved_without_asking() {
    let c = Cluster::new(2);
    let id = create_entry(&c[0], BASE)["id"].clone();
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    edit(&c[0], &id, MINE, "a");
    c[1].clock().advance(2_000);
    edit(&c[1], &id, YOURS, "b");
    c[1].clock().advance(1_000);
    delete_entry(&c[1], &id);
    c.sync(0, 1);
    for d in 0..2 {
        assert!(
            waiting(&c[d]).is_empty(),
            "device {d} is not asked about something deleted"
        );
        assert_eq!(body(&c[d], &id), "", "device {d}: still deleted");
    }
    // The conflict is on record as resolved, the same one on both.
    let rows: Vec<String> = (0..2)
        .map(|d| {
            c[d].with_db(|db| {
                db.query_row(
                    "SELECT id || ':' || status || ':' || COALESCE(resolution, '') FROM sync_conflicts",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .unwrap()
            })
        })
        .collect();
    assert_eq!(rows[0], rows[1]);
    assert!(rows[0].ends_with(":resolved:deleted"), "{}", rows[0]);
}

// ---------------------------------------------------------------- Tasks and Comments

fn create_task(app: &TestApp, title: &str) -> Value {
    app.invoke("todo_task_create", json!({ "title": title }))
        .unwrap()
}

fn update_task(app: &TestApp, id: &Value, title: &str, notes: &str, session: &str) {
    app.invoke::<Value>(
        "todo_task_update",
        json!({ "id": id, "title": title, "notes": notes, "sessionId": session }),
    )
    .unwrap();
}

fn task(app: &TestApp, id: &Value) -> Value {
    app.invoke("todo_task", json!({ "id": id })).unwrap()
}

#[test]
fn task_notes_and_title_merge_apart_and_clash_apart() {
    let c = Cluster::new(2);
    let id = create_task(&c[0], "Plan weekend hike")["id"].clone();
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    update_task(
        &c[0],
        &id,
        "Plan the weekend hike",
        "Route: ridge loop from the north",
        "a",
    );
    c[1].clock().advance(2_000);
    update_task(
        &c[1],
        &id,
        "Plan weekend hike",
        "Route: lake path from the south",
        "b",
    );
    c.sync(0, 1);
    for d in 0..2 {
        let t = task(&c[d], &id);
        assert_eq!(
            t["title"], "Plan the weekend hike",
            "the title merged silently"
        );
        assert_eq!(
            t["notes"], "Route: lake path from the south",
            "the newest notes show"
        );
        let rows = waiting(&c[d]);
        assert_eq!(rows.len(), 1, "only the notes clashed");
        assert_eq!(rows[0]["subject"]["kind"], "task");
        assert_eq!(rows[0]["subject"]["field"], "notes");
        assert_eq!(rows[0]["subject"]["title"], "Plan the weekend hike");
        assert_eq!(rows[0]["module"], "todo");
    }
    assert_eq!(ids(&c[0]), ids(&c[1]));
    // Keep the other notes on one device.
    let conflict = ids(&c[0])[0].clone();
    resolve(&c[0], &conflict, json!({ "kind": "keep_other" }));
    c.sync(0, 1);
    for d in 0..2 {
        assert_eq!(
            task(&c[d], &id)["notes"],
            "Route: ridge loop from the north"
        );
        assert!(waiting(&c[d]).is_empty());
    }
}

#[test]
fn edit_merged_of_task_notes_keeps_a_later_edit_to_the_notes_and_the_title() {
    let c = Cluster::new(2);
    let id = create_task(&c[0], "Plan weekend hike")["id"].clone();
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    update_task(
        &c[0],
        &id,
        "Plan weekend hike",
        "Route: ridge loop from the north",
        "a",
    );
    c[1].clock().advance(2_000);
    update_task(
        &c[1],
        &id,
        "Plan weekend hike",
        "Route: lake path from the south",
        "b",
    );
    c.sync(0, 1);
    let conflict = ids(&c[1])[0].clone();
    // Later, on the device that is asked: the notes grow after the clash and the title changes.
    c[1].clock().advance(1_000);
    update_task(
        &c[1],
        &id,
        "Plan the weekend hike",
        "Route: lake path from the south\nBring water",
        "later",
    );
    assert_eq!(ids(&c[1]), std::slice::from_ref(&conflict));
    assert_eq!(
        draft(&c[1], &conflict, json!(["showing"])),
        "Route: lake path from the south\nBring water"
    );
    assert_eq!(
        draft(&c[1], &conflict, json!(["other"])),
        "Route: ridge loop from the north"
    );
    // The new line is part of the showing side of the clash (it is on the clashing line).
    let both = draft(&c[1], &conflict, json!(["both"]));
    assert!(both.contains("Bring water"), "{both}");
    assert!(both.contains("ridge loop from the north"), "{both}");
    resolve(&c[1], &conflict, json!({ "kind": "merged", "text": both }));
    c.sync(0, 1);
    for d in 0..2 {
        let t = task(&c[d], &id);
        assert_eq!(t["notes"], both.as_str(), "device {d}");
        assert_eq!(t["title"], "Plan the weekend hike", "device {d}");
        assert!(waiting(&c[d]).is_empty(), "device {d}");
    }
}

#[test]
fn comment_bodies_merge_three_way_and_keep_the_edited_mark() {
    let c = Cluster::new(2);
    let t = create_task(&c[0], "Trip")["id"].clone();
    let comment: Value = c[0]
        .invoke(
            "todo_comment_add",
            json!({ "taskId": t, "body": "Bring the stove and the gas" }),
        )
        .unwrap();
    let cid = comment["id"].clone();
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>(
        "todo_comment_edit",
        json!({ "id": cid, "body": "Bring the big stove and the gas" }),
    )
    .unwrap();
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>(
        "todo_comment_edit",
        json!({ "id": cid, "body": "Bring the stove and the spare gas" }),
    )
    .unwrap();
    c.sync(0, 1);
    for d in 0..2 {
        let comments: Value = c[d]
            .invoke("todo_comments", json!({ "taskId": t }))
            .unwrap();
        assert_eq!(
            comments[0]["body"], "Bring the big stove and the spare gas",
            "device {d}"
        );
        assert_eq!(comments[0]["edited"], true);
        assert!(waiting(&c[d]).is_empty());
    }
    // And a clash on the same word raises a Conflict about the Comment.
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>(
        "todo_comment_edit",
        json!({ "id": cid, "body": "Bring the big stove and the spare gas, please" }),
    )
    .unwrap();
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>(
        "todo_comment_edit",
        json!({ "id": cid, "body": "Bring the big stove and the spare gas, thanks" }),
    )
    .unwrap();
    c.sync(0, 1);
    for d in 0..2 {
        let rows = waiting(&c[d]);
        assert_eq!(rows.len(), 1, "device {d}");
        assert_eq!(rows[0]["subject"]["kind"], "comment");
        assert_eq!(rows[0]["subject"]["task_title"], "Trip");
    }
    assert_eq!(ids(&c[0]), ids(&c[1]));
}

// ---------------------------------------------------------------- three devices

/// Three devices edit one Entry offline, then sync in `order`; returns each device's text and
/// conflict ids once they have all caught up. (The ids depend on the Entry, which is new each
/// time, so only a run's devices are compared by id.)
fn three_devices(order: &[(usize, usize)]) -> Vec<(String, Vec<String>)> {
    let c = Cluster::new(3);
    let id = create_entry(&c[0], "one two three four five six\nsecond line stays\n")["id"].clone();
    c.sync_all();
    c[0].clock().advance(1_000);
    edit(
        &c[0],
        &id,
        "ONE two three four five six\nsecond line stays\n",
        "a",
    );
    c[1].clock().advance(2_000);
    edit(
        &c[1],
        &id,
        "one two three FOUR five six\nsecond line stays\n",
        "b",
    );
    c[2].clock().advance(3_000);
    edit(
        &c[2],
        &id,
        "One two three four five SIX\nsecond line stays\n",
        "s-c",
    );
    for &(a, b) in order {
        c.sync(a, b);
    }
    c.sync_all();
    (0..3).map(|d| (body(&c[d], &id), ids(&c[d]))).collect()
}

#[test]
fn three_devices_agree_on_the_text_and_the_conflict_whatever_order_they_sync_in() {
    let orders: [&[(usize, usize)]; 6] = [
        &[(0, 1), (1, 2), (0, 2)],
        &[(1, 2), (0, 1), (0, 2)],
        &[(0, 2), (1, 2), (0, 1)],
        &[(2, 1), (2, 0), (1, 0)],
        &[(0, 1)],
        &[(1, 2)],
    ];
    for (n, order) in orders.iter().enumerate() {
        let run = three_devices(order);
        // The ONE/One clash on the first word is the only one.
        assert_eq!(
            run[0].0, "One two three FOUR five SIX\nsecond line stays\n",
            "order {n}"
        );
        assert_eq!(run[0].1.len(), 1, "order {n}");
        for d in 1..3 {
            assert_eq!(run[d], run[0], "order {n} device {d}");
        }
    }
}

// ---------------------------------------------------------------- held while editing

#[test]
fn text_saved_while_editing_is_merged_with_what_was_held_when_the_editor_closes() {
    let c = Cluster::new(2);
    let id = create_entry(&c[0], "alpha beta gamma\ndelta epsilon\n")["id"].clone();
    c.sync(0, 1);
    c[1].invoke::<bool>(
        "editing_hold",
        json!({ "entity": ENTRY, "entityId": id, "scope": "*" }),
    )
    .unwrap();
    c[0].clock().advance(1_000);
    edit(&c[0], &id, "alpha beta GAMMA\ndelta epsilon\n", "a");
    c.sync(0, 1);
    assert_eq!(
        body(&c[1], &id),
        "alpha beta gamma\ndelta epsilon\n",
        "held"
    );
    // The person types somewhere else in the text and saves, then closes the editor.
    c[1].clock().advance(2_000);
    edit(&c[1], &id, "alpha beta gamma\ndelta EPSILON\n", "b");
    c[1].invoke::<()>(
        "editing_release",
        json!({ "entity": ENTRY, "entityId": id, "scope": "*" }),
    )
    .unwrap();
    assert_eq!(
        body(&c[1], &id),
        "alpha beta GAMMA\ndelta EPSILON\n",
        "both edits stand"
    );
    assert!(waiting(&c[1]).is_empty());
    c.sync(0, 1);
    assert_eq!(body(&c[0], &id), body(&c[1], &id));
}

#[test]
fn text_typed_over_the_same_words_while_editing_raises_a_conflict_after_release() {
    let c = Cluster::new(2);
    let id = create_entry(&c[0], BASE)["id"].clone();
    c.sync(0, 1);
    c[1].invoke::<bool>(
        "editing_hold",
        json!({ "entity": ENTRY, "entityId": id, "scope": "*" }),
    )
    .unwrap();
    c[0].clock().advance(1_000);
    edit(&c[0], &id, MINE, "a");
    c.sync(0, 1);
    assert!(
        waiting(&c[1]).is_empty(),
        "nothing is decided while it is held"
    );
    c[1].clock().advance(2_000);
    edit(&c[1], &id, YOURS, "b");
    c[1].invoke::<()>(
        "editing_release",
        json!({ "entity": ENTRY, "entityId": id, "scope": "*" }),
    )
    .unwrap();
    assert_eq!(body(&c[1], &id), YOURS);
    assert_eq!(waiting(&c[1]).len(), 1);
    c.sync(0, 1);
    assert_eq!(body(&c[0], &id), YOURS);
    assert_eq!(ids(&c[0]), ids(&c[1]));
}

/// An editing session that saves again after another session has built on its first save gets a
/// new clock, and the text that other session was made from is no longer in the log. The
/// author's own text must still be what every device replays the records to (found by
/// `MNEMORA_CONVERGE_SEED=137`).
#[test]
fn a_rewritten_editing_session_leaves_the_author_with_the_text_everyone_replays() {
    let c = Cluster::new(2);
    let base = "one\ntwo\nthree\n";
    let id = create_entry(&c[0], base)["id"].clone();
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    edit(&c[0], &id, "ONE\ntwo\nthree\n", "s");
    c[0].clock().advance(1_000);
    edit(&c[0], &id, "ONE\nTWO\nthree\n", "t");
    c[0].clock().advance(1_000);
    // Session `s` saves again: its record moves after `t`, and `t` was made from the first save.
    edit(&c[0], &id, "ONE\nTWO\nTHREE\n", "s");
    c.sync(0, 1);
    c.sync(0, 1);
    let (a, b) = (body(&c[0], &id), body(&c[1], &id));
    assert_eq!(a, b, "both devices hold the same text");
}

/// A session record rewritten in place on its author moves from one place to another before the
/// point the receiver's replay had got to; the replay must start over, not carry on from what it
/// worked out from the old version (found by seeds 88, 133 and 326).
#[test]
fn a_replay_is_redone_when_a_session_record_it_covered_was_rewritten_elsewhere() {
    let c = Cluster::new(2);
    let id = create_entry(&c[0], "one\ntwo\nthree\n")["id"].clone();
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    edit(&c[0], &id, "ONE\ntwo\nthree\n", "s");
    c.sync(0, 1);
    c[1].clock().advance(5_000);
    edit(&c[1], &id, "ONE\ntwo\nTHREE\n", "u");
    // B's own save is replayed (its text memo now covers both records).
    c[1].invoke::<()>(
        "editing_release",
        json!({ "entity": ENTRY, "entityId": id, "scope": "*" }),
    )
    .unwrap();
    // A saves its session again before it has heard of B's: the record moves, B's stays.
    c[0].clock().advance(1_000);
    edit(&c[0], &id, "ONE\nTWO\nthree\n", "s");
    c.sync(0, 1);
    c.sync(0, 1);
    let (a, b) = (body(&c[0], &id), body(&c[1], &id));
    assert_eq!(a, b, "both devices hold the same text");
    assert_eq!(a, "ONE\nTWO\nTHREE\n", "both edits are in it");
}

// ---------------------------------------------------------------- N1: one clash, one Conflict

fn hold(app: &TestApp, id: &Value) {
    app.invoke::<bool>(
        "editing_hold",
        json!({ "entity": ENTRY, "entityId": id, "scope": "*" }),
    )
    .unwrap();
}

fn release(app: &TestApp, id: &Value) {
    app.invoke::<()>(
        "editing_release",
        json!({ "entity": ENTRY, "entityId": id, "scope": "*" }),
    )
    .unwrap();
}

/// The device pass (qa/e2e-device FINAL.md, case 5): the phone types "-PH" in an open editor, the
/// Mac (editor closed) receives it live, the Mac opens its editor and types "-MAC" after it, then
/// the phone types "-X" in the same editor and leaves. Both appended at the same point: one
/// clash, one Conflict, the same on both devices. The device showed two, the second against the
/// stale text from before either typed.
#[test]
fn one_clash_over_a_session_that_saved_twice_raises_one_conflict() {
    for mac_holds in [false, true] {
        let c = Cluster::new(2);
        let (mac, phone) = (&c[0], &c[1]);
        let id = create_entry(mac, "ENTRY2 line-TYPED2")["id"].clone();
        c.sync(0, 1);
        hold(phone, &id);
        phone.clock().advance(1_000);
        edit(phone, &id, "ENTRY2 line-TYPED2-PH", "phone");
        c.sync(0, 1);
        assert_eq!(body(mac, &id), "ENTRY2 line-TYPED2-PH");
        if mac_holds {
            hold(mac, &id);
        }
        mac.clock().advance(1_000);
        edit(mac, &id, "ENTRY2 line-TYPED2-PH-MAC", "mac");
        c.sync(0, 1);
        assert_eq!(
            body(phone, &id),
            "ENTRY2 line-TYPED2-PH",
            "held on the phone"
        );
        phone.clock().advance(1_000);
        edit(phone, &id, "ENTRY2 line-TYPED2-PH-X", "phone");
        release(phone, &id);
        c.sync(0, 1);
        if mac_holds {
            release(mac, &id);
        }
        c.sync(0, 1);
        c.sync(0, 1);
        assert_eq!(body(mac, &id), body(phone, &id), "mac_holds={mac_holds}");
        assert_eq!(ids(mac), ids(phone), "mac_holds={mac_holds}");
        assert_eq!(
            ids(mac).len(),
            1,
            "one clash, one Conflict (mac_holds={mac_holds})"
        );
        // It is about the text both started from, not the one the Mac built on.
        let open = compare(mac, &ids(mac)[0]);
        assert_eq!(
            open["original"], "ENTRY2 line-TYPED2",
            "mac_holds={mac_holds}"
        );
    }
}

// ---------------------------------------------------------------- R5: deleted after the clash

fn conflict_rows(app: &TestApp) -> Vec<String> {
    app.with_db(|db| {
        let mut st = db
            .prepare(
                "SELECT status || ':' || COALESCE(resolution, '') FROM sync_conflicts ORDER BY id",
            )
            .unwrap();
        st.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    })
}

/// Story 70 / F14: a Conflict on a record that is then deleted, here or on another device, is
/// settled and logged, not left in Needs you with Compare still offering it.
#[test]
fn a_conflict_raised_before_the_entry_was_deleted_is_resolved_on_both_devices() {
    for deleter in [0, 1] {
        let (c, id) = clashing();
        assert_eq!(ids(&c[0]).len(), 1, "raised first (deleter {deleter})");
        let conflict = ids(&c[0])[0].clone();
        c[deleter].clock().advance(10_000);
        delete_entry(&c[deleter], &id);
        assert!(
            waiting(&c[deleter]).is_empty(),
            "the device that deleted is not asked any more (deleter {deleter})"
        );
        c.sync(0, 1);
        c.sync(0, 1);
        for d in 0..2 {
            assert!(waiting(&c[d]).is_empty(), "device {d} (deleter {deleter})");
            let shown: Option<Value> = c[d]
                .invoke("sync_conflict", json!({ "id": conflict }))
                .unwrap();
            assert!(
                shown.is_none_or(|s| s["open"] == false),
                "Compare no longer offers it on device {d} (deleter {deleter})"
            );
            assert_eq!(
                conflict_rows(&c[d]),
                ["resolved:deleted"],
                "device {d} (deleter {deleter})"
            );
        }
    }
}
