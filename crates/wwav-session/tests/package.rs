//! The package around session.json (`docs/SPEC.md` 6.5). Each test names
//! the fail it catches.

mod common;

use std::fs;
use std::io::Read;
use std::path::Path;

use common::{sample_session, FakeClock, MIDI_CLIP, T0};
use wwav_session::model::Device;
use wwav_session::{
    copying_sentence, folder_name, older_version_sentence, Autosaver, Link, Package,
};

const MINUTE: u64 = 60_000;
const DAY: u64 = 24 * 60 * MINUTE;

#[test]
fn a_new_package_has_its_folders_and_is_named_by_its_ulid() {
    // Fails if: a folder from 6.5 is missing, or the package isn't
    // `<ULID>.wwavsession`.
    let tmp = tempfile::tempdir().unwrap();
    let pkg = Package::create(tmp.path(), sample_session(0), FakeClock::at(T0)).unwrap();
    assert_eq!(
        pkg.dir(),
        tmp.path().join("01JC5Q8V3M2T7R9X4K6W0YHZN0.wwavsession")
    );
    for name in ["media", "plugin-state", "renders", "journal", "cache"] {
        assert!(pkg.dir().join(name).is_dir(), "{name}/ missing");
    }
    assert!(pkg.dir().join("session.json").is_file());
    assert!(pkg.dir().join("journal/undo.ndjson").is_file());
    // A second create of the same session refuses rather than overwrite.
    assert!(Package::create(tmp.path(), sample_session(0), FakeClock::at(T0)).is_err());
}

#[test]
fn a_package_missing_a_folder_gets_it_back_on_open() {
    let tmp = tempfile::tempdir().unwrap();
    let pkg = Package::create(tmp.path(), sample_session(0), FakeClock::at(T0)).unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    fs::remove_dir(dir.join("cache")).unwrap();
    Package::open(&dir, FakeClock::at(T0)).unwrap();
    assert!(dir.join("cache").is_dir());
}

#[test]
fn a_newer_major_version_is_refused_in_words() {
    let tmp = tempfile::tempdir().unwrap();
    let pkg = Package::create(tmp.path(), sample_session(0), FakeClock::at(T0)).unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    let path = dir.join("session.json");
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("\"0.1\"", "\"1.0\"");
    fs::write(&path, text).unwrap();
    let err = Package::open(&dir, FakeClock::at(T0)).err().unwrap();
    assert_eq!(
        err.to_string(),
        "This session was saved by a newer Wi_WWAV (wwavsession 1.0)."
    );
}

#[test]
fn recovery_names_how_many_changes_and_from_when() {
    // Fails if: unsaved journalled changes aren't replayed on open, or the
    // sentence differs from 6.5's.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    for i in 0..14 {
        clock.advance(1000);
        pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 100 * i + 1)
            .unwrap();
    }
    let dir = pkg.dir().to_path_buf();
    let expected = pkg.session().to_json_bytes();
    drop(pkg); // never saved

    let (pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert_eq!(pkg.session().to_json_bytes(), expected);
    let r = opened.recovered.unwrap();
    assert_eq!(r.changes, 14);
    assert_eq!(r.since_ms, T0 + 1000);
    // T0 is 21:12 UTC; the app passes the local offset.
    assert_eq!(r.sentence(0), "Recovered 14 changes from 21:12.");
    assert_eq!(r.sentence(-7 * 60), "Recovered 14 changes from 14:12.");
    assert_eq!(r.sentence(5 * 60 + 30), "Recovered 14 changes from 02:42.");
    assert!(pkg.is_dirty(), "the recovered changes still need writing");
}

#[test]
fn one_recovered_change_is_singular() {
    let r = wwav_session::Recovered {
        changes: 1,
        since_ms: T0,
    };
    assert_eq!(r.sentence(0), "Recovered 1 change from 21:12.");
}

