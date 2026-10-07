//! The MCP helper next to the app (docs/SPEC.md 8.8; PLAN S2.8's app half and
//! S2.11's Claude half). `wi-mcp` is a second process writing the library the
//! app has open. What a fail looks like:
//! - the app doesn't notice a change the helper made while it is open, or
//!   tells the views about its own changes twice;
//! - Claude's change isn't labelled as Claude's in the Edit menu, or Settings →
//!   Claude doesn't list it with its reason and an Undo that works;
//! - Undo takes back a change that something later changed on top of;
//! - a tool switched off in Settings is still offered or answers;
//! - a change the helper made, open or closed, doesn't go up when the app syncs;
//! - interleaved writes from both lose one, or the app doesn't show each;
//! - Settings → Claude's lines aren't the exact ones with the helper's path;
//! - the app can't start and run Heat without the audio engine.

use std::sync::Arc;
use std::time::Duration;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

const T: Duration = Duration::from_secs(5);

fn heat_event(rx: &std::sync::mpsc::Receiver<wi_core::Event>) -> Vec<String> {
    let e = wait_event(rx, "heat", T);
    e.payload["kinds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn the_app_notices_what_the_helper_writes_while_it_is_open_and_not_its_own() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    snap(&core, "2026-10-07");
    let events = core.events();
    let mut helper = McpHelper::start(core.library());

    // The helper adds a task. The app tells the views, with the kind, and the
    // Edit menu reads it as Claude's.
    let added = helper.call("add_task", json!({"title": "Grammar quiz 4", "due": "2026-10-09T23:59:00-04:00", "source_id": "gm-1", "reason": "The notice gives a Friday deadline."})).unwrap();
    assert_eq!(added["undo_label"], "Undo Claude's task");
    assert_eq!(heat_event(&events), ["task"]);
    let menu = loop {
        let e = wait_event(&events, "history", T);
        if e.payload["room"] == "heat" && e.payload["undo"] == "Undo Claude's task" {
            break e;
        }
    };
    assert_eq!(menu.payload["redo"], Value::Null);
    assert_eq!(
        ok(&core, "history.get", json!({"room": "heat"}))["undo"],
        "Undo Claude's task"
    );
    let shown = snap(&core, "2026-10-07");
    let task = records(&shown, "task")
        .into_iter()
        .find(|t| t["sourceId"] == "gm-1")
        .unwrap();
    assert_eq!(
        (task["source"].as_str(), task["claudeReason"].as_str()),
        (Some("claude"), Some("The notice gives a Friday deadline."))
    );
    assert_eq!(
        shown["derived"]["tasks"][task["id"].as_str().unwrap()]["heat"]["level"],
        "Warm"
    );

    // The app's own change is announced once, by the app, and not again by the watcher.
    std::thread::sleep(Duration::from_millis(1300));
    drain(&events, "heat");
    let space = records(&shown, "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    add_task(&core, &space, "My own", json!({}));
    std::thread::sleep(Duration::from_millis(1300));
    assert_eq!(
        drain(&events, "heat").len(),
        1,
        "one `heat` event for one change of the app's own"
    );
}

#[test]
fn settings_claude_lists_claudes_changes_with_their_reasons_and_undoes_one_out_of_order() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let space = records(&snap(&core, "2026-10-07"), "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let task = add_task(
        &core,
        &space,
        "Essay draft",
        json!({"due": ny("2026-10-12 23:59")}),
    );
    let events = core.events();
    let mut helper = McpHelper::start(core.library());
    helper.call("update_task", json!({"id": task["id"], "difficulty": 4, "estimate_min": 90, "reason": "Two thousand words."})).unwrap();
    heat_event(&events);
    helper
        .call(
            "add_task",
            json!({"title": "Lab 5a", "source_id": "gm-2", "reason": "From the syllabus."}),
        )
        .unwrap();
    heat_event(&events);

    let got = ok(&core, "heat.claude.get", json!({}));
    let recent = got["recent"].as_array().unwrap();
    assert_eq!(
        recent
            .iter()
            .map(|r| r["label"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Claude's task", "Claude's estimate"],
        "newest first"
    );
    assert_eq!(
        (recent[1]["reason"].as_str(), recent[1]["undone"].clone()),
        (Some("Two thousand words."), json!(false))
    );
    assert!(
        recent[0]["at"].as_f64().unwrap() > 1_700_000_000_000.0
            && recent[0]["txnId"].as_str().is_some()
    );

    // The estimate is Claude's and shows as such, with its reason.
    let shown = snap(&core, "2026-10-07");
    assert_eq!(
        shown["derived"]["tasks"][task["id"].as_str().unwrap()]["estimate"],
        json!({"min": 90, "by": "claude", "reason": "Two thousand words.", "typeFrom": null})
    );
    // Undo of Claude's estimate is refused once the person changed the task since.
    ok(
        &core,
        "heat.patch",
        json!({"kind": "task", "id": task["id"], "set": {"notes": "chapter 4"}}),
    );
    let refused_undo = refused(
        &core,
        "history.undoEntry",
        json!({"txnId": recent[1]["txnId"]}),
    );
    assert_eq!(
        refused_undo,
        (
            "cant_undo".to_string(),
            "This changed again since. Undo the later change first.".to_string()
        )
    );
    // Claude's other change, which nothing later touched, undoes out of order.
    let undone = ok(
        &core,
        "history.undoEntry",
        json!({"txnId": recent[0]["txnId"]}),
    );
    assert_eq!(undone["label"], "Claude's task");
    let after = snap(&core, "2026-10-07");
    assert!(records(&after, "task")
        .iter()
        .all(|t| t["sourceId"] != "gm-2"));
    let list = ok(&core, "heat.claude.get", json!({}));
    assert_eq!(list["recent"][0]["undone"], true);
    assert_eq!(
        refused(
            &core,
            "history.undoEntry",
            json!({"txnId": recent[0]["txnId"]})
        )
        .1,
        "Claude's task is already undone."
    );
    assert_eq!(
        refused(&core, "history.undoEntry", json!({"txnId": "nope"})).1,
        "That change isn't in the journal any more."
    );
    // ⌘Z still works on the person's own changes, in order.
    assert_eq!(
        ok(&core, "history.undo", json!({"room": "heat"}))["label"],
        "edit task"
    );
    assert_eq!(
        snap(&core, "2026-10-07")["derived"]["tasks"][task["id"].as_str().unwrap()]["estimate"]
            ["by"],
        "claude"
    );
    // And now the estimate, which nothing is on top of, undoes from the list.
    assert_eq!(
        ok(
            &core,
            "history.undoEntry",
            json!({"txnId": recent[1]["txnId"]})
        )["label"],
        "Claude's estimate"
    );
}

#[test]
fn a_tool_switched_off_in_settings_is_missing_and_refused_by_the_helper() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    snap(&core, "2026-10-07");
    let mut helper = McpHelper::start(core.library());
    assert_eq!(helper.tools().len(), 21);
    let tools = |core: &Core| -> Vec<(String, bool)> {
        ok(core, "heat.claude.get", json!({}))["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                (
                    t["name"].as_str().unwrap().to_string(),
                    t["on"].as_bool().unwrap(),
                )
            })
            .collect()
    };
    let before = tools(&core);
    assert_eq!(before.len(), 21);
    assert!(
        before.iter().all(|(_, on)| *on),
        "every tool is on until switched off"
    );
    assert_eq!(before[0].0, "list_tasks");
    assert_eq!(
        ok(
            &core,
            "heat.claude.setTool",
            json!({"name": "log_focus", "on": false})
        ),
        json!({})
    );
    assert!(tools(&core).contains(&("log_focus".to_string(), false)));
    assert!(
        !helper.tools().contains(&"log_focus".to_string()),
        "a tool switched off is missing from the list"
    );
    assert_eq!(
        helper
            .call(
                "log_focus",
                json!({"task_id": "x", "minutes": 5, "reason": "r"})
            )
            .unwrap_err(),
        "This tool is switched off in Wi_WWAV."
    );
    ok(
        &core,
        "heat.claude.setTool",
        json!({"name": "log_focus", "on": true}),
    );
    assert!(helper.tools().contains(&"log_focus".to_string()));
    assert_eq!(
        refused(
            &core,
            "heat.claude.setTool",
            json!({"name": "delete_everything", "on": true})
        )
        .1,
        "There's no tool called delete_everything."
    );
}

