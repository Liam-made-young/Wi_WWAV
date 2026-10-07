//! F6 in `docs/PLAN.md`. The `.wwavsession` fails if:
//!
//! 1. saving the same session twice gives different bytes;
//! 2. save, quit and reopen changes `session.json` by a byte or changes
//!    ⌘Z's label;
//! 3. killing the process mid-edit loses a journalled change;
//! 4. a crash mid-write leaves a half-written `session.json`.
//!
//! 1 and 2 are here; 3 and 4 kill a real child process in `crash.rs`.

mod common;

use std::fs;

use common::{note, sample_session, FakeClock, MIDI_CLIP, T0};
use serde_json::{json, Value};
use wwav_session::model::{Event, Session};
use wwav_session::Package;

fn session_json(pkg: &Package) -> Vec<u8> {
    fs::read(pkg.dir().join("session.json")).unwrap()
}

#[test]
fn saving_the_same_session_twice_gives_the_same_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let s = sample_session(40);

    // The same session written by two packages.
    let a = Package::create(&tmp.path().join("a"), s.clone(), clock.clone()).unwrap();
    let b = Package::create(&tmp.path().join("b"), s.clone(), clock.clone()).unwrap();
    assert_eq!(session_json(&a), session_json(&b));
    assert_eq!(session_json(&a), s.to_json_bytes());

    // A real second write of the same state: edit, save, undo, save.
    let mut a = a;
    let first = session_json(&a);
    a.edit("move clip", |s| s.tracks[0].events[0].at_ms = 4000)
        .unwrap();
    a.save().unwrap();
    assert_ne!(session_json(&a), first);
    a.undo().unwrap();
    a.save().unwrap();
    assert_eq!(session_json(&a), first);

    // The same state reached by another route: notes added in another
    // order, unknown keys inserted in another order.
    let mut x = s.clone();
    let mut y = s.clone();
    x.extra.insert("zeta".into(), json!(1));
    x.extra.insert("alpha".into(), json!({"b": 2, "a": 1}));
    y.extra.insert("alpha".into(), json!({"a": 1, "b": 2}));
    y.extra.insert("zeta".into(), json!(1));
    x.midi
        .insert("01JC5Q8V3M2T7R9X4K6W0YHZNZ".into(), vec![note(1)]);
    y.midi.clear();
    y.midi
        .insert("01JC5Q8V3M2T7R9X4K6W0YHZNZ".into(), vec![note(1)]);
    y.midi.insert(MIDI_CLIP.into(), s.midi[MIDI_CLIP].clone());
    assert_eq!(x.to_json_bytes(), y.to_json_bytes());

    // A file written with every object's keys reversed reads back to the
    // same bytes.
    let mut v: Value = serde_json::from_slice(&x.to_json_bytes()).unwrap();
    reverse_keys(&mut v);
    let scrambled = serde_json::to_vec_pretty(&v).unwrap();
    assert_ne!(scrambled, x.to_json_bytes());
    let back = Session::from_json_bytes(&scrambled).unwrap();
    assert_eq!(back.to_json_bytes(), x.to_json_bytes());
}

