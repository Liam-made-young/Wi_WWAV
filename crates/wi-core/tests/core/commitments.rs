//! Commitments through the commands (docs/COMMITMENTS.md). What a fail
//! looks like:
//! - a class is missing from a week of the term, or shows in a break;
//! - Plan my day puts a draft in a class, a shift, a buffer or sleep, or
//!   the free-time line says more than is there;
//! - a schedule is written before it is accepted, or takes more than one
//!   undo to take back; a week's shifts touch another week;
//! - Claude is asked without the person asking;
//! - a mail that cancels a class changes it without a tap.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

const DAY: &str = "2026-10-07";
const NOW: &str = "2026-10-07 09:00";
const T: Duration = Duration::from_secs(20);

fn commitments(core: &Core) -> Value {
    snap(core, DAY)["commitments"].clone()
}

/// The days of a window a commitment is drawn on.
fn days_of(core: &Core, from: &str, to: &str, title: &str) -> Vec<String> {
    let s = ok(
        core,
        "heat.snapshot",
        json!({"date": DAY, "from": from, "to": to}),
    );
    let mut out: Vec<String> = s["commitments"]["days"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(day, _)| day.as_str() >= from && day.as_str() <= to)
        .filter(|(_, list)| list.as_array().unwrap().iter().any(|o| o["title"] == title))
        .map(|(day, _)| day.clone())
        .collect();
    out.sort();
    out
}

fn school(core: &Core) {
    ok(
        core,
        "heat.school.set",
        json!({"name": "URI", "host": "brightspace.uri.edu", "codePattern": "([A-Z]{2,4})\\s?(\\d{3})", "termStart": "2026-09-09", "termEnd": "2026-12-11"}),
    );
}

fn jpn(core: &Core) -> Value {
    ok(
        core,
        "heat.commitment.create",
        json!({"course": "JPN 101", "days": ["MO", "WE", "FR"], "start": "10:00", "end": "10:50",
               "location": "Swan Hall 201", "bufferBefore": 25, "bufferAfter": 10}),
    )
}

#[test]
fn a_class_shows_every_week_of_the_term_and_skips_the_breaks() {
    let setup = Setup::new();
    let core = heat_core(&setup, NOW);
    school(&core);
    let made = jpn(&core);
    assert_eq!(made["undo"], "Undo add commitment");
    assert_eq!(made["line"], "JPN 101, MWF 10:00 AM to 10:50 AM.");
    let c = &made["commitment"];
    assert_eq!(
        (c["kind"].as_str(), c["start"].as_i64(), c["end"].as_i64()),
        (Some("class"), Some(600), Some(650))
    );
    assert_eq!(c["rrule"], "FREQ=WEEKLY;BYDAY=MO,WE,FR");
    assert_eq!(c["from"], "2026-09-09", "a class starts with the term");
    // The class named a course Learn didn't hold: a stub, in the same entry.
    let courses = records(&snap(&core, DAY), "course");
    assert_eq!(courses.len(), 1);
    assert_eq!(
        (courses[0]["code"].as_str(), courses[0]["status"].as_str()),
        (Some("JPN 101"), Some("stub"))
    );
    assert_eq!(c["courseId"], courses[0]["id"]);
    assert_eq!(made["course"]["code"], "JPN 101");
    // It sits with the courses, and takes that space's colour.
    let occ = &commitments(&core)["days"][DAY][0];
    assert_eq!(
        (
            occ["title"].as_str(),
            occ["start"].as_i64(),
            occ["bufferBefore"].as_i64()
        ),
        (Some("JPN 101"), Some(600), Some(25))
    );
    assert!(occ["hue"].is_number(), "{occ}");

    assert_eq!(
        days_of(&core, "2026-10-04", "2026-10-17", "JPN 101"),
        [
            "2026-10-05",
            "2026-10-07",
            "2026-10-09",
            "2026-10-12",
            "2026-10-14",
            "2026-10-16"
        ]
    );
    // Columbus Day and Thanksgiving go in as breaks; the class skips them.
    ok(
        &core,
        "heat.put",
        json!({"kind": "termBreak", "record": {"title": "Columbus Day", "from": "2026-10-12"}}),
    );
    let recess = ok(
        &core,
        "heat.put",
        json!({"kind": "termBreak", "record": {"title": "Thanksgiving recess", "from": "2026-11-25", "to": "2026-11-29"}}),
    );
    assert_eq!(recess["undo"], "Undo add term break");
    assert_eq!(
        days_of(&core, "2026-10-04", "2026-10-17", "JPN 101"),
        [
            "2026-10-05",
            "2026-10-07",
            "2026-10-09",
            "2026-10-14",
            "2026-10-16"
        ]
    );
    assert_eq!(
        days_of(&core, "2026-11-22", "2026-11-30", "JPN 101"),
        ["2026-11-23", "2026-11-30"]
    );
    // Every week, to the term's last day and not past it.
    assert_eq!(
        days_of(&core, "2026-09-01", "2026-12-31", "JPN 101").len(),
        2 + 13 * 3 - 3
    );
    assert_eq!(
        days_of(&core, "2026-12-10", "2026-12-31", "JPN 101"),
        ["2026-12-11"]
    );
    assert_eq!(commitments(&core)["list"][0]["range"], "Sep 9 to Dec 11");
    // One undo takes the class and its stub course away.
    ok(&core, "history.undo", json!({"room": "heat"}));
    ok(&core, "history.undo", json!({"room": "heat"}));
    ok(&core, "history.undo", json!({"room": "heat"}));
    let back = snap(&core, DAY);
    assert!(records(&back, "commitment").is_empty() && records(&back, "course").is_empty());
}

