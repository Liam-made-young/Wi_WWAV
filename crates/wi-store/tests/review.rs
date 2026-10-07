//! An independent reviewer's adversarial tests for wi-store (docs/PLAN.md F5,
//! S1.7 store side, S1.8, S1.9 query half). Tests that expose a real defect
//! are kept `#[ignore]`d with the finding named, so the suite stays green
//! until the builder fixes them; `cargo test -p wi-store -- --ignored` runs them.

mod common;

use common::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::{Duration, Instant};
use wi_store::*;

fn import(store: &mut Store, dir: &std::path::Path, room: Room, name: &str) -> String {
    let src = source_file(dir, name, format!("bytes of {name}").as_bytes());
    store.import(room, &src, &ByExtension).unwrap().id
}

fn edit(store: &mut Store, room: Room, label: &str, f: impl FnOnce(&mut Txn) -> Result<()>) {
    let mut tx = store.begin(room, label).unwrap();
    f(&mut tx).unwrap();
    tx.commit().unwrap();
}

// --- F5 / S1.9: work that left the machine ---------------------------------

/// Finding: a whole-row journal snapshot taken while a clip was queued is
/// written back after the server acknowledged it, so the clip forgets the
/// server has it (remote_id back to NULL), goes back in the upload queue, and
/// its publish becomes undoable again: "work that has left the machine can't
/// be undone" (2.7, 9.6) no longer holds.
#[test]
#[ignore = "defect: undo of a delete restores a stale remote_id (review finding 1)"]
fn a_clip_the_server_has_stays_up_through_undo_and_redo_of_its_delete() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "World Ending.wav");
    edit(&mut store, Room::Space, "publish 'World Ending'", |tx| {
        tx.publish(&clip)
    });
    edit(&mut store, Room::Library, "delete clip", |tx| {
        tx.delete_clip(&clip)
    });
    assert_eq!(
        store.undo(Room::Library).unwrap().as_deref(),
        Some("delete clip")
    );
    assert!(store.clip(&clip).unwrap().unwrap().is_queued());
    // The upload finishes: the server has it now.
    assert!(store.mark_uploaded(&clip, "trk_1").unwrap());
    // ⌘⇧Z then ⌘Z in the library: delete it again, and bring it back.
    assert_eq!(
        store.redo(Room::Library).unwrap().as_deref(),
        Some("delete clip")
    );
    assert_eq!(
        store.undo(Room::Library).unwrap().as_deref(),
        Some("delete clip")
    );
    let back = store.clip(&clip).unwrap().unwrap();
    assert_eq!(
        back.remote_id.as_deref(),
        Some("trk_1"),
        "the server still has it, so the clip must still say so"
    );
    assert!(
        store.upload_queue().unwrap().is_empty(),
        "a clip the server has must not go back in the upload queue"
    );
    assert_eq!(
        store.history(Room::Space).unwrap().undo_text(),
        "Can't undo a publish. Unpublish 'World Ending'…"
    );
}

// --- non-finite numbers ----------------------------------------------------

/// Finding (low): a BPM of infinity goes through serde_json, which can't hold
/// it, so the store silently writes "no BPM" instead (and journals nothing
/// when the clip had none). The journal stays exact, but the edit is neither
/// kept nor refused with a sentence.
#[test]
#[ignore = "defect: a non-finite BPM is silently stored as no BPM (review finding)"]
fn a_non_finite_bpm_is_kept_or_refused_never_silently_cleared() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "Low Tide.wav");
    edit(&mut store, Room::Library, "set bpm", |tx| {
        tx.set_bpm(&clip, Some(120.0))
    });
    let mut tx = store.begin(Room::Library, "set bpm").unwrap();
    let r = tx.set_bpm(&clip, Some(f64::INFINITY));
    if r.is_ok() {
        tx.commit().unwrap();
        assert_eq!(
            store.clip(&clip).unwrap().unwrap().bpm,
            Some(f64::INFINITY),
            "set_bpm(Some(inf)) returned Ok, so the clip must hold it"
        );
    } else {
        assert!(matches!(r, Err(Error::Refused(_))));
    }
}

