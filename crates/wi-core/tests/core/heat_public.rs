//! Public and private (docs/SPEC.md 3.15, 8.7; PLAN S2.9 and S2.11's Privacy
//! half), against tools/mock-server. What a fail looks like:
//! - a private record, grade or note appears in `/api/heat/public/:userId` or
//!   in the preview, or a private grade or course leaves the Mac at all;
//! - a public record shows more than the fields 3.15 lists, or a public grade
//!   puts more than its copy on the server;
//! - switching it back doesn't remove its copy at the next sync;
//! - a Now making line outlives its `clearsAt`, with the Mac off or on;
//! - a response carries a count or a total;
//! - a grade's switch doesn't say what it does;
//! - the preview and what the server serves are two different things;
//! - Settings → Privacy can't list every public item with its switch.

use std::collections::BTreeSet;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::{Core, SecretStore};

fn token(setup: &Setup) -> String {
    let saved: Value = serde_json::from_str(
        &setup
            .secrets
            .get("mi-wwav.com account")
            .unwrap()
            .expect("signed in"),
    )
    .unwrap();
    saved["access"].as_str().unwrap().to_string()
}

/// What the server holds of this account's Heat, field by field.
fn held(server: &MockServer, setup: &Setup) -> Vec<Value> {
    let r = ureq::get(&format!(
        "{}/api/heat/changes?cursor=0&limit=500",
        server.url
    ))
    .set("Authorization", &format!("Bearer {}", token(setup)))
    .call()
    .unwrap();
    let body: Value = serde_json::from_str(&r.into_string().unwrap()).unwrap();
    body["changes"].as_array().unwrap().clone()
}

fn public_view(server: &MockServer) -> Value {
    let (status, body) = server.call("GET", "/api/heat/public/1", None);
    assert_eq!(status, 200);
    body
}

/// Every number in an answer, with where it sits, so a count can't hide.
fn numbers(v: &Value, path: &str, out: &mut Vec<String>) {
    match v {
        Value::Number(_) => out.push(path.to_string()),
        Value::Array(a) => a
            .iter()
            .enumerate()
            .for_each(|(i, x)| numbers(x, &format!("{path}[{i}]"), out)),
        Value::Object(m) => m
            .iter()
            .for_each(|(k, x)| numbers(x, &format!("{path}.{k}"), out)),
        _ => {}
    }
}

/// An answer with its list items in id order, so two can be compared.
fn tidy(mut v: Value) -> Value {
    if let Some(items) = v["items"].as_object_mut() {
        for list in items.values_mut() {
            if let Some(a) = list.as_array_mut() {
                a.sort_by_key(|x| x["id"].as_str().unwrap_or("").to_string());
            }
        }
    }
    v
}

struct Mac {
    setup: Setup,
    server: MockServer,
    core: Core,
}

