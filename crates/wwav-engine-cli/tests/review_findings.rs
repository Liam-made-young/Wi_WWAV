//! Adversarial review of build/wire. Each test here exposed a defect the
//! review found in wwav-engine-cli, named in its doc comment; the defects are
//! fixed, and the tests stay to keep them so.

use serde_json::{json, Value};
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;
use wwav_wire::frame;
use wwav_wire::msg::Request;

/// A private scratch directory for one round (this crate has no tempfile).
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cli-review-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs the CLI over `--connect` against a fake engine that answers every
/// request with `reply(id, op)` (raw frames, written in one go), and returns
/// its exit status and the lines it printed.
fn run_against(
    name: &str,
    script: &str,
    extra: &[&str],
    reply: fn(u64, &str) -> Vec<u8>,
) -> (i32, Vec<Value>) {
    let dir = scratch(name);
    let sock = dir.join("engine.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    let fake = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        while let Ok(Some(m)) = frame::read(&mut s) {
            let req = Request::from_object(m).unwrap();
            let bytes = reply(req.id, &req.op);
            if s.write_all(&bytes).is_err() {
                break;
            }
        }
    });
    let path = dir.join("script.txt");
    std::fs::write(&path, script).unwrap();
    let mut args: Vec<String> = vec!["--connect".into(), sock.display().to_string()];
    args.extend(extra.iter().map(|s| s.to_string()));
    args.push(path.display().to_string());
    let mut out = Vec::new();
    let code = wwav_engine_cli::run(&args, &mut out);
    drop(fake);
    let _ = std::fs::remove_dir_all(&dir);
    let lines = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (code, lines)
}

/// Finding (medium): the CLI says it prints every response and event "in
/// the order they arrived". After a response it drains the event channel,
/// which by then can hold events the reader thread took off the wire *after*
/// that response, so an event is printed before the response it followed.
/// And over `--connect` nothing waits at the end of the script, so an event
/// that follows the last response is never printed at all.
#[test]
fn every_event_prints_after_the_response_it_followed() {
    // The engine's wire order, every time: the response, then one event.
    fn reply(id: u64, op: &str) -> Vec<u8> {
        let mut bytes = frame::encode(&json!({"id": id, "ok": true, "result": {}})).unwrap();
        bytes.extend(frame::encode(&json!({"ev": "after", "op": op, "id_was": id})).unwrap());
        bytes
    }
    let (mut reordered, mut lost) = (0, 0);
    for round in 0..20 {
        let (_, lines) = run_against(&format!("order-{round}"), "ping\nping\nping\n", &[], reply);
        // Response k, then its event k, for k = 1, 2, 3.
        let seen: Vec<String> = lines
            .iter()
            .filter_map(|l| match (l.get("ok"), l.get("ev")) {
                (Some(_), _) => Some(format!("R{}", l["id"])),
                (_, Some(_)) => Some(format!("E{}", l["id_was"])),
                _ => None,
            })
            .collect();
        if seen.len() < 6 {
            lost += 1;
        } else if seen != ["R1", "E1", "R2", "E2", "R3", "E3"] {
            reordered += 1;
        }
    }
    assert_eq!(
        (reordered, lost),
        (0, 0),
        "of 20 runs, {reordered} printed an event before its response and {lost} lost one"
    );
}

/// Finding (low): the exit status is 0 whatever the engine did: a request
/// that timed out, a connection that closed, or (with --spawn) an engine
/// still running 2 s after its stdin closed, against ENGINE.md §1. Only a
/// script line that doesn't parse fails the run, so a CI step that runs a
/// script to prove the contract passes when the engine breaks it.
#[test]
fn a_request_that_fails_fails_the_run() {
    // An engine that never answers.
    fn silent(_: u64, _: &str) -> Vec<u8> {
        Vec::new()
    }
    let (code, lines) = run_against("silent", "ping\n", &["--timeout", "200ms"], silent);
    let failed: Vec<&Value> = lines.iter().filter_map(|l| l.get("failed")).collect();
    assert_eq!(failed.len(), 1, "{lines:?}");
    assert_eq!(failed[0]["error"], "timeout");
    assert_ne!(code, 0, "the ping timed out, yet the run passed");
}