// --- F5: a second random run, with a different mix ---------------------------

/// What must hold between any two steps: foreign keys, each clip's tag count,
/// and no pin naming a row that isn't there.
fn invariants_hold(root: &std::path::Path, seed: u64, step: usize) {
    let conn = rusqlite::Connection::open(root.join("library.sqlite")).unwrap();
    let broken: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        broken, 0,
        "seed {seed} step {step}: a foreign key is broken"
    );
    let miscounted: i64 = conn
        .query_row(
            "SELECT count(*) FROM clips c
             WHERE tag_count <> (SELECT count(*) FROM clip_tags t WHERE t.clip_id = c.id)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        miscounted, 0,
        "seed {seed} step {step}: a tag count is wrong"
    );
    let dangling: i64 = conn
        .query_row(
            "SELECT count(*) FROM pins p, json_each(p.slots) s
             WHERE s.value IS NOT NULL
               AND NOT EXISTS (SELECT 1 FROM clips WHERE 'clip:' || id = s.value)
               AND NOT EXISTS (SELECT 1 FROM smart_folders WHERE 'folder:' || id = s.value)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(dangling, 0, "seed {seed} step {step}: a pin names nothing");
}

/// High contention: three clips, every room editing the same few values, long
/// undo/redo bursts, and new edits landing on top of undone work. The Edit
/// menu must say what ⌘Z and ⌘⇧Z then do, the invariants must hold between
/// any two steps, and undo-all and redo-all must give the two ends byte for
/// byte.
fn contended_run(seed: u64, steps: usize) -> (usize, usize, usize) {
    let (dir, mut store) = library();
    let root = store.root().to_path_buf();
    let mut rng = StdRng::seed_from_u64(seed);
    let clips: Vec<String> = (0..3)
        .map(|n| import(&mut store, dir.path(), Room::Library, &format!("c{n}.wav")))
        .collect();
    // The imports are journaled in the library and the run may undo them
    // too, so the first state is the empty library: undo them to see it.
    let first_floor = {
        undo_all(&mut store);
        let d = dump(&root);
        redo_all(&mut store);
        d
    };
    let (mut commits, mut undos, mut redos) = (0, 0, 0);
    let mut folders: Vec<String> = Vec::new();
    for step in 0..steps {
        invariants_hold(&root, seed, step);
        let room = ROOMS[rng.gen_range(0..ROOMS.len())];
        let roll = rng.gen_range(0..100);
        if roll < 45 {
            let undo = roll < 28;
            for _ in 0..rng.gen_range(1..=6) {
                let h = store.history(room).unwrap();
                let before = dump(&root);
                let r = if undo {
                    store.undo(room)
                } else {
                    store.redo(room)
                };
                let menu = if undo { &h.undo } else { &h.redo };
                match (r, menu) {
                    (Ok(Some(label)), Menu::Ready(l)) => {
                        assert_eq!(&label, l, "seed {seed} step {step}");
                        if undo {
                            undos += 1
                        } else {
                            redos += 1
                        }
                    }
                    (Ok(None), Menu::Nothing) => assert_eq!(before, dump(&root)),
                    (Err(Error::Refused(s)), Menu::Held(why)) => {
                        assert_eq!(&s, why);
                        assert_eq!(before, dump(&root));
                    }
                    (r, m) => panic!("seed {seed} step {step}: menu {m:?} but {r:?}"),
                }
            }
            continue;
        }
        let clip = clips[rng.gen_range(0..clips.len())].clone();
        let label = format!("e{step}");
        let mut tx = store.begin(room, &label).unwrap();
        let tags = [
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
        ];
        let r: Result<()> = match rng.gen_range(0..12) {
            0 => tx.rename_clip(&clip, ["x", "y", "z"][rng.gen_range(0..3)]),
            1 => tx.set_colour(&clip, [None, Some(Colour::Red)][rng.gen_range(0..2)]),
            2 | 3 => tx.add_tag(&clip, tags[rng.gen_range(0..tags.len())], TagKind::User),
            4 => tx.remove_tag(&clip, tags[rng.gen_range(0..tags.len())], TagKind::User),
            5 => tx.pin(&Pin::Clip(clip.clone())).map(|_| ()),
            6 => tx.unpin(&Pin::Clip(clip.clone())),
            7 => tx.delete_clip(&clip),
            8 => tx.publish(&clip),
            9 => {
                let id = tx.save_smart_folder("f", &SmartRule::default());
                id.map(|id| folders.push(id))
            }
            10 => match folders.last() {
                Some(f) if rng.gen_bool(0.5) => tx.pin(&Pin::Folder(f.clone())).map(|_| ()),
                Some(f) => tx.delete_smart_folder(f),
                None => Ok(()),
            },
            _ => {
                let key = ["1", "2"][rng.gen_range(0..2)];
                if rng.gen_bool(0.7) {
                    tx.put_doc("task", key, &serde_json::json!({"s": step}), "task")
                } else {
                    tx.delete_doc("task", key)
                }
            }
        };
        match r {
            Ok(()) => {
                tx.commit().unwrap();
                commits += 1;
            }
            Err(Error::Refused(_)) => drop(tx),
            Err(e) => panic!("seed {seed} step {step}: {e}"),
        }
    }
    redo_all(&mut store);
    let last = dump(&root);
    undo_all(&mut store);
    assert_eq!(first_floor, dump(&root), "seed {seed}: undo-all");
    assert!(journal(&root).values().all(|(_, s)| s == "undone"));
    search_indexes_agree(&root);
    redo_all(&mut store);
    assert_eq!(last, dump(&root), "seed {seed}: redo-all");
    search_indexes_agree(&root);
    (commits, undos, redos)
}

