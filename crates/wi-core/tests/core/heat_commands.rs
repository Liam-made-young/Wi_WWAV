//! Heat's commands through the core (docs/HEAT.md; PLAN S2.2 to S2.4, S2.6,
//! S2.10). What a fail looks like:
//! - Plan my day uses anything but the written rule, a draft lacks its
//!   reason, Return doesn't accept all, or Esc doesn't clear them (S2.2);
//! - a round or break starts without a press, minutes don't reach the
//!   task's time, the timer is lost between launches, or the chime sounds
//!   while off (S2.3);
//! - a tab lacks its act, a seventh habit fits, a streak shows by default,
//!   Mail can be written to, or a grade's arithmetic differs (S2.4);
//! - moving in changes an id, or the first calendar sync finds the moved
//!   tasks new (S2.6);
//! - Capture loses a line, a note can't go public on its own, the weekly
//!   review shows a score, a streak or a comparison, or writes the note
//!   itself (S2.10).

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;
use wi_heat_store::{mcp, Clock};

fn classes(core: &Core) -> String {
    records(&snap(core, "2026-10-07"), "space")[0]["id"].as_str().unwrap().to_string()
}

fn undo_label(core: &Core) -> Value {
    ok(core, "history.get", json!({"room": "heat"}))["undo"].clone()
}

#[test]
fn opening_heat_makes_classes_wwav_and_personal_once() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let first = snap(&core, "2026-10-07");
    let names: Vec<Value> = records(&first, "space").iter().map(|s| s["name"].clone()).collect();
    assert_eq!(names, [json!("Classes"), json!("WWAV"), json!("Personal")]);
    assert_eq!(first["derived"]["status"], "Saved on this Mac");
    assert_eq!(first["zone"], "America/New_York");
    // ⌘Z never takes a person's own spaces away, and a second look makes none.
    assert_eq!(undo_label(&core), Value::Null);
    assert_eq!(records(&snap(&core, "2026-10-07"), "space").len(), 3);
    // A person who deleted them all doesn't get them back.
    for s in records(&first, "space") {
        ok(&core, "heat.delete", json!({"kind": "space", "id": s["id"]}));
    }
    assert!(records(&snap(&core, "2026-10-07"), "space").is_empty());
}

#[test]
fn s2_2_plan_my_day_is_the_written_rule_and_return_accepts_all() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let space = classes(&core);
    let t1 = add_task(&core, &space, "Grammar quiz 4", json!({"type": "Quiz", "due": ny("2026-10-07 17:00"), "difficulty": 2, "estMin": 45}));
    let t2 = add_task(&core, &space, "Essay draft", json!({"due": ny("2026-10-09 23:59"), "difficulty": 3, "estMin": 100}));
    let t3 = add_task(&core, &space, "Read chapter 4", json!({"due": ny("2026-10-13 23:59"), "difficulty": 1, "estMin": 20}));
    let before = undo_label(&core);

    let made = ok(&core, "heat.plan.make", json!({"date": "2026-10-07"}));
    let drafts = made["drafts"].as_array().unwrap();
    // Heat order; each estimate rounded up to 15 minutes and capped at 90; the
    // first gap that fits from now; and every draft says why.
    let shape: Vec<(String, f64, f64)> = drafts
        .iter()
        .map(|d| (d["taskId"].as_str().unwrap().to_string(), d["start"].as_f64().unwrap(), d["minutes"].as_f64().unwrap()))
        .collect();
    let id = |t: &Value| t["id"].as_str().unwrap().to_string();
    assert_eq!(shape, [(id(&t1), 540.0, 45.0), (id(&t2), 585.0, 90.0), (id(&t3), 675.0, 30.0)]);
    assert_eq!(drafts[0]["reason"], "Due today 5:00 PM, Hot.");
    assert_eq!(drafts[1]["reason"], "Due Friday 11:59 PM, Warm.");
    assert_eq!(drafts[2]["reason"], "Due Tuesday 11:59 PM, Cool.");
    assert_eq!(drafts[1]["leftLine"], "15m left to plan");
    assert!(drafts[0]["leftLine"].is_null());
    assert!(made["minutesLeft"].is_number() && made["unplanned"].is_array());
    // A draft is not a change: nothing to undo, and the snapshot shows them waiting.
    assert_eq!(undo_label(&core), before);
    assert_eq!(snap(&core, "2026-10-07")["heatState"]["planDrafts"].as_array().unwrap().len(), 3);

    // Esc clears them, and nothing was made.
    assert_eq!(ok(&core, "heat.plan.clear", json!({})), json!({}));
    let cleared = snap(&core, "2026-10-07");
    assert_eq!(cleared["heatState"]["planDrafts"], json!([]));
    assert!(records(&cleared, "timeBlock").is_empty());

    // A click accepts one; the rest wait.
    ok(&core, "heat.plan.make", json!({"date": "2026-10-07"}));
    let one = ok(&core, "heat.plan.accept", json!({"date": "2026-10-07", "taskIds": [id(&t2)]}));
    assert_eq!(one["undo"], "Undo accept draft");
    assert_eq!(one["blocks"].as_array().unwrap().len(), 1);
    assert_eq!(one["blocks"][0]["origin"], "plan");
    assert_eq!(snap(&core, "2026-10-07")["heatState"]["planDrafts"].as_array().unwrap().len(), 2);
    ok(&core, "history.undo", json!({"room": "heat"}));

    // Return accepts all of them, as one change.
    ok(&core, "heat.plan.make", json!({"date": "2026-10-07"}));
    let all = ok(&core, "heat.plan.accept", json!({"date": "2026-10-07"}));
    assert_eq!((all["undo"].as_str(), all["blocks"].as_array().unwrap().len()), (Some("Undo plan my day"), 3));
    let after = snap(&core, "2026-10-07");
    assert_eq!(after["heatState"]["planDrafts"], json!([]));
    assert_eq!(after["derived"]["today"]["header"], "3 blocks · 2h 45m planned · 1 due today");
    let planned: Vec<Value> = after["derived"]["today"]["planned"].as_array().unwrap().clone();
    let by_start: Vec<f64> = planned
        .iter()
        .map(|b| records(&after, "timeBlock").iter().find(|r| r["id"] == *b).unwrap()["start"].as_f64().unwrap())
        .collect();
    assert_eq!(by_start, [540.0, 585.0, 675.0], "planned, in time order");
    // ⌘Z takes all three back at once.
    assert_eq!(ok(&core, "history.undo", json!({"room": "heat"}))["label"], "plan my day");
    assert!(records(&snap(&core, "2026-10-07"), "timeBlock").is_empty());

    // Yesterday is over; the day ends where the setting says.
    assert_eq!(
        refused(&core, "heat.plan.make", json!({"date": "2026-10-06"})).1,
        "That day is over. Plan today or a day ahead."
    );
    let early = ok(&core, "heat.plan.make", json!({"date": "2026-10-07", "dayEnds": 10.0 * 60.0}));
    let ends: Vec<f64> = early["drafts"].as_array().unwrap().iter().map(|d| d["start"].as_f64().unwrap() + d["minutes"].as_f64().unwrap()).collect();
    assert!(ends.iter().all(|e| *e <= 600.0), "nothing runs past 'Day ends at': {ends:?}");
}

