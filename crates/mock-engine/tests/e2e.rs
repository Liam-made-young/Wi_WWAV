//! End to end: mock-engine started the way the app starts an engine, driven
//! through wwav-engine-cli's script runner and through `EngineProcess`.
//! These are the checks S0.2 and S3.4 lean on: the clock moves with time,
//! mute reaches the meters within a block, a kill is seen as a closed socket
//! and an exit, a protocol mismatch is refused, and the engine leaves when
//! the app's pipe closes.

use serde_json::{json, Value};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::time::{Duration, Instant};
use wwav_wire::process::{EngineConfig, EngineProcess};
use wwav_wire::shm::{crumb_hash, ClockFields};

const MOCK: &str = env!("CARGO_BIN_EXE_mock-engine");
const T: Duration = Duration::from_secs(5);

/// Runs the CLI with `flags` and returns its exit status and every line it
/// printed, parsed.
fn cli_status(flags: &[String], script: &str) -> (i32, Vec<Value>) {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("script.txt");
    std::fs::write(&path, script).unwrap();
    let mut args = flags.to_vec();
    args.push(path.display().to_string());
    let mut out = Vec::new();
    let code = wwav_engine_cli::run(&args, &mut out);
    let text = String::from_utf8(out).unwrap();
    let lines = text
        .lines()
        .map(|l| {
            serde_json::from_str(l)
                .unwrap_or_else(|e| panic!("not one JSON object per line ({e}): {l}"))
        })
        .collect();
    (code, lines)
}

/// Runs a script through the CLI against a fresh mock-engine.
fn cli_spawn(flags: &[&str], script: &str) -> (i32, Vec<Value>) {
    let tmp = tempfile::tempdir().unwrap();
    let mut args: Vec<String> = vec![
        "--spawn".into(),
        MOCK.into(),
        "--tmp".into(),
        tmp.path().display().to_string(),
    ];
    args.extend(flags.iter().map(|s| s.to_string()));
    cli_status(&args, script)
}

/// The same, for a script where nothing fails.
fn cli(flags: &[&str], script: &str) -> Vec<Value> {
    let (code, lines) = cli_spawn(flags, script);
    assert_eq!(code, 0, "the CLI failed:\n{lines:#?}");
    lines
}

fn with_key<'a>(lines: &'a [Value], key: &str) -> Vec<&'a Value> {
    lines.iter().filter_map(|l| l.get(key)).collect()
}

fn responses(lines: &[Value]) -> Vec<&Value> {
    lines.iter().filter(|l| l.get("ok").is_some()).collect()
}

fn events<'a>(lines: &'a [Value], ev: &str) -> Vec<&'a Value> {
    lines.iter().filter(|l| l["ev"] == ev).collect()
}

fn graph() -> Value {
    json!({
        "sample_rate": 48000,
        "tracks": [
            {"id": "vox", "kind": "stem", "role": "vocals"},
            {"id": "drm", "kind": "stem", "role": "drums", "gain_db": -6.0},
            {"id": "keys", "kind": "audio", "role": "other", "pan": -0.5,
             "devices": [{"id": "tape", "format": "vst3", "uid": "TAPEECHO", "params": {"p0": 0.25}}]},
            {"id": "bass", "kind": "audio", "role": "bass", "mute": true}
        ],
        "master": {"gain_db": 0.0}
    })
}

fn load_line(playhead: i64) -> String {
    format!(
        "session.load {}\n",
        json!({"graph": graph(), "playhead": playhead})
    )
}

