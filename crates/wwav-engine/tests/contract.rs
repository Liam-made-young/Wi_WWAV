//! The engine against its contract: the suites in `engine/tests`, which
//! speak `docs/ENGINE.md` from Python with nothing but the standard library,
//! run against this crate's binary. They are the tests the JUCE engine was
//! built to; an engine that passes them answers as it does.
//!
//! They need `python3` and the reference formats in `formats/` (in a git
//! worktree: `tools/local_formats.sh`). Without either, the tests say so
//! and pass, since there is nothing here to hold the engine to.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn suite(name: &str) {
    let tests = root().join("engine/tests");
    let pack = root().join("formats/prana/tools/wwav_pack.py");
    let python = Command::new("python3").arg("--version").output();
    if !pack.exists() || python.is_err() {
        eprintln!("skipped {name}: it needs python3 and formats/ (tools/local_formats.sh)");
        return;
    }
    let out = Command::new("python3")
        .args(["-m", "unittest", name])
        .current_dir(&tests)
        .env("WWAV_ENGINE", env!("CARGO_BIN_EXE_wwav-engine"))
        .env("WWAV_PACK", &pack)
        .output()
        .expect("python3 runs");
    let said = String::from_utf8_lossy(&out.stderr);
    // The engine's own stderr is in there too; the verdict is unittest's.
    let verdict: Vec<&str> = said
        .lines()
        .filter(|l| !l.contains("wwav-engine: "))
        .collect();
    assert!(
        out.status.success(),
        "{name} failed:\n{}",
        verdict.join("\n")
    );
    let ran = verdict
        .iter()
        .rev()
        .find(|l| l.starts_with("Ran "))
        .copied()
        .unwrap_or("");
    eprintln!("{name}: {ran}");
}

// The suites share a machine's one clock and one set of cores, and several
// of them time the engine to the millisecond: so they run one after another.
#[test]
fn the_shared_suites_pass() {
    for name in [
        "test_protocol",
        "test_playback",
        "test_render",
        "test_restart",
        "test_record",
        "test_wwav",
        "test_convert",
    ] {
        suite(name);
    }
}

/// These two make 40 s songs in pure Python first, which takes from twenty
/// seconds to minutes with the machine's load:
/// `cargo test -p wwav-engine --test contract -- --ignored`.
#[test]
#[ignore = "up to minutes of fixture-making in Python"]
fn the_slow_suites_pass() {
    for name in ["test_streaming", "test_review"] {
        suite(name);
    }
}