#[test]
fn a_block_snaps_to_fifteen_minutes_and_is_moved_and_removed_with_its_words() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let task = add_task(&core, &classes(&core), "Mix the second verse", json!({}));
    let made = ok(&core, "heat.block.put", json!({"taskId": task["id"], "date": "2026-10-07", "start": 612, "minutes": 40}));
    assert_eq!(made["undo"], "Undo add block");
    let block = made["block"].clone();
    assert_eq!((block["start"].as_f64(), block["minutes"].as_f64(), block["origin"].as_str()), (Some(615.0), Some(45.0), Some("you")));
    let moved = ok(&core, "heat.block.put", json!({"id": block["id"], "taskId": task["id"], "date": "2026-10-07", "start": 720, "minutes": 45}));
    assert_eq!(moved["undo"], "Undo move block");
    let resized = ok(&core, "heat.block.put", json!({"id": block["id"], "taskId": task["id"], "date": "2026-10-07", "start": 720, "minutes": 90}));
    assert_eq!(resized["undo"], "Undo resize block");
    // Resizing changes the block, never the task's estimate.
    assert!(records(&snap(&core, "2026-10-07"), "task")[0]["estMin"].is_null());
    let gone = ok(&core, "heat.delete", json!({"kind": "timeBlock", "id": block["id"]}));
    assert_eq!(gone["undo"], "Undo remove block");
    assert_eq!(
        refused(&core, "heat.block.put", json!({"date": "2026-10-07", "start": 600, "minutes": 30})).1,
        "A block is for one task or one habit."
    );
}

#[test]
fn s2_3_focus_starts_only_on_a_press_and_its_minutes_reach_the_task() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let task = add_task(&core, &classes(&core), "Mix the second verse", json!({}));
    let id = task["id"].as_str().unwrap();

    // Making a task current starts nothing, and with no task, no press starts anything.
    assert_eq!(
        refused(&core, "heat.focus.start", json!({})).1,
        "Nothing is current. Pick a task and press C, or drag one here."
    );
    assert_eq!(ok(&core, "heat.current.set", json!({"taskId": id})), json!({}));
    let idle = snap(&core, "2026-10-07");
    assert_eq!(idle["heatState"]["currentTaskId"], id);
    assert_eq!((idle["heatState"]["timer"]["phase"].as_str(), idle["heatState"]["timer"]["endsAt"].clone()), (Some("idle"), Value::Null));
    assert_eq!(idle["derived"]["timer"]["digits"], "25:00");

    // A press starts it: 25 minutes, ending at a time.
    let started = ok(&core, "heat.focus.start", json!({}));
    let timer = &started["heatState"]["timer"];
    assert_eq!((timer["phase"].as_str(), timer["round"].as_f64(), timer["endsAt"].as_f64()), (Some("focus"), Some(1.0), Some(ny("2026-10-07 09:25"))));
    // Starting what runs doesn't pause it.
    assert_eq!(ok(&core, "heat.focus.start", json!({}))["heatState"]["timer"]["running"], true);

    // It lives in the core, so it survives the window and the app closing.
    drop(core);
    let core = heat_core(&setup, "2026-10-07 09:10");
    let back = snap(&core, "2026-10-07");
    assert_eq!(back["heatState"]["timer"]["endsAt"].as_f64(), Some(ny("2026-10-07 09:25")));
    assert_eq!(back["derived"]["timer"]["digits"], "15:00");
    assert_eq!(back["derived"]["timer"]["strip"], "focus 15:00 left");

    // I marks "Pulled away": paused, with the interruption counted.
    let paused = ok(&core, "heat.focus.interrupt", json!({}));
    assert_eq!(paused["heatState"]["timer"]["running"], false);
    pin(&core, "2026-10-07 09:20");
    assert_eq!(snap(&core, "2026-10-07")["derived"]["timer"]["digits"], "15:00", "a paused timer holds its time");
    ok(&core, "heat.focus.resume", json!({}));

    // Time's up: the round logs its minutes and then waits. It starts nothing itself.
    pin(&core, "2026-10-07 09:45");
    let done = ok(&core, "heat.focus.finish", json!({}));
    assert_eq!(done["logged"]["focusMin"], 25, "a pause stops the clock: the round holds 25 minutes of focus");
    assert_eq!(done["logged"]["taskId"], id);
    assert_eq!(done["undo"], "Undo focus session");
    assert!(done.get("chime").is_none() && done["logged"]["interruptions"] == 1);
    let t = &done["heatState"]["timer"];
    assert_eq!((t["phase"].as_str(), t["running"].clone(), t["endsAt"].clone()), (Some("break"), json!(false), Value::Null));
    pin(&core, "2026-10-07 11:00");
    let waiting = snap(&core, "2026-10-07");
    assert_eq!(waiting["derived"]["timer"]["line"], "Break 5:00. Press F to start it.");
    assert_eq!(waiting["heatState"]["timer"]["running"], false, "hours later, still waiting for a press");

    // The minutes are the task's: actualMin, and the status line when it is done.
    assert_eq!(waiting["derived"]["tasks"][id]["actualMin"], 25);
    let checked = ok(&core, "heat.done", json!({"taskId": id, "done": true}));
    assert_eq!(checked["took"], "Done. Took 25m across 1 focus session.");
    assert_eq!(checked["undo"], "Undo mark done");

    // The break, when pressed, runs; when over it waits for the next focus.
    ok(&core, "heat.focus.start", json!({}));
    pin(&core, "2026-10-07 11:06");
    let after_break = ok(&core, "heat.focus.finish", json!({}));
    assert_eq!(after_break["heatState"]["timer"]["phase"], "idle");
    assert_eq!(after_break["heatState"]["timer"]["round"], 2);
    assert_eq!(after_break["heatState"]["timer"]["note"], "Break done. Press F to start focus 2 of 4.");
    assert!(after_break["logged"].is_null());
}

