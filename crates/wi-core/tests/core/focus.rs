//! The Focus layout's rules through the core (docs/FOCUS.md): entropy, the
//! Now task, and the one interrupt. What a fail looks like:
//! - entropy counts a task that has time set aside, counts an overdue task
//!   twice, or misses a grade waiting for its score;
//! - Focus shows anything but the current task, then the fix when entropy is
//!   high, then the top task by heat; or offers a fix again the day it was
//!   put off;
//! - a due date Claude moved shows no line, shows more than one, or the
//!   person's own edit shows one;
//! - a dismissed line comes back, an event that changes nothing about what
//!   to do next is shown, or more than one line shows at a time;
//! - a focus round is broken into by anything but what can't wait;
//! - the views aren't told when the entropy changes, or are told when it didn't.

use std::time::Duration;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;
use wi_heat_store::{mcp, ops, Clock};

const T: Duration = Duration::from_secs(5);
const TODAY: &str = "2026-10-07";

fn classes(core: &Core) -> String {
    records(&snap(core, TODAY), "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn id(record: &Value) -> String {
    record["id"].as_str().unwrap().to_string()
}

/// `snapshot.focus` for today.
fn focus(core: &Core) -> Value {
    snap(core, TODAY)["focus"].clone()
}

/// A task due on a day, with half an hour of work in it.
fn due(core: &Core, space: &str, title: &str, when: &str) -> Value {
    add_task(
        core,
        space,
        title,
        json!({"due": ny(when), "estMin": 30, "difficulty": 2}),
    )
}

/// Four tasks due this week, none with time set aside.
fn four_this_week(core: &Core, space: &str) -> Vec<Value> {
    vec![
        due(core, space, "Grammar quiz 4", "2026-10-08 17:00"),
        due(core, space, "Essay draft", "2026-10-09 23:59"),
        due(core, space, "Read chapter 4", "2026-10-12 23:59"),
        // A week to the minute is still this week.
        due(core, space, "Lab 5a", "2026-10-14 09:00"),
    ]
}

/// The clock another process on this library would read.
fn clock() -> Clock {
    Clock::at(
        ny("2026-10-07 09:00"),
        jiff::tz::TimeZone::get("America/New_York").unwrap(),
    )
}

/// Moves a task's due date from outside the app: a journal entry on the
/// library's file through a connection of its own, which is all the app sees
/// of the MCP helper. (None of the helper's tools moves a due date today.)
fn moved_from_outside(core: &Core, task: &str, to: f64) {
    let mut store = wi_store::Store::open(core.library()).unwrap();
    let set = json!({ "due": to });
    ops::patch_record(&mut store, &clock(), "task", task, set.as_object().unwrap()).unwrap();
}

fn raise(core: &Core, id: &str, line: &str, extra: Value) -> Value {
    let mut args = json!({"id": id, "source": "notes", "line": line, "changesNext": true});
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        args[k] = v;
    }
    ok(core, "heat.interrupt.raise", args)
}

#[test]
fn an_empty_library_is_clear_and_shows_nothing() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let shown = focus(&core);
    assert_eq!(
        shown["entropy"],
        json!({"score": 0, "level": "clear", "parts": {"unplanned": 0, "overdue": 0, "mail": 0, "grades": 0}})
    );
    assert_eq!(
        shown["now"],
        json!({"kind": "clear", "taskId": null, "why": null, "fix": null})
    );
    assert_eq!(
        (
            shown["next"].clone(),
            shown["interrupt"].clone(),
            shown["queued"].clone()
        ),
        (Value::Null, Value::Null, json!(0))
    );
    // The one config goes with it, so a view never holds a threshold of its own.
    assert_eq!(
        (
            shown["config"]["horizonDays"].clone(),
            shown["config"]["highAt"].clone(),
            shown["config"]["weights"].clone()
        ),
        (
            json!(7),
            json!(0.5),
            json!({"overdue": 3, "unplanned": 2, "mail": 1.5, "grades": 0.5})
        )
    );
    // The same thing, asked for on its own.
    assert_eq!(ok(&core, "heat.focus.state", json!({"date": TODAY})), shown);
    assert_eq!(ok(&core, "heat.entropy", json!({})), shown["entropy"]);
}

