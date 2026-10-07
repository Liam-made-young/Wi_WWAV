//! Adversarial review of F6 (`docs/PLAN.md`) and 6.5 (`docs/SPEC.md`).
//! Each test names the finding it exposes.

mod common;

use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use common::{sample_session, FakeClock, T0};
use serde_json::{json, Value};
use wwav_session::{Link, Package, SystemClock};

fn session_json(pkg: &Package) -> Vec<u8> {
    fs::read(pkg.dir().join("session.json")).unwrap()
}

/// Mirrors every track's pan, as a "swap left and right" would. A centred
/// track's pan becomes -0.0, which `Value`'s == calls equal to 0.0.
fn mirror_pans(pkg: &mut Package) {
    pkg.edit("mirror pan", |s| {
        for t in &mut s.tracks {
            t.pan = -t.pan;
        }
    })
    .unwrap();
}

#[test]
fn negative_zero_edit_save_undo_save_gives_the_first_bytes() {
    // F6.1 as the builder tests it (edit, save, undo, save gives the first
    // bytes), with an edit that also turns a 0.0 into -0.0. The diff
    // compares with Value's ==, where -0.0 == 0.0, so that part of the
    // change gets no row: undo can't take it back and the file keeps
    // "-0.0" where it had "0.0".
    let tmp = tempfile::tempdir().unwrap();
    let mut s = sample_session(0);
    s.tracks[1].pan = 0.5; // track 0 stays centred
    let mut pkg = Package::create(tmp.path(), s, FakeClock::at(T0)).unwrap();
    let first = session_json(&pkg);

    mirror_pans(&mut pkg);
    pkg.save().unwrap();
    pkg.undo().unwrap();
    pkg.save().unwrap();
    let text = String::from_utf8(session_json(&pkg)).unwrap();
    assert!(
        session_json(&pkg) == first,
        "undo + save didn't give the first bytes back:\n{}",
        text.lines()
            .filter(|l| l.contains("\"pan\""))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn negative_zero_edit_reopens_as_it_was_before_the_kill() {
    // F6.3's own check (the reopened session equals, byte for byte, the
    // session the process held) fails for the same reason: the journal
    // has no row for 0.0 -> -0.0, so replay gives "0.0" where the session
    // held "-0.0".
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut s = sample_session(0);
    s.tracks[1].pan = 0.5;
    let mut pkg = Package::create(tmp.path(), s, clock.clone()).unwrap();
    mirror_pans(&mut pkg);
    let held = pkg.session().to_json_bytes();
    let dir = pkg.dir().to_path_buf();
    drop(pkg); // killed before any save

    let (pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert!(opened.recovered.is_some());
    assert!(
        pkg.session().to_json_bytes() == held,
        "the reopened session differs from the one the process held"
    );
}

#[test]
fn a_clean_save_and_quit_reports_nothing_recovered() {
    // 6.5: "On open, newer journal entries are replayed: 'Recovered 14
    // changes from 21:12.'" After a save and a clean quit there are none,
    // but a save that finds the file already holds the session (an edit
    // undone, then autosave) writes no "save" line, so the next open
    // replays the edit and its undo and says "Recovered 2 changes".
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 4000)
        .unwrap();
    pkg.save().unwrap();
    // Try something, take it back, autosave, quit.
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 9000)
        .unwrap();
    pkg.undo().unwrap();
    pkg.save().unwrap();
    assert!(!pkg.is_dirty());
    let dir = pkg.dir().to_path_buf();
    drop(pkg);

    let (pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert_eq!(
        opened
            .recovered
            .as_ref()
            .map(|r| r.sentence(0))
            .unwrap_or_default(),
        "",
        "a clean save and quit, then a recovery sentence on open"
    );
    assert!(!pkg.is_dirty(), "reopened dirty after a clean save");
}

#[test]
fn a_journal_line_cut_off_mid_session_does_not_cost_the_unsaved_changes() {
    // Stand-in for an append that failed part way (ENOSPC or EFBIG after a
    // short write: write_all returns Err with part of the line on disk).
    // The real failure is shown by
    // a_failed_append_then_more_edits_loses_every_unsaved_change below; this
    // one runs anywhere. The next successful append lands after the
    // fragment, the journal then has a bad line in the middle, and open
    // throws the whole history away: every unsaved change, including the
    // ones journalled before the failure, is gone.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    for i in 1..=3 {
        pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = i)
            .unwrap();
    }
    let journal = pkg.dir().join("journal/undo.ndjson");
    fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .unwrap()
        .write_all(b"{\"op\":\"txn\",\"id\":\"01JC")
        .unwrap();
    for i in 4..=5 {
        pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = i)
            .unwrap();
    }
    let held = pkg.session().to_json_bytes();
    let dir = pkg.dir().to_path_buf();
    drop(pkg); // killed before the autosave

    let (pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert!(
        !opened.history_reset,
        "the journal was thrown away; recovered: {:?}",
        opened.recovered
    );
    assert_eq!(pkg.session().to_json_bytes(), held);
}