#[test]
#[ignore = "defect: undoing an unpin can pin a deleted clip (review finding); the per-step invariant catches it at seed 105"]
fn a_contended_random_run_undoes_to_the_first_state_and_redoes_to_the_last() {
    let seeds: u64 = std::env::var("REVIEW_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    for seed in 100..100 + seeds {
        let (c, u, r) = contended_run(seed, 1200);
        eprintln!("seed {seed}: {c} commits, {u} undos, {r} redos");
        assert!(c > 150 && u > 100 && r > 50);
    }
}

// --- S1.8: search, with queries chosen to be slow ----------------------------

fn word(rng: &mut StdRng) -> String {
    const SYLLABLES: [&str; 24] = [
        "lo", "ti", "de", "glas", "hour", "wor", "end", "ing", "be", "yon", "cé", "ra", "mu", "sa",
        "ka", "ne", "on", "vel", "dri", "ft", "noc", "tur", "ne", "ah",
    ];
    (0..rng.gen_range(1..=3))
        .map(|_| SYLLABLES[rng.gen_range(0..SYLLABLES.len())])
        .collect()
}

/// The builder's queries all stop early (LIMIT 200 over the newest matches).
/// These make SQLite visit every match: a one-letter prefix narrowed by a rule
/// nothing passes, several short prefixes intersected, and a rule alone that
/// nothing passes. Text clips with a paragraph each make the index bigger.
#[test]
fn search_stays_under_50_ms_at_50000_clips_on_queries_that_visit_every_match() {
    let (_dir, mut store) = library();
    let mut rng = StdRng::seed_from_u64(7);
    for batch in 0..10 {
        let mut tx = store.begin(Room::Library, "import").unwrap();
        for n in 0..5_000 {
            let id = wwav_ids::ulid();
            let text_clip = n % 5 == 0;
            let title: Vec<String> = (0..rng.gen_range(1..=4)).map(|_| word(&mut rng)).collect();
            let info = Inspection {
                artist: format!("{} {}", word(&mut rng), word(&mut rng)),
                bpm: Some(rng.gen_range(60..180) as f64),
                key: Some(["A minor", "C major"][rng.gen_range(0..2)].to_string()),
                text: text_clip.then(|| {
                    (0..80)
                        .map(|_| word(&mut rng))
                        .collect::<Vec<_>>()
                        .join(" ")
                }),
                ..Inspection::new(
                    if text_clip { Kind::Text } else { Kind::Wwav },
                    &title.join(" "),
                )
            };
            let clip = NewClip {
                file: format!("media/{id}.wwav"),
                id,
                sha256: "0".repeat(64),
                bytes: 1,
                info,
                from_sequence: None,
            };
            let id = tx.add_clip(clip).unwrap();
            if (batch * 5_000 + n) % 7 == 0 {
                tx.add_tag(&id, "live-drums", TagKind::User).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    let none = SmartRule {
        colour: Some(Colour::Purple),
        ..SmartRule::default()
    };
    let rare_tag = SmartRule {
        tags: vec!["nothing-has-this".into()],
        ..SmartRule::default()
    };
    let narrow = SmartRule {
        tags: vec!["live-drums".into()],
        bpm_min: Some(179.0),
        key: Some("minor".into()),
        kind: Some(Kind::Text),
        ..SmartRule::default()
    };
    let queries: [(&str, &SmartRule); 8] = [
        ("l", &none),
        ("n", &rare_tag),
        ("", &none),
        ("", &narrow),
        ("t", &narrow),
        ("l t d r", &SmartRule::default()),
        ("l t d r", &none),
        ("ne on ra sa ka", &none),
    ];
    let mut worst = Duration::ZERO;
    for (q, rule) in queries {
        store.search(q, rule, 200).unwrap();
        let mut slowest = Duration::ZERO;
        for _ in 0..5 {
            let t = Instant::now();
            store.search(q, rule, 200).unwrap();
            slowest = slowest.max(t.elapsed());
        }
        eprintln!("{q:>16?}: slowest of 5 {slowest:.2?}");
        worst = worst.max(slowest);
    }
    eprintln!("worst {worst:.2?}");
    assert!(worst < Duration::from_millis(50), "worst {worst:?}");
}

// --- search input never errors ---------------------------------------------

#[test]
fn nothing_typed_into_search_is_an_error() {
    let (dir, mut store) = library();
    import(&mut store, dir.path(), Room::Library, "Low Tide.wav");
    let mut tx = store.begin(Room::Heat, "add task").unwrap();
    tx.put_doc("task", "1", &serde_json::json!({}), "mix the EP")
        .unwrap();
    tx.commit().unwrap();
    let pieces = [
        "\"",
        "'",
        "*",
        "^",
        "(",
        ")",
        "-",
        "+",
        ":",
        "NEAR",
        "AND",
        "OR",
        "NOT",
        "{",
        "}",
        "a",
        "é",
        "\u{301}",
        "ʹ",
        "ǅ",
        "²",
        "٣",
        "漢",
        "🎵",
        "\u{200b}",
        "_",
        ".",
        "\\",
        "ﬁ",
        "İ",
        "ß",
        "lo",
        " ",
        "\t",
        "\u{0300}a",
        "a\u{0300}",
        "ᾈ",
        "Ⅻ",
        "⁹",
    ];
    let mut rng = StdRng::seed_from_u64(9);
    for _ in 0..20_000 {
        let q: String = (0..rng.gen_range(1..6))
            .map(|_| pieces[rng.gen_range(0..pieces.len())])
            .collect();
        if let Err(e) = store.search(&q, &SmartRule::default(), 20) {
            panic!("search {q:?}: {e}");
        }
        if let Err(e) = store.search_docs(&q, 20) {
            panic!("search_docs {q:?}: {e}");
        }
    }
}

// --- S1.9: the queue, and what a resume skips -------------------------------

/// Finding (design gap): parts recorded for an upload that ⌘Z cancelled are
/// kept, so publishing again "resumes" an upload that was cancelled and skips
/// parts the new upload never sent.
#[test]
#[ignore = "gap: a cancelled upload's parts survive into the next publish (review finding)"]
fn a_cancelled_upload_leaves_no_parts_for_the_next_publish_to_skip() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "World Ending.wav");
    edit(&mut store, Room::Space, "publish", |tx| tx.publish(&clip));
    store.record_upload_part(&clip, 1, "etag-1").unwrap();
    store.undo(Room::Space).unwrap(); // ⌘Z cancels the queued upload
    edit(&mut store, Room::Space, "publish", |tx| tx.publish(&clip));
    assert!(
        store.upload_parts(&clip).unwrap().is_empty(),
        "a new upload must not skip parts of a cancelled one"
    );
}

#[test]
fn the_queue_query_survives_a_second_handle_and_a_late_ack() {
    let (dir, mut store) = library();
    let root = store.root().to_path_buf();
    let clip = import(&mut store, dir.path(), Room::Library, "a.wav");
    edit(&mut store, Room::Space, "publish", |tx| tx.publish(&clip));
    // Another handle (wi-core's uploader on another thread) sees the queue.
    let mut uploader = Store::open(&root).unwrap();
    assert_eq!(uploader.upload_queue().unwrap().len(), 1);
    store.undo(Room::Space).unwrap();
    assert!(!uploader.mark_uploaded(&clip, "late").unwrap());
    assert!(store.upload_queue().unwrap().is_empty());
    assert!(store.clip(&clip).unwrap().unwrap().remote_id.is_none());
}

// --- S1.7 (store side) edges ------------------------------------------------

#[test]
fn tags_lowercase_unicode_and_the_thirteenth_is_refused_with_the_sentence() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "a.wav");
    edit(&mut store, Room::Library, "tag", |tx| {
        tx.add_tag(&clip, "ÉTÉ", TagKind::User)?;
        tx.add_tag(&clip, "Live", TagKind::User)?;
        tx.add_tag(&clip, "LIVE", TagKind::User) // the same tag again
    });
    let names: Vec<String> = store
        .tags_of(&clip)
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(names, ["live", "été"]);
    edit(&mut store, Room::Space, "tag", |tx| {
        for n in 0..10 {
            tx.add_tag(&clip, &format!("t{n}"), TagKind::Card)?;
        }
        Ok(())
    });
    assert_eq!(store.tags_of(&clip).unwrap().len(), 12);
    let mut tx = store.begin(Room::Heat, "tag").unwrap();
    match tx.add_tag(&clip, "Thirteen", TagKind::System) {
        Err(Error::Refused(s)) => assert_eq!(s, "A clip holds 12 tags. Remove one first."),
        other => panic!("{other:?}"),
    }
    // Re-adding one it has is not a 13th.
    tx.add_tag(&clip, "live", TagKind::User).unwrap();
    assert_eq!(tx.commit().unwrap(), None);
    // Undo across rooms keeps the count honest: remove in Heat, undo Space's ten.
    edit(&mut store, Room::Heat, "untag", |tx| {
        tx.remove_tag(&clip, "live", TagKind::User)
    });
    assert!(
        store.undo(Room::Space).is_err(),
        "Heat's untag touched the count later"
    );
    store.undo(Room::Heat).unwrap();
    store.undo(Room::Space).unwrap();
    assert_eq!(store.tags_of(&clip).unwrap().len(), 2);
}