#[test]
fn a_script_plays_and_the_clock_and_meters_follow() {
    let script = format!(
        "# hello, load, play, and watch the clock and the meters\n\
         hello {{\"protocol\": 1, \"client\": \"e2e\"}}\n\
         {}\
         sleep 100ms\n\
         meters\n\
         transport.play\n\
         sleep 300ms\n\
         clock\n\
         sleep 300ms\n\
         clock\n\
         meters\n\
         param.set {{\"node\": \"vox\", \"param\": \"mute\", \"value\": true}}\n\
         sleep 50ms\n\
         meters\n\
         transport.stop\n\
         sleep 100ms\n\
         clock\n",
        load_line(0)
    );
    let lines = cli(&["--device", "null"], &script);

    assert!(with_key(&lines, "spawned")[0]["pid"].as_u64().unwrap() > 0);
    let r = responses(&lines);
    assert_eq!(r.len(), 5, "{lines:#?}");
    assert!(r.iter().all(|r| r["ok"] == true), "{r:#?}");
    let hello = &r[0]["result"];
    assert_eq!(
        (hello["protocol"].as_u64(), hello["shm_layout"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(hello["sample_rate"], 48000);
    assert_eq!(hello["device"], "null");

    let load = &r[1]["result"];
    assert_eq!(
        load["nodes"],
        4 + 1 + 5,
        "tracks, devices, four buses and the master"
    );
    let slots = &load["meter_slots"];
    for (i, node) in [
        "vox",
        "drm",
        "keys",
        "bass",
        "bus:vocals",
        "bus:drums",
        "bus:other",
        "bus:bass",
        "master",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(slots[node], i, "{node}");
    }
    assert!(load["latency"]["tape"].is_u64());

    // The clock: it moves at the rate, with time.
    let clocks = with_key(&lines, "clock");
    let (a, b, stopped) = (clocks[0], clocks[1], clocks[2]);
    assert_eq!(
        (a["state"].as_u64(), a["rate"].as_f64()),
        (Some(1), Some(48000.0))
    );
    let elapsed_ns = (b["now_ns"].as_u64().unwrap() - a["now_ns"].as_u64().unwrap()) as f64;
    let expected = elapsed_ns * 48000.0 / 1e9;
    let moved = (b["sample_pos"].as_i64().unwrap() - a["sample_pos"].as_i64().unwrap()) as f64;
    assert!(
        (moved - expected).abs() <= expected * 0.1 + 256.0,
        "moved {moved}, expected about {expected}"
    );
    // And the cursor extrapolated from it lands within a couple of blocks.
    let at = |c: &Value| {
        c["sample_pos"].as_f64().unwrap()
            + (c["now_ns"].as_f64().unwrap() - c["host_time_ns"].as_f64().unwrap()) * 48000.0 / 1e9
    };
    assert!(
        (at(b) - at(a) - expected).abs() <= 256.0,
        "{} vs {expected}",
        at(b) - at(a)
    );
    assert!(b["callbacks"].as_u64() > a["callbacks"].as_u64());
    assert_eq!(
        (stopped["state"].as_u64(), stopped["rate"].as_f64()),
        (Some(0), Some(0.0))
    );

    // The meters: silent while stopped; a muted track is silent; its bus
    // carries it and nothing else of its role; the master is the buses' sum.
    let meters = with_key(&lines, "meters");
    let (still, playing, muted) = (meters[0], meters[1], meters[2]);
    let still = still["slots"].as_object().unwrap();
    assert_eq!(still.len(), 9, "a slot for each track, bus and the master");
    assert!(still.values().all(|s| s == &json!([0.0, 0.0, 0.0, 0.0])));
    let peak = |m: &Value, node: &str| m["slots"][node][0].as_f64().unwrap();
    assert!(
        peak(playing, "vox") > 0.0 && peak(playing, "drm") > 0.0 && peak(playing, "keys") > 0.0
    );
    assert_eq!(peak(playing, "bass"), 0.0, "bass is muted in the graph");
    assert_eq!(peak(playing, "vox"), peak(playing, "bus:vocals"));
    assert!(
        peak(playing, "drm") < peak(playing, "vox"),
        "drums sit 6 dB down"
    );
    let keys = &playing["slots"]["keys"];
    assert!(keys[0].as_f64() > keys[1].as_f64(), "keys pan left");
    let buses = ["bus:vocals", "bus:drums", "bus:other", "bus:bass"];
    let sum: f64 = buses.iter().map(|b| peak(playing, b)).sum();
    assert!((peak(playing, "master") - sum).abs() < 1e-6);
    assert_eq!(peak(muted, "vox"), 0.0);
    assert_eq!(peak(muted, "bus:vocals"), 0.0);
    assert_eq!(peak(muted, "drm"), peak(playing, "drm"));
    assert!(peak(muted, "master") < peak(playing, "master"));

    // Transport events, and the engine leaves when the CLI closes its stdin.
    let states: Vec<&Value> = events(&lines, "transport")
        .iter()
        .map(|e| &e["state"])
        .collect();
    assert_eq!(
        states,
        [&json!("stopped"), &json!("playing"), &json!("stopped")],
        "locate on load, play, stop"
    );
    let exited = with_key(&lines, "exited");
    assert_eq!(exited.last().unwrap()["code"], 0, "{exited:?}");
    assert!(exited.last().unwrap()["after_ms"].as_u64().unwrap() < 1000);
}

#[test]
fn solo_silences_the_rest() {
    let script = format!(
        "hello {{\"protocol\": 1, \"client\": \"e2e\"}}\n{}\
         param.set {{\"node\": \"drm\", \"param\": \"solo\", \"value\": true}}\n\
         transport.play\nsleep 100ms\nmeters\n",
        load_line(0)
    );
    let lines = cli(&["--device", "null"], &script);
    let m = with_key(&lines, "meters")[0];
    let peak = |node: &str| m["slots"][node][0].as_f64().unwrap();
    assert!(peak("drm") > 0.0);
    assert_eq!(
        (peak("vox"), peak("keys"), peak("bus:vocals")),
        (0.0, 0.0, 0.0)
    );
    assert_eq!(peak("master"), peak("bus:drums"));
}

#[test]
fn kill_9_is_seen_as_a_closed_socket_and_an_exit() {
    let script = format!(
        "hello {{\"protocol\": 1, \"client\": \"e2e\"}}\n{}transport.play\nsleep 100ms\nkill9\nping\n",
        load_line(0)
    );
    let (code, lines) = cli_spawn(&["--device", "null"], &script);
    assert_eq!(code, 1, "the ping after the kill fails the run");
    let closed = with_key(&lines, "closed");
    assert!(closed[0]["after_ms"].as_u64().unwrap() < 1000, "{closed:?}");
    let exited = with_key(&lines, "exited");
    assert_eq!(exited[0]["signal"], 9, "{exited:?}");
    let failed = with_key(&lines, "failed");
    assert_eq!(failed[0]["op"], "ping");
    assert_eq!(failed[0]["error"], "closed");
}

#[test]
fn after_a_kill_the_cli_respawns_and_the_transport_comes_back_stopped() {
    let script = format!(
        "hello {{\"protocol\": 1, \"client\": \"e2e\"}}\n{}transport.play\nsleep 100ms\nclock\nkill9\nrespawn\n\
         hello {{\"protocol\": 1, \"client\": \"e2e\"}}\n\
         session.load {}\nsleep 100ms\nclock\n",
        load_line(0),
        json!({"graph": graph(), "playhead": 48000})
    );
    let lines = cli(&["--device", "null"], &script);
    let spawned = with_key(&lines, "spawned")[0]["pid"].as_u64().unwrap();
    let respawned = with_key(&lines, "respawned")[0];
    assert_ne!(respawned["pid"].as_u64().unwrap(), spawned);
    assert!(
        respawned["after_ms"].as_u64().unwrap() < 2000,
        "back within 2 s: {respawned}"
    );
    let clocks = with_key(&lines, "clock");
    assert_eq!(clocks[0]["state"], 1);
    assert_eq!(clocks[1]["state"], 0, "nothing resumes on its own");
    assert_eq!(clocks[1]["sample_pos"], 48000);
    assert!(
        responses(&lines).iter().all(|r| r["ok"] == true),
        "{lines:#?}"
    );
}

#[test]
fn the_cli_connects_to_a_running_engine() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = EngineConfig::new(MOCK);
    c.tmp_dir = tmp.path().to_path_buf();
    let (mut p, _events) = EngineProcess::spawn(c).unwrap();
    // The engine takes one client at a time: this one steps aside.
    p.client().close();
    let flags: Vec<String> = vec![
        "--connect".into(),
        p.socket().display().to_string(),
        "--shm".into(),
        p.shm().name().into(),
    ];
    let script = format!("hello {{\"protocol\": 1, \"client\": \"e2e\"}}\n{}transport.play\nsleep 100ms\nclock\nmeters\nkill9\n", load_line(0));
    let (code, lines) = cli_status(&flags, &script);
    assert_eq!(code, 0, "{lines:#?}");
    assert_eq!(
        with_key(&lines, "connected")[0]["socket"],
        p.socket().display().to_string()
    );
    assert_eq!(with_key(&lines, "clock")[0]["state"], 1);
    assert!(
        with_key(&lines, "meters")[0]["slots"]["vox"][0]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(with_key(&lines, "kill9")[0]["pid"], p.pid());
    assert!(!with_key(&lines, "closed").is_empty(), "{lines:#?}");
    assert!(with_key(&lines, "exited").is_empty(), "not the CLI's child");
    assert_eq!(p.wait(T).unwrap().unwrap().signal(), Some(9));
}

#[test]
fn a_line_that_does_not_read_is_reported_and_fails_the_run() {
    let tmp = tempfile::tempdir().unwrap();
    let flags: Vec<String> = vec![
        "--spawn".into(),
        MOCK.into(),
        "--tmp".into(),
        tmp.path().display().to_string(),
    ];
    let (code, lines) = cli_status(
        &flags,
        "hello {\"protocol\": 1, \"client\": \"e2e\"}\nping {\nsleep soon\nping\n",
    );
    assert_eq!(code, 1);
    let failed = with_key(&lines, "failed");
    assert_eq!(
        (failed[0]["line"].as_u64(), failed[1]["line"].as_u64()),
        (Some(2), Some(3))
    );
    assert_eq!(responses(&lines).len(), 2, "the lines after it still run");
}

#[test]
fn a_protocol_mismatch_is_refused_and_the_connection_closes() {
    let (code, lines) = cli_spawn(
        &[],
        "hello {\"protocol\": 2, \"client\": \"from the future\"}\nping\n",
    );
    assert_eq!(code, 1, "the ping on the closed connection fails the run");
    let r = responses(&lines);
    assert_eq!(r[0]["ok"], false);
    assert_eq!(r[0]["error"]["code"], "protocol");
    assert_eq!(
        r[0]["error"]["message"],
        "This engine speaks protocol 1; the app speaks 2."
    );
    assert_eq!(with_key(&lines, "failed")[0]["error"], "closed");
}

#[test]
fn hello_comes_first_and_unknown_ops_are_named() {
    let lines = cli(&[], "ping\nhello {\"protocol\": 1, \"client\": \"e2e\"}\nping\nno.such.op\ndebug.crash {\"in\": \"audio\"}\n");
    let r = responses(&lines);
    assert_eq!(r[0]["error"]["code"], "hello_first");
    assert_eq!(r[1]["ok"], true);
    assert!(r[2]["result"]["t"].as_u64().unwrap() > 0);
    assert_eq!(r[3]["error"]["code"], "unknown_op");
    assert_eq!(r[4]["error"]["code"], "unknown_op", "debug ops need --test");
}

/// Waits for a block to publish a clock that passes `ok`, rather than
/// guessing how long a busy machine takes to run one.
fn clock_when(p: &EngineProcess, ok: impl Fn(&ClockFields) -> bool) -> ClockFields {
    let deadline = Instant::now() + T;
    loop {
        let c = p.shm().region().clock.read().unwrap();
        if ok(&c) {
            return c;
        }
        assert!(
            Instant::now() < deadline,
            "the clock never got there: {c:?}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn engine(tmp: &Path) -> EngineProcess {
    let mut c = EngineConfig::new(MOCK);
    c.tmp_dir = tmp.to_path_buf();
    c.device = Some("null".into());
    c.test = true;
    let (p, _events) = EngineProcess::spawn(c).unwrap();
    p.hello("e2e").unwrap();
    p
}

#[test]
fn closing_stdin_stops_the_engine_within_a_second() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = engine(tmp.path());
    p.client()
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    p.client().call("transport.play", Value::Null, T).unwrap();
    let started = Instant::now();
    p.close_stdin();
    let status = p
        .wait(Duration::from_secs(1))
        .unwrap()
        .expect("still running a second after stdin closed");
    assert!(status.success(), "{status}");
    assert!(p.client().wait_closed(T));
    eprintln!("exited {:?} after stdin closed", started.elapsed());
}

#[test]
fn a_kill_closes_the_socket_and_the_child_exits_by_signal() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = engine(tmp.path());
    assert!(p.is_alive());
    p.kill().unwrap();
    assert!(p.client().wait_closed(Duration::from_secs(1)));
    let status = p.wait(Duration::from_secs(1)).unwrap().unwrap();
    assert_eq!(status.signal(), Some(9));
    assert!(!p.is_alive());
}

#[test]
fn the_engine_writes_its_header_clock_and_crumbs() {
    let tmp = tempfile::tempdir().unwrap();
    let p = engine(tmp.path());
    let h = p.shm().region().header.read().unwrap();
    assert_eq!((h.sample_rate, h.block_size), (48000, 128));
    assert_eq!(h.engine_pid, p.pid() as u64);
    p.client()
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    let crumbs = p.shm().region().crumb.seq();
    let callbacks = p.shm().region().clock.read().unwrap().callbacks;
    clock_when(&p, |c| c.callbacks >= callbacks + 10);
    assert!(
        p.shm().region().crumb.seq() >= crumbs + 2 * 10,
        "a set and a clear per device per block"
    );
    assert_eq!(p.shm().region().crumb.get(), 0, "cleared between calls");
    assert!(
        p.shm().region().clock.read().unwrap().callbacks >= callbacks + 10,
        "the timer runs while stopped"
    );
}

#[test]
fn a_crash_inside_a_node_leaves_its_crumb() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = engine(tmp.path());
    p.client()
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    p.client()
        .call("debug.crumb", json!({"node": "tape"}), T)
        .unwrap();
    assert!(p.client().wait_closed(Duration::from_secs(1)));
    let status = p.wait(Duration::from_secs(1)).unwrap().unwrap();
    assert_eq!(status.signal(), Some(6), "abort()");
    let crumb = p.shm().region().crumb.get();
    assert_eq!(crumb, crumb_hash("tape"));
    assert_eq!(
        wwav_wire::shm::crumb_node(crumb, ["vox", "tape"]),
        Some("tape")
    );
}

#[test]
fn crashes_and_hangs_on_each_thread() {
    let tmp = tempfile::tempdir().unwrap();
    // A crash on the message thread has no answer: the connection closing is the answer.
    let mut p = engine(tmp.path());
    let r = p.client().call("debug.crash", json!({"in": "message"}), T);
    assert!(
        matches!(r, Err(wwav_wire::client::CallError::Closed)),
        "{r:?}"
    );
    assert_eq!(p.wait(T).unwrap().unwrap().signal(), Some(6));

    // A crash on the audio thread is answered, then the engine dies.
    let events = p.respawn().unwrap();
    drop(events);
    p.hello("e2e").unwrap();
    p.client()
        .call("debug.crash", json!({"in": "audio"}), T)
        .unwrap();
    assert!(p.client().wait_closed(Duration::from_secs(1)));
    assert_eq!(p.wait(T).unwrap().unwrap().signal(), Some(6));

    // A hung audio thread stalls the clock while pings are still answered.
    p.respawn().unwrap();
    p.hello("e2e").unwrap();
    p.client()
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    p.client().call("transport.play", Value::Null, T).unwrap();
    // Let a block publish the playing clock before the audio thread hangs.
    clock_when(&p, |c| c.state == 1);
    p.client()
        .call("debug.hang", json!({"in": "audio"}), T)
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    let a = p.shm().region().clock.read().unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let b = p.shm().region().clock.read().unwrap();
    assert_eq!(a, b, "the clock stalls for 500 ms while playing");
    assert_eq!(b.state, 1);
    p.client()
        .call("ping", Value::Null, Duration::from_secs(1))
        .unwrap();

    // A hung message thread leaves pings unanswered.
    p.respawn().unwrap();
    p.hello("e2e").unwrap();
    p.client()
        .call("debug.hang", json!({"in": "message"}), T)
        .unwrap();
    let r = p.client().call("ping", Value::Null, Duration::from_secs(1));
    assert!(
        matches!(r, Err(wwav_wire::client::CallError::Timeout(_))),
        "{r:?}"
    );
    p.kill().unwrap();
}

#[test]
fn a_respawned_engine_uses_the_same_region_and_comes_back_stopped() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = engine(tmp.path());
    p.client()
        .call(
            "session.load",
            json!({"graph": graph(), "playhead": 96000}),
            T,
        )
        .unwrap();
    p.client().call("transport.play", Value::Null, T).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let before = clock_when(&p, |c| c.state == 1);
    let meters_before = p
        .shm()
        .region()
        .meters
        .meter_write
        .load(std::sync::atomic::Ordering::Relaxed);
    let name = p.shm().name().to_string();
    p.kill().unwrap();
    p.respawn().unwrap();
    p.hello("e2e").unwrap();
    assert_eq!(p.shm().name(), name);
    // The app restores the playhead the clock last showed.
    p.client()
        .call(
            "session.load",
            json!({"graph": graph(), "playhead": before.sample_pos}),
            T,
        )
        .unwrap();
    let loaded = p.shm().region().clock.read().unwrap().callbacks;
    let after = clock_when(&p, |c| c.callbacks > loaded);
    assert_eq!(after.state, 0, "nothing resumes on its own");
    assert_eq!(after.sample_pos, before.sample_pos);
    let meters_after = p
        .shm()
        .region()
        .meters
        .meter_write
        .load(std::sync::atomic::Ordering::Relaxed);
    assert!(
        meters_after > meters_before,
        "the meter count carries on rather than going back"
    );
}

#[test]
fn render_writes_each_stem_and_their_sum() {
    let tmp = tempfile::tempdir().unwrap();
    let p = engine(tmp.path());
    p.client()
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    let render = |dir: &Path, format: &str| {
        p.client()
            .call(
                "render",
                json!({"out_dir": dir, "start": 1000, "len": 48000, "master": true, "stems": true, "format": format}),
                T,
            )
            .unwrap()
    };
    let a = render(&tmp.path().join("a"), "f32");
    let b = render(&tmp.path().join("b"), "f32");
    assert_eq!(a["frames"], 48000);
    assert_eq!(a["sha256"], b["sha256"], "a render is the same every time");
    for part in ["master", "vocals", "drums", "other", "bass"] {
        let bytes = std::fs::read(a["files"][part].as_str().unwrap()).unwrap();
        assert_eq!(a["sha256"][part], sha256_hex(&bytes), "{part}");
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
    }
    let samples = |part: &str| -> Vec<f32> {
        let bytes = std::fs::read(a["files"][part].as_str().unwrap()).unwrap();
        bytes[44..]
            .chunks(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    };
    let master = samples("master");
    assert_eq!(master.len(), 48000 * 2);
    let stems: Vec<Vec<f32>> = ["vocals", "drums", "other", "bass"]
        .iter()
        .map(|s| samples(s))
        .collect();
    for i in 0..master.len() {
        let sum = stems[0][i] + stems[1][i] + stems[2][i] + stems[3][i];
        assert_eq!(
            master[i], sum,
            "sample {i}: the master is the sum of the stems"
        );
    }
    assert!(
        stems[3].iter().all(|&s| s == 0.0),
        "the muted bass track leaves its stem silent"
    );
    assert!(stems[0].iter().any(|&s| s != 0.0));

    let s16 = render(&tmp.path().join("c"), "s16");
    let bytes = std::fs::read(s16["files"]["master"].as_str().unwrap()).unwrap();
    assert_eq!(bytes.len(), 44 + 48000 * 2 * 2);
}

#[test]
fn render_reports_progress_and_stops_playback() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = EngineConfig::new(MOCK);
    c.tmp_dir = tmp.path().to_path_buf();
    let (p, events) = EngineProcess::spawn(c).unwrap();
    p.hello("e2e").unwrap();
    p.client()
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    p.client().call("transport.play", Value::Null, T).unwrap();
    let dir = tmp.path().join("out");
    p.client()
        .call(
            "render",
            json!({"out_dir": dir, "start": 0, "len": 200000, "stems": false}),
            T,
        )
        .unwrap();
    let got: Vec<_> = events.try_iter().collect();
    let progress: Vec<_> = got.iter().filter(|e| e.ev == "render.progress").collect();
    assert!(
        progress.iter().all(|e| e.fields["stage"] == "master"),
        "stems weren't asked for"
    );
    assert_eq!(progress.last().unwrap().fields["done"], 200000);
    assert_eq!(progress.last().unwrap().fields["total"], 200000);
    let last_transport = got.iter().rev().find(|e| e.ev == "transport").unwrap();
    assert_eq!(
        last_transport.fields["state"], "stopped",
        "a render comes back stopped"
    );
    assert!(!dir.join("vocals.wav").exists());
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(bytes))
}