fn reverse_keys(v: &mut Value) {
    match v {
        Value::Object(m) => {
            let mut entries: Vec<(String, Value)> = std::mem::take(m).into_iter().collect();
            entries.reverse();
            for (k, mut x) in entries {
                reverse_keys(&mut x);
                m.insert(k, x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(reverse_keys),
        _ => {}
    }
}

#[test]
fn session_json_is_utf8_lf_two_space_indented() {
    let mut s = sample_session(1);
    s.title = "Låg tide — 夜".into();
    let bytes = s.to_json_bytes();
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains('\r'));
    assert!(text.ends_with("}\n"));
    assert!(
        text.contains("\"title\": \"Låg tide — 夜\""),
        "UTF-8, not \\u escapes"
    );
    assert!(text.starts_with("{\n  \"wwavsession\": \"0.1\",\n  \"id\": "));
    assert!(text.contains("\n    {\n      \"id\": \"01JC5Q8V3M2T7R9X4K6W0YHZT0\""));
}

#[test]
fn save_quit_and_reopen_changes_no_byte_and_keeps_the_undo_label() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(8), clock.clone()).unwrap();
    let first = pkg.session().to_json_bytes();

    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 2500)
        .unwrap();
    pkg.edit("add note", |s| {
        s.midi.get_mut(MIDI_CLIP).unwrap().push(note(99))
    })
    .unwrap();
    pkg.edit("set tempo", |s| s.tempo_map[0].bpm = 86.0)
        .unwrap();
    pkg.undo().unwrap();
    assert_eq!(pkg.undo_label(), "Undo add note");
    assert_eq!(pkg.redo_label(), "Redo set tempo");
    pkg.save().unwrap();
    let saved = session_json(&pkg);
    let dir = pkg.dir().to_path_buf();
    drop(pkg); // quit

    let (mut pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert!(
        opened.recovered.is_none(),
        "nothing to recover after a save"
    );
    assert!(!opened.history_reset);
    assert_eq!(session_json(&pkg), saved, "opening rewrote session.json");
    assert_eq!(pkg.session().to_json_bytes(), saved);
    assert_eq!(pkg.undo_label(), "Undo add note");
    assert_eq!(pkg.redo_label(), "Redo set tempo");

    // Saving the reopened session changes nothing either.
    pkg.save().unwrap();
    assert_eq!(session_json(&pkg), saved);

    // The history still works after the relaunch.
    assert_eq!(pkg.undo().unwrap().as_deref(), Some("add note"));
    assert_eq!(pkg.undo().unwrap().as_deref(), Some("move clip"));
    assert_eq!(pkg.undo().unwrap(), None);
    assert_eq!(pkg.undo_label(), "Nothing to undo.");
    assert_eq!(pkg.session().to_json_bytes(), first);
    assert_eq!(pkg.redo().unwrap().as_deref(), Some("move clip"));
    assert_eq!(pkg.redo_label(), "Redo add note");
}

#[test]
fn a_change_with_no_effect_is_not_journalled() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pkg = Package::create(tmp.path(), sample_session(0), FakeClock::at(T0)).unwrap();
    assert!(!pkg.edit("nothing", |_| {}).unwrap());
    assert_eq!(pkg.undo_label(), "Nothing to undo.");
    assert_eq!(pkg.redo_label(), "Nothing to redo.");
}

#[test]
fn a_change_that_could_not_be_reopened_is_refused() {
    // JSON has no NaN: serde writes it as null, which would leave a
    // session.json that no longer opens.
    let tmp = tempfile::tempdir().unwrap();
    let mut pkg = Package::create(tmp.path(), sample_session(0), FakeClock::at(T0)).unwrap();
    let before = pkg.session().to_json_bytes();
    assert!(pkg
        .edit("set gain", |s| s.tracks[0].gain_db = f64::NAN)
        .is_err());
    assert_eq!(pkg.session().to_json_bytes(), before);
    assert_eq!(pkg.undo_label(), "Nothing to undo.");
}

#[test]
fn what_is_journalled_is_what_reopens() {
    // An optional float set to NaN is written as null and reads back as
    // absent. The session the package holds must be the one that reopens,
    // or a later undo would fit the document in memory and not the file.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    pkg.edit("set beat", |s| s.tracks[0].events[0].at_beats = Some(2.0))
        .unwrap();
    pkg.edit("set beat", |s| {
        s.tracks[0].events[0].at_beats = Some(f64::NAN)
    })
    .unwrap();
    assert_eq!(pkg.session().tracks[0].events[0].at_beats, None);
    let held = pkg.session().to_json_bytes();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    let (mut pkg, _) = Package::open(&dir, clock.clone()).unwrap();
    assert_eq!(pkg.session().to_json_bytes(), held);
    pkg.undo().unwrap();
    assert_eq!(pkg.session().tracks[0].events[0].at_beats, Some(2.0));
}