#[test]
fn a_fifth_pin_never_fits_through_undo_and_redo_across_rooms() {
    let (dir, mut store) = library();
    let clips: Vec<String> = (0..6)
        .map(|n| import(&mut store, dir.path(), Room::Library, &format!("p{n}.wav")))
        .collect();
    let rooms = [Room::Heat, Room::Space, Room::Console, Room::Unquantized];
    for (room, clip) in rooms.iter().zip(&clips) {
        edit(&mut store, *room, "pin", |tx| {
            tx.pin(&Pin::Clip(clip.clone())).map(|_| ())
        });
    }
    let mut tx = store.begin(Room::Library, "pin").unwrap();
    match tx.pin(&Pin::Clip(clips[4].clone())) {
        Err(Error::Refused(s)) => assert_eq!(s, "Pins hold 4. Unpin one first."),
        other => panic!("{other:?}"),
    }
    drop(tx);
    // Heat's pin can't be undone before the later pins (they all touch the row).
    assert!(store.undo(Room::Heat).is_err());
    store.undo(Room::Unquantized).unwrap();
    edit(&mut store, Room::Library, "pin", |tx| {
        tx.pin(&Pin::Clip(clips[5].clone())).map(|_| ())
    });
    // The Unquantized pin's redo was built on the old row: it can't come back
    // as a fifth.
    assert!(matches!(store.redo(Room::Unquantized), Ok(None)));
    assert_eq!(
        store.pins().unwrap().iter().filter(|p| p.is_some()).count(),
        4
    );
}

