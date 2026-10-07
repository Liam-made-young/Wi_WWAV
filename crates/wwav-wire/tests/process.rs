//! Starting the engine (docs/ENGINE.md §1) with stand-in engines that go
//! wrong in each way the app must survive. mock-engine's own tests cover an
//! engine that goes right.

use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use wwav_wire::process::{EngineConfig, EngineProcess, StartError};

fn fake_engine(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("fake-engine");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn config(tmp: &Path, binary: PathBuf) -> EngineConfig {
    let mut c = EngineConfig::new(binary);
    c.tmp_dir = tmp.to_path_buf();
    c.start_timeout = Duration::from_millis(500);
    c
}

/// Exec of a script another test thread has just written can fail with
/// ETXTBSY while a sibling's fork still holds it; try again.
fn spawn(c: EngineConfig) -> Result<EngineProcess, StartError> {
    for _ in 0..20 {
        match EngineProcess::spawn(c.clone()) {
            Err(StartError::Io(e)) if e.raw_os_error() == Some(ETXTBSY) => {
                std::thread::sleep(Duration::from_millis(20))
            }
            other => return other.map(|(p, _events)| p),
        }
    }
    panic!("the script stayed busy");
}

/// The same number on Linux and macOS.
const ETXTBSY: i32 = 26;

#[test]
fn the_engine_gets_its_flags_a_private_dir_and_a_region() {
    let tmp = tempfile::tempdir().unwrap();
    let args = tmp.path().join("args");
    let dir_mode = tmp.path().join("dir-mode");
    let bin = fake_engine(
        tmp.path(),
        &format!(
            "echo \"$@\" > {}\nls -ld \"$(dirname \"$2\")\" > {}\nexit 3",
            args.display(),
            dir_mode.display()
        ),
    );
    let mut c = config(tmp.path(), bin);
    c.device = Some("null".into());
    c.rate = Some(44100);
    c.block = Some(256);
    c.test = true;
    let err = spawn(c).err().unwrap();
    assert!(
        matches!(err, StartError::Exited(s) if s.code() == Some(3)),
        "{err}"
    );

    let pid = std::process::id();
    let dir = tmp.path().join(format!("wwav-{pid}"));
    let got = std::fs::read_to_string(&args).unwrap();
    // The socket is named for the region: /wwav-<pid>-<n> listens on engine-<n>.sock.
    let words: Vec<&str> = got.split_whitespace().collect();
    let n = words[3]
        .strip_prefix(&format!("/wwav-{pid}-"))
        .unwrap_or_else(|| panic!("{got}"));
    let socket = format!("{}/engine-{n}.sock", dir.display());
    assert_eq!(
        words[..4],
        ["--socket", &socket, "--shm", words[3]],
        "{got}"
    );
    assert!(
        got.trim_end()
            .ends_with("--device null --rate 44100 --block 256 --test"),
        "{got}"
    );
    let mode = std::fs::read_to_string(&dir_mode).unwrap();
    assert!(mode.starts_with("drwx------"), "{mode}");
    assert!(
        !dir.exists(),
        "an engine that didn't start leaves no directory"
    );
}

#[test]
fn an_engine_that_never_listens_is_killed_after_the_timeout() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = fake_engine(tmp.path(), "exec sleep 30");
    let started = Instant::now();
    let err = spawn(config(tmp.path(), bin)).err().unwrap();
    assert!(matches!(err, StartError::NoListen(_)), "{err}");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn an_engine_that_prints_anything_else_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = fake_engine(tmp.path(), "echo 'hello there'\nexec sleep 30");
    let err = spawn(config(tmp.path(), bin)).err().unwrap();
    assert!(
        matches!(&err, StartError::Stdout(line) if line == "hello there"),
        "{err}"
    );
}

#[test]
fn a_missing_binary_says_so() {
    let tmp = tempfile::tempdir().unwrap();
    let err = spawn(config(tmp.path(), tmp.path().join("no-such-engine")))
        .err()
        .unwrap();
    assert!(
        matches!(&err, StartError::Io(e) if e.kind() == std::io::ErrorKind::NotFound),
        "{err}"
    );
}

#[test]
fn someone_elses_directory_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(format!("wwav-{}", std::process::id()));
    std::fs::DirBuilder::new().mode(0o755).create(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let bin = fake_engine(tmp.path(), "exit 0");
    let err = spawn(config(tmp.path(), bin)).err().unwrap();
    assert!(err.to_string().contains("private"), "{err}");
}
