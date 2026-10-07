//! Adversarial review of build/wire. Each test here exposes a defect the
//! review found; it is `#[ignore]`d, with the finding named, until fixed.
//! Run them with `cargo test -p wwav-wire --test review_findings -- --ignored`.

use serde_json::{json, Map};
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use wwav_wire::frame;
use wwav_wire::msg::Request;
use wwav_wire::process::{EngineConfig, EngineProcess, StartError};

/// One `param.set` through the envelope: encode, frame, read, decode.
fn through_the_envelope(value: f64) -> f64 {
    let mut args = Map::new();
    args.insert("value".into(), json!(value));
    let req = Request {
        id: 1,
        op: "param.set".into(),
        args: Some(args),
    };
    let bytes = frame::encode(&req).unwrap();
    let back = frame::read(&mut Cursor::new(bytes)).unwrap().unwrap();
    Request::from_object(back).unwrap().args.unwrap()["value"]
        .as_f64()
        .unwrap()
}

/// Finding (high, F8): serde_json is built without `float_roundtrip`, so its
/// reader rounds about one f64 in ten off by one ULP. The text on the wire is
/// exact (ryu writes the shortest round-tripping digits, and `str::parse`
/// gets the original back); the parse in `frame::read` is what changes it.
/// A pan, a gain or a plugin param the app sends is not the one the engine
/// gets, and plugin states and graphs drift each time they cross.
#[test]
#[ignore = "finding: F8 - serde_json without float_roundtrip changes f64s by one ULP"]
fn a_float_crosses_the_envelope_bit_for_bit() {
    for x in [
        0.9856906946328695f64,
        0.21291890726713458,
        0.24602370589025346,
        -11.440349071321009,
    ] {
        assert_eq!(
            x.to_string().parse::<f64>().unwrap().to_bits(),
            x.to_bits(),
            "the JSON text itself is exact"
        );
        let got = through_the_envelope(x);
        assert_eq!(
            got.to_bits(),
            x.to_bits(),
            "sent {x:?}, the reader got {got:?}"
        );
    }
    // And over a sweep of pans in -1..1.
    let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut changed = 0;
    for _ in 0..100_000 {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let x = -1.0 + 2.0 * ((s >> 11) as f64 / (1u64 << 53) as f64);
        if through_the_envelope(x).to_bits() != x.to_bits() {
            changed += 1;
        }
    }
    assert_eq!(changed, 0, "{changed} of 100000 pans changed on the wire");
}

fn fake_engine(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("fake-engine");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// Starts `body` as the engine with a 500 ms start timeout; returns the
/// error and how long `spawn` took to give it.
fn start(body: &str) -> (StartError, Duration) {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = EngineConfig::new(fake_engine(tmp.path(), body));
    c.tmp_dir = tmp.path().to_path_buf();
    c.start_timeout = Duration::from_millis(500);
    for _ in 0..20 {
        let t = Instant::now();
        match EngineProcess::spawn(c.clone()) {
            // ETXTBSY from a sibling test's fork, as in tests/process.rs.
            Err(StartError::Io(e)) if e.raw_os_error() == Some(26) => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(e) => return (e, t.elapsed()),
            Ok(_) => panic!("the fake engine was accepted"),
        }
    }
    panic!("the script stayed busy");
}

/// Finding (medium): `launch` waits for the listening line with a timeout,
/// but when the stdout reader thread ends without a line (a first line that
/// isn't UTF-8, or stdout closed) it calls `child.wait()` with no timeout.
/// An engine that does that and keeps running blocks `EngineProcess::spawn`
/// (the app's start-up and every restart) until the engine exits on its own,
/// and then reports "exited before it was listening".
#[test]
#[ignore = "finding: EngineProcess::spawn ignores start_timeout when the first line isn't UTF-8"]
fn a_garbled_listening_line_is_refused_within_the_start_timeout() {
    let (err, took) = start("printf '\\377\\n'\nexec sleep 10");
    assert!(took < Duration::from_secs(3), "spawn took {took:?}: {err}");
}

#[test]
#[ignore = "finding: EngineProcess::spawn ignores start_timeout when the engine closes stdout"]
fn an_engine_that_closes_stdout_is_given_up_on_within_the_start_timeout() {
    let (err, took) = start("exec 1>&-\nexec sleep 10");
    assert!(took < Duration::from_secs(3), "spawn took {took:?}: {err}");
}