// --- more findings -----------------------------------------------------------

/// Finding (low): a NUL in what was typed (a paste) reaches FTS5 inside the
/// quoted query, which ends the string there: "unterminated string".
#[test]
#[ignore = "defect: a NUL in the search text is an SQLite error (review finding)"]
fn a_nul_in_the_search_text_is_not_an_error() {
    let (dir, mut store) = library();
    import(&mut store, dir.path(), Room::Library, "Low Tide.wav");
    store
        .search("low\0tide", &SmartRule::default(), 20)
        .expect("clip search");
    store.search_docs("low\0tide", 20).expect("record search");
}

/// The consequence of finding 1: after the redo/undo of a delete, the publish
/// of a clip the server has can be undone, and the Edit menu offers it.
#[test]
#[ignore = "defect: undo of a delete restores a stale remote_id (review finding 1)"]
fn a_publish_the_server_has_is_never_undone() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "World Ending.wav");
    edit(&mut store, Room::Space, "publish 'World Ending'", |tx| {
        tx.publish(&clip)
    });
    edit(&mut store, Room::Library, "delete clip", |tx| {
        tx.delete_clip(&clip)
    });
    store.undo(Room::Library).unwrap();
    assert!(store.mark_uploaded(&clip, "trk_1").unwrap());
    store.redo(Room::Library).unwrap();
    store.undo(Room::Library).unwrap();
    match store.undo(Room::Space) {
        Err(Error::Refused(s)) => {
            assert_eq!(s, "Can't undo a publish. Unpublish 'World Ending'…")
        }
        other => panic!("the server has 'World Ending', yet ⌘Z in Space gave {other:?}"),
    }
}

