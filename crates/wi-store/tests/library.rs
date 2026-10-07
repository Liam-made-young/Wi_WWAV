//! The library (docs/PLAN.md S1.7, store side; S1.9, the query half). It fails
//! if import names a file anything but a ULID; renaming a song renames a file;
//! a plain WAV comes in as anything but master only, without saying so; a tag
//! isn't lowercase or a clip takes a 13th; a fifth pin fits; a delete removes a
//! file; the queue is anything but `published_at IS NOT NULL AND remote_id IS
//! NULL`; or a resumed upload would re-send a finished part.

mod common;

use common::*;
use rusqlite::Connection;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;
use wi_store::*;

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn import(store: &mut Store, dir: &Path, name: &str) -> Clip {
    let src = source_file(dir, name, format!("bytes of {name}").as_bytes());
    store.import(Room::Library, &src, &ByExtension).unwrap()
}

fn edit(
    store: &mut Store,
    label: &str,
    f: impl FnOnce(&mut Txn) -> Result<()>,
) -> Result<Option<String>> {
    let mut tx = store.begin(Room::Library, label)?;
    f(&mut tx)?;
    tx.commit()
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn refused<T: std::fmt::Debug>(result: Result<T>) -> String {
    match result {
        Err(Error::Refused(sentence)) => sentence,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn opening_makes_the_library_folder() {
    let (_dir, store) = library();
    let root = store.root();
    assert!(root.join("library.sqlite").is_file());
    for folder in ["media", "sessions", "trash"] {
        assert!(root.join(folder).is_dir(), "{folder}/ is missing");
    }
    let conn = Connection::open(root.join("library.sqlite")).unwrap();
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    let version: usize = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    let fts5: bool = conn
        .query_row("SELECT sqlite_compileoption_used('ENABLE_FTS5')", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(fts5);
}

#[test]
fn migrations_run_once_and_a_newer_library_is_refused() {
    let (dir, mut store) = library();
    let root = store.root().to_path_buf();
    let clip = import(&mut store, dir.path(), "Low Tide.wav");
    drop(store);
    let store = Store::open(&root).unwrap();
    assert_eq!(store.clip(&clip.id).unwrap().unwrap().title, "Low Tide");
    drop(store);

    let conn = Connection::open(root.join("library.sqlite")).unwrap();
    conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    drop(conn);
    assert_eq!(
        refused(Store::open(&root).map(|_| ())),
        "This library was made by a newer Wi_WWAV. Update the app to open it."
    );
}

#[test]
fn import_names_the_file_by_ulid_and_hashes_it() {
    let (dir, mut store) = library();
    let bytes = b"RIFF and some audio";
    let src = source_file(dir.path(), "My Song.WAV", bytes);
    let clip = store.import(Room::Library, &src, &ByExtension).unwrap();

    assert!(wwav_ids::is_ulid(&clip.id), "{}", clip.id);
    assert_eq!(clip.file, format!("media/{}.wav", clip.id));
    assert_eq!(
        files_in(&store.root().join("media")),
        [format!("{}.wav", clip.id)]
    );
    assert_eq!(std::fs::read(store.path_of(&clip)).unwrap(), bytes);
    assert_eq!(clip.sha256, sha256(bytes));
    assert_eq!(clip.bytes, bytes.len() as u64);
    assert_eq!(clip.title, "My Song");
    assert_eq!(
        std::fs::read(&src).unwrap(),
        bytes,
        "the original stays where it was"
    );
}

#[test]
fn two_untitled_never_collide() {
    let (dir, mut store) = library();
    let src = source_file(dir.path(), "untitled.wav", b"one");
    let a = store.import(Room::Library, &src, &ByExtension).unwrap();
    std::fs::write(&src, b"two").unwrap();
    let b = store.import(Room::Library, &src, &ByExtension).unwrap();
    assert_ne!(a.file, b.file);
    assert_eq!(
        (a.title.as_str(), b.title.as_str()),
        ("untitled", "untitled")
    );
    assert_eq!(std::fs::read(store.path_of(&a)).unwrap(), b"one");
    assert_eq!(std::fs::read(store.path_of(&b)).unwrap(), b"two");
}

#[test]
fn renaming_never_touches_the_file() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), "World Ending.wwav");
    let before = files_in(&store.root().join("media"));
    edit(&mut store, "rename clip", |tx| {
        tx.rename_clip(&clip.id, "World Ending (final)")
    })
    .unwrap();
    let renamed = store.clip(&clip.id).unwrap().unwrap();
    assert_eq!(renamed.title, "World Ending (final)");
    assert_eq!(renamed.file, clip.file);
    assert_eq!(files_in(&store.root().join("media")), before);
    assert_eq!(
        sha256(&std::fs::read(store.path_of(&renamed)).unwrap()),
        clip.sha256
    );
}

#[test]
fn plain_audio_comes_in_as_master_only_and_says_so() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), "break.wav");
    assert_eq!(clip.kind, Kind::Audio);
    assert_eq!(clip.verdict, "Plain audio comes in as master only.");

    let note = source_file(
        dir.path(),
        "lyrics.md",
        "# Low Tide\nthe water knows".as_bytes(),
    );
    let note = store.import(Room::Library, &note, &ByExtension).unwrap();
    assert_eq!((note.kind, note.duration_ms), (Kind::Text, 0));
    assert_eq!(note.text.as_deref(), Some("# Low Tide\nthe water knows"));

    let pdf = source_file(dir.path(), "notes.pdf", b"%PDF");
    assert_eq!(
        refused(store.import(Room::Library, &pdf, &ByExtension)),
        "Wi_WWAV can't open .pdf files."
    );
    assert_eq!(store.clips().unwrap().len(), 2);
}