#[test]
fn s2_11_settings_claude_shows_the_exact_lines_with_the_helpers_real_path() {
    let setup = Setup::new();
    let dir = setup
        .dir
        .path()
        .join("Apps")
        .join("Wi_WWAV.app")
        .join("Contents")
        .join("Helpers");
    std::fs::create_dir_all(&dir).unwrap();
    let helper = dir.join("wi-mcp");
    std::fs::write(&helper, b"#!/bin/sh\n").unwrap();
    let mut config = setup.config(NO_SERVER, no_browser());
    config.helper = Some(helper.clone());
    let core = Core::open(&setup.library(), config).unwrap();
    let got = ok(&core, "heat.claude.get", json!({}));
    let path = helper.display().to_string();
    assert_eq!(got["helper"], path.as_str());
    assert_eq!(
        got["desktop"],
        format!(
            "{{ \"mcpServers\": {{ \"wi-wwav\": {{ \"command\": {} }} }} }}",
            serde_json::to_string(&path).unwrap()
        )
    );
    assert_eq!(
        got["code"],
        format!("claude mcp add --scope user wi-wwav -- {path}")
    );
    assert!(
        got.get("note").is_none(),
        "the helper is there, so nothing needs building"
    );
    assert_eq!(got["tools"].as_array().unwrap().len(), 21);
    drop(core);

    // A path with a space is quoted in the shell line, and escaped in the JSON.
    let spaced = setup.dir.path().join("My Apps").join("wi-mcp");
    std::fs::create_dir_all(spaced.parent().unwrap()).unwrap();
    std::fs::write(&spaced, b"x").unwrap();
    let mut config = setup.config(NO_SERVER, no_browser());
    config.helper = Some(spaced.clone());
    let core = Core::open(&setup.library(), config).unwrap();
    let got = ok(&core, "heat.claude.get", json!({}));
    assert_eq!(
        got["code"],
        format!(
            "claude mcp add --scope user wi-wwav -- '{}'",
            spaced.display()
        )
    );
    drop(core);

    // No helper where the lines point: they still read, with one sentence on how to build it.
    let mut config = setup.config(NO_SERVER, no_browser());
    config.helper = Some(setup.dir.path().join("target/debug/wi-mcp"));
    let core = Core::open(&setup.library(), config).unwrap();
    let got = ok(&core, "heat.claude.get", json!({}));
    assert_eq!(got["note"], "Build the helper first: cargo build -p wi-mcp");
    assert!(got["code"]
        .as_str()
        .unwrap()
        .ends_with("target/debug/wi-mcp"));
}