#[test]
fn a_commitment_is_checked_before_it_is_kept() {
    let setup = Setup::new();
    let core = heat_core(&setup, NOW);
    for (args, sentence) in [
        (
            json!({"title": "", "start": 600, "end": 650}),
            "Give the commitment a name first.",
        ),
        (
            json!({"title": "Gym", "start": "10:00"}),
            "A commitment needs a start and an end, such as 10:00 and 10:50.",
        ),
        (
            json!({"title": "Gym", "start": "11:00", "end": "10:00"}),
            "A commitment ends after it starts. One that runs past midnight is two.",
        ),
        (
            json!({"title": "Gym", "start": 600, "end": 660, "days": ["Someday"]}),
            "Days are weekdays, such as MO, WE, FR.",
        ),
        (
            json!({"title": "Gym", "start": 600, "end": 660, "rrule": "FREQ=HOURLY"}),
            "Learn can't read that repeat rule.",
        ),
        (
            json!({"title": "Gym", "start": 600, "end": 660, "kind": "party"}),
            "A commitment is a class, work, a commute or other.",
        ),
        (
            json!({"title": "Gym", "start": 600, "end": 660, "from": "2026-10-10", "until": "2026-10-01"}),
            "A commitment's last day is on or after its first.",
        ),
        (
            json!({"title": "Gym", "start": 600, "end": 660, "colour": "red"}),
            "A commitment has no field called 'colour'.",
        ),
        (
            json!({"title": "Gym", "start": 600, "end": 660, "space": "Nowhere"}),
            "No space is called Nowhere.",
        ),
    ] {
        assert_eq!(
            refused(&core, "heat.commitment.create", args.clone()),
            ("refused".to_string(), sentence.to_string()),
            "{args}"
        );
    }
    // One day, said in words; travel is kept within four hours.
    let shift = ok(
        &core,
        "heat.commitment.create",
        json!({"title": "Bookstore", "kind": "work", "date": "2026-10-10", "start": "9am", "end": "5pm", "bufferBefore": 999}),
    );
    let c = &shift["commitment"];
    assert_eq!(
        (
            c["from"].as_str(),
            c["start"].as_i64(),
            c["end"].as_i64(),
            c["bufferBefore"].as_i64()
        ),
        (Some("2026-10-10"), Some(540), Some(1020), Some(240))
    );
    assert!(c.get("rrule").is_none());
    assert_eq!(shift["line"], "Bookstore, Sat Oct 10, 9:00 AM to 5:00 PM.");
    // An edit by its title, since only one has it.
    let edited = ok(
        &core,
        "heat.commitment.update",
        json!({"id": "bookstore", "set": {"end": "16:00", "bufferBefore": 20, "hardness": "flexible"}}),
    );
    assert_eq!(edited["undo"], "Undo edit commitment");
    assert_eq!(
        (
            edited["commitment"]["end"].as_i64(),
            edited["commitment"]["hardness"].as_str()
        ),
        (Some(960), Some("flexible"))
    );
    assert_eq!(
        refused(
            &core,
            "heat.commitment.update",
            json!({"id": "Nothing", "set": {}})
        )
        .1,
        "No commitment is called Nothing."
    );
    // The generic door checks the same things.
    assert_eq!(
        refused(
            &core,
            "heat.put",
            json!({"kind": "commitment", "record": {"title": "Gym", "start": 700, "end": 600}})
        )
        .1,
        "A commitment ends after it starts. One that runs past midnight is two."
    );
    ok(
        &core,
        "heat.delete",
        json!({"kind": "commitment", "id": c["id"]}),
    );
    assert!(records(&snap(&core, DAY), "commitment").is_empty());
}

#[test]
fn one_day_is_skipped_or_moved_and_only_a_day_it_meets_on() {
    let setup = Setup::new();
    let core = heat_core(&setup, NOW);
    school(&core);
    let id = jpn(&core)["commitment"]["id"].as_str().unwrap().to_string();
    let skipped = ok(
        &core,
        "heat.commitment.addException",
        json!({"id": id, "date": "2026-10-09", "kind": "skip"}),
    );
    assert_eq!(skipped["undo"], "Undo skip class");
    assert_eq!(skipped["line"], "JPN 101 on Friday, Oct 9 is skipped.");
    assert_eq!(
        days_of(&core, "2026-10-04", "2026-10-10", "JPN 101"),
        ["2026-10-05", "2026-10-07"]
    );
    assert_eq!(
        refused(
            &core,
            "heat.commitment.addException",
            json!({"id": id, "date": "2026-10-08", "kind": "skip"})
        )
        .1,
        "JPN 101 doesn't meet on Thursday, Oct 8."
    );
    assert_eq!(
        refused(
            &core,
            "heat.commitment.addException",
            json!({"id": id, "date": "2026-10-05", "kind": "move"})
        )
        .1,
        "Say where it moves to: a day, a time, or both."
    );
    // Monday's class moves to Tuesday at 2.
    let moved = ok(
        &core,
        "heat.commitment.addException",
        json!({"id": "JPN 101", "date": "2026-10-12", "kind": "move", "toDate": "2026-10-13", "start": "14:00"}),
    );
    assert_eq!(moved["undo"], "Undo move class");
    let week = ok(
        &core,
        "heat.snapshot",
        json!({"date": DAY, "from": "2026-10-11", "to": "2026-10-17"}),
    );
    assert!(week["commitments"]["days"].get("2026-10-12").is_none());
    let tuesday = &week["commitments"]["days"]["2026-10-13"][0];
    assert_eq!(
        (
            tuesday["start"].as_i64(),
            tuesday["end"].as_i64(),
            tuesday["movedFrom"].as_str()
        ),
        (Some(840), Some(890), Some("2026-10-12"))
    );
    // The day goes as usual again.
    let back = ok(
        &core,
        "heat.commitment.removeException",
        json!({"id": id, "date": "2026-10-09"}),
    );
    assert_eq!(back["undo"], "Undo restore class");
    assert_eq!(
        days_of(&core, "2026-10-04", "2026-10-10", "JPN 101"),
        ["2026-10-05", "2026-10-07", "2026-10-09"]
    );
}