#[test]
fn entropy_counts_this_weeks_tasks_with_no_plan_and_overdue_ones_once() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let space = classes(&core);
    let tasks = four_this_week(&core, &space);
    // A minute past the week, and no due date at all: neither counts.
    due(&core, &space, "Problem set 6", "2026-10-14 09:01");
    add_task(&core, &space, "Tidy the desk", json!({}));
    let parts = |core: &Core| {
        let e = ok(core, "heat.entropy", json!({}));
        (
            e["parts"]["unplanned"].as_u64().unwrap(),
            e["parts"]["overdue"].as_u64().unwrap(),
            e["score"].as_f64().unwrap(),
            e["level"].as_str().unwrap().to_string(),
        )
    };
    // 4 × 2 of 12, to two places.
    assert_eq!(parts(&core), (4, 0, 0.67, "high".to_string()));

    // A time block takes one out.
    ok(
        &core,
        "heat.block.put",
        json!({"taskId": id(&tasks[0]), "date": "2026-10-08", "start": 10 * 60, "minutes": 60}),
    );
    assert_eq!(parts(&core), (3, 0, 0.5, "high".to_string()));
    // So does a day it is scheduled for.
    ok(
        &core,
        "heat.patch",
        json!({"kind": "task", "id": id(&tasks[1]), "set": {"scheduledDate": "2026-10-09"}}),
    );
    assert_eq!(parts(&core), (2, 0, 0.33, "busy".to_string()));
    // A day that has gone by is no plan.
    ok(
        &core,
        "heat.patch",
        json!({"kind": "task", "id": id(&tasks[2]), "set": {"scheduledDate": "2026-10-06"}}),
    );
    assert_eq!(parts(&core), (2, 0, 0.33, "busy".to_string()));

    // An overdue task is overdue, and not also unplanned: 3 + 2 × 2 of 12.
    due(&core, &space, "Lab 4 report", "2026-10-06 12:00");
    assert_eq!(parts(&core), (2, 1, 0.58, "high".to_string()));
}

#[test]
fn a_grade_waiting_for_its_score_counts_and_is_the_fix_when_nothing_else_is() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let term = id(&ok(
        &core,
        "heat.put",
        json!({"kind": "term", "record": {"name": "Fall 2026"}}),
    )["record"]);
    ok(
        &core,
        "heat.put",
        json!({"kind": "course", "record": {"termId": term, "code": "JPN 201", "name": "Japanese 2", "categories": [
            {"name": "Exams", "weight": 50, "keywords": ["exam"]}, {"name": "Final", "weight": 50, "keywords": ["final"]}]}}),
    );
    // Claude records a notice with no score.
    let mut store = wi_store::Store::open(core.library()).unwrap();
    let args = json!({"course": "JPN 201", "item": "Exam 1", "reason": "A grade notice."});
    let grade = mcp::call(
        &mut store,
        &clock(),
        "add_pending_grade",
        args.as_object().unwrap(),
    )
    .unwrap()["grade"]
        .clone();
    drop(store);

    let shown = focus(&core);
    assert_eq!(
        shown["entropy"],
        json!({"score": 0.04, "level": "calm", "parts": {"unplanned": 0, "overdue": 0, "mail": 0, "grades": 1}})
    );
    // Nothing is open, so what is left to put in order is what Focus shows.
    assert_eq!(
        shown["now"],
        json!({"kind": "fix", "taskId": null, "why": null, "fix": {
            "kind": "grades", "line": "1 grade is waiting for its score.", "ask": "Enter it?",
            "action": {"label": "Open Grades", "do": "view", "view": "grades"},
        }})
    );
    assert_eq!(
        shown["interrupt"],
        json!({
            "id": format!("grade:{}:1", id(&grade)),
            "source": "gradeWaiting",
            "line": format!("A grade is waiting for its score: {}.", grade["title"].as_str().unwrap()),
            "action": {"label": "Open Grades", "do": "view", "view": "grades"},
            "priority": "low",
        })
    );

    // The person types the score, and it is waiting no more.
    ok(
        &core,
        "heat.score",
        json!({"gradeId": id(&grade), "score": 88}),
    );
    let after = focus(&core);
    assert_eq!(
        (
            after["entropy"]["level"].as_str(),
            after["now"]["kind"].as_str(),
            after["interrupt"].clone()
        ),
        (Some("clear"), Some("clear"), Value::Null)
    );
}