#[test]
fn after_a_failed_append_save_quit_and_reopen_keeps_the_undo_label() {
    // F6.2 after the same failed append: everything is saved, the app quits
    // cleanly, and reopening still changes the Edit menu, because open
    // refuses the journal for its one bad line and starts a new history.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 1)
        .unwrap();
    let journal = pkg.dir().join("journal/undo.ndjson");
    // What a write_all that failed part way leaves (see above).
    fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .unwrap()
        .write_all(b"{\"op\":\"txn\",\"id\":\"01JC")
        .unwrap();
    pkg.edit("rename", |s| s.title = "Later".into()).unwrap();
    pkg.save().unwrap();
    let label = pkg.undo_label();
    let saved = session_json(&pkg);
    let dir = pkg.dir().to_path_buf();
    drop(pkg); // a clean quit

    let (pkg, _) = Package::open(&dir, clock.clone()).unwrap();
    assert_eq!(session_json(&pkg), saved);
    assert_eq!(pkg.undo_label(), label);
}

const CHILD_DIR: &str = "WWAV_REVIEW_DIR";

/// A line to the parent, flushed at once: the child then blocks on stdin.
fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{line}").unwrap();
    out.flush().unwrap();
}

/// The child, killed when dropped so a failing test leaves nothing running.
struct Kid(std::process::Child);

impl Drop for Kid {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "the child a_failed_append_then_more_edits_loses_every_unsaved_change runs"]
fn review_child() {
    let Ok(dir) = env::var(CHILD_DIR) else {
        return;
    };
    let (mut pkg, _) = Package::open(&PathBuf::from(dir), Arc::new(SystemClock)).unwrap();
    let mut stdin = std::io::stdin().lock();
    let mut wait = || {
        let mut line = String::new();
        stdin.read_line(&mut line).unwrap();
    };
    for i in 1..=3 {
        pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = i)
            .unwrap();
    }
    say("done 3");
    wait(); // the parent lowers the file size limit
    let r = pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 4);
    say(if r.is_err() {
        "edit 4: failed"
    } else {
        "edit 4: ok"
    });
    wait(); // the parent lifts it again: space was freed
    for i in 5..=6 {
        pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = i)
            .unwrap();
    }
    say("done 6");
    std::thread::sleep(std::time::Duration::from_secs(60));
}