/// A signed-in core with a space, a course and a task or two of each kind.
fn mac() -> (Mac, Value) {
    let server = MockServer::start();
    let setup = Setup::new();
    let core = sign_in(&setup, &server);
    pin_real_zone(&core);
    let space = records(&snap(&core, "2026-10-07"), "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let ids = json!({"space": space});
    (
        Mac {
            setup,
            server,
            core,
        },
        ids,
    )
}

fn pin_real_zone(core: &Core) {
    ok(
        core,
        "app.settings.set",
        json!({"patch": {"heat": {"timeZone": "America/New_York"}}}),
    );
}

#[test]
fn s2_9_a_private_record_never_reaches_the_public_view_and_a_public_one_shows_only_its_fields() {
    let (m, ids) = mac();
    let (core, space) = (&m.core, ids["space"].clone());
    let secret = add_task(
        core,
        space.as_str().unwrap(),
        "Therapy",
        json!({"notes": "private", "difficulty": 3}),
    );
    let shown = add_task(
        core,
        space.as_str().unwrap(),
        "Mix the second verse",
        json!({"due": 1_791_500_000_000_i64, "notes": "not this", "difficulty": 4, "estMin": 90}),
    );
    let diary = ok(
        core,
        "heat.put",
        json!({"kind": "note", "record": {"title": "Diary", "markdown": "not shown"}}),
    )["record"]
        .clone();
    let open_note = ok(
        core,
        "heat.put",
        json!({"kind": "note", "record": {"title": "Liner notes", "markdown": "Two guitars."}}),
    )["record"]
        .clone();
    let habit = ok(core, "heat.put", json!({"kind": "habit", "record": {"title": "Scales", "log": {"2026-10-05": true, "2026-10-06": true}, "showCounter": true}}))["record"].clone();
    ok(
        core,
        "heat.public.set",
        json!({"kind": "task", "id": shown["id"], "public": true}),
    );
    ok(
        core,
        "heat.public.set",
        json!({"kind": "note", "id": open_note["id"], "public": true}),
    );
    ok(
        core,
        "heat.public.set",
        json!({"kind": "habit", "id": habit["id"], "public": true}),
    );
    core.sync_heat().unwrap();

    let body = public_view(&m.server);
    assert_eq!(
        body["items"]["task"],
        json!([{"id": shown["id"], "title": "Mix the second verse", "due": 1_791_500_000_000_i64, "done": false}])
    );
    assert_eq!(
        body["items"]["note"],
        json!([{"id": open_note["id"], "title": "Liner notes", "markdown": "Two guitars."}])
    );
    assert_eq!(
        body["items"]["habit"],
        json!([{"id": habit["id"], "title": "Scales", "log": {"2026-10-05": true, "2026-10-06": true}}])
    );
    let text = body.to_string();
    for private in ["Therapy", "Diary", "not shown", "not this", "private"] {
        assert!(!text.contains(private), "'{private}' is in the public view");
    }
    // The preview is the same thing, as the person would see it.
    assert_eq!(
        tidy(ok(core, "heat.publicView", json!({}))),
        tidy(body.clone())
    );
    let _ = (secret, diary);

    // Switching back removes the copy at the next sync, and public stays the person's own switch.
    ok(
        core,
        "heat.public.set",
        json!({"kind": "task", "id": shown["id"], "public": false}),
    );
    ok(
        core,
        "heat.public.set",
        json!({"kind": "note", "id": open_note["id"], "public": false}),
    );
    core.sync_heat().unwrap();
    let after = public_view(&m.server);
    assert!(after["items"].get("task").is_none() && after["items"].get("note").is_none());
    assert_eq!(tidy(ok(core, "heat.publicView", json!({}))), tidy(after));
}

#[test]
fn s2_9_a_private_grade_never_leaves_the_mac_and_a_public_one_goes_up_as_its_copy() {
    let (m, ids) = mac();
    let (core, space) = (&m.core, ids["space"].as_str().unwrap().to_string());
    let term = ok(
        core,
        "heat.put",
        json!({"kind": "term", "record": {"name": "Fall 2026"}}),
    )["record"]["id"]
        .clone();
    let course = ok(core, "heat.put", json!({"kind": "course", "record": {"termId": term, "code": "JPN 201", "name": "Japanese 2"}}))["record"].clone();
    let quizzes = ok(core, "heat.patch", json!({"kind": "course", "id": course["id"], "set": {"categories": [{"name": "Quizzes", "weight": 100, "keywords": ["quiz"]}]}}))["record"]["categories"][0]["id"].clone();
    let private = ok(core, "heat.put", json!({"kind": "grade", "record": {"courseId": course["id"], "categoryId": quizzes, "title": "Quiz 3 (private)", "score": 70, "outOf": 100, "link": "https://brightspace.uri.edu/x"}}))["record"].clone();
    let open = ok(core, "heat.put", json!({"kind": "grade", "record": {"courseId": course["id"], "categoryId": quizzes, "title": "Quiz 4", "score": 9, "outOf": 10, "link": "https://brightspace.uri.edu/y"}}))["record"].clone();
    let _ = space;
    core.sync_heat().unwrap();
    let rows = held(&m.server, &m.setup);
    assert!(
        rows.iter()
            .all(|c| c["kind"] != "grade" && c["kind"] != "course"),
        "a private grade or course left the Mac"
    );
    assert!(
        rows.iter().any(|c| c["kind"] == "term"),
        "the rest of Learn syncs"
    );

    // The switch says what it does, in the words of 3.15.
    let on = ok(
        core,
        "heat.public.set",
        json!({"kind": "grade", "id": open["id"], "public": true}),
    );
    assert_eq!(on["undo"], "Undo make public");
    assert_eq!(
        on["sentence"],
        "Grades are private by default. Your school keeps them as education records. Turning this on shows this grade to anyone who opens your sun."
    );
    core.sync_heat().unwrap();
    let body = public_view(&m.server);
    assert_eq!(
        body["items"]["grade"],
        json!([{"id": open["id"], "title": "Quiz 4", "score": 9, "outOf": 10, "course": "JPN 201"}])
    );
    assert!(
        body["items"].get("course").is_none(),
        "the course stays private"
    );
    assert!(
        !body.to_string().contains("private") && !body.to_string().contains("brightspace.uri.edu")
    );
    // On the server a public grade is its copy and nothing more of the grade.
    let copy: BTreeSet<String> = held(&m.server, &m.setup)
        .iter()
        .filter(|c| c["kind"] == "grade" && c["id"] == open["id"])
        .map(|c| c["field"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        copy,
        ["courseId", "outOf", "public", "score", "title"]
            .map(String::from)
            .into_iter()
            .collect()
    );
    assert!(
        held(&m.server, &m.setup)
            .iter()
            .all(|c| c["id"] != private["id"]),
        "the private grade never went up"
    );
    // The course went up as its code and name alone, because the public grade names it: not its weights, scale or term.
    let named: BTreeSet<String> = held(&m.server, &m.setup)
        .iter()
        .filter(|c| c["kind"] == "course" && c["id"] == course["id"])
        .map(|c| c["field"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        named,
        ["code", "name"].map(String::from).into_iter().collect()
    );
    assert_eq!(tidy(ok(core, "heat.publicView", json!({}))), tidy(body));
    // A score typed afterwards reaches the copy.
    ok(
        core,
        "heat.score",
        json!({"gradeId": open["id"], "score": 10}),
    );
    core.sync_heat().unwrap();
    assert_eq!(public_view(&m.server)["items"]["grade"][0]["score"], 10);

    // Switched back, its copy is deleted at the next sync, and it comes back when switched on.
    assert_eq!(
        ok(
            core,
            "heat.public.set",
            json!({"kind": "grade", "id": open["id"], "public": false})
        )["undo"],
        "Undo make private"
    );
    core.sync_heat().unwrap();
    assert!(public_view(&m.server)["items"].get("grade").is_none());
    assert_eq!(ok(core, "heat.publicView", json!({}))["items"], json!({}));
    // With no public grade to name it, the course's code and name come down too.
    assert!(
        held(&m.server, &m.setup)
            .iter()
            .any(|c| c["kind"] == "course" && c["field"] == "deleted" && c["value"] == true),
        "the course was still up with nothing public to name it"
    );
    ok(
        core,
        "heat.public.set",
        json!({"kind": "grade", "id": open["id"], "public": true}),
    );
    core.sync_heat().unwrap();
    assert_eq!(public_view(&m.server)["items"]["grade"][0]["score"], 10);
    assert_eq!(
        public_view(&m.server)["items"]["grade"][0]["course"],
        "JPN 201"
    );

    // A course has its own switch: its code and name, and nothing of its grades or weights.
    ok(
        core,
        "heat.public.set",
        json!({"kind": "course", "id": course["id"], "public": true}),
    );
    core.sync_heat().unwrap();
    let shown_course = public_view(&m.server);
    assert_eq!(
        shown_course["items"]["course"],
        json!([{"id": course["id"], "code": "JPN 201", "name": "Japanese 2"}])
    );
    let up: BTreeSet<String> = held(&m.server, &m.setup)
        .iter()
        .filter(|c| c["kind"] == "course" && c["id"] == course["id"])
        .map(|c| c["field"].as_str().unwrap().to_string())
        .collect();
    assert!(
        ["categories", "scale", "termId", "weights"]
            .iter()
            .all(|f| !up.contains(*f)),
        "more than the copy went up: {up:?}"
    );
    assert_eq!(
        tidy(ok(core, "heat.publicView", json!({}))),
        tidy(shown_course)
    );
    ok(
        core,
        "heat.public.set",
        json!({"kind": "course", "id": course["id"], "public": false}),
    );
    core.sync_heat().unwrap();
    assert!(
        public_view(&m.server)["items"].get("course").is_none(),
        "switched back, the course's copy is gone"
    );
    assert_eq!(
        public_view(&m.server)["items"]["grade"][0]["course"],
        "JPN 201",
        "the public grade still names it"
    );

    // Deleting a public grade removes its copy too, and the course with it.
    ok(
        core,
        "heat.delete",
        json!({"kind": "grade", "id": open["id"]}),
    );
    core.sync_heat().unwrap();
    assert!(public_view(&m.server)["items"].get("grade").is_none());
    assert!(held(&m.server, &m.setup)
        .iter()
        .any(|c| c["kind"] == "course" && c["field"] == "deleted" && c["value"] == true));
    // And a second Mac of the same account never receives a grade or a course, public or not.
    let air_setup = Setup::new();
    std::fs::create_dir_all(air_setup.dir.path()).unwrap();
    let air = sign_in(&air_setup, &m.server);
    air.sync_heat().unwrap();
    assert!(records(&snap(&air, "2026-10-07"), "grade").is_empty());
    assert!(records(&snap(&air, "2026-10-07"), "course").is_empty());
    assert_eq!(
        records(&snap(&air, "2026-10-07"), "term").len(),
        1,
        "what isn't held back arrives"
    );
}

#[test]
fn s2_9_the_now_making_line_and_a_timeline_show_and_clear() {
    let (m, ids) = mac();
    let (core, space) = (&m.core, ids["space"].as_str().unwrap().to_string());
    let task = add_task(
        core,
        &space,
        "Mix the second verse",
        json!({"notes": "secret notes"}),
    );
    // Nothing is public until Show.
    core.sync_heat().unwrap();
    assert_eq!(
        public_view(&m.server),
        json!({"now": null, "timelines": [], "items": {}})
    );
    let shown = ok(
        core,
        "heat.share.now",
        json!({"taskId": task["id"], "text": "Now making: the second verse of More Love"}),
    );
    assert_eq!(shown["undo"], "Undo show Now making");
    let share = shown["share"].clone();
    let week = 7.0 * 86_400_000.0;
    assert!(
        (share["clearsAt"].as_f64().unwrap() - core_now_ms()).abs() < week + 60_000.0
            && share["clearsAt"].as_f64().unwrap() > core_now_ms() + week - 60_000.0,
        "7 days on"
    );
    core.sync_heat().unwrap();
    assert_eq!(
        public_view(&m.server)["now"],
        json!({"text": "Now making: the second verse of More Love"})
    );
    assert!(
        !public_view(&m.server).to_string().contains("secret notes"),
        "never the task, its notes or its space"
    );
    assert_eq!(
        tidy(ok(core, "heat.publicView", json!({}))),
        tidy(public_view(&m.server))
    );

    // The server stops returning it at its time, with the Mac off; the preview does too.
    let advanced = m.server.call(
        "POST",
        "/__mock/clock",
        Some(json!({"advanceMs": 8.0 * 86_400_000.0})),
    );
    assert_eq!(advanced.0, 200);
    assert_eq!(public_view(&m.server)["now"], Value::Null);
    core.set_now(Some(core_now_ms() + 8.0 * 86_400_000.0));
    assert_eq!(ok(core, "heat.publicView", json!({}))["now"], Value::Null);
    assert!(
        records(&snap(core, "2026-10-14"), "profileShare").is_empty(),
        "and Settings → Privacy no longer lists it"
    );
    // From here the core and the server both stand eight days on.

    // A line clears quietly when its task is done, and hiding it is a change with words.
    let second = ok(
        core,
        "heat.share.now",
        json!({"taskId": task["id"], "text": "Now making: the bridge"}),
    )["share"]
        .clone();
    core.sync_heat().unwrap();
    assert_eq!(
        public_view(&m.server)["now"],
        json!({"text": "Now making: the bridge"})
    );
    ok(
        core,
        "heat.done",
        json!({"taskId": task["id"], "done": true}),
    );
    core.sync_heat().unwrap();
    assert_eq!(public_view(&m.server)["now"], Value::Null);
    ok(core, "history.undo", json!({"room": "heat"}));
    assert_eq!(
        ok(core, "heat.share.hide", json!({"id": second["id"]}))["undo"],
        "Undo hide Now making"
    );
    assert_eq!(
        refused(
            core,
            "heat.share.now",
            json!({"taskId": "nope", "text": "x"})
        )
        .1,
        "Pick an open task to show."
    );
    assert_eq!(
        refused(
            core,
            "heat.share.now",
            json!({"taskId": task["id"], "text": "x".repeat(201)})
        )
        .1,
        "Keep the line to 200 characters."
    );

    // A project's timeline: its beads, with titles, dates and whether each is reached.
    let project = ok(core, "heat.put", json!({"kind": "project", "record": {"spaceId": space, "title": "Low Tide EP", "status": "active"}}))["record"].clone();
    for (title, date, done) in [
        ("Mixed", "2026-11-01", false),
        ("Written", "2026-10-01", true),
    ] {
        ok(
            core,
            "heat.put",
            json!({"kind": "milestone", "record": {"spaceId": space, "projectId": project["id"], "title": title, "date": date, "done": done}}),
        );
    }
    let line = ok(
        core,
        "heat.share.timeline",
        json!({"projectId": project["id"], "targetId": "system-7"}),
    );
    assert_eq!(line["undo"], "Undo show timeline");
    core.sync_heat().unwrap();
    let body = public_view(&m.server);
    let beads = |v: &Value| {
        v["timelines"][0]["milestones"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| {
                (
                    b["title"].as_str().unwrap().to_string(),
                    b["done"].as_bool().unwrap(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        beads(&body),
        [("Written".to_string(), true), ("Mixed".to_string(), false)]
    );
    assert_eq!(
        (
            body["timelines"][0]["title"].as_str(),
            body["timelines"][0]["targetId"].as_str()
        ),
        (Some("Low Tide EP"), Some("system-7"))
    );
    assert_eq!(tidy(ok(core, "heat.publicView", json!({}))), tidy(body));
    assert_eq!(
        ok(core, "heat.share.hide", json!({"id": line["share"]["id"]}))["undo"],
        "Undo hide timeline"
    );
    core.sync_heat().unwrap();
    assert_eq!(public_view(&m.server)["timelines"], json!([]));
}

fn core_now_ms() -> f64 {
    jiff::Timestamp::now().as_millisecond() as f64
}

#[test]
fn s2_9_no_answer_carries_a_count_or_a_total() {
    let (m, ids) = mac();
    let (core, space) = (&m.core, ids["space"].as_str().unwrap().to_string());
    let task = add_task(core, &space, "Mix", json!({"due": 1_791_500_000_000_i64}));
    ok(
        core,
        "heat.public.set",
        json!({"kind": "task", "id": task["id"], "public": true}),
    );
    ok(
        core,
        "heat.share.now",
        json!({"taskId": task["id"], "text": "Now making: mix"}),
    );
    let session = ok(core, "heat.put", json!({"kind": "focusSession", "record": {"taskId": task["id"], "startedAt": 1_791_400_000_000_i64, "focusMin": 25}}))["record"].clone();
    ok(
        core,
        "heat.public.set",
        json!({"kind": "focusSession", "id": session["id"], "public": true}),
    );
    let habit = ok(
        core,
        "heat.put",
        json!({"kind": "habit", "record": {"title": "Scales", "log": {"2026-10-05": true}}}),
    )["record"]
        .clone();
    ok(
        core,
        "heat.public.set",
        json!({"kind": "habit", "id": habit["id"], "public": true}),
    );
    core.sync_heat().unwrap();
    for body in [
        public_view(&m.server),
        ok(core, "heat.publicView", json!({})),
    ] {
        let mut at = Vec::new();
        numbers(&body, "", &mut at);
        let allowed = |p: &String| {
            p.ends_with(".due")
                || p.ends_with(".score")
                || p.ends_with(".outOf")
                || p.ends_with(".startedAt")
                || p.ends_with(".focusMin")
        };
        assert_eq!(
            at.into_iter().filter(|p| !allowed(p)).collect::<Vec<_>>(),
            Vec::<String>::new(),
            "{body}"
        );
        let text = body.to_string().to_lowercase();
        for word in [
            "count", "total", "sum", "average", "rank", "streak", "percent", "letter",
        ] {
            assert!(!text.contains(&format!("\"{word}")), "{word}");
        }
        assert_eq!(
            body["items"]["focusSession"][0]["task"], "Mix",
            "a focus record names its task by title"
        );
    }
    assert_eq!(m.server.call("GET", "/api/heat/public/999", None).0, 404);
}

#[test]
fn s2_9_the_switch_is_the_persons_alone() {
    let (m, ids) = mac();
    let (core, space) = (&m.core, ids["space"].as_str().unwrap().to_string());
    let task = add_task(core, &space, "Mix", json!({}));
    let id = task["id"].as_str().unwrap();
    let said = "The Public switch has its own command.";
    assert_eq!(
        refused(
            core,
            "heat.patch",
            json!({"kind": "task", "id": id, "set": {"public": true}})
        )
        .1,
        said
    );
    assert_eq!(refused(core, "heat.put", json!({"kind": "task", "record": {"id": id, "spaceId": space, "title": "Mix", "public": true}})).1, said);
    // A put that replaces a record leaves its switch as it was.
    ok(
        core,
        "heat.public.set",
        json!({"kind": "task", "id": id, "public": true}),
    );
    let replaced = ok(
        core,
        "heat.put",
        json!({"kind": "task", "record": {"id": id, "spaceId": space, "title": "Mix again", "public": false}}),
    );
    assert_eq!(replaced["record"]["public"], true);
    for kind in [
        "space",
        "term",
        "capture",
        "timeBlock",
        "taskOccurrence",
        "profileShare",
        "calendar",
    ] {
        assert!(
            core.invoke(
                "heat.public.set",
                json!({"kind": kind, "id": "x", "public": true})
            )
            .is_err(),
            "{kind} has no switch"
        );
    }
}

#[test]
fn s2_11_privacy_lists_every_public_item_with_its_switch() {
    let (m, ids) = mac();
    let (core, space) = (&m.core, ids["space"].as_str().unwrap().to_string());
    let task = add_task(core, &space, "Mix", json!({}));
    let private_task = add_task(core, &space, "Therapy", json!({}));
    let note = ok(
        core,
        "heat.put",
        json!({"kind": "note", "record": {"markdown": "Two guitars."}}),
    )["record"]
        .clone();
    let day = ok(
        core,
        "heat.put",
        json!({"kind": "dailyNote", "record": {"date": "2026-10-07", "markdown": "Mixed the bridge."}}),
    );
    assert_eq!(day["undo"], "Undo add daily note");
    let term = ok(
        core,
        "heat.put",
        json!({"kind": "term", "record": {"name": "Fall 2026"}}),
    )["record"]["id"]
        .clone();
    let course = ok(
        core,
        "heat.put",
        json!({"kind": "course", "record": {"termId": term, "code": "JPN 201"}}),
    )["record"]
        .clone();
    let grade = ok(core, "heat.put", json!({"kind": "grade", "record": {"courseId": course["id"], "title": "Quiz 4", "score": 9, "outOf": 10}}))["record"].clone();
    for (kind, id) in [
        ("task", &task["id"]),
        ("note", &note["id"]),
        ("dailyNote", &json!("2026-10-07")),
        ("course", &course["id"]),
        ("grade", &grade["id"]),
    ] {
        ok(
            core,
            "heat.public.set",
            json!({"kind": kind, "id": id, "public": true}),
        );
    }
    ok(
        core,
        "heat.share.now",
        json!({"taskId": task["id"], "text": "Now making: mix"}),
    );
    let shown = snap(core, "2026-10-07");
    let mut public: Vec<(String, String)> = Vec::new();
    for kind in [
        "task",
        "project",
        "milestone",
        "habit",
        "focusSession",
        "course",
        "grade",
        "dailyNote",
        "note",
    ] {
        for r in records(&shown, kind) {
            if r["public"] == true {
                let key = if kind == "dailyNote" {
                    r["date"].clone()
                } else {
                    r["id"].clone()
                };
                public.push((kind.to_string(), key.as_str().unwrap().to_string()));
            }
        }
    }
    public.sort();
    let mut want = vec![
        (
            "course".to_string(),
            course["id"].as_str().unwrap().to_string(),
        ),
        ("dailyNote".to_string(), "2026-10-07".to_string()),
        (
            "grade".to_string(),
            grade["id"].as_str().unwrap().to_string(),
        ),
        ("note".to_string(), note["id"].as_str().unwrap().to_string()),
        ("task".to_string(), task["id"].as_str().unwrap().to_string()),
    ];
    want.sort();
    assert_eq!(public, want, "every public item, and not the private task");
    assert!(records(&shown, "task")
        .iter()
        .any(|t| t["id"] == private_task["id"] && t["public"] == false));
    assert_eq!(
        records(&shown, "profileShare").len(),
        1,
        "the line on show is listed too"
    );
    // Each one has its switch: turning it off takes it from the list.
    ok(
        core,
        "heat.public.set",
        json!({"kind": "dailyNote", "id": "2026-10-07", "public": false}),
    );
    let after = snap(core, "2026-10-07");
    assert_eq!(records(&after, "dailyNote")[0]["public"], false);
    let preview = ok(core, "heat.publicView", json!({}));
    assert!(
        preview["items"].get("dailyNote").is_none()
            && preview["items"]["grade"][0]["course"] == "JPN 201"
    );
}