#[test]
fn stopping_logs_the_minutes_so_far_and_less_than_a_minute_logs_nothing() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let task = add_task(&core, &classes(&core), "Mix the second verse", json!({}));
    let id = task["id"].as_str().unwrap();
    ok(&core, "heat.focus.start", json!({"taskId": id, "length": 50}));
    assert_eq!(snap(&core, "2026-10-07")["heatState"]["currentTaskId"], id, "focus on a selection makes it current");
    pin(&core, "2026-10-07 09:30");
    let stopped = ok(&core, "heat.focus.stop", json!({}));
    assert_eq!(stopped["logged"]["focusMin"], 30);
    assert_eq!(stopped["heatState"]["timer"]["note"], "Focus stopped. 30m logged to Mix the second verse.");
    assert_eq!(stopped["heatState"]["timer"]["phase"], "idle");
    ok(&core, "heat.focus.start", json!({}));
    pin(&core, "2026-10-07 09:30");
    let nothing = ok(&core, "heat.focus.stop", json!({}));
    assert!(nothing["logged"].is_null() && nothing["undo"].is_null());
    assert_eq!(
        refused(&core, "heat.focus.start", json!({"length": 7})).1,
        "A focus is 25 or 50 minutes, or any whole number from 10 to 90."
    );
    // Total: Get Info's "Took" sets what the time adds up to by hand.
    let took = ok(&core, "heat.tookTime", json!({"taskId": id, "minutes": 75}));
    assert_eq!(took["undo"], "Undo change time taken");
    assert_eq!(snap(&core, "2026-10-07")["derived"]["tasks"][id]["actualMin"], 75);
}

#[test]
fn s2_4_tasks_are_edited_checked_off_and_deleted_with_what_belongs_to_them() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let space = classes(&core);
    let made = ok(&core, "heat.put", json!({"kind": "task", "record": {"spaceId": space, "title": "  Grammar quiz 4  ", "due": ny("2026-10-08 23:59")}}));
    assert_eq!(made["undo"], "Undo add task");
    let task = made["record"].clone();
    let id = task["id"].as_str().unwrap();
    assert!(wwav_ids_is_ulid(id));
    // What a task holds when a view leaves things out, and that it is private.
    assert_eq!(
        (task["title"].as_str(), task["difficulty"].as_f64(), task["done"].as_bool(), task["source"].as_str(), task["public"].as_bool()),
        (Some("Grammar quiz 4"), Some(3.0), Some(false), Some("you"), Some(false))
    );
    assert_eq!(refused(&core, "heat.put", json!({"kind": "task", "record": {"spaceId": space, "title": "  "}})).1, "Give the task a name first.");
    assert_eq!(
        refused(&core, "heat.put", json!({"kind": "task", "record": {"spaceId": space, "title": "x", "colour": "red"}})).1,
        "A task has no field called 'colour'."
    );
    assert_eq!(
        refused(&core, "heat.put", json!({"kind": "task", "record": {"spaceId": space, "title": "x", "public": true}})).1,
        "The Public switch has its own command."
    );
    assert_eq!(refused(&core, "heat.patch", json!({"kind": "task", "id": id, "set": {"public": true}})).1, "The Public switch has its own command.");

    // The words for each kind of edit.
    assert_eq!(ok(&core, "heat.patch", json!({"kind": "task", "id": id, "set": {"title": "Grammar quiz 5"}}))["undo"], "Undo rename task");
    assert_eq!(ok(&core, "heat.patch", json!({"kind": "task", "id": id, "set": {"notes": "lesson 9"}}))["undo"], "Undo edit task");
    let est = ok(&core, "heat.estimate", json!({"taskId": id, "difficulty": 2, "estMin": 9000}));
    assert_eq!((est["undo"].as_str(), est["clamped"].clone(), est["task"]["estMin"].clone()), (Some("Undo estimate"), json!(true), json!(600)));
    assert_eq!((est["task"]["estBy"].as_str(), est["task"]["difficulty"].as_f64()), (Some("you"), Some(2.0)));
    let low = ok(&core, "heat.estimate", json!({"taskId": id, "estMin": 1}));
    assert_eq!(low["task"]["estMin"], 5, "minutes are clamped to 5 to 600");
    let derived = &snap(&core, "2026-10-07")["derived"]["tasks"][id];
    assert_eq!(derived["estimate"], json!({"min": 5, "by": "you", "reason": null}));
    assert_eq!(derived["heat"]["level"], "Warm");
    assert_eq!(refused(&core, "heat.estimate", json!({"taskId": id})).1, "Give a difficulty, an estimate in minutes, or both.");

    // A block and a Now making line belong to the task: they go with it, in one entry.
    ok(&core, "heat.block.put", json!({"taskId": id, "date": "2026-10-07", "start": 600, "minutes": 30}));
    ok(&core, "heat.share.now", json!({"taskId": id, "text": "Now making: the quiz"}));
    assert_eq!(records(&snap(&core, "2026-10-07"), "profileShare").len(), 1);
    let gone = ok(&core, "heat.delete", json!({"kind": "task", "id": id}));
    assert_eq!(gone["undo"], "Undo delete task");
    let after = snap(&core, "2026-10-07");
    assert!(records(&after, "task").is_empty() && records(&after, "timeBlock").is_empty() && records(&after, "profileShare").is_empty());
    ok(&core, "history.undo", json!({"room": "heat"}));
    let back = snap(&core, "2026-10-07");
    assert_eq!((records(&back, "task").len(), records(&back, "timeBlock").len(), records(&back, "profileShare").len()), (1, 1, 1));

    // Done, and not done; a task done with nothing logged doesn't claim a time.
    let done = ok(&core, "heat.done", json!({"taskId": id, "done": true}));
    assert!(done.get("took").is_none());
    assert_eq!((done["task"]["done"].clone(), done["undo"].clone()), (json!(true), json!("Undo mark done")));
    assert!(records(&snap(&core, "2026-10-07"), "profileShare").is_empty(), "a Now making line clears when its task is done");
    assert_eq!(ok(&core, "heat.done", json!({"taskId": id, "done": false}))["undo"], "Undo mark not done");
    assert_eq!(ok(&core, "heat.done", json!({"taskId": id, "done": false}))["undo"], Value::Null, "nothing to undo when nothing changed");
}

