//! F6's crash clauses with a real crash. The fails, written first:
//!
//! - Killing the process mid-edit loses a journalled change: an edit whose
//!   call returned (the child said "done k") is missing after reopening.
//! - A crash mid-write leaves a half-written `session.json`: after any kill,
//!   `session.json` doesn't parse, or isn't a state the journal recorded.
//!
//! The child is this test binary run again as `crash_child`, which edits a
//! session until it is killed with SIGKILL (`Child::kill`).

mod common;

use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Lines};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{note, sample_session, FakeClock, CLIP, MIDI_CLIP, T0};
use wwav_session::model::Event;
use wwav_session::{Package, SystemClock};

const DIR: &str = "WWAV_SESSION_CRASH_DIR";
const MODE: &str = "WWAV_SESSION_CRASH_MODE";
const START: &str = "WWAV_SESSION_CRASH_START";

/// Op k of the run, the same in the child and in the parent's model.
fn op(pkg: &mut Package, k: u64) {
    match k % 10 {
        3 | 7 => {
            pkg.undo().unwrap();
        }
        5 => {
            pkg.redo().unwrap();
        }
        0 => {
            pkg.edit("rename", |s| s.title = format!("Take {k}"))
                .unwrap();
        }
        1 | 4 => {
            pkg.edit("add note", |s| {
                s.midi.get_mut(MIDI_CLIP).unwrap().push(note(k))
            })
            .unwrap();
        }
        2 | 8 => {
            pkg.edit("move clip", |s| {
                if let Some(e) = s.tracks[(k % 2) as usize].events.first_mut() {
                    e.at_ms += 10;
                }
            })
            .unwrap();
        }
        6 => {
            pkg.edit("add clip", |s| {
                s.tracks[0].events.push(Event::new(CLIP, k as i64))
            })
            .unwrap();
        }
        _ => {
            pkg.edit("delete clip", |s| {
                s.tracks[1].events.pop();
            })
            .unwrap();
        }
    }
}

#[test]
#[ignore = "the child the crash tests run and kill"]
fn crash_child() {
    // Run with --include-ignored but not by a parent: nothing to do.
    let Ok(dir) = env::var(DIR) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let start: u64 = env::var(START).unwrap().parse().unwrap();
    let (mut pkg, _) = Package::open(&dir, Arc::new(SystemClock)).unwrap();
    match env::var(MODE).unwrap().as_str() {
        "ops" => {
            for k in start + 1.. {
                op(&mut pkg, k);
                println!("done {k}");
                if k % 3 == 0 {
                    pkg.save().unwrap();
                }
            }
        }
        "big" => {
            for k in start + 1.. {
                pkg.edit("add note", |s| {
                    s.midi.get_mut(MIDI_CLIP).unwrap().push(note(k))
                })
                .unwrap();
                println!("done {k}");
                pkg.save().unwrap();
            }
        }
        "rename" => {
            for k in start + 1..=start + 12 {
                pkg.edit("add note", |s| {
                    s.midi.get_mut(MIDI_CLIP).unwrap().push(note(k))
                })
                .unwrap();
                println!("done {k}");
            }
            // The new session.json is written and synced; the rename is next.
            let _staged = pkg.stage_save().unwrap();
            println!("staged");
            std::thread::sleep(Duration::from_secs(60));
        }
        other => panic!("unknown mode {other}"),
    }
}

/// The child, killed when dropped so a failing test leaves nothing running.
struct Kid(Child);

impl Drop for Kid {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn(dir: &Path, mode: &str, start: u64) -> (Kid, Lines<BufReader<ChildStdout>>) {
    let mut child = Command::new(env::current_exe().unwrap())
        .args([
            "crash_child",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(DIR, dir)
        .env(MODE, mode)
        .env(START, start.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let lines = BufReader::new(child.stdout.take().unwrap()).lines();
    (Kid(child), lines)
}

/// The k in "done k", for the child's lines that say one.
fn done(line: &str) -> Option<u64> {
    line.strip_prefix("done ").map(|k| k.parse().unwrap())
}

struct Rng(u64);
impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
}

/// session.json parses, and its sha256 is a state the journal recorded
/// (a "start" or "save" line), so it is whole and not some other bytes.
fn assert_whole(dir: &Path) {
    let bytes = fs::read(dir.join("session.json")).unwrap();
    serde_json::from_slice::<serde_json::Value>(&bytes).expect("a half-written session.json");
    let sha = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&bytes));
    let journal = fs::read_to_string(dir.join("journal/undo.ndjson")).unwrap();
    assert!(
        journal
            .lines()
            .any(|l| l.contains(&format!("\"sha256\":\"{sha}\""))),
        "session.json holds a state the journal never recorded"
    );
}

#[test]
fn killing_mid_edit_loses_no_journalled_change() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = Arc::new(SystemClock);
    // Big enough that a save takes a moment, so some kills land in one;
    // a_kill_mid_write_leaves_session_json_whole makes sure of it.
    let session = sample_session(1000);
    let pkg = Package::create(&tmp.path().join("live"), session.clone(), clock.clone()).unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    // The model runs the same ops in this process, uninterrupted.
    let mut model = Package::create(&tmp.path().join("model"), session, FakeClock::at(T0)).unwrap();
    let mut model_at = 0u64;