#[test]
fn plugin_state_is_deterministic_and_survives_a_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = engine(tmp.path());
    p.client()
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    p.client()
        .call(
            "param.set",
            json!({"node": "tape", "param": "p1", "value": 0.75}),
            T,
        )
        .unwrap();
    let state = p
        .client()
        .call("plugin.state", json!({"node": "tape"}), T)
        .unwrap();
    assert!(state["state"]["inline"].is_string());
    assert_eq!(
        state,
        p.client()
            .call("plugin.state", json!({"node": "tape"}), T)
            .unwrap()
    );

    // The app keeps the last state and loads it into the next engine.
    p.kill().unwrap();
    p.respawn().unwrap();
    p.hello("e2e").unwrap();
    let mut g = graph();
    g["tracks"][2]["devices"][0]["state"] = state["state"].clone();
    p.client()
        .call("session.load", json!({"graph": g}), T)
        .unwrap();
    let restored = p
        .client()
        .call("plugin.state", json!({"node": "tape"}), T)
        .unwrap();
    assert_eq!(restored["sha256"], state["sha256"]);
    let params = p
        .client()
        .call("plugin.params", json!({"node": "tape"}), T)
        .unwrap();
    assert_eq!(params["params"][1]["value"], 0.75);

    // A state over 256 KB goes to a file in state_dir, never inline.
    let mut g = graph();
    g["state_dir"] = json!(tmp.path().join("plugin-state"));
    g["tracks"][2]["devices"][0]["params"]["mock.state_bytes"] = json!(300_000);
    p.client()
        .call("session.load", json!({"graph": g}), T)
        .unwrap();
    let big = p
        .client()
        .call("plugin.state", json!({"node": "tape"}), T)
        .unwrap();
    assert_eq!(big["bytes"], 300_000);
    let file = big["state"]["file"].as_str().unwrap();
    assert_eq!(big["sha256"], sha256_hex(&std::fs::read(file).unwrap()));
}