#[test]
fn now_is_the_current_task_then_the_fix_when_entropy_is_high_then_the_top_task() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let space = classes(&core);
    let tasks = four_this_week(&core, &space);

    // Four tasks with no plan is high: the fix comes before the next task.
    let high = focus(&core);
    assert!(
        high["entropy"]["score"].as_f64().unwrap() >= high["config"]["highAt"].as_f64().unwrap()
    );
    let the_fix = json!({"kind": "fix", "taskId": null, "why": null, "fix": {
        "kind": "plan", "line": "4 tasks this week have no plan.", "ask": "Plan them?",
        "action": {"label": "Plan my day", "do": "plan"},
    }});
    assert_eq!(high["now"], the_fix);
    assert_eq!(
        high["interrupt"],
        Value::Null,
        "the fix is not an interrupt"
    );

    // The current task wins over it.
    let third = id(&tasks[2]);
    ok(&core, "heat.current.set", json!({"taskId": third}));
    assert_eq!(
        focus(&core)["now"],
        json!({"kind": "task", "taskId": third, "why": "current", "fix": null})
    );
    ok(&core, "heat.current.set", json!({}));
    assert_eq!(focus(&core)["now"], the_fix);

    // Put off, the fix gives way to the top task by heat, for the day.
    let events = core.events();
    assert_eq!(ok(&core, "heat.focus.snooze", json!({})), json!({}));
    assert_eq!(
        wait_event(&events, "heat", T).payload,
        json!({"kinds": ["focus"]}),
        "the views are told to look again"
    );
    let snoozed = snap(&core, TODAY);
    let top = snoozed["derived"]["lists"]["allOpen"][0].clone();
    assert_eq!(
        snoozed["focus"]["now"],
        json!({"kind": "task", "taskId": top, "why": "heat", "fix": null})
    );
    assert_eq!(
        snoozed["focus"]["entropy"]["level"], "high",
        "putting it off changes nothing about how things are"
    );
    // Tomorrow it is offered again.
    core.set_now(Some(ny("2026-10-08 09:00")));
    assert_eq!(snap(&core, "2026-10-08")["focus"]["now"]["kind"], "fix");
    core.set_now(Some(ny("2026-10-07 09:00")));

    // With nothing open, nothing.
    for t in &tasks {
        ok(&core, "heat.done", json!({"taskId": id(t), "done": true}));
    }
    assert_eq!(
        focus(&core)["now"],
        json!({"kind": "clear", "taskId": null, "why": null, "fix": null})
    );
}

