//! Labelled undo through the core (docs/PLAN.md S1.5's core part, and F5
//! through the core; docs/SPEC.md 2.7, 9.6). What a fail looks like:
//! - a change isn't named in the Edit menu ("Undo tag clip"), or undo
//!   doesn't answer with its label;
//! - work that left the machine (a sent message) is offered as undoable;
//! - after a random run of edits, undoing everything doesn't return the
//!   first state byte for byte, or redoing everything the last;
//! - ⌘Z acts outside the room it is pressed in;
//! - a label is lost on relaunch;
//! - any table has ON DELETE CASCADE.

use rand::seq::SliceRandom;
use rand::Rng;
use rusqlite::Connection;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

const ROOMS: [&str; 4] = ["heat", "space", "console", "library"];

/// Every journaled table, row by row, as text: "the state, byte for byte".
fn dump(core: &Core) -> String {
    let conn = Connection::open(core.library().join("library.sqlite")).unwrap();
    let mut out = String::new();
    for table in [
        "sequences",
        "clips",
        "tags",
        "clip_tags",
        "pins",
        "smart_folders",
        "docs",
    ] {
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM {table} ORDER BY id"))
            .unwrap();
        let cols = stmt.column_count();
        let mut rows = stmt.query([]).unwrap();
        while let Some(r) = rows.next().unwrap() {
            out += table;
            for c in 0..cols {
                out += &format!("|{:?}", r.get_ref(c).unwrap());
            }
            out.push('\n');
        }
    }
    out
}

/// Undoes (or redoes) every room until none has anything left, taking
/// rooms in turn: an entry held back by another room's later change waits.
fn unwind(core: &Core, back: bool) -> usize {
    let cmd = if back { "history.undo" } else { "history.redo" };
    let mut steps = 0;
    loop {
        let mut moved = false;
        for room in ROOMS {
            match core.invoke(cmd, json!({"room": room})) {
                Ok(r) => {
                    assert!(r["label"].as_str().is_some_and(|l| !l.is_empty()));
                    steps += 1;
                    moved = true;
                }
                Err(e) => assert!(
                    [
                        "nothing_to_undo",
                        "nothing_to_redo",
                        "cant_undo",
                        "cant_redo"
                    ]
                    .contains(&e.code.as_str()),
                    "{e}"
                ),
            }
        }
        if !moved {
            return steps;
        }
    }
}

fn random_edit(core: &Core, rng: &mut impl Rng, clips: &[String], n: usize) {
    let room = *ROOMS[..4].choose(rng).unwrap();
    let kind = ["task", "note", "block"].choose(rng).unwrap();
    let id = format!("r{}", rng.gen_range(0..6));
    let edit = match rng.gen_range(0..7) {
        0 | 1 => core.invoke(
            "records.mutate",
            json!({"label": format!("put {n}"), "room": room,
                   "ops": [{"op": "put", "kind": kind, "id": id, "value": {"title": format!("t{n}"), "n": n}}]}),
        ),
        2 => core.invoke(
            "records.mutate",
            json!({"label": format!("patch {n}"), "room": room,
                   "ops": [{"op": "patch", "kind": kind, "id": id, "value": {"done": rng.gen_bool(0.5)}}]}),
        ),
        3 => core.invoke(
            "records.mutate",
            json!({"label": format!("delete {n}"), "room": room,
                   "ops": [{"op": "delete", "kind": kind, "id": id}]}),
        ),
        4 => core.invoke(
            "library.tag",
            json!({"ids": [clips.choose(rng).unwrap()], "add": [format!("tag{}", rng.gen_range(0..4))], "label": format!("tag {n}")}),
        ),
        5 => {
            let colour = [json!("red"), json!("blue"), Value::Null].choose(rng).unwrap().clone();
            core.invoke(
                "library.colour",
                json!({"ids": [clips.choose(rng).unwrap()], "colour": colour, "label": format!("colour {n}")}),
            )
        }
        _ => core.invoke(
            "library.rename",
            json!({"id": clips.choose(rng).unwrap(), "title": format!("title {n}"), "label": format!("rename {n}")}),
        ),
    };
    // A refusal (patching a record that isn't there) changes nothing.
    if let Err(e) = edit {
        assert!(["not_found", "refused"].contains(&e.code.as_str()), "{e}");
    }
}