#[test]
fn a_stop_sends_each_plugins_state() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = EngineConfig::new(MOCK);
    c.tmp_dir = tmp.path().to_path_buf();
    let (p, events) = EngineProcess::spawn(c).unwrap();
    p.hello("e2e").unwrap();
    p.client()
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    p.client().call("transport.play", Value::Null, T).unwrap();
    p.client().call("transport.stop", Value::Null, T).unwrap();
    let state = p
        .client()
        .call("plugin.state", json!({"node": "tape"}), T)
        .unwrap();
    let ev = events.iter().find(|e| e.ev == "plugin.state").unwrap();
    assert_eq!(ev.fields["node"], "tape");
    assert_eq!(ev.fields["state"], state["state"]);
}

#[test]
fn every_op_answers() {
    let tmp = tempfile::tempdir().unwrap();
    let p = engine(tmp.path());
    let c = p.client();
    let devices = c.call("device.list", Value::Null, T).unwrap();
    assert_eq!(devices["devices"][0]["name"], "null");
    let opened = c
        .call(
            "device.open",
            json!({"name": null, "sample_rate": 44100, "block": 256}),
            T,
        )
        .unwrap();
    assert_eq!(
        (opened["sample_rate"].as_u64(), opened["block"].as_u64()),
        (Some(44100), Some(256))
    );
    assert_eq!(p.shm().region().header.read().unwrap().sample_rate, 44100);
    let err = c
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap_err();
    assert!(err.to_string().starts_with("rate_mismatch"), "{err}");
    c.call(
        "device.open",
        json!({"name": "null", "sample_rate": 48000, "block": 128}),
        T,
    )
    .unwrap();
    c.call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    for (op, args) in [
        (
            "param.set",
            json!({"node": "bus:drums", "param": "gain_db", "value": -3.0}),
        ),
        (
            "param.set",
            json!({"node": "master", "param": "gain_db", "value": -1.0}),
        ),
        (
            "param.set",
            json!({"node": "vox", "param": "send.reverb", "value": 0.3}),
        ),
        (
            "param.set",
            json!({"node": "vox", "param": "gain_db", "value": -2.0, "at": 480000}),
        ),
        ("transport.locate", json!({"sample": 4800})),
        (
            "transport.loop",
            json!({"on": true, "start": 0, "end": 48000}),
        ),
        ("plugin.editor.open", json!({"node": "tape"})),
        ("plugin.editor.close", json!({"node": "tape"})),
        ("plugin.params", json!({"node": "tape"})),
        ("midi.inputs", Value::Null),
        ("session.unload", Value::Null),
    ] {
        c.call(op, args.clone(), T)
            .unwrap_or_else(|e| panic!("{op} {args}: {e}"));
    }
    let inputs = c.call("midi.inputs", Value::Null, T).unwrap();
    let input = inputs["inputs"][0]["id"].as_str().unwrap().to_string();
    c.call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    c.call(
        "midi.route",
        json!({"input": input, "track": "keys", "channel": null}),
        T,
    )
    .unwrap();
    for (op, args, code) in [
        (
            "param.set",
            json!({"node": "nobody", "param": "gain_db", "value": 0.0}),
            "no_such_node",
        ),
        (
            "param.set",
            json!({"node": "vox", "param": "wobble", "value": 0.0}),
            "no_such_param",
        ),
        (
            "param.set",
            json!({"node": "vox", "param": "mute", "value": "yes"}),
            "bad_args",
        ),
        (
            "param.set",
            json!({"node": "tape", "param": "p9", "value": 0.5}),
            "no_such_param",
        ),
        ("transport.locate", json!({"sample": -1}), "bad_args"),
        (
            "transport.loop",
            json!({"on": true, "start": 10, "end": 5}),
            "bad_args",
        ),
        (
            "render",
            json!({"out_dir": "relative/dir", "len": 10}),
            "bad_args",
        ),
        ("plugin.state", json!({"node": "vox"}), "no_such_node"),
        (
            "device.open",
            json!({"name": "No Such Interface"}),
            "no_such_device",
        ),
        (
            "midi.route",
            json!({"input": "nope", "track": "keys"}),
            "no_such_input",
        ),
        (
            "session.load",
            json!({"graph": {"sample_rate": 48000, "tracks": [{"id": "x", "kind": "audio", "role": "keys"}]}}),
            "bad_session",
        ),
        (
            "session.load",
            json!({"graph": {"sample_rate": 48000, "tracks": [
            {"id": "x", "kind": "audio", "role": "bass"}, {"id": "x", "kind": "audio", "role": "bass"}]}}),
            "bad_session",
        ),
    ] {
        let r = c.request(op, args.clone(), T).unwrap();
        assert_eq!(r.outcome.as_ref().unwrap_err().code, code, "{op} {args}");
    }
    c.call("shutdown", Value::Null, T).unwrap();
    assert!(c.wait_closed(Duration::from_secs(1)));
}