/// What wi-core will plug in: the formats crate's reader.
struct Reader;

impl Inspector for Reader {
    fn inspect(&self, path: &Path) -> Result<Inspection> {
        let name = path.file_stem().unwrap().to_string_lossy();
        Ok(Inspection {
            artist: "LMY".into(),
            bpm: Some(86.0),
            key: Some("A minor".into()),
            duration_ms: 238_000,
            verdict: "4 stems, and the master".into(),
            ..Inspection::new(Kind::Wwav, &name)
        })
    }
}

#[test]
fn the_readers_verdict_is_kept_word_for_word() {
    let (dir, mut store) = library();
    let src = source_file(dir.path(), "Low Tide.wwav", b"RIFF");
    let clip = store.import(Room::Library, &src, &Reader).unwrap();
    assert_eq!(clip.kind, Kind::Wwav);
    assert_eq!(clip.verdict, "4 stems, and the master");
    assert_eq!(
        (clip.artist.as_str(), clip.bpm, clip.key.as_deref()),
        ("LMY", Some(86.0), Some("A minor"))
    );
    assert_eq!(clip.duration_ms, 238_000);
}

/// A reader that found no tempo it could trust.
struct Unmeasured;

impl Inspector for Unmeasured {
    fn inspect(&self, path: &Path) -> Result<Inspection> {
        Ok(Inspection {
            bpm: Some(f64::NAN),
            ..ByExtension.inspect(path)?
        })
    }
}

#[test]
fn a_bpm_the_reader_couldnt_measure_is_none() {
    let (dir, mut store) = library();
    let src = source_file(dir.path(), "noise.wav", b"RIFF");
    let clip = store.import(Room::Library, &src, &Unmeasured).unwrap();
    assert_eq!(clip.bpm, None);
    let mut tx = store.begin(Room::Library, "set bpm").unwrap();
    assert_eq!(
        refused(tx.set_bpm(&clip.id, Some(f64::NAN))),
        "A BPM is a number, such as 128."
    );
}

#[test]
fn songs_films_and_small_files_are_copied_and_the_rest_left_in_place() {
    const GB: u64 = 1_000_000_000;
    assert_eq!(Placement::for_file(Kind::Wwav, 3 * GB), Placement::Copy);
    assert_eq!(Placement::for_file(Kind::Swav, 3 * GB), Placement::Copy);
    assert_eq!(
        Placement::for_file(Kind::Video, 2 * GB - 1),
        Placement::Copy
    );
    assert_eq!(
        Placement::for_file(Kind::Video, 2 * GB),
        Placement::LeaveInPlace
    );
    assert_eq!(
        Placement::for_file(Kind::Audio, 5 * GB),
        Placement::LeaveInPlace
    );

    // A long raw video stays where it is, named by its path, hashed in place.
    let (dir, mut store) = library();
    let src = source_file(dir.path(), "A001_C002.mov", b"");
    std::fs::File::options()
        .write(true)
        .open(&src)
        .unwrap()
        .set_len(2 * GB)
        .unwrap(); // sparse
    let clip = store.import(Room::Library, &src, &ByExtension).unwrap();
    assert_eq!(clip.kind, Kind::Video);
    assert_eq!(store.path_of(&clip), src.canonicalize().unwrap());
    assert!(files_in(&store.root().join("media")).is_empty());
    assert_eq!(clip.bytes, 2 * GB);
}