#[test]
#[cfg(target_os = "linux")]
fn a_failed_append_then_more_edits_loses_every_unsaved_change() {
    // A real failed append: the child's file size limit (RLIMIT_FSIZE, set
    // from outside with prlimit, SIGXFSZ ignored) is lowered so edit 4's
    // line is cut off after 10 bytes and edit() returns Err, as on a full
    // disk. The limit is lifted ("space freed") and the child makes edits 5
    // and 6, which return Ok, then is killed before an autosave. Reopening
    // must bring back edits 1-3 and 5-6 (at_ms 6).
    if Command::new("prlimit").arg("--version").output().is_err() {
        eprintln!("prlimit not installed; skipped");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    let journal = dir.join("journal/undo.ndjson");

    let mut child = Kid(Command::new("sh")
        .arg("-c")
        .arg("trap '' XFSZ; exec \"$0\" \"$@\"")
        .arg(env::current_exe().unwrap())
        .args([
            "review_child",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_DIR, &dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap());
    let pid = child.0.id().to_string();
    let mut to_child = child.0.stdin.take().unwrap();
    let stdout = child.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if tx.send(line.unwrap()).is_err() {
                return;
            }
        }
    });
    let until = |want: &str| loop {
        let line = rx
            .recv_timeout(std::time::Duration::from_secs(60))
            .expect("the child stopped or hung");
        // The harness's "test review_child ... " has no newline, so the
        // first line said shares its line.
        if let Some(at) = line.find(want) {
            return line[at..].to_string();
        }
    };
    until("done 3");
    let size = fs::metadata(&journal).unwrap().len();
    let limit = format!("--fsize={}:unlimited", size + 10);
    assert!(Command::new("prlimit")
        .args(["--pid", &pid, &limit])
        .status()
        .unwrap()
        .success());
    writeln!(to_child).unwrap();
    assert_eq!(until("edit 4"), "edit 4: failed");
    assert!(Command::new("prlimit")
        .args(["--pid", &pid, "--fsize=unlimited:unlimited"])
        .status()
        .unwrap()
        .success());
    writeln!(to_child).unwrap();
    until("done 6");
    drop(child); // SIGKILL before any autosave

    let (pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert!(
        !opened.history_reset,
        "edits 1-3 and 5-6 returned Ok and were journalled, but open threw the journal \
         away: at_ms {} (want 6), recovered {:?}",
        pkg.session().tracks[0].events[0].at_ms,
        opened.recovered
    );
    assert_eq!(pkg.session().tracks[0].events[0].at_ms, 6);
}

#[test]
fn a_long_title_can_still_be_sent() {
    // PRANA's disc writer (whose sanitising 6.5 points to) cuts the name to
    // 39 characters; folder_name doesn't cut at all, so a long title makes
    // "<Title>.wwavsession.zip.part" longer than the 255 bytes a file name
    // may have and Send session fails.
    let tmp = tempfile::tempdir().unwrap();
    let mut s = sample_session(0);
    s.title = "夜".repeat(80); // 240 bytes of UTF-8
    let mut pkg = Package::create(&tmp.path().join("sessions"), s, FakeClock::at(T0)).unwrap();
    let out = tmp.path().join("out");
    fs::create_dir(&out).unwrap();
    let sent = pkg.send(&out);
    assert!(sent.is_ok(), "Send session failed: {}", sent.unwrap_err());
}

#[test]
#[ignore = "not fixed: session.json's numbers are I-JSON (docs/DECISIONS.md, 2026-10-07)"]
fn an_unknown_key_keeps_a_large_integer() {
    // Unknown keys must survive a load/save round trip. serde_json without
    // arbitrary_precision reads an integer past u64 as an f64, so the first
    // save after any edit rewrites it with fewer digits.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    let path = dir.join("session.json");
    let text = fs::read_to_string(&path).unwrap().replacen(
        "\"wwavsession\": \"0.1\",",
        "\"wwavsession\": \"0.1\",\n  \"isrc_numeric\": 123456789012345678901234567890,",
        1,
    );
    fs::write(&path, text).unwrap();
    let (mut pkg, _) = Package::open(&dir, clock.clone()).unwrap();
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 7)
        .unwrap();
    pkg.save().unwrap();
    let saved = fs::read_to_string(&path).unwrap();
    let line = saved.lines().find(|l| l.contains("isrc_numeric")).unwrap();
    assert!(
        line.contains("123456789012345678901234567890"),
        "unknown key rewritten: {line}"
    );
}