#[test]
fn a_plugins_latency_change_is_an_event_and_moves_the_clock() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = EngineConfig::new(MOCK);
    c.tmp_dir = tmp.path().to_path_buf();
    let (p, events) = EngineProcess::spawn(c).unwrap();
    p.hello("e2e").unwrap();
    let load = p
        .client()
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    let base = load["latency"]["tape"].as_i64().unwrap();
    p.client().call("transport.play", Value::Null, T).unwrap();
    let before = clock_when(&p, |c| c.state == 1);

    // The mock plugin's Lookahead (p3) adds 2048 samples at 1.0.
    p.client()
        .call(
            "param.set",
            json!({"node": "tape", "param": "p3", "value": 0.5}),
            T,
        )
        .unwrap();
    let ev = events
        .recv_timeout(T)
        .into_iter()
        .chain(events.try_iter())
        .find(|e| e.ev == "plugin.latency")
        .expect("a plugin.latency event");
    assert_eq!(ev.fields["node"], "tape");
    assert_eq!(ev.fields["samples"], base + 1024);
    // The clock subtracts plugin delay: it now runs 1024 samples further back.
    let set = p.shm().region().clock.read().unwrap().callbacks;
    let after = clock_when(&p, |c| c.callbacks > set);
    let travelled = (after.host_time_ns - before.host_time_ns) as f64 * 48000.0 / 1e9;
    let moved = (after.sample_pos - before.sample_pos) as f64;
    assert!(
        (travelled - 1024.0 - moved).abs() <= 128.0,
        "moved {moved}, travelled {travelled}"
    );

    // A change that leaves the latency where it was says nothing.
    p.client()
        .call(
            "param.set",
            json!({"node": "tape", "param": "p0", "value": 0.9}),
            T,
        )
        .unwrap();
    p.client().call("ping", Value::Null, T).unwrap();
    assert!(events.try_iter().all(|e| e.ev != "plugin.latency"));
}