#[test]
fn undo_everything_returns_the_first_state_and_redo_the_last() {
    let mut rng = rand::thread_rng();
    for run in 0..6 {
        let setup = Setup::new();
        let core = setup.core();
        let first = dump(&core);
        let clips = import(
            &core,
            &[
                &corpus("original.wwav"),
                &corpus("mono.wav"),
                &corpus("large.swav"),
            ],
        );
        let edits = rng.gen_range(20..60);
        for n in 0..edits {
            random_edit(&core, &mut rng, &clips, n);
            // Now and then, undo and redo a little in the middle of it.
            if rng.gen_bool(0.2) {
                let _ = core.invoke(
                    "history.undo",
                    json!({"room": ROOMS.choose(&mut rng).unwrap()}),
                );
            }
            if rng.gen_bool(0.1) {
                let _ = core.invoke(
                    "history.redo",
                    json!({"room": ROOMS.choose(&mut rng).unwrap()}),
                );
            }
        }
        // The last state is the run with everything redone.
        unwind(&core, false);
        let last = dump(&core);
        assert!(unwind(&core, true) > 0);
        assert_eq!(
            dump(&core),
            first,
            "run {run}: undo all didn't return the first state"
        );
        unwind(&core, false);
        assert_eq!(
            dump(&core),
            last,
            "run {run}: redo all didn't return the last state"
        );
    }
}

#[test]
fn undo_acts_only_in_its_room_and_is_labelled() {
    let setup = Setup::new();
    let core = setup.core();
    let events = core.events();
    ok(
        &core,
        "records.mutate",
        json!({"label": "mark done", "room": "heat",
        "ops": [{"op": "put", "kind": "task", "id": "t1", "value": {"title": "Grammar quiz 4", "done": true}}]}),
    );
    let h = wait_event(&events, "history", std::time::Duration::from_secs(2));
    assert!(h.payload["room"].is_string());
    ok(
        &core,
        "records.mutate",
        json!({"label": "add world", "room": "space",
        "ops": [{"op": "put", "kind": "saved", "id": "s1", "value": {"title": "glass hours"}}]}),
    );
    assert_eq!(
        ok(&core, "history.get", json!({"room": "heat"})),
        json!({"room": "heat", "undo": "Undo mark done", "redo": null, "cant": null, "cantRedo": null})
    );
    assert_eq!(
        ok(&core, "history.get", json!({"room": "space"}))["undo"],
        "Undo add world"
    );
    assert_eq!(
        ok(&core, "history.get", json!({"room": "console"}))["undo"],
        Value::Null
    );

    // ⌘Z in Heat undoes Heat's change, not Space's newer one.
    assert_eq!(
        ok(&core, "history.undo", json!({"room": "heat"}))["label"],
        "mark done"
    );
    assert_eq!(
        core.invoke("records.get", json!({"kind": "task", "id": "t1"}))
            .unwrap_err()
            .code,
        "not_found"
    );
    assert_eq!(
        ok(&core, "records.get", json!({"kind": "saved", "id": "s1"}))["record"]["title"],
        "glass hours"
    );
    assert_eq!(
        ok(&core, "history.get", json!({"room": "heat"}))["redo"],
        "Redo mark done"
    );
    let none = core
        .invoke("history.undo", json!({"room": "heat"}))
        .unwrap_err();
    assert_eq!(
        (none.code.as_str(), none.message.as_str()),
        ("nothing_to_undo", "Nothing to undo.")
    );

    // A change in one room held by a later one in another says so.
    ok(
        &core,
        "records.mutate",
        json!({"label": "rename task", "room": "heat",
        "ops": [{"op": "put", "kind": "task", "id": "t2", "value": {"title": "Essay"}}]}),
    );
    ok(
        &core,
        "records.mutate",
        json!({"label": "retitle", "room": "space",
        "ops": [{"op": "patch", "kind": "task", "id": "t2", "value": {"title": "Essay draft"}}]}),
    );
    let held = ok(&core, "history.get", json!({"room": "heat"}));
    assert_eq!(held["undo"], Value::Null);
    assert_eq!(
        held["cant"],
        "Can't undo rename task yet. Undo retitle in Space first."
    );
    let refused = core
        .invoke("history.undo", json!({"room": "heat"}))
        .unwrap_err();
    assert_eq!(refused.code, "cant_undo");
    assert_eq!(
        refused.message,
        "Can't undo rename task yet. Undo retitle in Space first."
    );
}