#[test]
fn a_due_date_moved_from_outside_is_one_interrupt_and_the_persons_own_edit_is_none() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let space = classes(&core);
    let essay = id(&due(&core, &space, "Essay draft", "2026-10-20 23:59"));
    let lab = id(&due(&core, &space, "Lab 5a", "2026-10-21 23:59"));
    assert_eq!(focus(&core)["interrupt"], Value::Null);

    // The person moves one themselves, in the app: they know, so no line.
    ok(
        &core,
        "heat.patch",
        json!({"kind": "task", "id": lab, "set": {"due": ny("2026-10-10 23:59")}}),
    );
    // Long enough for the watcher to have looked at the journal twice.
    std::thread::sleep(Duration::from_millis(1300));
    let own = focus(&core);
    assert_eq!(
        (own["interrupt"].clone(), own["queued"].clone()),
        (Value::Null, json!(0)),
        "the app's own change is not news"
    );

    // The same change from outside the app, as mail makes it.
    let events = core.events();
    let friday = ny("2026-10-09 23:59");
    moved_from_outside(&core, &essay, friday);
    loop {
        let e = wait_event(&events, "heat", T);
        if e.payload["kinds"] == json!(["task"]) {
            break;
        }
    }
    let shown = focus(&core);
    let line = shown["interrupt"]["line"].as_str().unwrap();
    assert!(line.starts_with("Due date moved:"), "{line}");
    assert_eq!(
        shown["interrupt"],
        json!({
            "id": format!("due:{essay}:{}", friday as i64),
            "source": "dueChanged",
            "line": "Due date moved: Essay draft is now due Friday 11:59 PM.",
            "action": {"label": "Show it", "do": "task", "taskId": essay},
            "priority": "normal",
        })
    );
    assert_eq!(shown["queued"], 0, "exactly one");

    // Sent away, it is gone, and it doesn't come back at the next snapshot.
    let events = core.events();
    assert_eq!(
        ok(
            &core,
            "heat.interrupt.dismiss",
            json!({"id": shown["interrupt"]["id"]})
        ),
        json!({})
    );
    assert_eq!(
        wait_event(&events, "heat", T).payload,
        json!({"kinds": ["focus"]})
    );
    for _ in 0..2 {
        let after = focus(&core);
        assert_eq!(
            (after["interrupt"].clone(), after["queued"].clone()),
            (Value::Null, json!(0))
        );
    }
    // Nor after the app closes and opens again: what was dismissed is kept.
    drop(core);
    let core = heat_core(&setup, "2026-10-07 09:00");
    assert_eq!(focus(&core)["interrupt"], Value::Null);
    // And none of it is in the journal: ⌘Z still reads the last real edit.
    assert_eq!(
        ok(&core, "history.get", json!({"room": "heat"}))["undo"],
        "Undo edit task"
    );
}

#[test]
fn a_raised_event_shows_only_if_it_changes_what_to_do_next_and_one_at_a_time() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    snap(&core, TODAY);

    // It changes nothing about what to do next: queued, and never shown.
    assert_eq!(
        raise(
            &core,
            "notes:1",
            "A note was saved.",
            json!({"changesNext": false})
        ),
        json!({"queued": true})
    );
    let quiet = focus(&core);
    assert_eq!(
        (quiet["interrupt"].clone(), quiet["queued"].clone()),
        (Value::Null, json!(0))
    );

    // It does: shown, as raised.
    let action = json!({"label": "Open Notes", "do": "view", "view": "notes"});
    raise(
        &core,
        "notes:2",
        "Your notes for JPN 201 are ready.",
        json!({"action": action}),
    );
    let one = focus(&core);
    assert_eq!(
        one["interrupt"],
        json!({"id": "notes:2", "source": "notes", "line": "Your notes for JPN 201 are ready.", "action": action, "priority": "normal"})
    );
    assert_eq!(one["queued"], 0);

    // A second waits behind the first; raising an id again replaces what it said.
    raise(&core, "notes:3", "Another.", json!({}));
    raise(&core, "notes:3", "Another, reworded.", json!({}));
    let two = focus(&core);
    assert_eq!(
        (two["interrupt"]["id"].as_str(), two["queued"].clone()),
        (Some("notes:2"), json!(1))
    );
    // The more pressing goes first, whenever it came.
    raise(
        &core,
        "notes:4",
        "This can't wait.",
        json!({"priority": "high"}),
    );
    let three = focus(&core);
    assert_eq!(
        (three["interrupt"]["id"].as_str(), three["queued"].clone()),
        (Some("notes:4"), json!(2))
    );
    // Each one dismissed shows the next, oldest first.
    ok(&core, "heat.interrupt.dismiss", json!({"id": "notes:4"}));
    ok(&core, "heat.interrupt.dismiss", json!({"id": "notes:2"}));
    let last = focus(&core);
    assert_eq!(
        (
            last["interrupt"]["id"].as_str(),
            last["interrupt"]["line"].as_str(),
            last["interrupt"]["action"].clone(),
            last["queued"].clone()
        ),
        (
            Some("notes:3"),
            Some("Another, reworded."),
            Value::Null,
            json!(0)
        )
    );

    // An id and a line are what an interrupt is.
    for args in [
        json!({"id": "", "source": "notes", "line": "A line.", "changesNext": true}),
        json!({"id": "notes:5", "source": "notes", "line": " ", "changesNext": true}),
        json!({"id": "notes:5", "source": "notes", "line": "A line."}),
        json!({"id": "notes:5", "source": "notes", "line": "A line.", "changesNext": true, "priority": "loud"}),
        json!({"id": "notes:5", "source": "notes", "line": "A line.", "changesNext": true, "action": {"label": "Go", "do": "dance"}}),
    ] {
        assert_eq!(
            refused(&core, "heat.interrupt.raise", args.clone()).0,
            "bad_args",
            "{args}"
        );
    }
    assert_eq!(focus(&core)["queued"], 0, "a refusal queues nothing");
}