fn wwav_ids_is_ulid(s: &str) -> bool {
    s.len() == 26 && s.chars().all(|c| c.is_ascii_alphanumeric())
}

#[test]
fn a_recurring_task_is_ticked_a_day_at_a_time_and_never_flips_to_done() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let task = add_task(&core, &classes(&core), "Practise scales", json!({"due": ny("2026-10-07 20:00"), "rrule": "FREQ=DAILY"}));
    let id = task["id"].as_str().unwrap();
    let first = snap(&core, "2026-10-07");
    assert_eq!(first["derived"]["tasks"][id]["next"], "2026-10-07");
    assert_eq!(first["derived"]["today"]["recurringToday"], json!([{"kind": "task", "id": id}]));
    let ticked = ok(&core, "heat.done", json!({"taskId": id, "done": true}));
    assert_eq!(ticked["undo"], "Undo mark done");
    let after = snap(&core, "2026-10-07");
    assert_eq!(records(&after, "taskOccurrence").len(), 1);
    assert_eq!(after["derived"]["tasks"][id]["next"], "2026-10-08");
    assert_eq!(records(&after, "task")[0]["done"], false, "the series never flips to done");
    let day = ok(&core, "heat.done", json!({"taskId": id, "done": true, "date": "2026-10-09"}));
    assert_eq!(day["undo"], "Undo mark done");
    assert_eq!(records(&snap(&core, "2026-10-07"), "taskOccurrence").len(), 2);
    let un = ok(&core, "heat.done", json!({"taskId": id, "done": false, "date": "2026-10-09"}));
    assert_eq!(un["undo"], "Undo mark not done");
    assert_eq!(records(&snap(&core, "2026-10-07"), "taskOccurrence").len(), 1);
}

#[test]
fn s2_4_habits_hold_six_and_show_a_record_not_a_streak() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let mut ids = Vec::new();
    for n in 0..6 {
        let made = ok(&core, "heat.put", json!({"kind": "habit", "record": {"title": format!("Habit {n}"), "log": {}, "showCounter": false}}));
        assert_eq!(made["undo"], "Undo add habit");
        assert_eq!(made["record"]["public"], false);
        ids.push(made["record"]["id"].as_str().unwrap().to_string());
    }
    assert_eq!(
        refused(&core, "heat.put", json!({"kind": "habit", "record": {"title": "Habit 7", "log": {}, "showCounter": false}})).1,
        "Habit limit reached"
    );
    // A tick is a change with its own words, and a growing record is what shows.
    let habit = ids[0].clone();
    for day in ["2026-10-05", "2026-10-06", "2026-10-07"] {
        let record = records(&snap(&core, "2026-10-07"), "habit").into_iter().find(|h| h["id"] == habit.as_str()).unwrap();
        let mut log = record["log"].clone();
        log[day] = json!(true);
        let ticked = ok(&core, "heat.patch", json!({"kind": "habit", "id": habit, "set": {"log": log}}));
        assert_eq!(ticked["undo"], "Undo tick habit");
    }
    let shown = snap(&core, "2026-10-07");
    assert_eq!(shown["derived"]["habits"][&habit], json!({"today": true, "record": "Done 3 days since October 5"}));
    assert_eq!(shown["derived"]["habits"][&ids[1]]["record"], "Not done yet");
    let counter = ok(&core, "heat.patch", json!({"kind": "habit", "id": habit, "set": {"showCounter": true}}));
    assert_eq!(counter["undo"], "Undo edit habit");
    assert_eq!(snap(&core, "2026-10-07")["derived"]["habits"][&habit]["record"], "3-day streak", "the counter shows only once it is switched on");
    let log = json!({"2026-10-05": true, "2026-10-06": true});
    assert_eq!(ok(&core, "heat.patch", json!({"kind": "habit", "id": habit, "set": {"log": log}}))["undo"], "Undo untick habit");
    // Taking one away makes room for another.
    ok(&core, "heat.delete", json!({"kind": "habit", "id": ids[5]}));
    ok(&core, "heat.put", json!({"kind": "habit", "record": {"title": "Habit 7", "log": {}, "showCounter": false}}));
}