#[test]
fn a_sent_message_is_never_offered_as_undoable() {
    let setup = Setup::new();
    let core = setup.core();
    ok(
        &core,
        "records.mutate",
        json!({"label": "save for later", "room": "space",
        "ops": [{"op": "put", "kind": "saved", "id": "s1", "value": {"title": "Low Tide"}}]}),
    );
    // A message goes out (wi-store records it as work that left the machine).
    let mut store = wi_store::Store::open(core.library()).unwrap();
    store
        .record_outward(wi_store::Room::Space, "a message")
        .unwrap();
    let h = ok(&core, "history.get", json!({"room": "space"}));
    assert_eq!(h["undo"], Value::Null);
    assert_eq!(h["cant"], "Can't undo a message.");
    assert_eq!(
        core.invoke("history.undo", json!({"room": "space"}))
            .unwrap_err()
            .message,
        "Can't undo a message."
    );
}

#[test]
fn there_are_three_views_and_the_library_drawer_and_no_fourth_room() {
    let setup = Setup::new();
    let core = setup.core();
    for room in ROOMS {
        ok(&core, "history.get", json!({"room": room}));
    }
    // The shop is gone, and the core's own journal for other devices' changes
    // is not a place ⌘Z is pressed.
    for room in ["unquantized", "sync", "kitchen"] {
        let e = core
            .invoke("history.get", json!({"room": room}))
            .unwrap_err();
        assert_eq!(e.code, "bad_args", "{room}");
        assert_eq!(
            e.message,
            format!("There is no view called '{room}'. The views are heat, space and console, and library is the drawer over them.")
        );
    }
}

#[test]
fn labels_survive_a_relaunch_and_nothing_cascades() {
    let setup = Setup::new();
    {
        let core = setup.core();
        ok(
            &core,
            "records.mutate",
            json!({"label": "plan my day", "room": "heat",
            "ops": [{"op": "put", "kind": "block", "id": "b1", "value": {"start": 540}}]}),
        );
        let id = import(&core, &[&corpus("original.wwav")])[0].clone();
        ok(
            &core,
            "library.tag",
            json!({"ids": [id], "add": ["demo"], "label": "tag clip"}),
        );
        ok(&core, "history.undo", json!({"room": "library"}));
    }
    let core = setup.core();
    let heat = ok(&core, "history.get", json!({"room": "heat"}));
    assert_eq!(heat["undo"], "Undo plan my day");
    let library = ok(&core, "history.get", json!({"room": "library"}));
    assert_eq!(library["undo"], "Undo import");
    assert_eq!(library["redo"], "Redo tag clip");

    let conn = Connection::open(core.library().join("library.sqlite")).unwrap();
    let mut stmt = conn
        .prepare("SELECT name, sql FROM sqlite_master WHERE sql IS NOT NULL")
        .unwrap();
    let tables: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(tables.len() > 10);
    for (name, sql) in tables {
        assert!(!sql.to_uppercase().contains("CASCADE"), "{name} cascades");
    }
}