#[test]
fn a_torn_last_journal_line_is_dropped_not_fatal() {
    // A crash mid-append leaves part of a line. That edit never returned, so
    // it was never journalled; everything before it must still come back.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 1234)
        .unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    let journal = dir.join("journal/undo.ndjson");
    let mut bytes = fs::read(&journal).unwrap();
    bytes.extend_from_slice(b"{\"op\":\"txn\",\"id\":\"01JC");
    fs::write(&journal, &bytes).unwrap();

    let (mut pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert_eq!(opened.recovered.unwrap().changes, 1);
    assert_eq!(pkg.session().tracks[0].events[0].at_ms, 1234);
    // And the journal keeps working after the torn line.
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 99)
        .unwrap();
    drop(pkg);
    let (pkg, _) = Package::open(&dir, clock.clone()).unwrap();
    assert_eq!(pkg.session().tracks[0].events[0].at_ms, 99);
}

#[test]
fn a_session_json_changed_outside_the_app_keeps_its_content_and_starts_a_new_history() {
    // Fails if: hand edits are overwritten by a replay, or old undo entries
    // are applied to a file they weren't made against.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 1234)
        .unwrap();
    pkg.save().unwrap();
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    let path = dir.join("session.json");
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("Low Tide", "High Tide");
    fs::write(&path, text).unwrap();

    let (pkg, opened) = Package::open(&dir, clock.clone()).unwrap();
    assert!(opened.history_reset);
    assert!(opened.recovered.is_none());
    assert_eq!(pkg.session().title, "High Tide");
    assert_eq!(pkg.undo_label(), "Nothing to undo.");
    // The old history is kept beside the new one, never deleted.
    let kept = fs::read_dir(dir.join("journal"))
        .unwrap()
        .filter(|e| {
            let name = e.as_ref().unwrap().file_name().into_string().unwrap();
            name.starts_with("undo-") && name.ends_with(".ndjson")
        })
        .count();
    assert_eq!(kept, 1);
}

#[test]
fn autosave_waits_two_seconds_after_the_last_change_or_a_transport_stop() {
    // Fails if: it saves before 2 s of quiet, doesn't save at 2 s, or a
    // transport stop doesn't save at once.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    let path = pkg.dir().join("session.json");
    let mut auto = Autosaver::new();
    assert_eq!(auto.due_in_ms(&pkg), None);
    assert!(!auto.tick(&mut pkg).unwrap());

    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 1)
        .unwrap();
    assert_eq!(auto.due_in_ms(&pkg), Some(2000));
    clock.advance(1500);
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 2)
        .unwrap();
    clock.advance(1999);
    assert_eq!(auto.due_in_ms(&pkg), Some(1));
    assert!(!auto.tick(&mut pkg).unwrap());
    assert!(!fs::read_to_string(&path).unwrap().contains("\"at_ms\": 2"));
    clock.advance(1);
    assert_eq!(auto.due_in_ms(&pkg), Some(0));
    assert!(auto.tick(&mut pkg).unwrap());
    assert!(fs::read_to_string(&path).unwrap().contains("\"at_ms\": 2"));
    assert!(!pkg.is_dirty());
    assert_eq!(auto.due_in_ms(&pkg), None);

    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 3)
        .unwrap();
    clock.advance(100);
    auto.transport_stopped();
    assert_eq!(auto.due_in_ms(&pkg), Some(0));
    assert!(auto.tick(&mut pkg).unwrap());
    assert!(fs::read_to_string(&path).unwrap().contains("\"at_ms\": 3"));
    // A stop with nothing changed saves nothing.
    auto.transport_stopped();
    assert!(!auto.tick(&mut pkg).unwrap());
}

fn snapshot_count(pkg: &Package) -> usize {
    pkg.snapshots().unwrap().len()
}