#[test]
fn a_leftover_empty_file_under_the_media_name_is_not_present_media() {
    // On Linux, reflink-copy creates the destination under its final name
    // (create_new), then tries FICLONE, then removes it on failure; on a
    // filesystem that clones, the clone of a large file also takes time
    // under that name. A kill in that window leaves an empty file named
    // media/<ULID>.<ext>, and the next import trusts any file at the name:
    // Link::Present, with the media missing from the session.
    let tmp = tempfile::tempdir().unwrap();
    let pkg = Package::create(tmp.path(), sample_session(0), FakeClock::at(T0)).unwrap();
    let src = tmp.path().join("01JC5Q8V3M2T7R9X4K6W0YHZNB.wav");
    fs::write(&src, vec![5u8; 50_000]).unwrap();
    // What the kill leaves behind.
    fs::write(pkg.dir().join("media/01JC5Q8V3M2T7R9X4K6W0YHZNB.wav"), b"").unwrap();

    let got = pkg.import_media(&src).unwrap();
    let in_session = fs::read(pkg.dir().join(&got.file)).unwrap();
    assert!(
        in_session.len() == 50_000,
        "import said {:?} and left {} bytes of 50000 in media/",
        got.link,
        in_session.len()
    );
    assert_ne!(got.link, Link::Present);
}

#[test]
fn a_journal_from_a_newer_app_keeps_its_unsaved_changes() {
    // The model keeps a newer app's unknown keys and words, but the
    // journal's `op` is a closed enum: one line with an op this version
    // doesn't know makes the whole journal "damaged", and open drops every
    // unsaved change instead of skipping the line.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 1)
        .unwrap();
    let journal = pkg.dir().join("journal/undo.ndjson");
    fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .unwrap()
        .write_all(b"{\"op\":\"bookmark\",\"id\":\"01JC5Q8V3M2T7R9X4K6W0YHZNB\",\"at\":1}\n")
        .unwrap();
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 2)
        .unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    let (pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert!(!opened.history_reset);
    assert_eq!(pkg.session().tracks[0].events[0].at_ms, 2);
}

#[test]
fn the_spec_shaped_track_gain_is_read() {
    // 5.12 and the brief name a track's and an event's level `gain`; the
    // model reads and writes `gain_db`, so a session.json written to the
    // spec's shape opens at 0 dB with its level parked in `extra`.
    let mut v: Value = serde_json::from_slice(&sample_session(0).to_json_bytes()).unwrap();
    let t = v["tracks"][0].as_object_mut().unwrap();
    t.remove("gain_db");
    t.insert("gain".into(), json!(-6.0));
    let s = wwav_session::Session::from_json_bytes(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(s.tracks[0].gain_db, -6.0);
}

#[test]
fn sending_into_the_package_itself_finishes() {
    // On Windows a session is a plain folder, so the Send dialog can point
    // at it. The zip's .part file is then one of the files being zipped:
    // reading it while the writer appends to it never reaches the end, and
    // the zip grows until the disk is full. Only run on a small filesystem
    // (WWAV_SMALL_FS, e.g. a 2 MB tmpfs), where it ends in ENOSPC.
    //
    // Fixed by refusing a folder inside the package before writing
    // anything; package.rs has a check of the refusal that can't fill a
    // disk if it regresses.
    let Ok(small) = env::var("WWAV_SMALL_FS") else {
        eprintln!("WWAV_SMALL_FS not set; skipped");
        return;
    };
    let tmp = tempfile::tempdir_in(small).unwrap();
    let mut pkg = Package::create(tmp.path(), sample_session(0), FakeClock::at(T0)).unwrap();
    let dir = pkg.dir().to_path_buf();
    let sent = pkg.send(&dir);
    let part = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().ends_with(".zip.part"))
        .map(|e| e.metadata().unwrap().len());
    assert_eq!(part, None, "a .part was left behind");
    assert_eq!(
        sent.unwrap_err().to_string(),
        "A session can't be sent into its own folder. Choose a folder outside it."
    );
}