/// A random run where uploads land at random moments: once the server has a
/// clip, every later state that holds the clip must say so.
#[test]
#[ignore = "defect: undo of a delete restores a stale remote_id (review finding 1)"]
fn a_clip_the_server_acknowledged_never_forgets_it_in_a_random_run() {
    for seed in 0..20u64 {
        let (dir, mut store) = library();
        let mut rng = StdRng::seed_from_u64(seed);
        let clips: Vec<String> = (0..3)
            .map(|n| import(&mut store, dir.path(), Room::Library, &format!("u{n}.wav")))
            .collect();
        let mut acked: std::collections::BTreeMap<String, String> = Default::default();
        for step in 0..400 {
            let room = ROOMS[rng.gen_range(0..ROOMS.len())];
            let clip = clips[rng.gen_range(0..clips.len())].clone();
            match rng.gen_range(0..10) {
                0..=2 => {
                    let _ = store.undo(room);
                }
                3..=4 => {
                    let _ = store.redo(room);
                }
                5 => {
                    let remote = format!("trk_{step}");
                    if store.mark_uploaded(&clip, &remote).unwrap() {
                        acked.insert(clip.clone(), remote);
                    }
                }
                6 => {
                    let mut tx = store.begin(room, "publish").unwrap();
                    if tx.publish(&clip).is_ok() {
                        tx.commit().unwrap();
                    }
                }
                7 => {
                    let mut tx = store.begin(room, "delete clip").unwrap();
                    if tx.delete_clip(&clip).is_ok() {
                        tx.commit().unwrap();
                    }
                }
                _ => {
                    let mut tx = store.begin(room, "rename clip").unwrap();
                    if tx.rename_clip(&clip, &format!("t{step}")).is_ok() {
                        tx.commit().unwrap();
                    }
                }
            }
            for (id, remote) in &acked {
                if let Some(c) = store.clip(id).unwrap() {
                    assert_eq!(
                        c.remote_id.as_deref(),
                        Some(remote.as_str()),
                        "seed {seed} step {step}: the server has {id}"
                    );
                }
            }
        }
    }
}