#[test]
fn tags_are_lowercase_and_twelve_at_most() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), "Low Tide.wav").id;
    edit(&mut store, "tag clip", |tx| {
        tx.add_tag(&clip, "  Live-Drums ", TagKind::User)
    })
    .unwrap();
    assert_eq!(
        store.tags_of(&clip).unwrap(),
        [Tag {
            name: "live-drums".into(),
            kind: TagKind::User
        }]
    );
    assert_eq!(
        refused(edit(&mut store, "tag clip", |tx| tx.add_tag(
            &clip,
            "  ",
            TagKind::User
        ))),
        "A tag needs a name."
    );

    edit(&mut store, "tag clip", |tx| {
        for n in 1..=10 {
            tx.add_tag(&clip, &format!("Tag{n}"), TagKind::User)?;
        }
        tx.add_tag(&clip, "covers", TagKind::System) // places count too
    })
    .unwrap();
    assert_eq!(store.tags_of(&clip).unwrap().len(), 12);
    // Adding one it already has changes nothing.
    assert_eq!(
        edit(&mut store, "tag clip", |tx| tx.add_tag(
            &clip,
            "LIVE-DRUMS",
            TagKind::User
        ))
        .unwrap(),
        None
    );

    let journal_before = journal(store.root());
    let records_before = dump(store.root());
    assert_eq!(
        refused(edit(&mut store, "tag clip", |tx| tx.add_tag(
            &clip,
            "thirteen",
            TagKind::User
        ))),
        "A clip holds 12 tags. Remove one first."
    );
    assert_eq!(journal(store.root()), journal_before);
    assert_eq!(dump(store.root()), records_before);

    edit(&mut store, "untag clip", |tx| {
        tx.remove_tag(&clip, "tag3", TagKind::User)
    })
    .unwrap();
    edit(&mut store, "tag clip", |tx| {
        tx.add_tag(&clip, "thirteen", TagKind::User)
    })
    .unwrap();
    assert!(store
        .tags_of(&clip)
        .unwrap()
        .iter()
        .all(|t| t.name == t.name.to_lowercase()));
}

#[test]
fn pins_hold_four() {
    let (dir, mut store) = library();
    let clips: Vec<String> = (0..5)
        .map(|n| import(&mut store, dir.path(), &format!("{n}.wav")).id)
        .collect();
    for (slot, clip) in clips[..4].iter().enumerate() {
        let mut tx = store.begin(Room::Library, "pin clip").unwrap();
        assert_eq!(tx.pin(&Pin::Clip(clip.clone())).unwrap(), slot);
        tx.commit().unwrap();
    }
    assert_eq!(
        refused(edit(&mut store, "pin clip", |tx| tx
            .pin(&Pin::Clip(clips[4].clone()))
            .map(|_| ()))),
        "Pins hold 4. Unpin one first."
    );
    // Pinning what is already pinned keeps its place.
    let mut tx = store.begin(Room::Library, "pin clip").unwrap();
    assert_eq!(tx.pin(&Pin::Clip(clips[2].clone())).unwrap(), 2);
    assert_eq!(tx.commit().unwrap(), None);

    edit(&mut store, "unpin clip", |tx| {
        tx.unpin(&Pin::Clip(clips[1].clone()))
    })
    .unwrap();
    let pins = |s: &Store| s.pins().unwrap();
    let c = |n: usize| Some(Pin::Clip(clips[n].clone()));
    assert_eq!(
        pins(&store),
        [c(0), c(2), c(3), None],
        "unpinning closes the gap"
    );

    let folder = edit_returning(&mut store, |tx| {
        tx.save_smart_folder("minor", &SmartRule::default())
    });
    edit(&mut store, "pin folder", |tx| {
        tx.pin(&Pin::Folder(folder.clone())).map(|_| ())
    })
    .unwrap();
    assert_eq!(pins(&store)[3], Some(Pin::Folder(folder)));

    edit(&mut store, "delete clip", |tx| tx.delete_clip(&clips[2])).unwrap();
    assert_eq!(pins(&store)[..2], [c(0), c(3)]);
    store.undo(Room::Library).unwrap();
    assert_eq!(pins(&store)[..3], [c(0), c(2), c(3)]);
}