#[test]
fn a_session_opened_twice_keeps_both_windows_changes() {
    // Nothing stops a package being opened by two Package values (two
    // windows, or the app and a helper). Both append to one journal; when
    // they touch the same value, replay finds a row that doesn't fit and
    // open throws the whole history away.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    let (mut a, _) = Package::open(&dir, clock.clone()).unwrap();
    let opened_twice = Package::open(&dir, clock.clone());
    let Ok((mut b, _)) = opened_twice else {
        return; // refused: the finding is fixed
    };
    a.edit("move clip", |s| s.tracks[0].events[0].at_ms = 1)
        .unwrap();
    b.edit("move clip", |s| s.tracks[0].events[0].at_ms = 2)
        .unwrap();
    drop(a);
    drop(b);
    let (_, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert!(
        !opened.history_reset,
        "two open packages made one journal that no longer replays"
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "timed, so run in release: cargo test --release -p wwav-session --test review edit_latency"
)]
fn edit_latency_on_a_large_session() {
    let tmp = tempfile::tempdir().unwrap();
    let mut s = sample_session(50_000);
    for i in 0..60 {
        let mut t = s.tracks[0].clone();
        t.id = format!("01JC5Q8V3M2T7R9X4K6W0YH{i:03}");
        for k in 0..200 {
            t.events
                .push(wwav_session::model::Event::new(common::CLIP, k * 100));
        }
        s.tracks.push(t);
    }
    let mut pkg = Package::create(tmp.path(), s, FakeClock::at(T0)).unwrap();
    let n = 20;
    let started = std::time::Instant::now();
    for i in 0..n {
        pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = i)
            .unwrap();
    }
    let per_edit = started.elapsed() / n as u32;
    let started = std::time::Instant::now();
    pkg.undo().unwrap();
    let undo = started.elapsed();
    eprintln!(
        "session.json {} bytes; one-number edit {:?}; undo {:?}",
        pkg.session().to_json_bytes().len(),
        per_edit,
        undo
    );
    // Writing the whole session as JSON took 237-305 ms an edit and an undo
    // 159-649 ms here; part by part it is a few ms, with the fsync.
    let budget = std::time::Duration::from_millis(50);
    assert!(per_edit < budget, "one-number edit took {per_edit:?}");
    assert!(undo < budget, "undo took {undo:?}");
}

#[test]
fn an_older_session_json_restored_from_git_opens_as_restored() {
    // 6.5 keeps sessions in git ("diffs line by line"), and the package's
    // own rule is that a session.json changed outside the app opens as it
    // is. But a session.json put back to an earlier saved version (git
    // checkout of the file, a backup) matches an old "save" line, so open
    // replays every later journal entry over it: the restore is silently
    // undone and reported as "Recovered N changes".
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    pkg.edit("rename", |s| s.title = "Version one".into())
        .unwrap();
    pkg.save().unwrap();
    let v1 = session_json(&pkg);
    pkg.edit("rename", |s| s.title = "Version two".into())
        .unwrap();
    pkg.save().unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    fs::write(dir.join("session.json"), &v1).unwrap(); // git checkout HEAD~1 -- session.json

    let (pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert_eq!(
        pkg.session().title,
        "Version one",
        "the restored file was replayed over: {:?}",
        opened.recovered.map(|r| r.sentence(0))
    );
}

#[test]
fn undo_after_two_windows_saved_never_panics() {
    // Two Package values on one package (a second window, or a helper), each
    // makes one change and saves. Reopening anchors on the last save line,
    // which names window B's file, and rebuilds an undo stack holding A's
    // change too. A's rows don't fit B's file, and Package::step `expect`s
    // that they do: the second ⌘Z panics, which takes the app down.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    let (mut a, _) = Package::open(&dir, clock.clone()).unwrap();
    let Ok((mut b, _)) = Package::open(&dir, clock.clone()) else {
        return; // refused: the finding is fixed
    };
    a.edit("rename", |s| s.title = "From A".into()).unwrap();
    a.save().unwrap();
    b.edit("move clip", |s| s.tracks[0].events[0].at_ms = 2)
        .unwrap();
    b.save().unwrap();
    drop(a);
    drop(b);

    let (mut pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    if opened.history_reset {
        return; // refused the mixed history instead: fine
    }
    let undone = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let first = pkg.undo();
        let second = pkg.undo();
        (first.is_ok(), second.is_ok())
    }));
    assert!(undone.is_ok(), "⌘Z panicked after reopening");
}