#[test]
fn s2_4_grades_are_worked_out_by_the_core_and_only_the_person_types_a_score() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let term = ok(&core, "heat.put", json!({"kind": "term", "record": {"name": "Fall 2026"}}))["record"]["id"].as_str().unwrap().to_string();
    let course = ok(
        &core,
        "heat.put",
        json!({"kind": "course", "record": {"termId": term, "code": "JPN 201", "name": "Japanese 2", "categories": [
            {"name": "Exams", "weight": 50, "keywords": ["exam"]}, {"name": "Final", "weight": 45, "keywords": ["final"]}]}}),
    );
    let course_id = course["record"]["id"].as_str().unwrap().to_string();
    assert!(course["record"]["categories"][0]["id"].as_str().is_some(), "each category gets its id");
    let exams = course["record"]["categories"][0]["id"].clone();
    let grade = ok(&core, "heat.put", json!({"kind": "grade", "record": {"courseId": course_id, "categoryId": exams, "title": "Exam 1", "score": 40, "outOf": 50}}));
    assert_eq!(grade["undo"], "Undo add grade");
    let shown = snap(&core, "2026-10-07");
    let c = &shown["derived"]["courses"][&course_id];
    assert_eq!((c["currentPct"].as_f64(), c["letter"].as_str()), (Some(80.0), Some("B-")));
    assert!((c["decidedPct"].as_f64().unwrap() - 52.631578947).abs() < 1e-6, "{c}");
    assert_eq!(c["weights"], "Weights add to 95%. The other 5% is unassigned.");
    assert_eq!(c["basedOn"], "Based on 52.6% of the course so far");
    let take = ok(&core, "heat.whatItWouldTake", json!({"courseId": course_id, "letter": "B"}));
    assert_eq!(take["text"], "To finish with a B (83%), you need 86.4% on the remaining 47.4%.", "{take}");
    let reach = ok(&core, "heat.whatItWouldTake", json!({"courseId": course_id, "letter": "A"}));
    assert_eq!(reach["text"], "An A is out of reach; the highest possible is 89.4% (B+).", "{reach}");
    assert_eq!(refused(&core, "heat.whatItWouldTake", json!({"courseId": course_id, "letter": "Q"})).1, "This course has no grade called Q.");

    // Claude records a pending grade, with no score; the person types one.
    let mut store = wi_store::Store::open(core.library()).unwrap();
    let clock = Clock::at(ny("2026-10-07 09:00"), jiff::tz::TimeZone::get("America/New_York").unwrap());
    let args = json!({"course": "JPN 201", "item": "Final exam", "reason": "A grade notice."});
    let pending = mcp::call(&mut store, &clock, "add_pending_grade", args.as_object().unwrap()).unwrap();
    drop(store);
    assert_eq!((pending["grade"]["score"].clone(), pending["grade"]["pending"].clone(), pending["grade"]["outOf"].clone()), (Value::Null, json!(true), json!(100)));
    let gid = pending["grade"]["id"].as_str().unwrap().to_string();
    let typed = ok(&core, "heat.score", json!({"gradeId": gid, "score": 88, "outOf": 100}));
    assert_eq!((typed["undo"].as_str(), typed["grade"]["pending"].clone(), typed["grade"]["score"].clone()), (Some("Undo enter score"), json!(false), json!(88)));
    assert_eq!(refused(&core, "heat.score", json!({"gradeId": gid, "score": -1})).1, "A score is a number, 0 or more.");
}

#[test]
fn s2_4_mail_lists_only_what_claude_recorded_and_the_person_can_not_write_to_it() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let space = classes(&core);
    let task = add_task(&core, &space, "Grammar quiz 4", json!({}));
    let mut store = wi_store::Store::open(core.library()).unwrap();
    let clock = Clock::at(ny("2026-10-07 09:00"), jiff::tz::TimeZone::get("America/New_York").unwrap());
    let args = json!({"thread_id": "th-1", "subject": "Quiz 4 is Friday", "from": "D2L", "received_at": "2026-10-07T08:41:00-04:00",
        "state": "task", "task_id": task["id"], "reason": "It names a Friday deadline."});
    mcp::call(&mut store, &clock, "record_mail_thread", args.as_object().unwrap()).unwrap();
    drop(store);
    let shown = snap(&core, "2026-10-07");
    let mail = records(&shown, "mailThread");
    assert_eq!(mail.len(), 1);
    assert_eq!((mail[0]["recordedBy"].as_str(), mail[0]["state"].as_str()), (Some("claude"), Some("task")));
    let id = mail[0]["id"].as_str().unwrap();
    let sentence = "Mail lists only what Claude recorded. Ask Claude to read it.";
    assert_eq!(refused(&core, "heat.put", json!({"kind": "mailThread", "record": {"subject": "x"}})).1, sentence);
    assert_eq!(refused(&core, "heat.patch", json!({"kind": "mailThread", "id": id, "set": {"state": "nothing"}})).1, sentence);
    assert_eq!(refused(&core, "heat.delete", json!({"kind": "mailThread", "id": id})).1, sentence);
    assert_eq!(refused(&core, "heat.public.set", json!({"kind": "mailThread", "id": id, "public": true})).1, "Mail is never public.");
    assert_eq!(records(&snap(&core, "2026-10-07"), "mailThread").len(), 1);
}

