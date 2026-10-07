//! The undo journal (docs/PLAN.md F5). It fails if, after a random run of
//! edits, undo-all doesn't return the first state byte for byte or redo-all
//! the last; if ⌘Z acts outside the current room; if a label is lost on
//! relaunch; or if any table has ON DELETE CASCADE.

mod common;

use common::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde_json::json;
use std::collections::BTreeMap;
use wi_store::*;

const TITLES: [&str; 6] = [
    "untitled",
    "Low Tide",
    "glass hours",
    "World Ending",
    "Beyoncé",
    "break",
];
const KEYS: [&str; 4] = ["A minor", "C major", "F# minor", "Eb major"];
const TAGS: [&str; 15] = [
    "live-drums",
    "wip",
    "dark",
    "rough",
    "sketch",
    "keeper",
    "vocal",
    "lofi",
    "tape",
    "night",
    "dub",
    "choir",
    "bass",
    "fast",
    "slow",
];
const DOC_KINDS: [&str; 3] = ["task", "space", "time_block"];
const DOC_KEYS: [&str; 4] = ["1", "2", "em-77", "gp-3"];

struct Known {
    clips: Vec<String>,
    folders: Vec<String>,
    sequences: Vec<String>,
}

impl Known {
    /// What the library holds now, so most edits land.
    fn read(store: &Store) -> Known {
        Known {
            clips: store.clips().unwrap().into_iter().map(|c| c.id).collect(),
            folders: store
                .smart_folders()
                .unwrap()
                .into_iter()
                .map(|f| f.id)
                .collect(),
            sequences: store
                .sequences()
                .unwrap()
                .into_iter()
                .map(|s| s.id)
                .collect(),
        }
    }
}

fn any<'a>(rng: &mut StdRng, ids: &'a [String]) -> &'a str {
    if ids.is_empty() || rng.gen_bool(0.03) {
        "01JC5Q8V3M2T7R9X4K6W0YHZNB" // not in the library: the edit is refused
    } else {
        &ids[rng.gen_range(0..ids.len())]
    }
}

fn pick<T: Copy>(rng: &mut StdRng, from: &[T]) -> T {
    from[rng.gen_range(0..from.len())]
}

/// A BPM as a reader measures it: any f64, most of which take all 17
/// significant digits to write down (121.60764625443461), so the journal's
/// JSON must carry them exactly.
fn measured(rng: &mut StdRng) -> f64 {
    rng.gen_range(40.0..250.0)
}

/// [`ByExtension`], with a measured BPM.
struct Measured(f64);

impl Inspector for Measured {
    fn inspect(&self, path: &std::path::Path) -> Result<Inspection> {
        Ok(Inspection {
            bpm: Some(self.0),
            ..ByExtension.inspect(path)?
        })
    }
}

fn tag_kind(rng: &mut StdRng) -> TagKind {
    pick(
        rng,
        &[TagKind::User, TagKind::User, TagKind::Card, TagKind::System],
    )
}