const SECOND_DIR: &str = "WWAV_REVIEW_SECOND_DIR";

#[test]
#[ignore = "the second window a_second_window_killed_mid_save_leaves_session_json_whole runs"]
fn review_second_window() {
    let Ok(dir) = env::var(SECOND_DIR) else {
        return;
    };
    let (mut pkg, _) = Package::open(&PathBuf::from(dir), Arc::new(SystemClock)).unwrap();
    say("open");
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).unwrap();
    for i in 0.. {
        pkg.edit("move clip", |s| s.tracks[1].events[0].at_ms = 5000 + i)
            .unwrap();
        let _ = pkg.save();
    }
}

#[test]
#[cfg(unix)]
fn a_second_window_killed_mid_save_leaves_session_json_whole() {
    // F6.4 with two Package values on one package: window A has staged a
    // save (session.json.tmp written and synced, the journal's save line
    // written). Window B, another process, saves: File::create truncates
    // the same session.json.tmp and starts writing, and is killed part way.
    // A's rename then puts B's half-written file under session.json, and
    // the session no longer opens.
    //
    // Fixed by the package lock: while B holds the package, A's open is
    // refused in words, so there is one writer; B is then killed part way
    // through a save, and session.json opens whole.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let pkg = Package::create(tmp.path(), sample_session(60_000), clock.clone()).unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);

    let mut child = Kid(Command::new(env::current_exe().unwrap())
        .args([
            "review_second_window",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(SECOND_DIR, &dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap());
    let mut to_child = child.0.stdin.take().unwrap();
    let mut from_child = BufReader::new(child.0.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        assert!(
            from_child.read_line(&mut line).unwrap() > 0,
            "the child stopped"
        );
        if line.contains("open") {
            break;
        }
    }

    // A, the second window.
    match Package::open(&dir, clock.clone()) {
        Ok(_) => panic!("a second window opened a package another process holds"),
        Err(e) => assert_eq!(
            e.to_string(),
            "This session is already open in another window."
        ),
    }

    writeln!(to_child).unwrap(); // B: edit and save, over and over
    let started = std::time::Instant::now();
    let writing = || {
        fs::read_dir(&dir).unwrap().any(|e| {
            let e = e.unwrap();
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with("session.json.")
                && name.ends_with(".tmp")
                && e.metadata().is_ok_and(|m| m.len() > 0)
        })
    };
    while !writing() {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(120),
            "B never started a save"
        );
    }
    drop(child); // SIGKILL mid-write

    let bytes = fs::read(dir.join("session.json")).unwrap();
    let opened = Package::open(&dir, clock.clone());
    assert!(
        opened.is_ok(),
        "session.json is half-written ({} bytes): {}",
        bytes.len(),
        opened.err().unwrap()
    );
}

#[test]
fn sessions_the_crate_calls_equal_save_to_the_same_bytes() {
    // F6.1 directly: two sessions the model calls the same (Session's ==,
    // like the journal's Value ==, has -0.0 == 0.0) are written as
    // different bytes, "pan": -0.0 against "pan": 0.0. Either the writer
    // should write one zero or the journal should tell them apart; as it
    // is, the journal records no change where the file changes.
    let a = sample_session(0);
    let mut b = a.clone();
    b.tracks[0].pan = -0.0;
    assert_eq!(a, b, "the model calls them the same session");
    assert!(
        a.to_json_bytes() == b.to_json_bytes(),
        "the same session saved twice gave different bytes"
    );
}