#[test]
fn the_snapshot_orders_the_lists_and_bounds_blocks_and_events() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let space = classes(&core);
    let cool = add_task(&core, &space, "Later", json!({"due": ny("2026-10-30 12:00")}));
    let hot = add_task(&core, &space, "Soon", json!({"due": ny("2026-10-07 18:00"), "difficulty": 3}));
    let undated = add_task(&core, &space, "Someday-ish", json!({}));
    let done = add_task(&core, &space, "Finished", json!({}));
    ok(&core, "heat.done", json!({"taskId": done["id"], "done": true}));
    ok(&core, "heat.capture.add", json!({"text": "fix the snare at 1:32"}));
    ok(&core, "heat.block.put", json!({"taskId": hot["id"], "date": "2026-10-07", "start": 600, "minutes": 30}));
    ok(&core, "heat.block.put", json!({"taskId": hot["id"], "date": "2027-03-01", "start": 600, "minutes": 30}));
    let s = snap(&core, "2026-10-07");
    let ids = |v: &Value| -> Vec<String> { v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect() };
    let name = |id: &str| records(&s, "task").into_iter().find(|t| t["id"] == id).unwrap()["title"].as_str().unwrap().to_string();
    let open: Vec<String> = ids(&s["derived"]["lists"]["allOpen"]).iter().map(|i| name(i)).collect();
    assert_eq!(open, ["Soon", "Later", "Someday-ish"], "heat order, undated last");
    assert_eq!(ids(&s["derived"]["lists"]["done"]).iter().map(|i| name(i)).collect::<Vec<_>>(), ["Finished"]);
    assert_eq!(ids(&s["derived"]["lists"]["inbox"]).len(), 1);
    assert_eq!(ids(&s["derived"]["lists"]["hot"]).iter().map(|i| name(i)).collect::<Vec<_>>(), ["Soon"]);
    assert_eq!(ids(&s["derived"]["hotTasks"]), ids(&s["derived"]["lists"]["hot"]));
    assert_eq!(records(&s, "timeBlock").len(), 1, "a block in March isn't in October's window");
    let wide = ok(&core, "heat.snapshot", json!({"date": "2026-10-07", "from": "2026-10-01", "to": "2027-03-31"}));
    assert_eq!(records(&wide, "timeBlock").len(), 2);
    assert_eq!(s["derived"]["tasks"][cool["id"].as_str().unwrap()]["heat"]["level"], "Cool");
    assert_eq!(s["derived"]["tasks"][undated["id"].as_str().unwrap()]["heat"], json!({"v": 0.05, "level": "Cool"}));
    assert_eq!(refused(&core, "heat.snapshot", json!({"date": "yesterday"})).1, "A day is written YYYY-MM-DD.");
}

#[test]
fn heat_commands_say_so_when_they_are_wrong() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    assert_eq!(core.invoke("heat.nothing", json!({})).unwrap_err().code, "unknown_command");
    assert_eq!(refused(&core, "heat.put", json!({"kind": "gizmo", "record": {}})).1, "Heat keeps no gizmo.");
    assert_eq!(refused(&core, "heat.put", json!({"kind": "task"})).0, "refused");
    assert_eq!(refused(&core, "heat.patch", json!({"kind": "task", "id": "nope", "set": {"title": "x"}})).1, "No task has that id.");
    assert_eq!(refused(&core, "heat.delete", json!({"kind": "task", "id": "nope"})).1, "No task has that id.");
    assert_eq!(refused(&core, "heat.done", json!({"taskId": "nope", "done": true})).1, "No task has that id.");
    assert_eq!(refused(&core, "heat.done", json!({"taskId": "nope"})).0, "bad_args");
    assert_eq!(refused(&core, "heat.public.set", json!({"kind": "space", "id": "x", "public": true})).1, "A space has no Public switch.");
    // A space that still holds tasks can't go; nor can a calendar be put by hand.
    let space = classes(&core);
    add_task(&core, &space, "x", json!({}));
    assert_eq!(
        refused(&core, "heat.delete", json!({"kind": "space", "id": space})).1,
        "This space still holds tasks, projects or milestones. Move or delete them first."
    );
    assert_eq!(refused(&core, "heat.put", json!({"kind": "calendar", "record": {"name": "x"}})).1, "Calendars are added and removed in Settings → Heat.");
}

#[test]
fn s2_6_moving_in_keeps_every_id_and_everything_comes_in_private() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let export = artifact_export();
    let first = ok(&core, "heat.import", json!({"json": export}));
    assert_eq!(first["undo"], "Undo move in");
    assert_eq!(first["counts"]["task"], 3);
    let shown = snap(&core, "2026-10-07");
    let mut ids: Vec<String> = records(&shown, "task").iter().map(|t| t["id"].as_str().unwrap().to_string()).collect();
    ids.sort();
    assert_eq!(ids, ["em-7c1f9a", "evt-google-1", "t-own-1"], "every id is kept");
    for kind in ["task", "milestone", "habit", "grade", "course"] {
        assert!(records(&shown, kind).iter().all(|r| r["public"] == false), "{kind} comes in private");
    }
    let mail_task = records(&shown, "task").into_iter().find(|t| t["id"] == "em-7c1f9a").unwrap();
    assert_eq!((mail_task["source"].as_str(), mail_task["sourceId"].as_str()), (Some("mail"), Some("em-7c1f9a")));
    assert_eq!(records(&shown, "space").len(), 3, "the workspaces moved into the spaces Heat made");
    assert_eq!(records(&shown, "habit")[0]["log"], json!({"2026-10-05": true}));
    assert_eq!(records(&shown, "term")[0]["name"], "Fall 2026");
    // A second import, from a fresher download, defines nothing twice and clobbers nothing.
    ok(&core, "heat.patch", json!({"kind": "task", "id": "t-own-1", "set": {"notes": "edited in the app"}}));
    let again = ok(&core, "heat.import", json!({"json": export}));
    assert_eq!(again["counts"]["task"], 0);
    let after = snap(&core, "2026-10-07");
    assert_eq!(records(&after, "task").len(), 3);
    assert_eq!(records(&after, "task").into_iter().find(|t| t["id"] == "t-own-1").unwrap()["notes"], "edited in the app");
    assert_eq!(records(&after, "space").len(), 3);
    // Undo takes all of it back.
    ok(&core, "history.undo", json!({"room": "heat"})); // the notes edit
    assert_eq!(refused(&core, "heat.import", json!({"json": {"format": "something else"}})).1, "This file isn’t a Heat export.");
}