/// One random edit inside an open transaction.
fn random_edit(tx: &mut Txn, rng: &mut StdRng, known: &mut Known) -> Result<()> {
    let clip = any(rng, &known.clips).to_string();
    match rng.gen_range(0..17) {
        0 => tx.rename_clip(&clip, pick(rng, &TITLES)),
        1 => tx.set_artist(&clip, pick(rng, &["LMY", "Ana", ""])),
        2 => {
            let colour = pick(
                rng,
                &[
                    None,
                    Some(Colour::Red),
                    Some(Colour::Blue),
                    Some(Colour::Purple),
                ],
            );
            tx.set_colour(&clip, colour)
        }
        3 => {
            let bpm = match rng.gen_range(0..5) {
                4 => Some(measured(rng)),
                n => [None, Some(86.0), Some(128.0), Some(130.5)][n],
            };
            tx.set_bpm(&clip, bpm)
        }
        4 => tx.set_key(&clip, pick(rng, &[None, Some("A minor"), Some("C major")])),
        5 | 6 => tx.add_tag(&clip, pick(rng, &TAGS), tag_kind(rng)),
        7 => tx.remove_tag(&clip, pick(rng, &TAGS), tag_kind(rng)),
        8 => {
            let target = if rng.gen_bool(0.7) {
                Pin::Clip(clip)
            } else {
                Pin::Folder(any(rng, &known.folders).to_string())
            };
            if rng.gen_bool(0.6) {
                tx.pin(&target).map(|_| ())
            } else {
                tx.unpin(&target)
            }
        }
        9 => tx.publish(&clip),
        10 => tx.delete_clip(&clip),
        11 => {
            if rng.gen_bool(0.6) {
                let rule = SmartRule {
                    tags: vec![pick(rng, &TAGS).to_string()],
                    bpm_min: Some(120.0),
                    key: Some(pick(rng, &["minor", "A minor"]).to_string()),
                    ..SmartRule::default()
                };
                let id = tx.save_smart_folder(pick(rng, &["minor", "live"]), &rule)?;
                known.folders.push(id);
                Ok(())
            } else {
                tx.delete_smart_folder(any(rng, &known.folders))
            }
        }
        12 => {
            let (kind, key) = (pick(rng, &DOC_KINDS), pick(rng, &DOC_KEYS));
            let n = rng.gen_range(0..1000);
            let json =
                json!({"id": key, "title": format!("task {n}"), "estMin": n, "done": n % 2 == 0});
            tx.put_doc(kind, key, &json, &format!("task {n}"))
        }
        13 => tx.delete_doc(pick(rng, &DOC_KINDS), pick(rng, &DOC_KEYS)),
        14 => {
            let seq = tx.add_sequence(pick(rng, &TITLES))?;
            known.sequences.push(seq.id);
            Ok(())
        }
        15 => {
            let seq = any(rng, &known.sequences).to_string();
            if rng.gen_bool(0.5) {
                tx.rename_sequence(&seq, pick(rng, &TITLES))
            } else {
                tx.delete_sequence(&seq)
            }
        }
        _ => {
            // A render: a new clip that points back at its sequence.
            let id = wwav_ids::ulid();
            let bpm = rng.gen_bool(0.5).then(|| measured(rng));
            let render = NewClip {
                file: format!("media/{id}.wwav"),
                id,
                sha256: "0".repeat(64),
                bytes: 44,
                info: Inspection {
                    bpm,
                    key: Some(pick(rng, &KEYS).to_string()),
                    verdict: "4 stems, and the master".to_string(),
                    ..Inspection::new(Kind::Wwav, pick(rng, &TITLES))
                },
                from_sequence: Some(any(rng, &known.sequences).to_string()),
            };
            known.clips.push(tx.add_clip(render)?);
            Ok(())
        }
    }
}

/// Exactly one journal entry changed, it belongs to `room`, and it went from
/// `from` to `to`.
fn only_one_moved(
    before: &BTreeMap<String, (String, String)>,
    after: &BTreeMap<String, (String, String)>,
    room: Room,
    from: &str,
    to: &str,
) {
    let moved: Vec<_> = before
        .iter()
        .filter(|(id, v)| after.get(*id) != Some(v))
        .collect();
    assert_eq!(
        moved.len(),
        1,
        "one step must move exactly one entry: {moved:?}"
    );
    let (id, (entry_room, state)) = moved[0];
    assert_eq!(
        entry_room,
        room.as_str(),
        "⌘Z in {room:?} moved an entry of {entry_room}"
    );
    assert_eq!(state, from);
    assert_eq!(after[id].1, to);
    assert_eq!(
        before.len(),
        after.len(),
        "a step never adds or drops entries"
    );
}

#[derive(Debug, Default)]
struct Tally {
    commits: usize,
    refused_edits: usize,
    undos: usize,
    redos: usize,
    held_back: usize,
}