fn edit_returning(store: &mut Store, f: impl FnOnce(&mut Txn) -> Result<String>) -> String {
    let mut tx = store.begin(Room::Library, "new smart folder").unwrap();
    let id = f(&mut tx).unwrap();
    tx.commit().unwrap();
    id
}

#[test]
fn smart_folders_are_and_rules_evaluated_in_sql() {
    let (dir, mut store) = library();
    let mut make = |name: &str, bpm: f64, key: &str, tags: &[&str], colour: Option<Colour>| {
        let src = source_file(dir.path(), &format!("{name}.wwav"), name.as_bytes());
        let id = store.import(Room::Library, &src, &Reader).unwrap().id;
        edit(&mut store, "edit clip", |tx| {
            tx.set_bpm(&id, Some(bpm))?;
            tx.set_key(&id, Some(key))?;
            tx.set_colour(&id, colour)?;
            tags.iter()
                .try_for_each(|t| tx.add_tag(&id, t, TagKind::User))
        })
        .unwrap();
        id
    };
    let a = make(
        "a",
        128.0,
        "A minor",
        &["live-drums", "wip"],
        Some(Colour::Red),
    );
    let b = make("b", 132.0, "C minor", &["live-drums"], None);
    let _c = make("c", 130.0, "A major", &["live-drums"], None);
    let d = make("d", 140.0, "A minor", &["live-drums"], Some(Colour::Red));
    let _e = make("e", 129.0, "D minor", &[], None);
    let plain = import(&mut store, dir.path(), "plain.wav").id;

    let ids = |rule: SmartRule, store: &Store| {
        let mut ids: Vec<String> = store
            .matching(&rule)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        ids.sort();
        ids
    };
    let sorted = |mut v: Vec<String>| {
        v.sort();
        v
    };
    let live_minor = SmartRule {
        tags: vec!["live-drums".into()],
        bpm_min: Some(128.0),
        bpm_max: Some(132.0),
        key: Some("minor".into()),
        ..SmartRule::default()
    };
    assert_eq!(
        ids(live_minor.clone(), &store),
        sorted(vec![a.clone(), b.clone()])
    );
    assert_eq!(
        ids(
            SmartRule {
                key: Some("a MINOR".into()),
                ..SmartRule::default()
            },
            &store
        ),
        sorted(vec![a.clone(), d.clone()])
    );
    assert_eq!(
        ids(
            SmartRule {
                colour: Some(Colour::Red),
                bpm_max: Some(130.0),
                ..SmartRule::default()
            },
            &store
        ),
        [a.as_str()]
    );
    assert_eq!(
        ids(
            SmartRule {
                tags: vec!["live-drums".into(), "wip".into()],
                ..SmartRule::default()
            },
            &store
        ),
        [a.as_str()]
    );
    assert_eq!(
        ids(
            SmartRule {
                kind: Some(Kind::Audio),
                ..SmartRule::default()
            },
            &store
        ),
        [plain]
    );
    assert_eq!(ids(SmartRule::default(), &store).len(), 6);

    let folder = edit_returning(&mut store, |tx| {
        tx.save_smart_folder("128–132 BPM · minor · tag:live-drums", &live_minor)
    });
    let saved = store.smart_folders().unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(
        (saved[0].id.as_str(), saved[0].name.as_str()),
        (folder.as_str(), "128–132 BPM · minor · tag:live-drums")
    );
    assert_eq!(saved[0].rule, live_minor);
}

#[test]
fn a_delete_keeps_the_file_and_undo_brings_the_sound_back() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), "World Ending.wav");
    edit(&mut store, "tag clip", |tx| {
        tx.add_tag(&clip.id, "keeper", TagKind::User)
    })
    .unwrap();
    edit(&mut store, "delete clip", |tx| tx.delete_clip(&clip.id)).unwrap();

    assert!(store.clip(&clip.id).unwrap().is_none());
    assert!(store.tags_of(&clip.id).unwrap().is_empty());
    let path = store.root().join(&clip.file);
    assert_eq!(
        sha256(&std::fs::read(&path).unwrap()),
        clip.sha256,
        "a delete removes rows, never files"
    );
    assert!(
        store.unused_media().unwrap().paths.is_empty(),
        "the journal still names the file"
    );

    assert_eq!(
        store.undo(Room::Library).unwrap().as_deref(),
        Some("delete clip")
    );
    let back = store.clip(&clip.id).unwrap().unwrap();
    assert_eq!(back, clip);
    assert_eq!(store.tags_of(&clip.id).unwrap().len(), 1);
    assert_eq!(
        sha256(&std::fs::read(store.path_of(&back)).unwrap()),
        clip.sha256
    );
}