#[test]
fn a_change_the_helper_made_goes_up_when_the_app_syncs_open_or_closed() {
    let server = MockServer::start();
    let setup = Setup::new();
    let core = sign_in(&setup, &server);
    snap(&core, "2026-10-07");
    let events = core.events();
    let mut helper = McpHelper::start(core.library());
    helper
        .call(
            "add_task",
            json!({"title": "While open", "source_id": "open-1", "reason": "r"}),
        )
        .unwrap();
    heat_event(&events);
    core.sync_heat().unwrap();
    let uploaded = |title: &str| -> bool {
        let token: Value = serde_json::from_str(
            &wi_core::SecretStore::get(&*setup.secrets, "mi-wwav.com account")
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let r = ureq::get(&format!(
            "{}/api/heat/changes?cursor=0&limit=500",
            server.url
        ))
        .set(
            "Authorization",
            &format!("Bearer {}", token["access"].as_str().unwrap()),
        )
        .call()
        .unwrap();
        let body: Value = serde_json::from_str(&r.into_string().unwrap()).unwrap();
        body["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["field"] == "title" && c["value"] == title)
    };
    assert!(
        uploaded("While open"),
        "the helper has no network: the app carried it up"
    );

    // Written while the app was closed: it goes up when the app next opens.
    drop(core);
    helper
        .call(
            "add_task",
            json!({"title": "While closed", "source_id": "closed-1", "reason": "r"}),
        )
        .unwrap();
    let core = setup.core_on(&server.url);
    // The core finds what was written behind its back as it opens, and the next sync carries it.
    assert!(
        eventually(Duration::from_secs(10), || {
            core.sync_heat().unwrap();
            uploaded("While closed")
        }),
        "a change made while the app was closed never went up"
    );
    // The estimate Claude sets reaches the server field by field.
    let id = records(&snap(&core, "2026-10-07"), "task")
        .into_iter()
        .find(|t| t["sourceId"] == "closed-1")
        .unwrap()["id"]
        .clone();
    helper
        .call(
            "update_task",
            json!({"id": id, "difficulty": 5, "reason": "Hard."}),
        )
        .unwrap();
    let events = core.events();
    heat_event(&events);
    core.sync_heat().unwrap();
    let token: Value = serde_json::from_str(
        &wi_core::SecretStore::get(&*setup.secrets, "mi-wwav.com account")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let r = ureq::get(&format!(
        "{}/api/heat/changes?cursor=0&limit=500",
        server.url
    ))
    .set(
        "Authorization",
        &format!("Bearer {}", token["access"].as_str().unwrap()),
    )
    .call()
    .unwrap();
    let body: Value = serde_json::from_str(&r.into_string().unwrap()).unwrap();
    assert!(body["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == id && c["field"] == "difficulty" && c["value"] == 5));
}

#[test]
fn s2_8_interleaved_writes_from_the_app_and_the_helper_lose_none_and_the_app_shows_each() {
    let setup = Setup::new();
    let core = Arc::new(heat_core(&setup, "2026-10-07 09:00"));
    let space = records(&snap(&core, "2026-10-07"), "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let events = core.events();
    let mut helper = McpHelper::start(core.library());
    let app = {
        let (core, space) = (core.clone(), space.clone());
        std::thread::spawn(move || {
            for n in 0..150 {
                add_task(&core, &space, &format!("app {n}"), json!({}));
            }
        })
    };
    for n in 0..150 {
        helper.call("add_task", json!({"title": format!("claude {n}"), "source_id": format!("mail-{n}"), "reason": "r"})).unwrap();
    }
    app.join().unwrap();
    let shown = snap(&core, "2026-10-07");
    let tasks = records(&shown, "task");
    assert_eq!(tasks.len(), 300, "none lost");
    assert_eq!(
        tasks.iter().filter(|t| t["source"] == "claude").count(),
        150
    );
    let entries = ok(&core, "heat.claude.get", json!({}))["recent"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(
        entries, 50,
        "Settings lists the newest 50 of Claude's 150 entries"
    );
    // The app showed each change as it was made: the last `heat` event is not before the last write.
    std::thread::sleep(Duration::from_millis(1300));
    assert!(!drain(&events, "heat").is_empty());
    let store = wi_store::Store::open(core.library()).unwrap();
    let all = store.entries(false, 10_000).unwrap();
    assert_eq!(
        all.iter().filter(|e| e.label == "Claude's task").count(),
        150
    );
    assert_eq!(all.iter().filter(|e| e.label == "add task").count(), 150);
}

#[test]
fn heat_runs_whole_with_no_audio_engine_and_the_engine_says_so_once() {
    let setup = Setup::new();
    let mut config = setup.config(NO_SERVER, no_browser());
    config.engine_path = setup
        .dir
        .path()
        .join("Helpers/wwav-engine.app/Contents/MacOS/wwav-engine");
    let core = Core::open(&setup.library(), config).unwrap();
    let events = core.events();
    pin(&core, "2026-10-07 09:00");

    // The engine never starts: stopped, with its sentence, and it stays so.
    assert!(eventually(Duration::from_secs(12), || ok(
        &core,
        "engine.status",
        json!({})
    )["state"]
        == "stopped"));
    let status = ok(&core, "engine.status", json!({}));
    let why = status["why"].as_str().unwrap();
    assert!(why.starts_with("It couldn't start: "), "{why}");
    assert_eq!(
        (status["device"].clone(), status["sampleRate"].clone()),
        (Value::Null, Value::Null)
    );
    let seen = drain(&events, "engine").len();
    assert!(seen >= 1, "the status was announced");
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(
        ok(&core, "engine.status", json!({}))["state"],
        "stopped",
        "no restart loop"
    );
    assert_eq!(
        drain(&events, "engine").len(),
        0,
        "and nothing more said about it"
    );

    // Every Heat command works anyway.
    let space = records(&snap(&core, "2026-10-07"), "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let task = add_task(
        &core,
        &space,
        "Mix the second verse",
        json!({"due": ny("2026-10-07 17:00"), "estMin": 45}),
    );
    let id = task["id"].as_str().unwrap();
    ok(
        &core,
        "heat.estimate",
        json!({"taskId": id, "difficulty": 2}),
    );
    ok(&core, "heat.plan.make", json!({"date": "2026-10-07"}));
    ok(&core, "heat.plan.accept", json!({"date": "2026-10-07"}));
    ok(&core, "heat.focus.start", json!({"taskId": id}));
    pin(&core, "2026-10-07 09:25");
    ok(&core, "heat.focus.stop", json!({}));
    ok(
        &core,
        "heat.capture.add",
        json!({"text": "fix the snare at 1:32"}),
    );
    ok(
        &core,
        "heat.share.now",
        json!({"taskId": id, "text": "Now making: the verse"}),
    );
    ok(
        &core,
        "heat.public.set",
        json!({"kind": "task", "id": id, "public": true}),
    );
    assert_eq!(
        ok(&core, "heat.publicView", json!({}))["items"]["task"][0]["title"],
        "Mix the second verse"
    );
    ok(
        &core,
        "heat.review.week",
        json!({"weekStart": "2026-09-30"}),
    );
    ok(&core, "heat.claude.get", json!({}));
    ok(&core, "heat.done", json!({"taskId": id, "done": true}));
    let shown = snap(&core, "2026-10-07");
    assert_eq!(shown["derived"]["tasks"][id]["actualMin"], 25);
    assert_eq!(
        ok(&core, "history.undo", json!({"room": "heat"}))["label"],
        "mark done"
    );
    let export = setup.dir.path().join("Export");
    ok(
        &core,
        "export.everything",
        json!({"to": export, "zip": false}),
    );
    assert!(export.join("heat.json").exists());
}