#[test]
fn unknown_keys_survive_open_edit_and_save() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let pkg = Package::create(tmp.path(), sample_session(2), clock.clone()).unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);

    // A newer app wrote keys this one doesn't know, at every depth.
    let path = dir.join("session.json");
    let mut v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    v["markers"] = json!([{"at_beats": 16.0, "label": "chorus"}]);
    v["tracks"][0]["color"] = json!("#ff6a00");
    v["tracks"][0]["events"][0]["fade_in_ms"] = json!(12);
    v["tracks"][0]["events"][0]["params"]["stretch"] = json!({"mode": "elastique"});
    v["midi"][MIDI_CLIP][0]["chance"] = json!(0.5);
    v["lineage"]["work"]["isrc"] = json!("QZ-ABC-26-00001");
    v["frame"] = json!({"w": 1920, "h": 1080, "fps": 24, "aspect": "16:9"});
    fs::write(&path, serde_json::to_vec_pretty(&v).unwrap()).unwrap();

    let (mut pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert!(
        opened.history_reset,
        "a file changed outside the app starts a new history"
    );
    pkg.edit("move clip", |s| s.tracks[1].events[0].at_ms = 7000)
        .unwrap();
    pkg.save().unwrap();

    let w: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(w["markers"], v["markers"]);
    assert_eq!(w["tracks"][0]["color"], json!("#ff6a00"));
    assert_eq!(w["tracks"][0]["events"][0]["fade_in_ms"], json!(12));
    assert_eq!(
        w["tracks"][0]["events"][0]["params"]["stretch"],
        json!({"mode": "elastique"})
    );
    assert_eq!(w["midi"][MIDI_CLIP][0]["chance"], json!(0.5));
    assert_eq!(w["lineage"]["work"]["isrc"], json!("QZ-ABC-26-00001"));
    assert_eq!(w["frame"]["aspect"], json!("16:9"));
    assert_eq!(w["tracks"][1]["events"][0]["at_ms"], json!(7000));

    // Unknown values of known kinds survive too.
    let mut v = w.clone();
    v["tracks"][0]["kind"] = json!("folder");
    v["tracks"][0]["events"][0]["params"]["time"] = json!("stretch");
    let s = Session::from_json_bytes(&serde_json::to_vec(&v).unwrap()).unwrap();
    let back: Value = serde_json::from_slice(&s.to_json_bytes()).unwrap();
    assert_eq!(back, v);
}

#[test]
fn notes_keep_where_they_were_played() {
    let mut s = sample_session(0);
    let mut n = note(3);
    n.at_beats = 12.5; // quantized
    n.played_at_beats = 12.4871;
    n.played_len_beats = 0.2310;
    s.midi.get_mut(MIDI_CLIP).unwrap().push(n);
    let back = Session::from_json_bytes(&s.to_json_bytes()).unwrap();
    let m = &back.midi[MIDI_CLIP][0];
    assert_eq!(m.at_beats, 12.5);
    assert_eq!(m.played_at_beats, 12.4871);
    assert_eq!(m.played_len_beats, 0.2310);
}

#[test]
fn a_history_that_does_not_fit_session_json_starts_again_and_never_panics() {
    // Two copies of one package edited apart (two windows, before the
    // lock), their journals joined: open anchors on B's save, but A's
    // change on the undo stack doesn't fit B's file. Fails if open keeps
    // that history (⌘Z then panicked), or loses B's saved session.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let pkg = Package::create(&tmp.path().join("a"), sample_session(0), clock.clone()).unwrap();
    let a_dir = pkg.dir().to_path_buf();
    drop(pkg);
    let b_dir = tmp.path().join("b.wwavsession");
    copy_dir(&a_dir, &b_dir);

    let (mut a, _) = Package::open(&a_dir, clock.clone()).unwrap();
    a.edit("rename", |s| s.title = "From A".into()).unwrap();
    a.save().unwrap();
    drop(a);
    let (mut b, _) = Package::open(&b_dir, clock.clone()).unwrap();
    b.edit("move clip", |s| s.tracks[0].events[0].at_ms = 2)
        .unwrap();
    b.save().unwrap();
    drop(b);
    // A's journal with B's lines after the start line they share, and B's
    // session.json.
    let a_lines = fs::read_to_string(a_dir.join("journal/undo.ndjson")).unwrap();
    let b_lines = fs::read_to_string(b_dir.join("journal/undo.ndjson")).unwrap();
    let joined: String = a_lines
        .lines()
        .chain(b_lines.lines().skip(1))
        .map(|l| format!("{l}\n"))
        .collect();
    fs::write(a_dir.join("journal/undo.ndjson"), joined).unwrap();
    let saved = fs::read(b_dir.join("session.json")).unwrap();
    fs::write(a_dir.join("session.json"), &saved).unwrap();

    let (mut pkg, opened) = Package::open(&a_dir, clock.clone()).unwrap();
    assert!(opened.history_reset);
    assert_eq!(session_json(&pkg), saved);
    assert_eq!(pkg.undo_label(), "Nothing to undo.");
    assert_eq!(pkg.undo().unwrap(), None);
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            copy_dir(&path, &to.join(entry.file_name()));
        } else {
            fs::copy(&path, to.join(entry.file_name())).unwrap();
        }
    }
}