/// An export as the artifact's Download JSON writes it: a mail task, a task
/// that came through Google Calendar, one of the person's own, a milestone, a
/// habit, a course with a grade.
pub fn artifact_export() -> Value {
    json!({
        "format": "heat-export", "version": 1,
        "exportedAt": "2026-10-06T12:40:00.000Z", "timeZone": "America/New_York",
        "workspaces": [
            {"key": "classes", "name": "Classes", "groupLabel": "Course", "types": ["Homework", "Quiz", "Other"], "persona": "a student"},
            {"key": "wwav", "name": "WWAV", "groupLabel": "Milestone", "types": ["Software", "Other"], "persona": "a founder"},
            {"key": "personal", "name": "Personal", "groupLabel": "Area", "types": ["Errand", "Other"], "persona": "a person"}
        ],
        "tasks": [
            {"id": "em-7c1f9a", "workspace": "classes", "title": "Reading response 3", "group": "WRT 104", "type": "Homework",
             "due": "2026-10-10T03:59:00.000Z", "difficulty": 2, "estMin": null, "actualMin": null, "notes": "", "done": false, "doneAt": null, "source": "gmail"},
            {"id": "evt-google-1", "workspace": "classes", "title": "Grammar quiz 4", "group": "JPN 201", "type": "Quiz",
             "due": "2026-10-08T03:59:00.000Z", "difficulty": 2, "estMin": 45, "actualMin": null, "notes": "", "done": false, "doneAt": null, "source": "calendar"},
            {"id": "t-own-1", "workspace": "personal", "title": "Renew passport", "group": "Admin", "type": "Errand",
             "due": null, "difficulty": 1, "estMin": null, "actualMin": 30, "notes": "", "done": true, "doneAt": "2026-10-01T15:00:00.000Z", "source": "manual"}
        ],
        "milestones": [{"id": "m-1", "workspace": "wwav", "title": "EP v1 mixed", "date": "2026-11-02", "done": false, "order": 0}],
        "habits": [{"id": "h-1", "title": "Scales", "log": {"2026-10-05": true}}],
        "term": "Fall 2026",
        "courses": [{"code": "JPN 201", "name": "Japanese 2", "categories": [{"name": "Quizzes", "weight": 40, "keywords": ["quiz"]}]}],
        "grades": [{"id": "g-1", "course": "JPN 201", "title": "Quiz 3", "category": "Quizzes", "score": 18, "outOf": 20, "dropped": false, "pending": false}],
        "processedMailIds": ["18f2a", "18f2b"], "lastSyncAt": "2026-10-06T12:30:00.000Z"
    })
}

#[test]
fn s2_10_capture_keeps_every_line_and_triage_empties_the_inbox() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let lines: Vec<String> = (0..60).map(|n| format!("line {n}: ünïcode ✓\nsecond line {n}")).collect();
    let mut last = Value::Null;
    for l in &lines {
        last = ok(&core, "heat.capture.add", json!({"text": l}));
        assert_eq!(last["undo"], "Undo capture");
    }
    assert_eq!(last["inbox"], "60 in inbox · captured ✓");
    let shown = snap(&core, "2026-10-07");
    let mut kept: Vec<String> = records(&shown, "capture").iter().map(|c| c["text"].as_str().unwrap().to_string()).collect();
    kept.sort();
    let mut want = lines.clone();
    want.sort();
    assert_eq!(kept, want, "no line was lost");
    assert_eq!(shown["derived"]["lists"]["inbox"].as_array().unwrap().len(), 60);
    assert_eq!(refused(&core, "heat.capture.add", json!({"text": "   "})).1, "Write something to capture first.");

    // Triage: a task, a note, a project, an upload.
    let caps = records(&shown, "capture");
    let to_task = ok(&core, "heat.capture.triage", json!({"id": caps[0]["id"], "to": "task"}));
    assert_eq!(to_task["undo"], "Undo triage capture");
    assert_eq!(to_task["result"]["record"]["title"], caps[0]["text"].as_str().unwrap().lines().next().unwrap());
    assert_eq!(to_task["result"]["record"]["notes"], caps[0]["text"].as_str().unwrap().lines().nth(1).unwrap());
    let to_note = ok(&core, "heat.capture.triage", json!({"id": caps[1]["id"], "to": "note"}));
    assert_eq!(to_note["result"]["record"]["markdown"], caps[1]["text"]);
    let to_project = ok(&core, "heat.capture.triage", json!({"id": caps[2]["id"], "to": "project", "record": {"spaceId": classes(&core)}}));
    assert_eq!(to_project["result"]["record"]["status"], "active");
    ok(&core, "heat.capture.triage", json!({"id": caps[3]["id"], "to": "upload"}));
    assert_eq!(refused(&core, "heat.capture.triage", json!({"id": caps[0]["id"], "to": "task"})).1, "That capture was triaged already.");
    let after = snap(&core, "2026-10-07");
    assert_eq!(after["derived"]["lists"]["inbox"].as_array().unwrap().len(), 56);
    let triaged = records(&after, "capture").into_iter().find(|c| c["id"] == caps[0]["id"]).unwrap();
    assert_eq!((triaged["resultType"].as_str(), triaged["resultId"].clone()), (Some("task"), to_task["result"]["id"].clone()));
    // Undo puts the capture back in the inbox and removes what it made.
    ok(&core, "history.undo", json!({"room": "heat"}));
    ok(&core, "history.undo", json!({"room": "heat"}));
    ok(&core, "history.undo", json!({"room": "heat"}));
    ok(&core, "history.undo", json!({"room": "heat"}));
    let undone = snap(&core, "2026-10-07");
    assert_eq!(undone["derived"]["lists"]["inbox"].as_array().unwrap().len(), 60);
    assert!(records(&undone, "task").is_empty() && records(&undone, "note").is_empty() && records(&undone, "project").is_empty());
}