fn random_run(seed: u64, steps: usize) -> Tally {
    let (dir, mut store) = library();
    let root = store.root().to_path_buf();
    let mut rng = StdRng::seed_from_u64(seed);
    // The first state already holds clips, bought, so no undo removes them:
    // every value the run changes on them, a measured BPM among them, must
    // come back exactly.
    for (n, title) in TITLES.iter().enumerate() {
        let src = source_file(dir.path(), &format!("{title}.wwav"), &[n as u8; 32]);
        let receipt = Receipt {
            remote_id: format!("pur_{n}"),
            file_name: format!("{title}.wwav"),
            bytes: 32,
            sha256: hex::encode(<sha2::Sha256 as sha2::Digest>::digest([n as u8; 32])),
            json: json!({}),
        };
        let bpm = measured(&mut rng);
        store
            .record_purchase(Room::Unquantized, &src, &receipt, &Measured(bpm))
            .unwrap();
    }
    let first = dump(&root);
    let mut tally = Tally::default();

    for step in 0..steps {
        let mut known = Known::read(&store);
        let room = pick(&mut rng, &ROOMS);
        let roll = rng.gen_range(0..100);
        if roll < 40 {
            // Several steps in a row, so undone changes pile up across rooms.
            let undo = roll < 25;
            for _ in 0..rng.gen_range(1..=4) {
                let before = journal(&root);
                let result = if undo {
                    store.undo(room)
                } else {
                    store.redo(room)
                };
                match result {
                    Ok(Some(_)) if undo => {
                        tally.undos += 1;
                        only_one_moved(&before, &journal(&root), room, "done", "undone");
                    }
                    Ok(Some(_)) => {
                        tally.redos += 1;
                        only_one_moved(&before, &journal(&root), room, "undone", "done");
                    }
                    Ok(None) => assert_eq!(before, journal(&root)),
                    Err(Error::Refused(_)) => {
                        tally.held_back += 1;
                        assert_eq!(before, journal(&root));
                    }
                    Err(e) => panic!("seed {seed} step {step}: {e}"),
                }
            }
        } else if roll < 46 {
            let bytes: Vec<u8> = (0..rng.gen_range(1..64)).map(|_| rng.gen()).collect();
            let name = format!(
                "{}.{}",
                pick(&mut rng, &TITLES),
                pick(&mut rng, &["wav", "wwav", "png"])
            );
            let src = source_file(dir.path(), &name, &bytes);
            let bpm = measured(&mut rng);
            store.import(room, &src, &Measured(bpm)).unwrap();
            tally.commits += 1;
        } else {
            let before = dump(&root);
            let label = format!("edit {step}");
            let mut tx = store.begin(room, &label).unwrap();
            let mut outcome = Ok(());
            for _ in 0..rng.gen_range(1..=3) {
                outcome = random_edit(&mut tx, &mut rng, &mut known);
                if outcome.is_err() {
                    break;
                }
            }
            match outcome {
                Err(Error::Refused(_)) => {
                    drop(tx);
                    tally.refused_edits += 1;
                    assert_eq!(
                        before,
                        dump(&root),
                        "seed {seed} step {step}: a refused edit left a trace"
                    );
                }
                Err(e) => panic!("seed {seed} step {step}: {e}"),
                Ok(()) => {
                    let committed = tx.commit().unwrap();
                    tally.commits += 1;
                    if committed.is_some() && rng.gen_bool(0.25) {
                        // The newest change always undoes, back to the state before it.
                        let after = dump(&root);
                        assert_eq!(store.undo(room).unwrap().as_deref(), Some(label.as_str()));
                        assert_eq!(
                            before,
                            dump(&root),
                            "seed {seed} step {step}: undo of {label}"
                        );
                        assert_eq!(store.redo(room).unwrap().as_deref(), Some(label.as_str()));
                        assert_eq!(
                            after,
                            dump(&root),
                            "seed {seed} step {step}: redo of {label}"
                        );
                        tally.undos += 1;
                        tally.redos += 1;
                    }
                }
            }
        }
    }

    redo_all(&mut store);
    let last = dump(&root);
    let labels: Vec<(String, String)> = ROOMS
        .iter()
        .map(|&r| {
            let h = store.history(r).unwrap();
            (h.undo_text(), h.redo_text())
        })
        .collect();

    undo_all(&mut store);
    assert_eq!(
        first,
        dump(&root),
        "seed {seed}: undo-all must return the first state byte for byte"
    );
    assert!(
        journal(&root).values().all(|(_, state)| state == "undone"),
        "seed {seed}: undo-all left work done"
    );
    search_indexes_agree(&root);

    redo_all(&mut store);
    assert_eq!(
        last,
        dump(&root),
        "seed {seed}: redo-all must return the last state byte for byte"
    );
    search_indexes_agree(&root);

    drop(store);
    let store = Store::open(&root).unwrap();
    for (room, labels) in ROOMS.iter().zip(&labels) {
        let h = store.history(*room).unwrap();
        assert_eq!(
            &(h.undo_text(), h.redo_text()),
            labels,
            "seed {seed}: {room:?} lost its labels on reopening"
        );
    }
    tally
}

#[test]
fn undo_all_returns_the_first_state_and_redo_all_the_last() {
    for seed in 1..=6 {
        let tally = random_run(seed, 1500);
        // The run must actually cross rooms and hit the rules, or it proves nothing.
        assert!(tally.commits > 600, "seed {seed}: {tally:?}");
        assert!(tally.refused_edits > 20, "seed {seed}: {tally:?}");
        assert!(
            tally.undos > 50 && tally.redos > 20,
            "seed {seed}: {tally:?}"
        );
        assert!(tally.held_back > 5, "seed {seed}: {tally:?}");
        eprintln!("seed {seed}: {tally:?}");
    }
}