#[test]
fn clean_up_lists_what_nothing_names_then_trashes_and_empties_it() {
    assert_eq!(
        Files {
            paths: vec!["x".into(); 214],
            bytes: 1_800_000_000
        }
        .sentence(),
        "1.8 GB in 214 files"
    );
    assert_eq!(
        Files {
            paths: vec!["x".into()],
            bytes: 210_400_000
        }
        .sentence(),
        "210 MB in 1 file"
    );
    assert_eq!(
        Files {
            paths: vec![],
            bytes: 0
        }
        .sentence(),
        "Nothing to clean up."
    );

    let (dir, mut store) = library();
    let root = store.root().to_path_buf();
    let kept = import(&mut store, dir.path(), "kept.wav");
    // An import whose entry left the journal: undone, then a new edit dropped its redo.
    let gone = import(&mut store, dir.path(), "gone.wav");
    store.undo(Room::Library).unwrap();
    edit(&mut store, "rename clip", |tx| {
        tx.rename_clip(&kept.id, "kept (v2)")
    })
    .unwrap();
    std::fs::write(root.join("media/stray.bin"), vec![7u8; 1500]).unwrap();
    std::fs::create_dir_all(root.join("sessions/01JC5Q8V3M2T7R9X4K6W0YHZNB.wwavsession/media"))
        .unwrap();
    std::fs::write(
        root.join("sessions/01JC5Q8V3M2T7R9X4K6W0YHZNB.wwavsession/session.json"),
        b"{}",
    )
    .unwrap();
    let mut tx = store.begin(Room::Console, "new session").unwrap();
    let session = tx.add_sequence("Low Tide").unwrap();
    tx.commit().unwrap();
    std::fs::create_dir_all(root.join(&session.package)).unwrap();

    let unused = store.unused_media().unwrap();
    let mut expected = vec![
        gone.file.clone(),
        "media/stray.bin".to_string(),
        "sessions/01JC5Q8V3M2T7R9X4K6W0YHZNB.wwavsession".to_string(),
    ];
    expected.sort();
    assert_eq!(unused.paths, expected);
    assert_eq!(unused.bytes, gone.bytes + 1500 + 2);

    let moved = store.move_to_trash(&unused).unwrap();
    assert_eq!(moved, unused);
    assert!(root.join(&kept.file).exists());
    assert!(root.join(&session.package).exists());
    assert!(!root.join(&gone.file).exists());
    assert_eq!(store.trash().unwrap().bytes, unused.bytes);
    assert_eq!(files_in(&root.join("trash")).len(), 3);

    let emptied = store.empty_trash().unwrap();
    assert_eq!(emptied.bytes, unused.bytes);
    assert!(files_in(&root.join("trash")).is_empty());
    assert!(store.unused_media().unwrap().paths.is_empty());
}