#[test]
fn snapshots_every_ten_minutes_of_work_kept_thirty_days() {
    // Fails if: two snapshots are less than 10 minutes apart, a quiet hour
    // makes one, or one older than 30 days survives the next save or open.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(0), clock.clone()).unwrap();
    assert_eq!(snapshot_count(&pkg), 1, "the session as it was made");

    // A change saved every minute for 25 minutes: snapshots at 10 and 20.
    for i in 1..=25 {
        clock.advance(MINUTE);
        pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = i)
            .unwrap();
        pkg.save().unwrap();
    }
    let snaps = pkg.snapshots().unwrap();
    assert_eq!(snaps.len(), 3);
    assert_eq!(snaps[0].at_ms, T0 + 20 * MINUTE, "newest first");
    assert_eq!(snaps[1].at_ms, T0 + 10 * MINUTE);
    assert_eq!(snaps[2].at_ms, T0);

    // An hour with no work makes none.
    clock.advance(60 * MINUTE);
    pkg.save().unwrap();
    assert_eq!(snapshot_count(&pkg), 3);

    // 29 days on, all three are kept; a day later the first goes.
    clock.advance(29 * DAY - 60 * MINUTE - 25 * MINUTE + 5 * MINUTE);
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 100)
        .unwrap();
    pkg.save().unwrap();
    assert_eq!(snapshot_count(&pkg), 4);
    clock.advance(DAY);
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 101)
        .unwrap();
    pkg.save().unwrap();
    let snaps = pkg.snapshots().unwrap();
    assert!(
        snaps.iter().all(|s| s.at_ms > T0),
        "the 30-day-old snapshot survived"
    );
    assert_eq!(snaps.len(), 4);

    // Opening prunes too.
    let dir = pkg.dir().to_path_buf();
    drop(pkg);
    clock.advance(31 * DAY);
    let (pkg, _) = Package::open(&dir, clock.clone()).unwrap();
    assert_eq!(snapshot_count(&pkg), 0);
}

#[test]
fn revert_to_a_snapshot_is_one_undoable_change() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut pkg = Package::create(tmp.path(), sample_session(3), clock.clone()).unwrap();
    let original = pkg.session().to_json_bytes();
    clock.advance(11 * MINUTE);
    pkg.edit("rename", |s| s.title = "Later".into()).unwrap();
    pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 999)
        .unwrap();
    pkg.save().unwrap();
    let later = pkg.session().to_json_bytes();
    let first = pkg.snapshots().unwrap().pop().unwrap();
    assert_eq!(first.at_ms, T0);

    pkg.revert_to(&first).unwrap();
    assert_eq!(pkg.session().to_json_bytes(), original);
    assert_eq!(pkg.undo_label(), "Undo revert");
    pkg.undo().unwrap();
    assert_eq!(pkg.session().to_json_bytes(), later);
}

#[test]
fn plugin_state_inline_up_to_256_kb_else_a_file_written_once() {
    // Fails if: a state over 256 KB sits inline, one at 256 KB doesn't, a
    // file state is rewritten when unchanged, or a state doesn't read back.
    let tmp = tempfile::tempdir().unwrap();
    let pkg = Package::create(tmp.path(), sample_session(0), FakeClock::at(T0)).unwrap();

    let small = vec![7u8; 256 * 1024];
    let s = pkg.store_plugin_state(&small, None).unwrap();
    assert!(s.inline.is_some() && s.file.is_none());
    assert_eq!(s.bytes, 256 * 1024);
    assert_eq!(pkg.read_plugin_state(&s).unwrap(), small);

    let big: Vec<u8> = (0..40 * 1024 * 1024u32).map(|i| (i % 251) as u8).collect();
    let b = pkg.store_plugin_state(&big, None).unwrap();
    assert!(b.inline.is_none());
    let file = b.file.clone().unwrap();
    assert!(file.starts_with("plugin-state/") && file.ends_with(".bin"));
    assert_eq!(
        file.len(),
        "plugin-state/".len() + 26 + ".bin".len(),
        "ULID-named"
    );
    assert_eq!(b.bytes, big.len() as u64);
    assert_eq!(pkg.read_plugin_state(&b).unwrap(), big);
    let path = pkg.dir().join(&file);
    let modified = fs::metadata(&path).unwrap().modified().unwrap();

    // Unchanged: the same reference back, the file untouched, no new file.
    std::thread::sleep(std::time::Duration::from_millis(20));
    let again = pkg.store_plugin_state(&big, Some(&b)).unwrap();
    assert_eq!(again, b);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    assert_eq!(
        fs::read_dir(pkg.dir().join("plugin-state"))
            .unwrap()
            .count(),
        1
    );

    // Changed: a new file; the old one stays for undo.
    let mut changed = big.clone();
    changed[0] ^= 1;
    let c = pkg.store_plugin_state(&changed, Some(&b)).unwrap();
    assert_ne!(c.file, b.file);
    assert!(path.exists());

    // A file that no longer matches its sha256 is refused.
    fs::write(&path, b"not the state").unwrap();
    assert!(pkg.read_plugin_state(&b).is_err());
}