fn import(store: &mut Store, dir: &std::path::Path, room: Room, name: &str) -> String {
    let src = source_file(dir, name, name.as_bytes());
    store.import(room, &src, &ByExtension).unwrap().id
}

fn rename(store: &mut Store, room: Room, label: &str, clip: &str, title: &str) {
    let mut tx = store.begin(room, label).unwrap();
    tx.rename_clip(clip, title).unwrap();
    tx.commit().unwrap();
}

fn title(store: &Store, clip: &str) -> String {
    store.clip(clip).unwrap().unwrap().title
}

#[test]
fn an_empty_room_says_nothing_to_undo() {
    let (_dir, mut store) = library();
    let h = store.history(Room::Heat).unwrap();
    assert_eq!(h.undo_text(), "Nothing to undo.");
    assert_eq!(h.redo_text(), "Nothing to redo.");
    assert_eq!(store.undo(Room::Heat).unwrap(), None);
    assert_eq!(store.redo(Room::Heat).unwrap(), None);
}

#[test]
fn undo_acts_only_in_its_room() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "Low Tide.wav");
    let mut tx = store.begin(Room::Heat, "add task").unwrap();
    tx.put_doc("task", "t1", &json!({"title": "mix EP"}), "mix EP")
        .unwrap();
    tx.commit().unwrap();
    rename(
        &mut store,
        Room::Console,
        "rename clip",
        &clip,
        "Low Tide v2",
    );

    assert_eq!(
        store.undo(Room::Space).unwrap(),
        None,
        "Space has nothing of its own to undo"
    );
    assert_eq!(store.undo(Room::Heat).unwrap().as_deref(), Some("add task"));
    assert!(store.doc("task", "t1").unwrap().is_none());
    assert_eq!(
        title(&store, &clip),
        "Low Tide v2",
        "⌘Z in Heat must not touch the Console's rename"
    );
    assert_eq!(
        store.history(Room::Console).unwrap().undo_text(),
        "Undo rename clip"
    );
    assert_eq!(
        store.history(Room::Heat).unwrap().undo_text(),
        "Nothing to undo."
    );
    assert_eq!(
        store.history(Room::Heat).unwrap().redo_text(),
        "Redo add task"
    );
}

#[test]
fn undo_waits_for_a_later_change_to_the_same_thing_in_another_room() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "break.wav");
    rename(&mut store, Room::Library, "rename clip", &clip, "A");
    rename(&mut store, Room::Space, "retitle", &clip, "B");

    let held = "Can't undo rename clip yet. Undo retitle in Space first.";
    assert_eq!(store.history(Room::Library).unwrap().undo_text(), held);
    match store.undo(Room::Library) {
        Err(Error::Refused(s)) => assert_eq!(s, held),
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(title(&store, &clip), "B");
    assert_eq!(store.undo(Room::Space).unwrap().as_deref(), Some("retitle"));
    assert_eq!(
        store.undo(Room::Library).unwrap().as_deref(),
        Some("rename clip")
    );
    assert_eq!(title(&store, &clip), "break");

    // Different values of the same clip don't hold each other back.
    let mut tx = store.begin(Room::Library, "colour clip").unwrap();
    tx.set_colour(&clip, Some(Colour::Green)).unwrap();
    tx.commit().unwrap();
    rename(&mut store, Room::Space, "retitle", &clip, "C");
    assert_eq!(
        store.undo(Room::Library).unwrap().as_deref(),
        Some("colour clip")
    );
    assert_eq!(store.clip(&clip).unwrap().unwrap().colour, None);
    assert_eq!(title(&store, &clip), "C");
}

#[test]
fn undoing_an_import_waits_for_a_tag_another_room_put_on_it() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "glass hours.wav");
    let mut tx = store.begin(Room::Space, "tag clip").unwrap();
    tx.add_tag(&clip, "covers", TagKind::System).unwrap();
    tx.commit().unwrap();
    assert!(matches!(store.undo(Room::Library), Err(Error::Refused(_))));
    assert!(store.clip(&clip).unwrap().is_some());
    store.undo(Room::Space).unwrap();
    store.undo(Room::Library).unwrap();
    assert!(store.clip(&clip).unwrap().is_none());
}