    let mut rng = Rng(0x5eed_cafe_f00d_1234);
    let mut at = 0u64; // ops the reopened session holds
    let mut recovered_rounds = 0;
    for round in 0..24 {
        let (kid, mut lines) = spawn(&dir, "ops", at);
        // Let it run a random number of ops, then a random moment more.
        let wait_for = 1 + rng.below(25);
        let mut printed = at;
        while printed < at + wait_for {
            let line = lines.next().expect("the child stopped early").unwrap();
            if let Some(k) = done(&line) {
                printed = k;
            }
        }
        let spin = Instant::now();
        let spin_for = Duration::from_micros(rng.below(4000));
        while spin.elapsed() < spin_for {}
        drop(kid); // SIGKILL, then reap
                   // Every op the child reported before it died, read to the end.
        for line in lines {
            if let Some(k) = done(&line.unwrap()) {
                printed = k;
            }
        }

        assert_whole(&dir);
        let (pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
        assert!(!opened.history_reset, "round {round}: the journal was lost");
        if opened.recovered.is_some() {
            recovered_rounds += 1;
        }
        // Every reported op is there; one more may be, if the child was
        // killed after journalling it and before saying so.
        while model_at < printed {
            model_at += 1;
            op(&mut model, model_at);
        }
        if pkg.session().to_json_bytes() != model.session().to_json_bytes() {
            model_at += 1;
            op(&mut model, model_at);
        }
        assert_eq!(
            pkg.session().to_json_bytes(),
            model.session().to_json_bytes(),
            "round {round}: reopened after op {printed} doesn't match the model at {model_at}"
        );
        assert_eq!(pkg.undo_label(), model.undo_label(), "round {round}");
        assert_eq!(pkg.redo_label(), model.redo_label(), "round {round}");
        at = model_at;
    }
    assert!(
        recovered_rounds > 0,
        "no kill ever left unsaved changes to recover"
    );
}

#[test]
fn a_kill_in_the_rename_window_leaves_the_old_session_json() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let pkg = Package::create(tmp.path(), sample_session(10), clock.clone()).unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    let old = fs::read(dir.join("session.json")).unwrap();

    let (kid, mut lines) = spawn(&dir, "rename", 0);
    let mut printed = 0;
    loop {
        let line = lines.next().expect("the child stopped early").unwrap();
        if let Some(k) = done(&line) {
            printed = k;
        }
        if line == "staged" {
            break;
        }
    }
    drop(kid);
    assert_eq!(printed, 12);

    // The old file, whole; the new one never took its name.
    assert_eq!(fs::read(dir.join("session.json")).unwrap(), old);
    assert!(dir.join("session.json.tmp").exists());
    assert_whole(&dir);

    let (pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert!(
        !dir.join("session.json.tmp").exists(),
        "open clears the leftover"
    );
    let recovered = opened.recovered.expect("the twelve notes come back");
    assert_eq!(recovered.changes, 12);
    assert_eq!(pkg.session().midi[MIDI_CLIP].len(), 10 + 12);
    assert_eq!(pkg.undo_label(), "Undo add note");
}

#[test]
fn a_kill_mid_write_leaves_session_json_whole() {
    // The kill lands while a new session.json is being written: the parent
    // kills as soon as the new file has bytes, or as soon as session.json
    // is shorter than it was (rewritten in place; it only grows here).
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut session = sample_session(10);
    // 8 MB, so each save takes long enough to be caught in the middle.
    session
        .extra
        .insert("padding".into(), "x".repeat(8 << 20).into());
    let pkg = Package::create(tmp.path(), session, clock.clone()).unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    let new_file = dir.join("session.json.tmp");

    let mut at = 0u64;
    let mut caught_mid_write = 0;
    for _ in 0..40 {
        if caught_mid_write == 3 {
            break;
        }
        let whole = fs::metadata(dir.join("session.json")).unwrap().len();
        let writing = || {
            fs::metadata(&new_file).is_ok_and(|m| m.len() > 0)
                || fs::metadata(dir.join("session.json")).is_ok_and(|m| m.len() < whole)
        };
        let (kid, lines) = spawn(&dir, "big", at);
        let waiting = Instant::now();
        while !writing() {
            assert!(
                waiting.elapsed() < Duration::from_secs(60),
                "no save ever started"
            );
        }
        drop(kid);
        let mut printed = at;
        for line in lines {
            if let Some(k) = done(&line.unwrap()) {
                printed = k;
            }
        }
        assert_whole(&dir);
        // Count the rounds whose new file was cut off part way.
        if let Ok(bytes) = fs::read(&new_file) {
            if serde_json::from_slice::<serde_json::Value>(&bytes).is_err() {
                caught_mid_write += 1;
            }
        }

        let (pkg, _) = Package::open(&dir, clock.clone()).unwrap();
        let notes = pkg.session().midi[MIDI_CLIP].len() as u64 - 10;
        assert!(
            notes == printed || notes == printed + 1,
            "{notes} notes after op {printed}"
        );
        at = notes;
    }
    assert_eq!(caught_mid_write, 3, "never killed a save part way through");
}
