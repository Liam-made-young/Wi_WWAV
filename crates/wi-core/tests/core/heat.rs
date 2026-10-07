//! Heat sync through the core, against tools/mock-server (docs/SPEC.md 2.8,
//! 8.7). What a fail looks like:
//! - a record written in Heat on one library doesn't reach another library
//!   of the same account, or comes back as anything but one undoable change;
//! - two devices editing different fields of one record lose one edit;
//! - an undo in Heat doesn't sync, or a grade leaves the machine;
//! - the app calls a model: there is no assist command.

use std::time::Duration;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

fn put(core: &Core, label: &str, kind: &str, id: &str, value: Value) {
    ok(
        core,
        "records.mutate",
        json!({"label": label, "room": "heat",
        "ops": [{"op": "put", "kind": kind, "id": id, "value": value}]}),
    );
}

fn record(core: &Core, kind: &str, id: &str) -> Option<Value> {
    core.invoke("records.get", json!({"kind": kind, "id": id}))
        .ok()
        .map(|r| r["record"].clone())
}

#[test]
fn two_libraries_of_one_account_meet_field_by_field() {
    let server = MockServer::start();
    let (a, b) = (Setup::new(), Setup::new());
    let mac = sign_in(&a, &server);
    let air = sign_in(&b, &server);
    let events = mac.events();

    put(
        &mac,
        "add task",
        "task",
        "t1",
        json!({"title": "Grammar quiz 4", "done": false, "notes": ""}),
    );
    let synced = mac.sync_heat().unwrap();
    let sentence = synced["sentence"].as_str().unwrap();
    let (synced_at, ampm) = sentence
        .strip_prefix("Synced ")
        .unwrap()
        .rsplit_once(' ')
        .unwrap();
    assert!(
        ["AM", "PM"].contains(&ampm) && synced_at.contains(':'),
        "{sentence}"
    );
    let status = loop {
        let e = wait_event(&events, "status", Duration::from_secs(2));
        if e.payload["area"] == "sync" {
            break e;
        }
        assert_eq!(
            e.payload,
            json!({"area": "save", "sentence": "Saved on this Mac"})
        );
    };
    assert_eq!(
        status.payload,
        json!({"area": "sync", "sentence": sentence})
    );

    air.sync_heat().unwrap();
    assert_eq!(
        record(&air, "task", "t1").unwrap(),
        json!({"title": "Grammar quiz 4", "done": false, "notes": "", "id": "t1"})
    );
    // It arrived as one change, which ⌘Z in Heat names.
    assert_eq!(
        ok(&air, "history.get", json!({"room": "heat"}))["undo"],
        "Undo changes from your other devices"
    );

    // Both edit the same task, different fields, before either syncs.
    ok(
        &mac,
        "records.mutate",
        json!({"label": "mark done", "room": "heat",
        "ops": [{"op": "patch", "kind": "task", "id": "t1", "value": {"done": true}}]}),
    );
    ok(
        &air,
        "records.mutate",
        json!({"label": "edit notes", "room": "heat",
        "ops": [{"op": "patch", "kind": "task", "id": "t1", "value": {"notes": "chapter 4"}}]}),
    );
    for _ in 0..2 {
        mac.sync_heat().unwrap();
        air.sync_heat().unwrap();
    }
    let both = json!({"title": "Grammar quiz 4", "done": true, "notes": "chapter 4", "id": "t1"});
    assert_eq!(record(&mac, "task", "t1").unwrap(), both);
    assert_eq!(record(&air, "task", "t1").unwrap(), both);
}

#[test]
fn an_undo_syncs_and_a_grade_stays_here() {
    let server = MockServer::start();
    let (a, b) = (Setup::new(), Setup::new());
    let mac = sign_in(&a, &server);
    let air = sign_in(&b, &server);
    put(&mac, "add task", "task", "t2", json!({"title": "Essay"}));
    put(&mac, "add grade", "grade", "g1", json!({"score": 91}));
    mac.sync_heat().unwrap();
    air.sync_heat().unwrap();
    assert!(record(&air, "task", "t2").is_some());
    assert!(
        record(&air, "grade", "g1").is_none(),
        "a grade left the machine"
    );

    // ⌘Z on the first Mac removes the task there, and on the other.
    assert_eq!(
        ok(&mac, "history.undo", json!({"room": "heat"}))["label"],
        "add grade"
    );
    assert_eq!(
        ok(&mac, "history.undo", json!({"room": "heat"}))["label"],
        "add task"
    );
    mac.sync_heat().unwrap();
    air.sync_heat().unwrap();
    assert!(record(&air, "task", "t2").is_none());
    // Redo brings it back everywhere.
    ok(&mac, "history.redo", json!({"room": "heat"}));
    mac.sync_heat().unwrap();
    air.sync_heat().unwrap();
    assert_eq!(record(&air, "task", "t2").unwrap()["title"], "Essay");
}

#[test]
fn the_app_calls_no_model() {
    // Claude reaches Heat through the MCP server (2.11); the core has no
    // job to give it, and no switch to ask first.
    let setup = Setup::new();
    let core = setup.core();
    let e = core
        .invoke("assist.call", json!({"task": "score", "body": {}}))
        .unwrap_err();
    assert_eq!(e.code, "unknown_command");
    let s = ok(&core, "app.settings.get", json!({}));
    assert!(s.get("claude").is_none(), "{s}");
}

#[test]
fn settings_change_only_what_exists() {
    let setup = Setup::new();
    let core = setup.core();
    let s = ok(&core, "app.settings.get", json!({}));
    assert_eq!(s["appearance"], "system");
    let s = ok(
        &core,
        "app.settings.set",
        json!({"patch": {"appearance": "dark", "audio": {"buffer": 256}}}),
    );
    assert_eq!(
        (s["appearance"].as_str(), s["audio"]["buffer"].as_u64()),
        (Some("dark"), Some(256))
    );
    let e = core
        .invoke("app.settings.set", json!({"patch": {"apperance": "dark"}}))
        .unwrap_err();
    assert_eq!(e.message, "There is no setting called 'apperance'.");
    let e = core
        .invoke(
            "app.settings.set",
            json!({"patch": {"audio": {"buffer": 100}}}),
        )
        .unwrap_err();
    assert_eq!(e.code, "bad_setting");
    drop(core);
    let core = setup.core();
    assert_eq!(
        ok(&core, "app.settings.get", json!({}))["appearance"],
        "dark"
    );
    assert_eq!(ok(&core, "app.hello", json!({}))["reduceMotion"], false);
}