#[test]
fn a_new_edit_discards_the_rooms_redo_branch() {
    let (dir, mut store) = library();
    let put = |store: &mut Store, room: Room, label: &str, key: &str| {
        let mut tx = store.begin(room, label).unwrap();
        tx.put_doc("task", key, &json!({"title": label}), label)
            .unwrap();
        tx.commit().unwrap();
    };
    put(&mut store, Room::Heat, "add a", "a");
    put(&mut store, Room::Heat, "add b", "b");
    store.undo(Room::Heat).unwrap();
    assert_eq!(store.history(Room::Heat).unwrap().redo_text(), "Redo add b");
    put(&mut store, Room::Heat, "add c", "c");
    assert_eq!(
        store.history(Room::Heat).unwrap().redo_text(),
        "Nothing to redo."
    );
    assert!(store.doc("task", "b").unwrap().is_none());

    // Another room's redo survives an edit elsewhere...
    put(&mut store, Room::Console, "add d", "d");
    store.undo(Room::Console).unwrap();
    put(&mut store, Room::Heat, "add e", "e");
    assert_eq!(
        store.history(Room::Console).unwrap().redo_text(),
        "Redo add d"
    );

    // ...unless the edit changed the same value, which it could no longer redo onto.
    let clip = import(&mut store, dir.path(), Room::Library, "untitled.wav");
    rename(&mut store, Room::Space, "retitle", &clip, "X");
    store.undo(Room::Space).unwrap();
    rename(&mut store, Room::Library, "rename clip", &clip, "Y");
    assert_eq!(
        store.history(Room::Space).unwrap().redo_text(),
        "Nothing to redo."
    );
}

#[test]
fn a_discarded_change_takes_the_undone_changes_built_on_it() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "orig.wav");
    rename(&mut store, Room::Library, "rename clip", &clip, "v");
    rename(&mut store, Room::Space, "retitle", &clip, "w"); // made over "v"
    store.undo(Room::Space).unwrap();
    store.undo(Room::Library).unwrap();
    assert_eq!(title(&store, &clip), "orig");

    // A new edit in the library ends its redo branch: "rename clip" is gone,
    // so "retitle", which was made over it, can't be redone either.
    let mut tx = store.begin(Room::Library, "colour clip").unwrap();
    tx.set_colour(&clip, Some(Colour::Blue)).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        store.history(Room::Library).unwrap().redo_text(),
        "Nothing to redo."
    );
    assert_eq!(
        store.history(Room::Space).unwrap().redo_text(),
        "Nothing to redo."
    );
    redo_all(&mut store);
    assert_eq!(
        title(&store, &clip),
        "orig",
        "nothing redoes onto a change that is gone"
    );
}

#[test]
fn labels_survive_reopening() {
    let (dir, mut store) = library();
    let root = store.root().to_path_buf();
    let clip = import(&mut store, dir.path(), Room::Console, "World Ending.wav");
    rename(
        &mut store,
        Room::Console,
        "move clip",
        &clip,
        "World Ending (edit)",
    );
    drop(store);

    let mut store = Store::open(&root).unwrap();
    assert_eq!(
        store.history(Room::Console).unwrap().undo_text(),
        "Undo move clip"
    );
    assert_eq!(
        store.undo(Room::Console).unwrap().as_deref(),
        Some("move clip")
    );
    drop(store);

    let store = Store::open(&root).unwrap();
    let h = store.history(Room::Console).unwrap();
    assert_eq!(h.undo_text(), "Undo import 'World Ending'");
    assert_eq!(h.redo_text(), "Redo move clip");
}

#[test]
fn publishing_undoes_only_while_queued() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "World Ending.wav");
    let publish = |store: &mut Store| {
        let mut tx = store.begin(Room::Space, "publish 'World Ending'").unwrap();
        tx.publish(&clip).unwrap();
        tx.commit().unwrap();
    };
    publish(&mut store);
    assert_eq!(ids(&store.upload_queue().unwrap()), [clip.as_str()]);
    assert!(store.clip(&clip).unwrap().unwrap().is_queued());

    // Undo while queued cancels the upload: the queue loses it, with no queue to edit.
    assert!(store.undo(Room::Space).unwrap().is_some());
    assert!(store.upload_queue().unwrap().is_empty());
    assert_eq!(store.clip(&clip).unwrap().unwrap().published_at, None);
    // An answer that arrives after the cancel doesn't publish it behind your back.
    assert!(!store.mark_uploaded(&clip, "late").unwrap());
    assert_eq!(store.clip(&clip).unwrap().unwrap().remote_id, None);

    store.redo(Room::Space).unwrap();
    assert!(store.mark_uploaded(&clip, "trk_42").unwrap());
    assert!(store.upload_queue().unwrap().is_empty());
    let up = store.clip(&clip).unwrap().unwrap();
    assert!(up.is_up() && !up.is_queued());

    let held = "Can't undo a publish. Unpublish 'World Ending'…";
    assert_eq!(store.history(Room::Space).unwrap().undo_text(), held);
    assert!(matches!(store.undo(Room::Space), Err(Error::Refused(s)) if s == held));
    let still = store.clip(&clip).unwrap().unwrap();
    assert!(still.published_at.is_some());
    assert_eq!(still.remote_id.as_deref(), Some("trk_42"));
}