#[test]
fn plan_my_day_never_lands_in_a_class_a_shift_or_a_buffer_and_says_what_is_free() {
    let setup = Setup::new();
    let core = heat_core(&setup, NOW);
    school(&core);
    jpn(&core);
    ok(
        &core,
        "heat.commitment.create",
        json!({"title": "Bookstore", "kind": "work", "date": DAY, "start": "13:00", "end": "17:00", "bufferBefore": 15, "bufferAfter": 15}),
    );
    let space = records(&snap(&core, DAY), "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    for n in 0..8 {
        add_task(
            &core,
            &space,
            &format!("Essay part {n}"),
            json!({"due": ny("2026-10-08 23:59"), "estMin": 90}),
        );
    }
    let plan = ok(&core, "heat.plan.make", json!({"date": DAY}));
    let drafts = plan["drafts"].as_array().unwrap();
    assert!(drafts.len() >= 4, "{plan}");
    // Taken: 9:35 to 11:00 (the class and its travel), 12:45 to 5:15 (the shift and its).
    let taken = [(575.0, 660.0), (765.0, 1035.0)];
    for d in drafts {
        let (a, b) = (
            d["start"].as_f64().unwrap(),
            d["start"].as_f64().unwrap() + d["minutes"].as_f64().unwrap(),
        );
        for (lo, hi) in taken {
            assert!(
                b <= lo || a >= hi,
                "a draft at {a} to {b} sits on {lo} to {hi}"
            );
        }
        assert!(
            (540.0..=1380.0).contains(&a) && b <= 1380.0,
            "a draft at {a} to {b} is outside the waking day"
        );
    }
    // 9:00 to 9:35 holds nothing 90 minutes long: the first draft is after class.
    assert_eq!(drafts[0]["start"], 660);
    // Free: 9:00 to 11 PM is 840 minutes, less 85 and 270.
    let free = &commitments(&core)["free"];
    assert_eq!(free["freeMin"], 840 - 85 - 270);
    assert_eq!(free["line"], "8h 5m free today.");
    // Accepted, the blocks are what is planned; with more planned than fits, it says so.
    ok(&core, "heat.plan.accept", json!({"date": DAY}));
    let free = &commitments(&core)["free"];
    assert_eq!(free["overMin"], 0);
    assert!(
        free["line"]
            .as_str()
            .unwrap()
            .starts_with("8h 5m free today. Planned "),
        "{free}"
    );
    // A day ahead is the whole day, by its name.
    let ahead = ok(
        &core,
        "heat.planner.freeTime",
        json!({"date": "2026-10-09"}),
    );
    assert_eq!(ahead["days"][0]["line"], "14h 35m free Friday.");
    assert_eq!(ahead["days"][0]["commitments"][0]["title"], "JPN 101");
    let range = ok(
        &core,
        "heat.planner.freeTime",
        json!({"from": "2026-10-10", "to": "2026-10-11"}),
    );
    assert_eq!(range["days"].as_array().unwrap().len(), 2);
    assert_eq!(
        range["days"][0]["freeMin"], 960,
        "a Saturday with nothing in it"
    );
}