#[test]
fn a_bad_frame_closes_the_connection_and_the_engine_takes_the_next_client() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    let tmp = tempfile::tempdir().unwrap();
    let mut c = EngineConfig::new(MOCK);
    c.tmp_dir = tmp.path().to_path_buf();
    let (p, _events) = EngineProcess::spawn(c).unwrap();
    p.client().close();
    for bad in [
        &[2u8, 0, 0, 0, b'[', b']'][..],
        &[3, 0, 0, 0, b'{', b'x', b'}'],
        &[0xff, 0xff, 0xff, 0x7f],
    ] {
        let mut s = UnixStream::connect(p.socket()).unwrap();
        s.set_read_timeout(Some(T)).unwrap();
        s.write_all(bad).unwrap();
        let mut rest = Vec::new();
        assert_eq!(
            s.read_to_end(&mut rest).unwrap(),
            0,
            "closed without an answer: {bad:?}"
        );
    }
    let (client, _events) = wwav_wire::client::Client::connect(p.socket()).unwrap();
    let hello = client
        .call("hello", json!({"protocol": 1, "client": "next"}), T)
        .unwrap();
    assert_eq!(hello["protocol"], 1);
}

#[test]
fn a_loop_goes_round_when_the_playhead_crosses_its_end() {
    let tmp = tempfile::tempdir().unwrap();
    let p = engine(tmp.path());
    let c = p.client();
    let loaded = c
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    // The tape plugin's delay comes off the clock (the null device has no
    // output latency).
    let delay = loaded["latency"]["tape"].as_i64().unwrap();
    // 100 ms of loop.
    c.call(
        "transport.loop",
        json!({"on": true, "start": 48000, "end": 52800}),
        T,
    )
    .unwrap();
    c.call("transport.locate", json!({"sample": 48000}), T)
        .unwrap();
    c.call("transport.play", Value::Null, T).unwrap();
    clock_when(&p, |k| k.state == 1);
    let start = p.shm().region().clock.read().unwrap().callbacks;
    // Three times round.
    let mut seen = Vec::new();
    while p.shm().region().clock.read().unwrap().callbacks < start + 3 * 4800 / 128 {
        seen.push(p.shm().region().clock.read().unwrap().sample_pos + delay);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        seen.iter().all(|s| (48000..52800).contains(s)),
        "the playhead stays in the loop: {seen:?}"
    );
    assert!(
        seen.windows(2).any(|w| w[1] < w[0]),
        "and goes back to its start: {seen:?}"
    );
}