#[test]
fn a_focus_round_is_broken_into_only_by_what_cant_wait() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let task = add_task(&core, &classes(&core), "Mix the second verse", json!({}));
    ok(&core, "heat.current.set", json!({"taskId": id(&task)}));
    ok(&core, "heat.focus.start", json!({}));
    let running = snap(&core, TODAY);
    assert_eq!(
        (
            running["heatState"]["timer"]["phase"].as_str(),
            running["heatState"]["timer"]["running"].as_bool()
        ),
        (Some("focus"), Some(true))
    );

    // Low and normal are held back while the round runs.
    raise(&core, "a", "It can wait.", json!({"priority": "low"}));
    raise(&core, "b", "So can this.", json!({}));
    let held = focus(&core);
    assert_eq!(
        (held["interrupt"].clone(), held["queued"].clone()),
        (Value::Null, json!(0))
    );
    // High is not.
    raise(&core, "c", "This can't.", json!({"priority": "high"}));
    let through = focus(&core);
    assert_eq!(
        (
            through["interrupt"]["id"].as_str(),
            through["queued"].clone()
        ),
        (Some("c"), json!(0))
    );

    // Paused, the ones held back have their turn: nothing was lost.
    ok(&core, "heat.focus.pause", json!({}));
    let paused = focus(&core);
    assert_eq!(
        (paused["interrupt"]["id"].as_str(), paused["queued"].clone()),
        (Some("c"), json!(2))
    );
    ok(&core, "heat.interrupt.dismiss", json!({"id": "c"}));
    assert_eq!(focus(&core)["interrupt"]["id"], "b", "normal before low");
}

#[test]
fn the_entropy_event_is_sent_when_the_entropy_changes_and_only_then() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let events = core.events();
    let space = classes(&core);
    // The first look says where things stand.
    let first = drain(&events, "entropy");
    assert_eq!(first.len(), 1);
    assert_eq!(
        first[0].payload,
        json!({"score": 0, "level": "clear", "parts": {"unplanned": 0, "overdue": 0, "mail": 0, "grades": 0}})
    );
    // Looking again changes nothing, and says nothing.
    snap(&core, TODAY);
    assert!(drain(&events, "entropy").is_empty());

    // A task due this week with no plan: said once, at the next look.
    let task = due(&core, &space, "Essay draft", "2026-10-09 23:59");
    assert!(
        drain(&events, "entropy").is_empty(),
        "entropy is worked out when it is asked for"
    );
    let shown = snap(&core, TODAY);
    let told = drain(&events, "entropy");
    assert_eq!(told.len(), 1);
    assert_eq!(
        told[0].payload,
        json!({"score": 0.17, "level": "calm", "parts": {"unplanned": 1, "overdue": 0, "mail": 0, "grades": 0}})
    );
    assert_eq!(told[0].payload, shown["focus"]["entropy"]);
    snap(&core, TODAY);
    assert!(drain(&events, "entropy").is_empty());

    // Done, and it is clear again.
    ok(
        &core,
        "heat.done",
        json!({"taskId": id(&task), "done": true}),
    );
    assert_eq!(ok(&core, "heat.entropy", json!({}))["level"], "clear");
    let cleared = drain(&events, "entropy");
    assert_eq!(cleared.len(), 1);
    assert_eq!(cleared[0].payload["score"], 0);
}