/// A tiny deterministic generator, so a failure names its seed.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// One random edit of the kinds the Console makes.
fn random_edit(pkg: &mut Package, rng: &mut Rng) {
    let k = rng.next();
    let r = rng.below(8);
    pkg.edit(&format!("edit {r}"), |s| match r {
        0 => s.tracks[(k % 2) as usize].events[0].at_ms = (k % 100_000) as i64,
        1 => s.midi.get_mut(MIDI_CLIP).unwrap().push(note(k)),
        2 => {
            let notes = s.midi.get_mut(MIDI_CLIP).unwrap();
            if !notes.is_empty() {
                let i = (k as usize) % notes.len();
                notes.remove(i);
            }
        }
        3 => {
            let notes = s.midi.get_mut(MIDI_CLIP).unwrap();
            let i = (k as usize) % (notes.len() + 1);
            notes.insert(i, note(k));
        }
        4 => s.tracks[1]
            .events
            .push(Event::new("01JC5Q8V3M2T7R9X4K6W0YHZNE", k as i64 % 5000)),
        5 => s.title = format!("Take {}", k % 1000),
        6 => s.tracks[0].gain_db = -((k % 600) as f64) / 10.0,
        _ => {
            s.midi.insert(
                format!("01JC5Q8V3M2T7R9X4K6W0YH{:03}", k % 1000),
                vec![note(k)],
            );
        }
    })
    .unwrap();
}

#[test]
fn undo_all_returns_the_first_state_and_redo_all_the_last_byte_for_byte() {
    for seed in 1..=12u64 {
        let tmp = tempfile::tempdir().unwrap();
        let clock = FakeClock::at(T0);
        let mut pkg = Package::create(tmp.path(), sample_session(30), clock.clone()).unwrap();
        let first = pkg.session().to_json_bytes();
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for i in 0..120 {
            random_edit(&mut pkg, &mut rng);
            // Undo now and then, so the run includes undos and new branches.
            if rng.below(5) == 0 {
                pkg.undo().unwrap();
            }
            if i % 40 == 39 {
                // Relaunch in the middle of the run, sometimes unsaved.
                if rng.below(2) == 0 {
                    pkg.save().unwrap();
                }
                let dir = pkg.dir().to_path_buf();
                drop(pkg);
                pkg = Package::open(&dir, clock.clone()).unwrap().0;
            }
        }
        // The last state is the end of the current branch.
        while pkg.redo().unwrap().is_some() {}
        let last = pkg.session().to_json_bytes();
        while pkg.undo().unwrap().is_some() {}
        assert_eq!(
            pkg.session().to_json_bytes(),
            first,
            "seed {seed}: undo all"
        );
        while pkg.redo().unwrap().is_some() {}
        assert_eq!(pkg.session().to_json_bytes(), last, "seed {seed}: redo all");
    }
}

proptest::proptest! {
    /// Any finite float a session holds reads back as the same float, so
    /// reopening never changes a byte.
    #[test]
    fn every_float_reads_back_as_written(gain in proptest::num::f64::NORMAL | proptest::num::f64::SUBNORMAL | proptest::num::f64::ZERO, at in 0.0..1e6f64, bpm in 20.0..400.0f64) {
        let mut s = sample_session(1);
        s.tracks[0].gain_db = gain;
        s.midi.get_mut(MIDI_CLIP).unwrap()[0].played_at_beats = at;
        s.tempo_map[0].bpm = bpm;
        let text = String::from_utf8(s.to_json_bytes()).unwrap();
        let back = Session::from_json_bytes(text.as_bytes()).unwrap();
        proptest::prop_assert_eq!(String::from_utf8(back.to_json_bytes()).unwrap(), text);
        // Zero has one form: -0.0 is written, and reads back, as 0.0.
        let want = if gain == 0.0 { 0.0f64 } else { gain };
        proptest::prop_assert_eq!(back.tracks[0].gain_db.to_bits(), want.to_bits());
    }
}