#[test]
fn clean_up_leaves_files_on_their_way_in() {
    let (_dir, mut store) = library();
    let root = store.root().to_path_buf();
    assert_eq!(
        refused(store.reserve_media("../x")),
        "A file in the library ends in letters and digits, as .wwav, not '.../x'."
    );

    // A render: the engine writes to a name the library reserved for it.
    let (id, file) = store.reserve_media("WWAV").unwrap();
    assert_eq!(file, format!("media/{id}.wwav"));
    std::fs::write(root.join(&file), b"render").unwrap();
    // An import's copy in flight, and the half copy a crash left.
    let (_, copying) = store.reserve_media("wav").unwrap();
    let in_flight = format!("media/.{}.part", &copying["media/".len()..]);
    std::fs::write(root.join(&in_flight), b"half").unwrap();
    let crashed = "media/.01JC5Q8V3M2T7R9X4K6W0YHZNB.wav.part".to_string();
    std::fs::write(root.join(&crashed), b"left").unwrap();
    assert_eq!(
        store.unused_media().unwrap().paths,
        std::slice::from_ref(&crashed)
    );

    let mut tx = store.begin(Room::Console, "render 'Low Tide'").unwrap();
    tx.add_clip(NewClip {
        id: id.clone(),
        file: file.clone(),
        sha256: sha256(b"render"),
        bytes: 6,
        info: Inspection::new(Kind::Wwav, "Low Tide"),
        from_sequence: None,
    })
    .unwrap();
    tx.commit().unwrap();

    // A day on, a reservation nothing recorded is a crash's leftover.
    let conn = Connection::open(root.join("library.sqlite")).unwrap();
    conn.execute(
        "UPDATE media_pending SET made_ms = made_ms - 2 * 24 * 60 * 60 * 1000",
        [],
    )
    .unwrap();
    let unused = store.unused_media().unwrap();
    let mut expected = vec![crashed, in_flight];
    expected.sort();
    assert_eq!(unused.paths, expected);
    assert_eq!(store.move_to_trash(&unused).unwrap(), unused);
    let reserved: i64 = conn
        .query_row("SELECT count(*) FROM media_pending", [], |r| r.get(0))
        .unwrap();
    assert_eq!(reserved, 0, "the stale reservation ends with the press");
    assert!(
        root.join(&file).exists(),
        "the render is recorded and stays"
    );
    assert!(store.unused_media().unwrap().paths.is_empty());
}

#[test]
fn upload_parts_are_kept_only_while_the_clip_is_queued() {
    let (dir, mut store) = library();
    let x = import(&mut store, dir.path(), "x.wav").id;
    assert!(
        !store.record_upload_part(&x, 1, "etag-1").unwrap(),
        "an unpublished clip has no upload to resume"
    );
    let mut tx = store.begin(Room::Space, "publish").unwrap();
    tx.publish(&x).unwrap();
    tx.commit().unwrap();
    assert!(store.record_upload_part(&x, 1, "etag-1").unwrap());
    edit(&mut store, "rename clip", |tx| tx.rename_clip(&x, "x2")).unwrap();
    assert_eq!(store.upload_parts(&x).unwrap().len(), 1, "still queued");

    // A delete takes it out of the queue, and ends the upload it was in.
    edit(&mut store, "delete clip", |tx| tx.delete_clip(&x)).unwrap();
    assert!(store.upload_parts(&x).unwrap().is_empty());
    assert!(!store.record_upload_part(&x, 2, "etag-2").unwrap());
    // Undo puts it back in the queue, to go up from the start.
    store.undo(Room::Library).unwrap();
    assert!(store.clip(&x).unwrap().unwrap().is_queued());
    assert!(store.upload_parts(&x).unwrap().is_empty());

    assert!(store.record_upload_part(&x, 1, "etag-1b").unwrap());
    store.discard_upload_parts(&x).unwrap();
    assert!(store.upload_parts(&x).unwrap().is_empty());
}

#[test]
fn the_upload_queue_is_a_query() {
    assert_eq!(
        UPLOAD_QUEUE,
        "published_at IS NOT NULL AND remote_id IS NULL"
    );
    let (dir, mut store) = library();
    let root = store.root().to_path_buf();
    let [x, y, z] = ["x.wav", "y.wav", "z.wav"].map(|n| import(&mut store, dir.path(), n).id);
    for clip in [&x, &y] {
        let mut tx = store.begin(Room::Space, "publish").unwrap();
        tx.publish(clip).unwrap();
        tx.commit().unwrap();
    }
    let queue = |s: &Store| {
        s.upload_queue()
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        queue(&store),
        [x.clone(), y.clone()],
        "first published goes first"
    );
    assert!(!queue(&store).contains(&z));

    store.record_upload_part(&x, 1, "etag-1").unwrap();
    store.record_upload_part(&x, 2, "etag-2").unwrap();
    store.record_upload_part(&x, 2, "etag-2").unwrap(); // a retry of the same part
    let journal_before = journal(&root);

    // A crash: nothing but the library survives, and the queue with it.
    drop(store);
    let mut store = Store::open(&root).unwrap();
    assert_eq!(queue(&store), [x.clone(), y.clone()]);
    assert_eq!(
        store.upload_parts(&x).unwrap(),
        [(1, "etag-1".to_string()), (2, "etag-2".to_string())]
    );
    assert!(store.upload_parts(&y).unwrap().is_empty());

    assert!(store.mark_uploaded(&x, "trk_1").unwrap());
    assert_eq!(queue(&store), [y]);
    assert!(store.upload_parts(&x).unwrap().is_empty());
    assert_eq!(
        journal(&root),
        journal_before,
        "uploading is never a journal entry"
    );
}