#[test]
fn a_request_behind_a_render_waits_for_it_and_a_ping_does_not() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = EngineConfig::new(MOCK);
    c.tmp_dir = tmp.path().to_path_buf();
    let (p, events) = EngineProcess::spawn(c).unwrap();
    let p = std::sync::Arc::new(p);
    p.hello("e2e").unwrap();
    p.client()
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    let render = {
        let p = p.clone();
        let out = tmp.path().join("out");
        std::thread::spawn(move || {
            p.client().request_ordered(
                "render",
                json!({"out_dir": out, "start": 0, "len": 48000 * 40, "stems": false}),
                Duration::from_secs(60),
            )
        })
    };
    std::thread::sleep(Duration::from_millis(50));
    let ping = p.client().request_ordered("ping", Value::Null, T).unwrap();
    let play = p
        .client()
        .request_ordered("transport.play", Value::Null, T)
        .unwrap();
    let rendered = render.join().unwrap().unwrap();
    assert!(rendered.response.outcome.is_ok(), "{rendered:?}");
    // Where each answer fell among the render's progress events, in the
    // order they came off the wire.
    let got: Vec<_> = events.try_iter().collect();
    let progress = got.iter().filter(|e| e.ev == "render.progress").count() as u64;
    assert_eq!(rendered.events_before, progress, "{got:?}");
    assert!(
        ping.events_before < progress,
        "the ping was answered during the render ({} of {progress} progress events before it)",
        ping.events_before
    );
    assert!(
        play.events_before >= rendered.events_before,
        "play waited its turn behind the render, as on the engine's worker"
    );
    clock_when(&p, |k| k.state == 1);
}