#[test]
fn an_import_the_server_has_waits_on_its_publish() {
    let (dir, mut store) = library();
    let clip = import(&mut store, dir.path(), Room::Library, "World Ending.wav");
    let mut tx = store.begin(Room::Space, "publish 'World Ending'").unwrap();
    tx.publish(&clip).unwrap();
    tx.commit().unwrap();
    assert!(store.mark_uploaded(&clip, "trk_1").unwrap());
    // The import published nothing; the publish it waits on can't be undone.
    assert_eq!(
        store.history(Room::Library).unwrap().undo_text(),
        "Can't undo import 'World Ending' yet. Undo publish 'World Ending' in Space first."
    );
    assert_eq!(
        store.history(Room::Space).unwrap().undo_text(),
        "Can't undo a publish. Unpublish 'World Ending'…"
    );

    // One change that both brought a clip in and published it is a publish.
    let src = source_file(dir.path(), "Low Tide.wav", b"Low Tide");
    let new = store.bring_in(&src, &ByExtension).unwrap();
    let mut tx = store.begin(Room::Space, "drop 'Low Tide'").unwrap();
    let low = tx.add_clip(new).unwrap();
    tx.publish(&low).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        store.history(Room::Space).unwrap().undo_text(),
        "Undo drop 'Low Tide'"
    );
    assert!(store.mark_uploaded(&low, "trk_2").unwrap());
    let held = "Can't undo a publish. Unpublish 'Low Tide'…";
    assert_eq!(store.history(Room::Space).unwrap().undo_text(), held);
    assert!(matches!(store.undo(Room::Space), Err(Error::Refused(s)) if s == held));
    assert!(store.clip(&low).unwrap().unwrap().is_up());
}

#[test]
fn work_that_left_the_machine_is_not_undone() {
    let (dir, mut store) = library();
    let mut tx = store.begin(Room::Unquantized, "save for later").unwrap();
    tx.put_doc("saved", "rec-1", &json!({"record": "rec-1"}), "")
        .unwrap();
    tx.commit().unwrap();
    let journal_before = journal(store.root());

    store
        .record_outward(Room::Unquantized, "a purchase")
        .unwrap();
    assert_eq!(
        journal(store.root()),
        journal_before,
        "a purchase is never a journal entry"
    );
    assert_eq!(
        store.history(Room::Unquantized).unwrap().undo_text(),
        "Can't undo a purchase."
    );
    assert!(
        matches!(store.undo(Room::Unquantized), Err(Error::Refused(s)) if s == "Can't undo a purchase.")
    );
    assert!(store.doc("saved", "rec-1").unwrap().is_some());

    // Other rooms carry on.
    let clip = import(&mut store, dir.path(), Room::Library, "a.wav");
    assert_eq!(
        store.history(Room::Library).unwrap().undo_text(),
        "Undo import 'a'"
    );
    assert!(store.undo(Room::Library).unwrap().is_some());
    assert!(store.clip(&clip).unwrap().is_none());
}

#[test]
fn no_table_cascades() {
    let (_dir, store) = library();
    let conn = rusqlite::Connection::open(store.root().join("library.sqlite")).unwrap();
    let mut stmt = conn
        .prepare("SELECT name, sql FROM sqlite_master WHERE sql IS NOT NULL")
        .unwrap();
    let schema: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        schema.iter().any(|(_, sql)| sql.contains("REFERENCES")),
        "the test needs foreign keys to look at"
    );
    for (name, sql) in &schema {
        assert!(
            !sql.to_uppercase().contains("CASCADE"),
            "{name} cascades: {sql}"
        );
    }
}

fn ids(clips: &[Clip]) -> Vec<String> {
    clips.iter().map(|c| c.id.clone()).collect()
}