#[test]
fn a_nightly_backup_keeps_seven_dated_copies() {
    let (dir, mut store) = library();
    import(&mut store, dir.path(), "Low Tide.wav");
    let backups = dir.path().join("backups");
    for day in 1..=9 {
        store
            .backup(&backups, &format!("2026-10-{day:02}"))
            .unwrap();
    }
    store.backup(&backups, "2026-10-09").unwrap(); // twice in a day replaces it
    let expected: Vec<String> = (3..=9)
        .map(|d| format!("library-2026-10-{d:02}.sqlite"))
        .collect();
    assert_eq!(files_in(&backups), expected);
    assert_eq!(
        refused(store.backup(&backups, "yesterday")),
        "A backup is named by its date, as 2026-10-07."
    );

    // The copy is a whole library: its search still finds the song.
    let restored = dir.path().join("restored");
    std::fs::create_dir_all(&restored).unwrap();
    std::fs::copy(
        backups.join("library-2026-10-09.sqlite"),
        restored.join("library.sqlite"),
    )
    .unwrap();
    search_indexes_agree(&restored);
    let restored = Store::open(&restored).unwrap();
    assert_eq!(
        restored
            .search("low", &SmartRule::default(), 10)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn room_records_are_journaled_and_searchable() {
    let (_dir, mut store) = library();
    let task = json!({"id": "t1", "title": "Mix the EP", "estMin": 90, "done": false});
    let mut tx = store.begin(Room::Heat, "add task").unwrap();
    tx.put_doc("task", "t1", &task, "Mix the EP vocals")
        .unwrap();
    tx.put_doc(
        "capture",
        "t1",
        &json!({"text": "call Ana"}),
        "call Ana about the mix",
    )
    .unwrap();
    tx.commit().unwrap();

    let doc = store.doc("task", "t1").unwrap().unwrap();
    assert_eq!(
        (doc.kind.as_str(), doc.key.as_str(), &doc.json),
        ("task", "t1", &task)
    );
    assert_eq!(
        store.docs("task").unwrap().len(),
        1,
        "the same key in two kinds is two records"
    );
    let found = |s: &Store, q: &str| {
        let mut kinds: Vec<String> = s
            .search_docs(q, 10)
            .unwrap()
            .into_iter()
            .map(|d| d.kind)
            .collect();
        kinds.sort();
        kinds
    };
    assert_eq!(found(&store, "mix"), ["capture", "task"]);
    assert_eq!(found(&store, "ana"), ["capture"]);

    let mut tx = store.begin(Room::Heat, "mark done").unwrap();
    tx.put_doc(
        "task",
        "t1",
        &json!({"id": "t1", "title": "Mix the EP", "done": true}),
        "Mix the EP",
    )
    .unwrap();
    tx.commit().unwrap();
    assert_eq!(found(&store, "vocals"), Vec::<String>::new());
    assert_eq!(
        store.history(Room::Heat).unwrap().undo_text(),
        "Undo mark done"
    );
    store.undo(Room::Heat).unwrap();
    assert_eq!(store.doc("task", "t1").unwrap().unwrap().json, task);
    assert_eq!(found(&store, "vocals"), ["task"]);

    let mut tx = store.begin(Room::Heat, "delete task").unwrap();
    tx.delete_doc("task", "t1").unwrap();
    tx.commit().unwrap();
    assert!(store.doc("task", "t1").unwrap().is_none());
    store.undo(Room::Heat).unwrap();
    assert_eq!(store.doc("task", "t1").unwrap().unwrap().json, task);
}

#[test]
fn search_finds_clips_by_their_words_and_a_rule() {
    let (dir, mut store) = library();
    let low = import(&mut store, dir.path(), "Low Tide.wav").id;
    let glass = import(&mut store, dir.path(), "glass hours.wav").id;
    let bey = import(&mut store, dir.path(), "Beyoncé.wav").id;
    edit(&mut store, "edit clip", |tx| {
        tx.set_artist(&glass, "LMY")?;
        tx.set_bpm(&glass, Some(120.0))
    })
    .unwrap();

    let ids = |q: &str, rule: &SmartRule, s: &Store| {
        s.search(q, rule, 50)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect::<Vec<_>>()
    };
    let any = SmartRule::default();
    assert_eq!(ids("low", &any, &store), [low.as_str()]);
    assert_eq!(ids("tid", &any, &store), [low.as_str()]);
    assert_eq!(
        ids("ide", &any, &store),
        Vec::<String>::new(),
        "words match from their start"
    );
    assert_eq!(ids("hou", &any, &store), [glass.as_str()]);
    assert_eq!(ids("beyonce", &any, &store), [bey.as_str()]);
    assert_eq!(ids("lmy", &any, &store), [glass.as_str()]);
    assert_eq!(
        ids("", &any, &store),
        [bey.clone(), glass.clone(), low.clone()],
        "newest first"
    );
    assert_eq!(
        ids("\"low (", &any, &store),
        [low.as_str()],
        "typed punctuation is never query syntax"
    );
    assert_eq!(
        ids("NOT low", &any, &store),
        Vec::<String>::new(),
        "NOT is a word, not an operator"
    );
    let fast = SmartRule {
        bpm_min: Some(100.0),
        ..SmartRule::default()
    };
    assert_eq!(ids("", &fast, &store), [glass.as_str()]);
    assert_eq!(ids("low", &fast, &store), Vec::<String>::new());
    edit(&mut store, "rename clip", |tx| {
        tx.rename_clip(&low, "High Tide")
    })
    .unwrap();
    assert_eq!(ids("high", &any, &store), [low]);
}

#[test]
fn sessions_are_sequences_and_renders_point_back() {
    let (_dir, mut store) = library();
    let mut tx = store.begin(Room::Console, "new session").unwrap();
    let session = tx.add_sequence("Low Tide").unwrap();
    let id = wwav_ids::ulid();
    let render = NewClip {
        file: format!("media/{id}.wwav"),
        id,
        sha256: "0".repeat(64),
        bytes: 0,
        info: Inspection::new(Kind::Wwav, "Low Tide"),
        from_sequence: Some(session.id.clone()),
    };
    let clip = tx.add_clip(render).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        session.package,
        format!("sessions/{}.wwavsession", session.id)
    );
    assert_eq!(store.sequences().unwrap(), std::slice::from_ref(&session));
    assert_eq!(
        store.clip(&clip).unwrap().unwrap().from_sequence.as_deref(),
        Some(session.id.as_str())
    );

    let mut tx = store.begin(Room::Console, "delete session").unwrap();
    tx.delete_sequence(&session.id).unwrap();
    tx.commit().unwrap();
    assert!(store.sequences().unwrap().is_empty());
    assert_eq!(store.clip(&clip).unwrap().unwrap().from_sequence, None);
    store.undo(Room::Console).unwrap();
    assert_eq!(
        store.clip(&clip).unwrap().unwrap().from_sequence.as_deref(),
        Some(session.id.as_str())
    );
}

#[test]
fn plugin_scans_are_kept_per_file() {
    let (_dir, mut store) = library();
    let tape = PluginScan {
        path: "/Library/Audio/Plug-Ins/VST3/Tape Echo.vst3".into(),
        format: "vst3".into(),
        uid: "0123456789abcdef0123456789abcdef".into(),
        name: "Tape Echo".into(),
        vendor: "Vndr".into(),
        version: "2.1.4".into(),
        modified_ms: 1_759_795_200_000,
        status: ScanStatus::Ok,
        reason: String::new(),
        checked_ms: 1_759_795_300_000,
    };
    let crasher = PluginScan {
        path: "/Library/Audio/Plug-Ins/VST3/crasher.vst3".into(),
        uid: String::new(),
        name: "crasher".into(),
        status: ScanStatus::Crashed,
        reason: "crashed while being checked".into(),
        ..tape.clone()
    };
    store
        .record_plugin_scan(&[tape.clone(), crasher.clone()])
        .unwrap();
    assert_eq!(
        store.plugin_scans().unwrap(),
        [crasher.clone(), tape.clone()]
    );

    let newer = PluginScan {
        version: "2.2.0".into(),
        ..tape
    };
    store
        .record_plugin_scan(std::slice::from_ref(&newer))
        .unwrap();
    assert_eq!(store.plugin_scans().unwrap(), [crasher, newer]);
    assert_eq!(
        store.history(Room::Console).unwrap().undo_text(),
        "Nothing to undo."
    );
}