#[test]
fn a_render_too_big_for_a_wave_file_is_refused_before_playback_stops() {
    let tmp = tempfile::tempdir().unwrap();
    let p = engine(tmp.path());
    p.client()
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    p.client().call("transport.play", Value::Null, T).unwrap();
    clock_when(&p, |k| k.state == 1);
    let out = tmp.path().join("out");
    for (len, format) in [
        (536_870_908u64, "f32"),
        (1_073_741_815, "s16"),
        (1_u64 << 40, "f32"),
    ] {
        let r = p
            .client()
            .request(
                "render",
                json!({"out_dir": out, "len": len, "format": format}),
                T,
            )
            .unwrap();
        assert_eq!(
            r.outcome.unwrap_err().code,
            "render_failed",
            "{len} {format}"
        );
    }
    assert!(!out.exists(), "no files, not even the directory");
    let now = p.shm().region().clock.read().unwrap().callbacks;
    let k = clock_when(&p, |k| k.callbacks > now);
    assert_eq!(k.state, 1, "still playing");
}

/// An engine that dies between the two halves of a clock write leaves `seq`
/// odd. The app writes a whole clock before it starts the next engine, so a
/// readable clock never depends on the next engine knowing to round `seq`
/// up (ENGINE.md §4.2).
#[test]
fn a_respawn_repairs_a_clock_its_engine_died_writing() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::Ordering::Relaxed;
    let tmp = tempfile::tempdir().unwrap();
    // The first engine is mock-engine; the next one never starts, so the
    // region is only ever what the app left in it.
    let ran = tmp.path().join("ran");
    let bin = tmp.path().join("first-time-only");
    std::fs::write(
        &bin,
        format!(
            "#!/bin/sh\n[ -e {ran} ] && exit 3\ntouch {ran}\nexec {MOCK} \"$@\"\n",
            ran = ran.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut c = EngineConfig::new(&bin);
    c.tmp_dir = tmp.path().to_path_buf();
    let (mut p, _events) = EngineProcess::spawn(c).unwrap();
    p.hello("e2e").unwrap();
    p.client()
        .call(
            "session.load",
            json!({"graph": graph(), "playhead": 96000}),
            T,
        )
        .unwrap();
    p.client().call("transport.play", Value::Null, T).unwrap();
    clock_when(&p, |k| k.state == 1);
    p.kill().unwrap();
    p.wait(T).unwrap().unwrap();
    let last = p.shm().region().clock.read().unwrap();
    // Killed mid-write.
    let seq = &p.shm().region().clock.seq;
    seq.store(seq.load(Relaxed) | 1, Relaxed);
    assert_eq!(p.shm().region().clock.read(), None);

    assert!(p.respawn().is_err(), "the second engine exits at once");
    let clock = &p.shm().region().clock;
    let k = clock.read().expect("a whole clock again");
    assert_eq!((k.state, k.rate), (0, 0.0), "stopped");
    assert_eq!(k.sample_pos, last.sample_pos, "where it stood");
    assert_eq!(clock.seq.load(Relaxed) % 2, 0);
}