/// Finding (low): after a purchase in a room, ⌘⇧Z there still redoes an
/// older change, but ⌘Z right after it reads "Can't undo a purchase." instead
/// of undoing the redo just made.
#[test]
#[ignore = "defect: a redo made after a purchase can't be undone (review finding)"]
fn a_redo_made_after_a_purchase_undoes() {
    let (_dir, mut store) = library();
    edit(&mut store, Room::Unquantized, "save for later", |tx| {
        tx.put_doc("saved", "r1", &serde_json::json!({}), "")
    });
    store.undo(Room::Unquantized).unwrap();
    store
        .record_outward(Room::Unquantized, "a purchase")
        .unwrap();
    assert_eq!(
        store.redo(Room::Unquantized).unwrap().as_deref(),
        Some("save for later")
    );
    assert_eq!(
        store.history(Room::Unquantized).unwrap().undo_text(),
        "Undo save for later"
    );
}

/// Finding: the "can't undo a publish" check compares the clip's current
/// published_at with the entry's `before` snapshot of the whole row, so any
/// change made to a clip before it was published (a rename, a tag, a colour)
/// is refused once the server has the clip, with the publish sentence, though
/// that change published nothing. It then blocks that room's whole history.
#[test]
#[ignore = "defect: an edit made before a publish is refused as a publish (review finding)"]
fn an_edit_made_before_the_publish_still_undoes_after_the_upload() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "Low Tide.wav");
    edit(&mut store, Room::Library, "rename clip", |tx| {
        tx.rename_clip(&clip, "World Ending")
    });
    edit(&mut store, Room::Space, "publish 'World Ending'", |tx| {
        tx.publish(&clip)
    });
    assert!(store.mark_uploaded(&clip, "trk_1").unwrap());
    // Space can't take the publish back: right.
    assert_eq!(
        store.history(Room::Space).unwrap().undo_text(),
        "Can't undo a publish. Unpublish 'World Ending'…"
    );
    // The library's rename published nothing, and nothing later touched the title.
    assert_eq!(
        store.history(Room::Library).unwrap().undo_text(),
        "Undo rename clip"
    );
    assert_eq!(
        store.undo(Room::Library).unwrap().as_deref(),
        Some("rename clip")
    );
    let c = store.clip(&clip).unwrap().unwrap();
    assert_eq!(c.title, "Low Tide");
    assert!(c.is_up(), "the clip stays up");
}