#[test]
fn an_older_installed_plugin_is_named() {
    let d = Device {
        name: "Tape Echo".into(),
        version: "2.1.4".into(),
        ..Device::default()
    };
    assert_eq!(
        d.older_version_sentence("2.0.0").as_deref(),
        Some("Saved with 'Tape Echo' 2.1.4. You have 2.0.0, so its settings may not load.")
    );
    assert_eq!(d.older_version_sentence("2.1.4"), None);
    assert_eq!(d.older_version_sentence("2.10.0"), None);
    assert_eq!(older_version_sentence("Grand Piano", "1.3.0", "1.3"), None);
    assert!(older_version_sentence("Grand Piano", "1.10", "1.9.9").is_some());
}

#[test]
fn media_is_linked_the_cheapest_correct_way_and_says_so() {
    // Fails if: a file on the same volume is copied when it could be
    // cloned or hard-linked, a file on another volume isn't copied, the
    // name isn't the library's ULID, or the sheet's sentence differs.
    let tmp = tempfile::tempdir().unwrap();
    let pkg = Package::create(tmp.path(), sample_session(0), FakeClock::at(T0)).unwrap();
    let library = tmp.path().join("library-media");
    fs::create_dir(&library).unwrap();
    let src = library.join("01JC5Q8V3M2T7R9X4K6W0YHZNB.wwav");
    fs::write(&src, vec![3u8; 100_000]).unwrap();

    let got = pkg.import_media(&src).unwrap();
    assert_eq!(got.id, "01JC5Q8V3M2T7R9X4K6W0YHZNB");
    assert_eq!(got.file, "media/01JC5Q8V3M2T7R9X4K6W0YHZNB.wwav");
    assert_eq!(got.bytes, 100_000);
    // This machine's ext4 can't clone; a filesystem that can (APFS, btrfs,
    // XFS) gives Clone.
    assert!(
        matches!(got.link, Link::Clone | Link::HardLink),
        "{:?}",
        got.link
    );
    assert_eq!(
        fs::read(pkg.dir().join(&got.file)).unwrap(),
        fs::read(&src).unwrap()
    );
    assert_eq!(
        pkg.media_path("01JC5Q8V3M2T7R9X4K6W0YHZNB"),
        Some(pkg.dir().join(&got.file))
    );

    // The same file again is already there.
    assert_eq!(pkg.import_media(&src).unwrap().link, Link::Present);

    // A file not named by a ULID gets one.
    let plain = library.join("Voice Memo.M4A");
    fs::write(&plain, b"memo").unwrap();
    let got = pkg.import_media(&plain).unwrap();
    assert!(wwav_ids::is_ulid(&got.id));
    assert_eq!(got.file, format!("media/{}.m4a", got.id));

    // Across volumes neither clone nor link works: a copy.
    let shm = Path::new("/dev/shm");
    if shm.is_dir() {
        let other = tempfile::tempdir_in(shm).unwrap();
        let far = other.path().join("01JC5Q8V3M2T7R9X4K6W0YHZNF.wav");
        fs::write(&far, vec![9u8; 5000]).unwrap();
        let got = pkg.import_media(&far).unwrap();
        assert_eq!(got.link, Link::Copy);
        assert_eq!(
            fs::read(pkg.dir().join(&got.file)).unwrap(),
            vec![9u8; 5000]
        );
    }

    assert_eq!(
        copying_sentence(1_400_000_000),
        "Copying 1.4 GB of media into the session."
    );
    assert_eq!(
        copying_sentence(210_000_000),
        "Copying 210 MB of media into the session."
    );
    assert_eq!(
        copying_sentence(48_000),
        "Copying 48 KB of media into the session."
    );
}

