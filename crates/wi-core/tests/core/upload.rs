//! The upload queue against tools/mock-server (docs/PLAN.md S1.9; docs/SPEC.md
//! 2.8, 9.7). What a fail looks like:
//! - the queue is anything but `published_at IS NOT NULL AND remote_id IS
//!   NULL`: it doesn't survive a relaunch, or an undo while queued leaves it;
//! - a resumed upload sends a finished part again;
//! - a retry posts the work twice;
//! - the status bar says anything but 2.8's sentences;
//! - once the server has it, ⌘Z is offered instead of "Can't undo a
//!   publish. Unpublish '…'…".

use std::time::Duration;

use rusqlite::Connection;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

const T: Duration = Duration::from_secs(20);
const COVERS: i64 = 2;

fn queue_by_query(core: &Core) -> Vec<String> {
    let conn = Connection::open(core.library().join("library.sqlite")).unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT id FROM clips WHERE published_at IS NOT NULL AND remote_id IS NULL ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn waiting(core: &Core) -> Vec<String> {
    let mut ids: Vec<String> = ok(core, "publish.queue", json!({}))["waiting"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["clip"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    ids
}

fn drop_on_covers(core: &Core, clip: &str) {
    let r = ok(
        core,
        "publish.drop",
        json!({"clip": clip, "target": {"kind": "system", "id": COVERS}, "label": "publish"}),
    );
    assert_eq!(r, json!({"queued": true}));
}

/// Waits for the upload status to say `sentence`.
fn wait_status(events: &std::sync::mpsc::Receiver<wi_core::Event>, sentence: &str) -> Vec<String> {
    let mut said = Vec::new();
    let deadline = std::time::Instant::now() + T;
    while std::time::Instant::now() < deadline {
        if let Ok(e) = events.recv_timeout(Duration::from_millis(100)) {
            if e.event == "status" && e.payload["area"] == "upload" {
                let s = e.payload["sentence"].as_str().unwrap().to_string();
                said.push(s.clone());
                if s == sentence {
                    return said;
                }
            }
        }
    }
    panic!("the status never said {sentence:?}; it said {said:?}");
}

fn tracks_from(server: &MockServer, clip: &str) -> Vec<Value> {
    server.state()["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| {
            t["versions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["clipId"] == clip)
        })
        .cloned()
        .collect()
}

#[test]
fn the_queue_is_the_query_and_undo_takes_a_queued_drop_back() {
    let setup = Setup::new();
    let core = setup.core();
    let ids = import(&core, &[&corpus("original.wwav"), &corpus("mono.wav")]);
    drop_on_covers(&core, &ids[0]);
    drop_on_covers(&core, &ids[1]);
    let mut both = ids.clone();
    both.sort();
    assert_eq!(waiting(&core), both);
    assert_eq!(queue_by_query(&core), both);
    let q = ok(&core, "publish.queue", json!({}));
    assert_eq!(
        q["sentence"],
        "2 works wait to go up; they leave once you sign in."
    );

    // It survives a relaunch: there is no queue file to lose.
    drop(core);
    let core = setup.core();
    assert_eq!(waiting(&core), both);

    // ⌘Z in Space while it waits: published_at clears and the queue loses it.
    assert_eq!(
        ok(&core, "history.get", json!({"room": "space"}))["undo"],
        "Undo publish"
    );
    assert_eq!(
        ok(&core, "history.undo", json!({"room": "space"}))["label"],
        "publish"
    );
    assert_eq!(waiting(&core), vec![ids[0].clone()]);
    assert_eq!(queue_by_query(&core), vec![ids[0].clone()]);
    let clip = ok(&core, "library.get", json!({"id": ids[1]}))["clip"].clone();
    assert_eq!(clip["published"], Value::Null);
    assert_eq!(clip["tags"], json!([]));
    assert_eq!(
        ok(&core, "publish.queue", json!({}))["sentence"],
        "1 work waits to go up; it leaves once you sign in."
    );
    let film = import(&core, &[&corpus("large.swav")])[0].clone();
    let refused = core
        .invoke(
            "publish.drop",
            json!({"clip": film, "target": {"kind": "system", "id": COVERS}, "label": "publish"}),
        )
        .unwrap_err();
    assert_eq!(refused.code, "not_yet");
}

#[test]
fn up_once_and_then_publish_cant_be_undone() {
    let server = MockServer::start();
    let setup = Setup::new();
    let core = sign_in(&setup, &server);
    let events = core.events();
    let id = import(&core, &[&corpus("original.wwav")])[0].clone();
    drop_on_covers(&core, &id);
    let said = wait_status(&events, "Up. Tést \"Song\" is in your galaxy.");
    assert!(
        said.contains(&"Uploading Tést \"Song\"".to_string()),
        "{said:?}"
    );
    assert!(eventually(T, || waiting(&core).is_empty()));
    assert!(queue_by_query(&core).is_empty());
    assert_eq!(
        ok(&core, "library.get", json!({"id": id}))["clip"]["published"],
        "up"
    );

    let works = tracks_from(&server, &id);
    assert_eq!(works.len(), 1);
    assert_eq!(
        works[0]["settings"],
        json!({"origin": "wi_wwav", "clipId": id})
    );
    let planets: Vec<Value> = server.state()["planets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["trackId"] == works[0]["trackId"])
        .cloned()
        .collect();
    assert_eq!(planets.len(), 1, "it landed in Covers");
    assert_eq!(planets[0]["systemId"], COVERS);

    let h = ok(&core, "history.get", json!({"room": "space"}));
    assert_eq!(h["undo"], Value::Null);
    assert_eq!(
        h["cant"],
        "Can't undo a publish. Unpublish 'Tést \"Song\"'…"
    );
}

#[test]
fn a_retry_after_a_lost_answer_never_posts_twice() {
    let server = MockServer::start();
    let setup = Setup::new();
    let core = sign_in(&setup, &server);
    let events = core.events();
    // The publish lands, but its answer is lost on the way back.
    let (status, _) = server.call(
        "POST",
        "/__mock/fail",
        Some(json!({"method": "POST", "path": "/api/publish", "drop": "after"})),
    );
    assert_eq!(status, 200);
    let id = import(&core, &[&corpus("original.wwav")])[0].clone();
    drop_on_covers(&core, &id);
    wait_status(&events, "Up. Tést \"Song\" is in your galaxy.");
    let works = tracks_from(&server, &id);
    assert_eq!(works.len(), 1, "a retry posted twice");
    assert_eq!(works[0]["versions"].as_array().unwrap().len(), 1);
    // The trackId the first sign gave was kept across the retry.
    let signed: Vec<Value> = server.state()["uploads"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|u| u["userId"] == 1 && u["trackId"].as_str().unwrap().starts_with("track_"))
        .cloned()
        .collect();
    let mine: Vec<&Value> = signed
        .iter()
        .filter(|u| u["trackId"] == works[0]["trackId"])
        .collect();
    assert_eq!(
        mine.len(),
        2,
        "one sign and one re-sign under the same trackId: {signed:?}"
    );
}

#[test]
fn a_big_file_resumes_without_sending_a_finished_part_again() {
    let server = MockServer::start_counting();
    let setup = Setup::new();
    // 251 MB of plain audio: over /sign's 250 MB, so it goes in 8 MiB parts.
    let big = setup.dir.path().join("Long Take.wav");
    std::fs::File::create(&big)
        .unwrap()
        .set_len(251 * 1024 * 1024)
        .unwrap();
    let id = {
        let core = sign_in(&setup, &server);
        let events = core.events();
        let id = import(&core, &[&big])[0].clone();
        std::fs::remove_file(&big).unwrap();
        drop_on_covers(&core, &id);
        wait_status(&events, "Uploading Long Take · part 4 of 32");
        id
        // The app quits mid-upload.
    };
    let parts_before = {
        let conn = Connection::open(setup.library().join("library.sqlite")).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM upload_part WHERE clip_id = ?1",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        n
    };
    assert!(
        (4..32).contains(&parts_before),
        "{parts_before} parts were up when it quit"
    );

    let core = setup.core_on(&server.url);
    let events = core.events();
    let said = wait_status(&events, "Up. Long Take is in your galaxy.");
    assert!(
        !said
            .iter()
            .any(|s| s == "Uploading Long Take · part 1 of 32"),
        "it started over: {said:?}"
    );
    let counted = server.counted();
    let uploads = counted["uploads"].as_array().unwrap();
    assert_eq!(uploads.len(), 1, "one multipart upload, resumed");
    let sends = uploads[0]["sends"].as_object().unwrap();
    assert_eq!(sends.len(), 32);
    for (n, sent) in sends {
        assert_eq!(sent, 1, "part {n} was sent {sent} times");
    }
    let works: Vec<&Value> = counted["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["settings"]["clipId"] == id.as_str())
        .collect();
    assert_eq!(works.len(), 1);
    assert_eq!(works[0]["versions"], 1);
    assert!(queue_by_query(&core).is_empty());
}

#[test]
fn offline_says_so() {
    let server = MockServer::start();
    let setup = Setup::new();
    let core = sign_in(&setup, &server);
    let events = core.events();
    drop(server);
    let ids = import(&core, &[&corpus("original.wwav"), &corpus("mono.wav")]);
    drop_on_covers(&core, &ids[0]);
    drop_on_covers(&core, &ids[1]);
    wait_status(
        &events,
        "Offline. 2 works wait to go up; they leave when you're back.",
    );
    assert_eq!(
        ok(&core, "publish.queue", json!({}))["sentence"],
        "Offline. 2 works wait to go up; they leave when you're back."
    );
}