#[test]
fn s2_10_captures_from_many_places_at_once_lose_none() {
    let setup = Setup::new();
    let core = std::sync::Arc::new(heat_core(&setup, "2026-10-07 09:00"));
    let threads: Vec<_> = (0..4)
        .map(|t| {
            let core = core.clone();
            std::thread::spawn(move || {
                for n in 0..25 {
                    ok(&core, "heat.capture.add", json!({"text": format!("thread {t} line {n}")}));
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(records(&snap(&core, "2026-10-07"), "capture").len(), 100);
}

#[test]
fn s2_10_a_note_goes_public_on_its_own_and_the_review_writes_nothing_for_you() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-14 09:00");
    let space = classes(&core);
    // Last week: two tasks done with time logged, a milestone reached.
    let quiz = add_task(&core, &space, "Grammar quiz 4", json!({"type": "Quiz", "estMin": 30}));
    let hw = add_task(&core, &space, "Homework 5", json!({"type": "Homework"}));
    pin(&core, "2026-10-08 10:00");
    ok(&core, "heat.focus.start", json!({"taskId": quiz["id"]}));
    pin(&core, "2026-10-08 10:25");
    ok(&core, "heat.focus.finish", json!({}));
    ok(&core, "heat.done", json!({"taskId": quiz["id"], "done": true}));
    ok(&core, "heat.tookTime", json!({"taskId": hw["id"], "minutes": 40}));
    ok(&core, "heat.done", json!({"taskId": hw["id"], "done": true}));
    ok(&core, "heat.put", json!({"kind": "milestone", "record": {"spaceId": space, "title": "EP v1 mixed", "date": "2026-10-09", "done": true}}));
    pin(&core, "2026-10-14 09:00");

    let week = ok(&core, "heat.review.week", json!({"weekStart": "2026-10-07"}));
    let lines: Vec<String> = week["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_string()).collect();
    assert!(lines.contains(&"Classes: 2 tasks done, 25m of focus".to_string()), "{lines:?}");
    assert!(lines.contains(&"Milestone reached: EP v1 mixed".to_string()), "{lines:?}");
    assert_eq!((week["facts"]["from"].as_str(), week["facts"]["to"].as_str()), (Some("2026-10-07"), Some("2026-10-13")));
    let titles: Vec<&str> = week["headings"].as_array().unwrap().iter().map(|h| h["title"].as_str().unwrap()).collect();
    assert_eq!(titles, ["What moved", "What slipped", "Next week's one thing"]);
    assert_eq!(week["headings"][2]["lines"], json!([]));
    // Facts only: no score, no streak, no comparison, and no note of its own.
    let text = week.to_string().to_lowercase();
    for word in ["score", "streak", "rank", "better", "worse", "compared", "average"] {
        assert!(!text.contains(word), "the review says '{word}': {text}");
    }
    assert!(week.get("note").is_none() && week["headings"].as_array().unwrap().iter().all(|h| h.get("markdown").is_none()));
    assert_eq!(refused(&core, "heat.review.complete", json!({"weekStart": "2026-10-07", "note": "  "})).1, "Write your note first.");
    let done = ok(&core, "heat.review.complete", json!({"weekStart": "2026-10-07", "note": "What moved: the quiz."}));
    assert_eq!(done["undo"], "Undo weekly review");
    assert_eq!(done["note"]["markdown"], "What moved: the quiz.", "exactly what the person wrote");
    assert_eq!(done["note"]["title"], "Weekly review, week of Oct 7");
    // Pressing it again changes the same week's note.
    ok(&core, "heat.review.complete", json!({"weekStart": "2026-10-07", "note": "Second thoughts."}));
    let notes = records(&snap(&core, "2026-10-14"), "note");
    assert_eq!((notes.len(), notes[0]["markdown"].as_str()), (1, Some("Second thoughts.")));

    // The note can go public on its own, and come back.
    let id = notes[0]["id"].as_str().unwrap();
    assert_eq!(notes[0]["public"], false);
    let public = ok(&core, "heat.public.set", json!({"kind": "note", "id": id, "public": true}));
    assert_eq!((public["undo"].as_str(), public["record"]["public"].clone()), (Some("Undo make public"), json!(true)));
    let view = ok(&core, "heat.publicView", json!({}));
    assert_eq!(view["items"]["note"][0]["markdown"], "Second thoughts.");
    assert_eq!(ok(&core, "heat.public.set", json!({"kind": "note", "id": id, "public": false}))["undo"], "Undo make private");
    assert_eq!(ok(&core, "heat.publicView", json!({}))["items"], json!({}));
}

#[test]
fn the_calendar_asks_the_snapshot_for_a_day_of_events_and_blocks() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    // An event another calendar held, written by sync outside the journal.
    {
        let mut store = wi_store::Store::open(core.library()).unwrap();
        let event = json!({"id": "cal-1/e1", "calendarId": "cal-1", "title": "Band practice", "start": ny("2026-10-07 18:00"), "end": ny("2026-10-07 20:00"), "allDay": false});
        store.set_doc("calendarEvent", "cal-1/e1", &event, "Band practice").unwrap();
    }
    let s = snap(&core, "2026-10-07");
    assert_eq!(s["events"].as_array().unwrap().len(), 1);
    assert_eq!(s["events"][0]["title"], "Band practice");
    let far = ok(&core, "heat.snapshot", json!({"date": "2027-03-01"}));
    assert!(far["events"].as_array().unwrap().is_empty(), "outside the window it isn't there");
}