/// Finding: `unpin` records no dependency on the row it unpinned (only `pin`
/// leans on it), so after another room deletes that clip, ⌘Z of the unpin
/// pins a clip that isn't in the library: a slot names nothing.
#[test]
#[ignore = "defect: undoing an unpin can pin a deleted clip (review finding)"]
fn undoing_an_unpin_never_pins_a_clip_another_room_deleted() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "Low Tide.wav");
    edit(&mut store, Room::Space, "pin clip", |tx| {
        tx.pin(&Pin::Clip(clip.clone())).map(|_| ())
    });
    edit(&mut store, Room::Space, "unpin clip", |tx| {
        tx.unpin(&Pin::Clip(clip.clone()))
    });
    edit(&mut store, Room::Library, "delete clip", |tx| {
        tx.delete_clip(&clip)
    });
    // ⌘Z in Space: either it waits for the library's delete, or the pin it
    // brings back names a clip that is there.
    let _ = store.undo(Room::Space);
    for pin in store.pins().unwrap().into_iter().flatten() {
        if let Pin::Clip(id) = pin {
            assert!(
                store.clip(&id).unwrap().is_some(),
                "a pin names {id}, which isn't in the library"
            );
        }
    }
}

/// The same for a smart folder.
#[test]
#[ignore = "defect: undoing an unpin can pin a deleted smart folder (review finding)"]
fn undoing_an_unpin_never_pins_a_folder_another_room_deleted() {
    let (_dir, mut store) = library();
    let mut tx = store.begin(Room::Library, "new smart folder").unwrap();
    let folder = tx
        .save_smart_folder("minor", &SmartRule::default())
        .unwrap();
    tx.commit().unwrap();
    edit(&mut store, Room::Space, "pin folder", |tx| {
        tx.pin(&Pin::Folder(folder.clone())).map(|_| ())
    });
    edit(&mut store, Room::Space, "unpin folder", |tx| {
        tx.unpin(&Pin::Folder(folder.clone()))
    });
    edit(&mut store, Room::Library, "delete smart folder", |tx| {
        tx.delete_smart_folder(&folder)
    });
    let _ = store.undo(Room::Space);
    let folders: Vec<String> = store
        .smart_folders()
        .unwrap()
        .into_iter()
        .map(|f| f.id)
        .collect();
    for pin in store.pins().unwrap().into_iter().flatten() {
        if let Pin::Folder(id) = pin {
            assert!(
                folders.contains(&id),
                "a pin names folder {id}, which is gone"
            );
        }
    }
}

/// Finding: a file brought into media/ but not yet recorded (a render the
/// engine is writing, or `bring_in` before `add_clip`) is "unused", so a
/// Clean up press in between moves it to the trash, and `add_clip` then
/// records a clip whose file isn't there; emptying the trash loses the sound.
#[test]
#[ignore = "defect: clean-up can trash a file between bring_in and add_clip (review finding)"]
fn clean_up_never_takes_a_file_that_is_about_to_be_recorded() {
    let (dir, mut store) = library();
    let src = source_file(dir.path(), "Low Tide.wav", b"RIFF....WAVE");
    let new = store.bring_in(&src, &ByExtension).unwrap();
    let unused = store.unused_media().unwrap();
    store.move_to_trash(&unused).unwrap();
    let mut tx = store.begin(Room::Library, "import 'Low Tide'").unwrap();
    let recorded = tx.add_clip(new.clone());
    if recorded.is_ok() {
        tx.commit().unwrap();
        store.empty_trash().unwrap();
        let clip = store.clip(&new.id).unwrap().unwrap();
        assert!(
            store.path_of(&clip).exists(),
            "the library holds '{}' but its file was cleaned up",
            clip.title
        );
    }
}

/// Finding (low): a backup dated before the seven kept is written and then
/// pruned at once, and the path returned names a file that isn't there.
#[test]
#[ignore = "defect: backup returns a pruned path (review finding)"]
fn the_path_a_backup_returns_exists() {
    let (dir, store) = library();
    let backups = dir.path().join("backups");
    for day in 1..=7 {
        store
            .backup(&backups, &format!("2026-10-{day:02}"))
            .unwrap();
    }
    let path = store.backup(&backups, "2026-09-30").unwrap(); // the clock stepped back
    assert!(path.exists(), "{} was returned but pruned", path.display());
}