#[test]
fn sleep_is_time_that_isnt_there_and_a_block_on_a_class_is_flagged() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 06:00");
    school(&core);
    jpn(&core);
    assert_eq!(
        commitments(&core)["sleep"],
        json!({"from": 1380, "to": 420})
    );
    // Up at 9, to bed at 1: the morning is gone, the late evening is there.
    ok(&core, "heat.sleep.set", json!({"from": 60, "to": 540}));
    assert_eq!(
        refused(&core, "heat.sleep.set", json!({"from": 600, "to": 480})).1,
        "That leaves under eight hours awake. Check the two times."
    );
    let space = records(&snap(&core, DAY), "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let task = add_task(&core, &space, "Read chapter 6", json!({"estMin": 30}));
    let plan = ok(
        &core,
        "heat.plan.make",
        json!({"date": DAY, "dayEnds": 1440}),
    );
    assert_eq!(
        plan["drafts"][0]["start"], 540,
        "nothing is planned before 9"
    );
    ok(&core, "heat.plan.clear", json!({}));
    // A block dragged onto the class is the person's choice: flagged, not refused.
    let on_class = ok(
        &core,
        "heat.block.put",
        json!({"taskId": task["id"], "date": DAY, "start": 615, "minutes": 30}),
    );
    let on_travel = ok(
        &core,
        "heat.block.put",
        json!({"taskId": task["id"], "date": DAY, "start": 540, "minutes": 45}),
    );
    let found = commitments(&core)["conflicts"].clone();
    let by_block = |id: &Value| {
        found
            .as_array()
            .unwrap()
            .iter()
            .find(|c| &c["blockId"] == id)
            .cloned()
            .unwrap()
    };
    assert_eq!(
        by_block(&on_class["block"]["id"])["line"],
        "Overlaps JPN 101 (10:00 AM to 10:50 AM)."
    );
    assert_eq!(by_block(&on_travel["block"]["id"])["bufferOnly"], true);
    // Late in the evening, more is planned than there is room for, and it says so.
    let late = Setup::new();
    let night = heat_core(&late, "2026-10-07 21:30");
    let space = records(&snap(&night, DAY), "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let essay = add_task(&night, &space, "Essay", json!({}));
    ok(
        &night,
        "heat.block.put",
        json!({"taskId": essay["id"], "date": DAY, "start": 1290, "minutes": 150}),
    );
    assert_eq!(
        commitments(&night)["free"]["line"],
        "1h 30m free today. Planned 2h 30m. Move 1h?"
    );
    // The readout, three hours ahead of class and then during it.
    core.set_now(Some(ny("2026-10-07 09:00")));
    assert_eq!(
        commitments(&core)["next"]["line"],
        "NEXT JPN 101 10:00 · LEAVE 9:35"
    );
    core.set_now(Some(ny("2026-10-07 10:20")));
    assert!(commitments(&core)["next"].is_null());
}

/// A stand-in for the command line: it keeps what it was given and answers
/// with `answer` as the structured output.
fn fake_claude(dir: &Path, answer: &Value) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("claude");
    std::fs::write(
        dir.join("answer.json"),
        json!({"is_error": false, "result": "", "structured_output": answer}).to_string(),
    )
    .unwrap();
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/args'\npwd -P > '{d}/cwd'\nls > '{d}/files'\ncat > '{d}/prompt'\ncat '{d}/answer.json'\n",
            d = dir.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn core_with_claude(setup: &Setup, claude: &Path) -> Core {
    let mut config = setup.config(NO_SERVER, no_browser());
    config.now = Some(ny(NOW));
    config.claude = Some(claude.to_path_buf());
    let core = Core::open(&setup.library(), config).unwrap();
    ok(
        &core,
        "app.settings.set",
        json!({"patch": {"heat": {"timeZone": "America/New_York"}}}),
    );
    core
}

fn item(
    title: &str,
    kind: &str,
    days: Value,
    date: Value,
    start: &str,
    end: &str,
    course: Value,
) -> Value {
    json!({"title": title, "kind": kind, "days": days, "date": date, "start": start, "end": end, "location": null, "course": course, "from": null, "until": null})
}

fn ready(core: &Core, id: &Value) -> Value {
    let mut found = Value::Null;
    assert!(
        eventually(T, || {
            let drafts = commitments(core)["drafts"].clone();
            found = drafts
                .as_array()
                .unwrap()
                .iter()
                .find(|d| &d["id"] == id)
                .cloned()
                .unwrap_or(Value::Null);
            found["state"] == "ready" || found["state"] == "failed"
        }),
        "the draft was never read"
    );
    found
}

#[test]
fn a_pasted_schedule_is_read_once_previewed_and_applied_as_one_undo() {
    let setup = Setup::new();
    let dir = tempfile::tempdir().unwrap();
    let claude = fake_claude(
        dir.path(),
        &json!({"items": [
            item("JPN 101", "class", json!(["MO", "WE", "FR"]), Value::Null, "10:00", "10:50", json!("JPN 101")),
            item("EGR 101", "class", json!(["TU", "TH"]), Value::Null, "11:00", "12:15", json!("EGR 101")),
            item("Garbled", "class", json!(["TU"]), Value::Null, "noon", "1", Value::Null),
        ], "breaks": []}),
    );
    let core = core_with_claude(&setup, &claude);
    school(&core);
    // Nothing is asked until the person pastes.
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        !dir.path().join("prompt").exists(),
        "Claude was asked without being asked"
    );
    let begun = ok(
        &core,
        "heat.commitment.importText",
        json!({"text": "JPN 101  MWF 10:00-10:50\nEGR 101  TTh 11-12:15"}),
    );
    assert_eq!(begun["draft"]["state"], "reading");
    assert!(begun["draft"].get("text").is_none());
    let draft = ready(&core, &begun["draft"]["id"]);
    assert_eq!(draft["state"], "ready", "{draft}");
    assert_eq!(
        draft["line"],
        "2 classes. 2 new courses. 1 line couldn't be read."
    );
    assert_eq!(draft["items"][1]["when"], "TuTh 11:00 AM to 12:15 PM");
    assert_eq!(draft["items"][0]["isNew"], true);
    // One run, no tools, the small model; the schedule and today were in the prompt.
    let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
    assert!(
        args.contains("--tools\n\n")
            && args.contains("--model\nhaiku")
            && args.contains("--json-schema"),
        "{args}"
    );
    let prompt = std::fs::read_to_string(dir.path().join("prompt")).unwrap();
    assert!(
        prompt.contains("EGR 101  TTh 11-12:15")
            && prompt.contains("Today is Wednesday, 2026-10-07.")
            && prompt.contains("The term runs 2026-09-09 to 2026-12-11."),
        "{prompt}"
    );
    // Previewed, not applied.
    let before = snap(&core, DAY);
    assert!(records(&before, "commitment").is_empty() && records(&before, "course").is_empty());
    let done = ok(
        &core,
        "heat.commitment.draft.accept",
        json!({"draftId": draft["id"]}),
    );
    assert_eq!(done["undo"], "Undo import schedule");
    assert_eq!(
        (
            done["commitments"].as_i64(),
            done["courses"].as_array().unwrap().len()
        ),
        (Some(2), 2)
    );
    let after = snap(&core, DAY);
    assert_eq!(records(&after, "commitment").len(), 2);
    assert_eq!(records(&after, "course").len(), 2);
    assert_eq!(
        records(&after, "term").len(),
        1,
        "one term for the two courses"
    );
    assert!(after["commitments"]["drafts"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        days_of(&core, "2026-10-04", "2026-10-10", "EGR 101"),
        ["2026-10-06", "2026-10-08"]
    );
    // The same schedule again adds nothing.
    let again = ok(
        &core,
        "heat.commitment.importText",
        json!({"text": "the same again"}),
    );
    let draft = ready(&core, &again["draft"]["id"]);
    assert_eq!(
        draft["line"],
        "Nothing to add. 2 already there. 1 line couldn't be read."
    );
    ok(
        &core,
        "heat.commitment.draft.discard",
        json!({"draftId": draft["id"]}),
    );
    // One undo takes all of it back: the commitments, the courses, the term.
    ok(&core, "history.undo", json!({"room": "heat"}));
    let back = snap(&core, DAY);
    assert!(
        records(&back, "commitment").is_empty()
            && records(&back, "course").is_empty()
            && records(&back, "term").is_empty()
    );
}

#[test]
fn a_photographed_work_schedule_becomes_that_weeks_shifts_and_no_other_weeks() {
    let setup = Setup::new();
    let dir = tempfile::tempdir().unwrap();
    let claude = fake_claude(
        dir.path(),
        &json!({"items": [
            item("Bookstore", "work", json!(["TH"]), Value::Null, "16:00", "20:00", Value::Null),
            item("Bookstore", "work", Value::Null, json!("2026-10-10"), "09:00", "17:00", Value::Null),
        ], "breaks": []}),
    );
    let core = core_with_claude(&setup, &claude);
    // Last week's shifts, and one this week from an earlier paste.
    let old = |date: &str, week: &str| {
        ok(
            &core,
            "heat.put",
            json!({"kind": "commitment", "record": {"title": "Bookstore", "kind": "work", "from": date, "start": 600, "end": 840, "source": "paste", "weekOf": week}}),
        );
    };
    old("2026-10-01", "2026-09-28");
    old("2026-10-06", "2026-10-05");
    old("2026-10-09", "2026-10-05");
    let photo = setup.dir.path().join("rota.png");
    std::fs::write(&photo, b"not really a png").unwrap();
    assert_eq!(
        refused(
            &core,
            "heat.commitment.importImage",
            json!({"path": setup.dir.path().join("rota.txt"), "mode": "week"})
        )
        .1,
        "Drop a photo or a screenshot of the schedule."
    );
    let begun = ok(
        &core,
        "heat.commitment.importImage",
        json!({"path": photo, "mode": "week"}),
    );
    assert_eq!(
        (
            begun["draft"]["state"].as_str(),
            begun["draft"]["source"].as_str(),
            begun["draft"]["weekOf"].as_str()
        ),
        (Some("reading"), Some("photo"), Some("2026-10-05"))
    );
    let draft = ready(&core, &begun["draft"]["id"]);
    assert_eq!(draft["state"], "ready", "{draft}");
    assert_eq!(
        draft["line"],
        "2 shifts. Replaces 2 shifts in the week of Oct 5."
    );
    assert_eq!(draft["items"][0]["when"], "Thu Oct 8, 4:00 PM to 8:00 PM");
    // The run could read, and only read; it stood in a folder holding the photo alone.
    let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
    assert!(
        args.contains("--tools\nRead\n") && args.contains("--model\nsonnet"),
        "{args}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("files"))
            .unwrap()
            .trim(),
        "schedule.png"
    );
    let stood = std::fs::read_to_string(dir.path().join("cwd")).unwrap();
    assert!(
        !Path::new(stood.trim()).exists(),
        "the folder made for the run is gone after it"
    );
    let prompt = std::fs::read_to_string(dir.path().join("prompt")).unwrap();
    assert!(
        prompt.contains("Open the image file schedule.png")
            && prompt.contains("Monday 2026-10-05 to Sunday 2026-10-11"),
        "{prompt}"
    );

    let done = ok(
        &core,
        "heat.commitment.draft.accept",
        json!({"draftId": draft["id"]}),
    );
    assert_eq!(done["undo"], "Undo this week's shifts");
    assert_eq!(
        (done["commitments"].as_i64(), done["replaced"].as_i64()),
        (Some(2), Some(2))
    );
    let mut days: Vec<String> = records(&snap(&core, DAY), "commitment")
        .iter()
        .map(|c| c["from"].as_str().unwrap().to_string())
        .collect();
    days.sort();
    assert_eq!(
        days,
        ["2026-10-01", "2026-10-08", "2026-10-10"],
        "last week's shift is untouched"
    );
    // One undo puts the week back as it was.
    ok(&core, "history.undo", json!({"room": "heat"}));
    let mut days: Vec<String> = records(&snap(&core, DAY), "commitment")
        .iter()
        .map(|c| c["from"].as_str().unwrap().to_string())
        .collect();
    days.sort();
    assert_eq!(days, ["2026-10-01", "2026-10-06", "2026-10-09"]);
}

#[test]
fn a_schedule_that_cant_be_read_says_so_and_leaves_nothing() {
    let setup = Setup::new();
    let dir = tempfile::tempdir().unwrap();
    let claude = fake_claude(dir.path(), &json!({"items": [], "breaks": []}));
    let core = core_with_claude(&setup, &claude);
    let begun = ok(
        &core,
        "heat.commitment.importText",
        json!({"text": "a shopping list", "mode": "week"}),
    );
    let draft = ready(&core, &begun["draft"]["id"]);
    assert_eq!(
        (draft["state"].as_str(), draft["error"].as_str()),
        (
            Some("failed"),
            Some("No shifts could be read from that. Try a clearer copy, or add them by hand.")
        )
    );
    assert_eq!(
        refused(
            &core,
            "heat.commitment.draft.accept",
            json!({"draftId": draft["id"]})
        )
        .1,
        "That schedule is still being read."
    );
    assert_eq!(
        refused(&core, "heat.commitment.importText", json!({"text": "  "})).1,
        "Paste the schedule first."
    );
    // With no Claude to ask, it says so at once and leaves no draft.
    let bare = Setup::new();
    let mut config = bare.config(NO_SERVER, no_browser());
    config.claude = Some(bare.dir.path().join("no-claude-here"));
    let alone = Core::open(&bare.library(), config).unwrap();
    let (code, _) = refused(
        &alone,
        "heat.commitment.importText",
        json!({"text": "JPN 101 MWF 10"}),
    );
    assert_eq!(code, "claude");
    assert!(commitments(&alone)["drafts"].as_array().unwrap().is_empty());
    // A caller that read the schedule itself hands it over, and it is ready at once.
    let given = ok(
        &alone,
        "heat.commitment.importText",
        json!({"mode": "breaks", "json": {"items": [], "breaks": [{"title": "Reading day", "from": "2026-12-12", "to": "2026-12-12"}]}}),
    );
    assert_eq!(
        (
            given["draft"]["state"].as_str(),
            given["draft"]["line"].as_str()
        ),
        (Some("ready"), Some("1 break."))
    );
    let done = ok(
        &alone,
        "heat.commitment.draft.accept",
        json!({"draftId": given["draft"]["id"]}),
    );
    assert_eq!(
        (done["undo"].as_str(), done["breaks"].as_i64()),
        (Some("Undo import breaks"), Some(1))
    );
}

const TIMETABLE: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\n\
BEGIN:VEVENT\r\nUID:jpn-101@uri\r\nSUMMARY:JPN 101\r\nLOCATION:Swan Hall 201\r\n\
DTSTART;TZID=America/New_York:20260909T100000\r\nDTEND;TZID=America/New_York:20260909T105000\r\n\
RRULE:FREQ=WEEKLY;WKST=SU;UNTIL=20261212T045959Z;BYDAY=MO,WE,FR\r\n\
EXDATE;TZID=America/New_York:20261012T100000\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:shift-1\r\nSUMMARY:Bookstore\r\nDTSTART:20261010T130000Z\r\nDTEND:20261010T210000Z\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:break\r\nSUMMARY:Thanksgiving recess\r\nDTSTART;VALUE=DATE:20261125\r\nDTEND;VALUE=DATE:20261130\r\nEND:VEVENT\r\n\
END:VCALENDAR\r\n";

#[test]
fn a_calendar_file_is_previewed_and_a_subscribed_one_is_kept_in_step() {
    let setup = Setup::new();
    let core = heat_core(&setup, NOW);
    let file = setup.dir.path().join("timetable.ics");
    std::fs::write(&file, TIMETABLE).unwrap();
    let begun = ok(&core, "heat.commitment.importIcs", json!({"path": file}));
    let draft = &begun["draft"];
    assert_eq!(
        (draft["state"].as_str(), draft["source"].as_str()),
        (Some("ready"), Some("ics"))
    );
    assert_eq!(draft["line"], "1 class and 1 commitment. 1 new course.");
    ok(
        &core,
        "heat.commitment.draft.accept",
        json!({"draftId": draft["id"]}),
    );
    assert_eq!(
        days_of(&core, "2026-10-04", "2026-10-17", "JPN 101"),
        [
            "2026-10-05",
            "2026-10-07",
            "2026-10-09",
            "2026-10-14",
            "2026-10-16"
        ],
        "the file's own skipped day is skipped"
    );
    assert_eq!(
        days_of(&core, "2026-10-04", "2026-10-17", "Bookstore"),
        ["2026-10-10"]
    );
    // The same file as an academic calendar gives its whole days.
    let breaks = ok(
        &core,
        "heat.commitment.importIcs",
        json!({"text": TIMETABLE, "mode": "breaks"}),
    );
    assert_eq!(breaks["draft"]["line"], "1 break.");
    ok(
        &core,
        "heat.commitment.draft.accept",
        json!({"draftId": breaks["draft"]["id"]}),
    );
    assert_eq!(
        days_of(&core, "2026-11-22", "2026-11-30", "JPN 101"),
        ["2026-11-23", "2026-11-30"]
    );
    assert_eq!(
        refused(
            &core,
            "heat.commitment.importIcs",
            json!({"text": "<html>Sign in</html>"})
        )
        .1,
        "That isn't a calendar file. Export it as .ics and try again."
    );

    // A subscribed address: read now, written as one entry, and read again on request.
    let feed = FeedServer::start(
        TIMETABLE
            .replace("jpn-101@uri", "work-1")
            .replace("JPN 101", "Lab shift")
            .as_bytes(),
    );
    let subscribed = ok(
        &core,
        "heat.commitment.importIcs",
        json!({"url": feed.address("secret"), "subscribe": true, "name": "Work rota"}),
    );
    assert_eq!(
        (
            subscribed["undo"].as_str(),
            subscribed["feed"]["name"].as_str()
        ),
        (Some("Undo schedule sync"), Some("Work rota"))
    );
    let c = commitments(&core);
    assert_eq!(c["feeds"][0]["name"], "Work rota");
    assert!(
        !c.to_string().contains("secret"),
        "the address is in the Keychain and nowhere else"
    );
    let lab = records(&snap(&core, DAY), "commitment")
        .into_iter()
        .find(|c| c["title"] == "Lab shift")
        .unwrap();
    // The person's own travel time and a skipped day survive the next read.
    ok(
        &core,
        "heat.commitment.update",
        json!({"id": lab["id"], "set": {"bufferBefore": 20}}),
    );
    ok(
        &core,
        "heat.commitment.addException",
        json!({"id": lab["id"], "date": "2026-10-16", "kind": "skip"}),
    );
    feed.serve(
        200,
        TIMETABLE
            .replace("jpn-101@uri", "work-1")
            .replace("JPN 101", "Lab shift")
            .replace("T100000", "T110000")
            .replace("T105000", "T115000")
            .as_bytes(),
    );
    let again = ok(&core, "heat.commitment.feed.sync", json!({}));
    assert_eq!(again["changed"], 1);
    let lab = records(&snap(&core, DAY), "commitment")
        .into_iter()
        .find(|c| c["title"] == "Lab shift")
        .unwrap();
    assert_eq!(
        (lab["start"].as_i64(), lab["bufferBefore"].as_i64()),
        (Some(660), Some(20))
    );
    assert!(lab["exceptions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["date"] == "2026-10-16"));
    assert_eq!(
        ok(&core, "heat.commitment.feed.sync", json!({}))["changed"],
        0,
        "nothing changed, nothing written"
    );
    ok(
        &core,
        "heat.commitment.feed.remove",
        json!({"id": subscribed["feed"]["id"]}),
    );
    assert!(commitments(&core)["feeds"].as_array().unwrap().is_empty());
}

fn record_mail(setup: &Setup, id: &str, subject: &str, course: &str, at: &str, text: &str) {
    let mut store = wi_store::Store::open(&setup.library()).unwrap();
    let clock = wi_heat_store::Clock::system();
    let thread = json!({"thread_id": id, "subject": subject, "from": "Prof <p@uri.edu>", "received_at": at, "course": course, "state": "nothing", "unread": true, "reason": "Class news."});
    wi_heat_store::mcp::call(
        &mut store,
        &clock,
        "record_mail_thread",
        thread.as_object().unwrap(),
    )
    .unwrap();
    let body = json!({"thread_id": id, "messages": [{"from": "Prof <p@uri.edu>", "sent_at": at, "text": text}]});
    wi_heat_store::mcp::call(
        &mut store,
        &clock,
        "save_mail_text",
        body.as_object().unwrap(),
    )
    .unwrap();
}

#[test]
fn mail_that_cancels_thursdays_class_waits_for_one_tap() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    school(&core);
    let egr = ok(
        &core,
        "heat.commitment.create",
        json!({"course": "EGR 101", "days": ["TU", "TH"], "start": "11:00", "end": "12:15"}),
    );
    let id = egr["commitment"]["id"].clone();
    record_mail(
        &setup,
        "th-cancel",
        "EGR 101: Class canceled Thursday",
        "EGR 101",
        "2026-10-07T08:15:00-04:00",
        "I am out sick. Class is canceled Thursday. See you next week.",
    );
    record_mail(
        &setup,
        "th-hw",
        "EGR 101: Homework 3 posted",
        "EGR 101",
        "2026-10-07T08:20:00-04:00",
        "Homework 3 is due Friday.",
    );
    let found = ok(&core, "heat.commitment.mail.check", json!({}));
    let pending = found["pending"].as_array().unwrap();
    assert_eq!(pending.len(), 1, "{found}");
    assert_eq!(
        pending[0]["line"],
        "EGR 101 is canceled Thursday, Oct 8. Skip it?"
    );
    assert_eq!(
        (
            pending[0]["act"].as_str(),
            pending[0]["commitmentId"].as_str()
        ),
        (Some("Skip it"), id.as_str())
    );
    // Never applied silently: Thursday's class is still there.
    assert_eq!(
        days_of(&core, "2026-10-04", "2026-10-10", "EGR 101"),
        ["2026-10-06", "2026-10-08"]
    );
    let s = snap(&core, DAY);
    assert_eq!(s["commitments"]["pending"][0]["id"], pending[0]["id"]);
    let notice = &s["notices"][0];
    assert_eq!(
        (notice["kind"].as_str(), notice["text"].as_str()),
        (
            Some("exception"),
            Some("EGR 101 is canceled Thursday, Oct 8. Skip it?")
        )
    );
    assert_eq!(
        notice["act"],
        json!({"label": "Skip it", "cmd": "heat.commitment.exception.confirm", "args": {"id": pending[0]["id"]}})
    );
    // Looking again finds nothing new, and doesn't ask twice.
    assert_eq!(
        ok(&core, "heat.commitment.mail.check", json!({}))["pending"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // One tap.
    let done = ok(
        &core,
        "heat.commitment.exception.confirm",
        json!({"id": pending[0]["id"]}),
    );
    assert_eq!(done["undo"], "Undo skip class");
    assert_eq!(
        days_of(&core, "2026-10-04", "2026-10-10", "EGR 101"),
        ["2026-10-06"]
    );
    let s = snap(&core, DAY);
    assert!(
        s["commitments"]["pending"].as_array().unwrap().is_empty()
            && s["notices"].as_array().unwrap().is_empty()
    );
    let kept = records(&s, "commitment")[0]["exceptions"][0].clone();
    assert_eq!(
        (kept["date"].as_str(), kept["source"].as_str()),
        (Some("2026-10-08"), Some("mail"))
    );
    // ⌘Z puts the class back, and the mail isn't offered a second time.
    ok(&core, "history.undo", json!({"room": "heat"}));
    assert_eq!(
        days_of(&core, "2026-10-04", "2026-10-10", "EGR 101"),
        ["2026-10-06", "2026-10-08"]
    );
    assert!(
        ok(&core, "heat.commitment.mail.check", json!({}))["pending"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    // "Not now" leaves the class alone for good.
    record_mail(
        &setup,
        "th-move",
        "EGR 101 moved",
        "EGR 101",
        "2026-10-07T08:30:00-04:00",
        "Tuesday's class is moved to 2 PM.",
    );
    let moved = ok(&core, "heat.commitment.mail.check", json!({}));
    assert_eq!(
        moved["pending"][0]["line"],
        "EGR 101 on Tuesday, Oct 13 moves to 2:00 PM. Move it?"
    );
    ok(
        &core,
        "heat.commitment.exception.dismiss",
        json!({"id": moved["pending"][0]["id"]}),
    );
    assert!(
        ok(&core, "heat.commitment.mail.check", json!({}))["pending"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(records(&snap(&core, DAY), "commitment")[0]["exceptions"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn commitments_stay_on_this_mac() {
    let setup = Setup::new();
    let server = MockServer::start();
    let core = sign_in(&setup, &server);
    pin(&core, NOW);
    jpn(&core);
    ok(
        &core,
        "heat.put",
        json!({"kind": "termBreak", "record": {"title": "Columbus Day", "from": "2026-10-12"}}),
    );
    let space = records(&snap(&core, DAY), "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    add_task(&core, &space, "Goes up", json!({}));
    core.sync_heat().unwrap();
    let state = server.state().to_string();
    assert!(state.contains("Goes up"), "the task synced");
    assert!(
        !state.contains("commitment")
            && !state.contains("Columbus")
            && !state.contains("termBreak"),
        "a commitment went up"
    );
}

#[test]
fn the_prompt_boxs_tools_stage_a_change_and_run_what_only_reads() {
    let setup = Setup::new();
    let core = heat_core(&setup, NOW);
    school(&core);
    let tools = ok(&core, "heat.tools.list", json!({}))["tools"].clone();
    assert_eq!(tools.as_array().unwrap().len(), 12);
    assert_eq!(tools[0]["mcpName"], "commitment_create");
    // "I work Saturday 9 to 5." Staged: checked, described, not written.
    let work = json!({"title": "Bookstore", "kind": "work", "date": "2026-10-10", "start": "9am", "end": "5pm"});
    let staged = ok(
        &core,
        "heat.tools.call",
        json!({"name": "commitment.create", "args": work, "stage": true}),
    );
    assert_eq!(
        staged["staged"]["line"],
        "New commitment: Bookstore, Sat Oct 10, 9:00 AM to 5:00 PM."
    );
    assert_eq!(staged["staged"]["cmd"], "heat.commitment.create");
    assert!(records(&snap(&core, DAY), "commitment").is_empty());
    // Applied, it is the command it named, with the arguments it gave.
    let applied = ok(
        &core,
        "heat.commitment.create",
        staged["staged"]["args"].clone(),
    );
    assert_eq!(applied["undo"], "Undo add commitment");
    // What can't be done is refused when it is staged, in the core's own sentence.
    let bad = json!({"title": "Backwards", "start": "17:00", "end": "09:00"});
    assert_eq!(
        refused(
            &core,
            "heat.tools.call",
            json!({"name": "commitment_create", "args": bad, "stage": true})
        )
        .1,
        "A commitment ends after it starts. One that runs past midnight is two."
    );
    let skip = ok(
        &core,
        "heat.tools.call",
        json!({"name": "commitment.add_exception", "args": {"commitment": "Bookstore", "date": "2026-10-10", "kind": "skip"}, "stage": true}),
    );
    assert_eq!(
        skip["staged"]["line"],
        "Bookstore on Saturday, Oct 10 is skipped."
    );
    let change = ok(
        &core,
        "heat.tools.call",
        json!({"name": "commitment.update", "args": {"commitment": "Bookstore", "set": {"end": "4pm", "buffer_before_min": 20}}, "stage": true}),
    );
    assert_eq!(
        change["staged"]["line"],
        "Change to: Bookstore, Sat Oct 10, 9:00 AM to 4:00 PM."
    );
    assert_eq!(change["staged"]["args"]["set"]["bufferBefore"], 20);
    // A tool that reads runs at once, staged or not.
    let free = ok(
        &core,
        "heat.tools.call",
        json!({"name": "planner.free_time", "args": {"date": "2026-10-10"}, "stage": true}),
    );
    assert_eq!(free["result"]["days"][0]["line"], "8h free Saturday.");
    // "Find my notes on te-form." "Move this note to EGR 101."
    ok(
        &core,
        "heat.commitment.create",
        json!({"course": "EGR 101", "days": ["TU"], "start": "11:00", "end": "12:15"}),
    );
    let made = ok(
        &core,
        "heat.tools.call",
        json!({"name": "note.create", "args": {"title": "Te-form", "markdown": "Group one verbs."}}),
    );
    assert_eq!(made["result"]["note"]["title"], "Te-form");
    let found = ok(
        &core,
        "heat.tools.call",
        json!({"name": "note.search", "args": {"query": "te-form"}}),
    );
    assert_eq!(found["result"]["hits"][0]["title"], "Te-form");
    let moved = ok(
        &core,
        "heat.tools.call",
        json!({"name": "note.file", "args": {"note": "Te-form", "course": "EGR 101"}, "stage": true}),
    );
    assert_eq!(moved["staged"]["line"], "Te-form: file to EGR 101");
    assert_eq!(
        ok(&core, "heat.note.file", moved["staged"]["args"].clone())["line"],
        "Filed to EGR 101"
    );
    let linked = ok(
        &core,
        "heat.tools.call",
        json!({"name": "note.link", "args": {"note": "Te-form", "to": "EGR 101"}}),
    );
    assert_eq!(linked["result"]["line"], "Linked to EGR 101");
    assert_eq!(
        refused(
            &core,
            "heat.tools.call",
            json!({"name": "note.file", "args": {"note": "Nowhere", "course": "EGR 101"}})
        )
        .1,
        "No note is called Nowhere."
    );
    assert_eq!(
        refused(
            &core,
            "heat.tools.call",
            json!({"name": "note.delete", "args": {}})
        )
        .0,
        "unknown_tool"
    );
    // A calendar file read as one week's shifts keeps to that week.
    let week = ok(
        &core,
        "heat.commitment.importIcs",
        json!({"text": TIMETABLE, "mode": "week", "weekOf": "2026-10-07"}),
    );
    assert_eq!(
        week["draft"]["line"], "1 shift. For the week of Oct 5. 1 line couldn't be read.",
        "the shift made by hand isn't a week's import, so it isn't replaced"
    );
}

#[test]
fn time_to_leave_is_said_once_when_the_travel_time_starts() {
    let setup = Setup::new();
    let mut config = setup.config(NO_SERVER, no_browser());
    config.now = Some(ny("2026-10-07 09:00"));
    config.leave_notices = true;
    let core = Core::open(&setup.library(), config).unwrap();
    ok(
        &core,
        "app.settings.set",
        json!({"patch": {"heat": {"timeZone": "America/New_York"}}}),
    );
    school(&core);
    jpn(&core);
    let leave = |core: &Core| -> Vec<Value> {
        snap(core, DAY)["notices"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["kind"] == "leave")
            .cloned()
            .collect()
    };
    // An hour ahead there is nothing to say.
    std::thread::sleep(Duration::from_millis(2500));
    assert!(leave(&core).is_empty());
    // 9:36: the 25 minutes of travel have started.
    core.set_now(Some(ny("2026-10-07 09:36")));
    // Any write wakes the worker; left alone it looks every twenty seconds.
    ok(
        &core,
        "heat.put",
        json!({"kind": "termBreak", "record": {"title": "Reading day", "from": "2026-12-12"}}),
    );
    assert!(
        eventually(Duration::from_secs(30), || !leave(&core).is_empty()),
        "it was never time to leave"
    );
    let said = leave(&core);
    assert_eq!(said.len(), 1);
    assert_eq!(
        said[0]["text"],
        "Time to leave for JPN 101 (10:00 AM, Swan Hall 201)."
    );
    // Dismissed, it isn't said again for that class that day.
    ok(&core, "heat.notice.dismiss", json!({"id": said[0]["id"]}));
    ok(&core, "heat.sleep.set", json!({"from": 1380, "to": 420}));
    std::thread::sleep(Duration::from_millis(4500));
    assert!(leave(&core).is_empty(), "said twice");
    // A core that wasn't told to never says it.
    let quiet = Setup::new();
    let other = heat_core(&quiet, "2026-10-07 09:36");
    school(&other);
    jpn(&other);
    std::thread::sleep(Duration::from_millis(2500));
    assert!(leave(&other).is_empty());
}
