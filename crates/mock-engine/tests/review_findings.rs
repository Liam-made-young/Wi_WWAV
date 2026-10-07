//! Adversarial review of build/wire. Each test here exposes a defect the
//! review found in mock-engine; it is `#[ignore]`d, with the finding named,
//! until fixed. Run them with
//! `cargo test -p mock-engine --test review_findings -- --ignored`.

use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use wwav_wire::process::{EngineConfig, EngineProcess};

const MOCK: &str = env!("CARGO_BIN_EXE_mock-engine");
const T: Duration = Duration::from_secs(5);

fn graph() -> Value {
    json!({
        "sample_rate": 48000,
        "tracks": [
            {"id": "vox", "kind": "stem", "role": "vocals"},
            {"id": "keys", "kind": "audio", "role": "other",
             "devices": [{"id": "tape", "format": "vst3", "uid": "TAPEECHO"}]}
        ]
    })
}

fn engine(tmp: &Path) -> EngineProcess {
    let mut c = EngineConfig::new(MOCK);
    c.tmp_dir = tmp.to_path_buf();
    c.device = Some("null".into());
    c.test = true;
    let (p, _events) = EngineProcess::spawn(c).unwrap();
    p.hello("review").unwrap();
    p.client()
        .call("session.load", json!({"graph": graph()}), T)
        .unwrap();
    p
}

/// Finding (medium): with a loop whose end is behind the playhead, each block
/// does `pos = start + (pos + block - end)`, so the playhead walks backwards
/// by (end - start - block) samples a block while the clock says
/// `state: playing, rate: 48000`. From 10 minutes in with a loop of 0..1000
/// it runs backwards at about 330,000 samples a second for some 90 s.
#[test]
#[ignore = "finding: mock-engine runs the playhead backwards under a loop behind it"]
fn a_loop_behind_the_playhead_never_runs_the_clock_backwards() {
    let tmp = tempfile::tempdir().unwrap();
    let p = engine(tmp.path());
    let c = p.client();
    c.call("transport.locate", json!({"sample": 28_800_000}), T)
        .unwrap();
    c.call(
        "transport.loop",
        json!({"on": true, "start": 0, "end": 1000}),
        T,
    )
    .unwrap();
    c.call("transport.play", Value::Null, T).unwrap();
    let mut last = None;
    for _ in 0..6 {
        std::thread::sleep(Duration::from_millis(50));
        let k = p.shm().region().clock.read().unwrap();
        assert_eq!((k.state, k.rate), (1, 48000.0));
        let in_loop = (0..1000).contains(&k.sample_pos);
        if let Some(prev) = last {
            assert!(
                in_loop || k.sample_pos >= prev,
                "the clock says it plays forward at 48000/s, yet sample_pos went {prev} -> {}",
                k.sample_pos
            );
        }
        last = Some(k.sample_pos);
    }
}

/// Finding (medium): render runs on the message thread, so nothing else is
/// answered until it ends. ENGINE.md §3.1: no answer to a ping within 1 s
/// means the engine has hung and the app kills it; a render that takes over
/// a second (one minute of audio here) gets the mock killed mid-render by the
/// supervisor it stands in for. §3.6 puts render on a non-real-time thread.
#[test]
#[ignore = "finding: mock-engine answers no ping during a render"]
fn a_ping_is_answered_during_a_render() {
    let tmp = tempfile::tempdir().unwrap();
    let p = Arc::new(engine(tmp.path()));
    let render = {
        let p = p.clone();
        let out = tmp.path().join("out");
        std::thread::spawn(move || {
            p.client().call(
                "render",
                json!({"out_dir": out, "start": 0, "len": 48000 * 60}),
                Duration::from_secs(120),
            )
        })
    };
    std::thread::sleep(Duration::from_millis(100));
    let ping = p.client().call("ping", Value::Null, Duration::from_secs(1));
    let rendered = render.join().unwrap();
    assert!(rendered.is_ok(), "{rendered:?}");
    assert!(ping.is_ok(), "a ping during the render: {ping:?}");
}

/// Finding (medium): `WavWriter::create` checks that the data size fits a u32
/// but then adds the 36 header bytes in u32: for 536,870,908 to 536,870,911
/// f32 frames (and 1,073,741,815 to 1,073,741,823 s16 frames) `36 + data`
/// overflows. The debug mock-engine the tests run panics on its message
/// thread and dies (a release build writes a wrong RIFF size instead); the
/// render should be refused like any longer one ("a render over 4 GB doesn't
/// fit a WAVE file").
#[test]
#[ignore = "finding: a render at the WAVE size limit crashes mock-engine"]
fn a_render_at_the_wave_size_limit_is_refused_not_a_crash() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = engine(tmp.path());
    let r = p.client().request(
        "render",
        json!({"out_dir": tmp.path().join("out"), "start": 0, "len": 536_870_911u64, "format": "f32"}),
        T,
    );
    assert!(r.is_ok(), "no answer: {r:?}");
    assert!(p.is_alive(), "the engine died");
}

/// Finding (high, F8) end to end: a plugin param the app sets is not the one
/// the engine reports back (see wwav-wire's review_findings for the cause).
/// The state blob, its sha256 and the session hash S3.4 compares move with it.
#[test]
#[ignore = "finding: F8 - serde_json without float_roundtrip changes f64s by one ULP"]
fn a_plugin_param_reads_back_as_it_was_set() {
    let tmp = tempfile::tempdir().unwrap();
    let p = engine(tmp.path());
    let c = p.client();
    for x in [0.9856906946328695f64, 0.21291890726713458] {
        c.call(
            "param.set",
            json!({"node": "tape", "param": "p0", "value": x}),
            T,
        )
        .unwrap();
        let r = c.call("plugin.params", json!({"node": "tape"}), T).unwrap();
        let back = r["params"][0]["value"].as_f64().unwrap();
        assert_eq!(back.to_bits(), x.to_bits(), "set {x:?}, read back {back:?}");
    }
}

/// Finding (low, wwav-wire `process`): every EngineProcess in one app process
/// uses the same `$TMPDIR/wwav-<app pid>/engine.sock`. A second one removes
/// the first's socket file to bind its own, and dropping it removes the
/// shared directory, after which the first can't restart its engine (the
/// engine can't bind and exits). wi-core's supervisor tests, run in parallel
/// in one test binary with the default tmp_dir, would trip over each other.
#[test]
#[ignore = "finding: two EngineProcesses in one process share one socket path"]
fn two_engines_in_one_process_keep_their_own_sockets() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = EngineConfig::new(MOCK);
    c.tmp_dir = tmp.path().to_path_buf();
    let (mut a, _a_events) = EngineProcess::spawn(c.clone()).unwrap();
    a.hello("a").unwrap();
    let (b, _b_events) = EngineProcess::spawn(c).unwrap();
    b.hello("b").unwrap();
    assert_ne!(a.socket(), b.socket(), "both engines listen on one path");
    drop(b);
    a.kill().unwrap();
    let respawned = a.respawn();
    assert!(
        respawned.is_ok(),
        "a can't restart its engine: {:?}",
        respawned.err().map(|e| e.to_string())
    );
}