#[test]
fn titles_become_folder_names_as_pranas_disc_writer_does() {
    assert_eq!(folder_name("Low Tide"), "Low Tide");
    assert_eq!(folder_name("a/b\\c:d*e?f\"g<h>i|j"), "a b c d e f g h i j");
    assert_eq!(folder_name("  what?  now  "), "what now");
    assert_eq!(folder_name("Låg tide — 夜"), "Låg tide — 夜");
    assert_eq!(folder_name("tab\there\nnewline"), "tab here newline");
    assert_eq!(folder_name("???"), "Untitled");
    assert_eq!(folder_name(""), "Untitled");
}

#[test]
fn send_session_writes_one_stored_zip_without_cache() {
    // Fails if: the zip is compressed, holds cache/, misses a file, isn't
    // named `<Title>.wwavsession.zip`, or isn't the saved session.
    let tmp = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(T0);
    let mut s = sample_session(5);
    s.title = "Low: Tide?".into();
    let mut pkg = Package::create(&tmp.path().join("sessions"), s, clock.clone()).unwrap();
    let media = tmp.path().join("01JC5Q8V3M2T7R9X4K6W0YHZNB.wav");
    fs::write(&media, vec![1u8; 70_000]).unwrap();
    pkg.import_media(&media).unwrap();
    let big: Vec<u8> = (0..300_000u32).map(|i| i as u8).collect();
    let state = pkg.store_plugin_state(&big, None).unwrap();
    fs::write(pkg.dir().join("cache/peaks.bin"), b"peaks").unwrap();
    fs::write(
        pkg.dir().join("renders/01JC5Q8V3M2T7R9X4K6W0YHZNR.wwav"),
        b"render",
    )
    .unwrap();
    pkg.edit("rename track", |s| s.tracks[0].name = "Keys".into())
        .unwrap(); // unsaved

    let out = tmp.path().join("out");
    fs::create_dir(&out).unwrap();
    let zip_path = pkg.send(&out).unwrap();
    assert_eq!(zip_path, out.join("Low Tide.wwavsession.zip"));
    assert!(!out.join("Low Tide.wwavsession.zip.part").exists());

    let mut zip = zip::ZipArchive::new(fs::File::open(&zip_path).unwrap()).unwrap();
    let mut names = Vec::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).unwrap();
        assert_eq!(
            f.compression(),
            zip::CompressionMethod::Stored,
            "{}",
            f.name()
        );
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).unwrap(); // checks the CRC
        names.push(f.name().to_string());
        if !f.is_dir() {
            let on_disk = fs::read(
                pkg.dir()
                    .join(f.name().strip_prefix("Low Tide.wwavsession/").unwrap()),
            )
            .unwrap();
            assert_eq!(bytes, on_disk, "{}", f.name());
        }
    }
    assert!(names.iter().all(|n| n.starts_with("Low Tide.wwavsession/")));
    assert!(names.iter().all(|n| !n.contains("/cache")), "{names:?}");
    for want in [
        "Low Tide.wwavsession/session.json",
        "Low Tide.wwavsession/media/01JC5Q8V3M2T7R9X4K6W0YHZNB.wav",
        "Low Tide.wwavsession/renders/01JC5Q8V3M2T7R9X4K6W0YHZNR.wwav",
        "Low Tide.wwavsession/journal/undo.ndjson",
        &format!("Low Tide.wwavsession/{}", state.file.unwrap()),
    ] {
        assert!(
            names.iter().any(|n| n == want),
            "{want} missing from {names:?}"
        );
    }
    // The zip holds the session as it stands, saved first.
    let mut json = String::new();
    zip.by_name("Low Tide.wwavsession/session.json")
        .unwrap()
        .read_to_string(&mut json)
        .unwrap();
    assert!(json.contains("\"name\": \"Keys\""));
    assert!(!pkg.is_dirty());

    // Unzipped elsewhere, it opens with its history.
    let there = tmp.path().join("there");
    zip.extract(&there).unwrap();
    let (other, opened) =
        Package::open(&there.join("Low Tide.wwavsession"), clock.clone()).unwrap();
    assert!(!opened.history_reset);
    assert_eq!(other.undo_label(), "Undo rename track");
    assert_eq!(other.session().midi[MIDI_CLIP].len(), 5);
}
