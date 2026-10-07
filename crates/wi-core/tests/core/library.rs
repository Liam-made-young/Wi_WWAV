//! The library through the core (docs/PLAN.md S1.7; docs/SPEC.md 2.5,
//! 2.14). What a fail looks like:
//! - import names a file anything but a ULID, or renaming a song renames
//!   (or rewrites) its file;
//! - a plain WAV comes in as anything but master only, or without saying so;
//! - a tag isn't lowercase, or a clip takes a 13th;
//! - a fifth pin fits;
//! - a delete removes a file;
//! - the folder sentence isn't 2.14's, or pressing import again brings a
//!   file in twice.

use std::path::Path;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

fn media_name(core: &Core, id: &str) -> String {
    let dir = core.library().join("media");
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(id))
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
    names[0].clone()
}

fn sha256(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(std::fs::read(path).unwrap()))
}

#[test]
fn files_are_named_by_ulid_and_renaming_touches_no_file() {
    let setup = Setup::new();
    let core = setup.core();
    let ids = import(
        &core,
        &[
            &corpus("original.wwav"),
            &corpus("mono.wav"),
            &corpus("large.swav"),
        ],
    );
    assert_eq!(ids.len(), 3);
    for id in &ids {
        assert!(wwav_ids::is_ulid(id), "{id}");
        let name = media_name(&core, id);
        let (stem, _) = name.split_once('.').unwrap();
        assert!(wwav_ids::is_ulid(stem), "{name}");
    }
    let song = &ids[0];
    let file = core.library().join("media").join(media_name(&core, song));
    let before = sha256(&file);
    let r = ok(
        &core,
        "library.rename",
        json!({"id": song, "title": "World Ending", "label": "rename song"}),
    );
    assert_eq!(r["clips"][0]["title"], "World Ending");
    assert_eq!(
        media_name(&core, song),
        file.file_name().unwrap().to_string_lossy()
    );
    assert_eq!(sha256(&file), before, "renaming rewrote the file");
    assert_eq!(
        ok(&core, "history.get", json!({"room": "library"}))["undo"],
        "Undo rename song"
    );
    // Two songs called the same never collide.
    std::fs::copy(
        corpus("dup-wmet.wwav"),
        setup.dir.path().join("untitled.wwav"),
    )
    .unwrap();
    std::fs::copy(
        corpus("no-wstm.wwav"),
        setup.dir.path().join("untitled2.wwav"),
    )
    .unwrap();
    let two = import(
        &core,
        &[
            &setup.dir.path().join("untitled.wwav"),
            &setup.dir.path().join("untitled2.wwav"),
        ],
    );
    for id in &two {
        ok(
            &core,
            "library.rename",
            json!({"id": id, "title": "untitled", "label": "rename"}),
        );
    }
    assert_ne!(media_name(&core, &two[0]), media_name(&core, &two[1]));
}

#[test]
fn a_plain_wav_comes_in_as_master_only_and_says_so() {
    let setup = Setup::new();
    let core = setup.core();
    let folder = setup.dir.path().join("Music");
    std::fs::create_dir_all(folder.join("sub")).unwrap();
    for (from, to) in [
        ("original.wwav", "a.wwav"),
        ("master-only.wwav", "sub/b.wwav"),
        ("large.swav", "c.swav"),
        ("mono.wav", "d.wav"),
        ("48k.wav", "sub/e.wav"),
        ("24bit.wav", "f.wav"),
    ] {
        std::fs::copy(corpus(from), folder.join(to)).unwrap();
    }
    std::fs::write(folder.join("notes.pdf"), b"%PDF").unwrap();
    std::fs::write(folder.join(".DS_Store"), b"").unwrap();
    let summary = ok(&core, "library.inspect", json!({"path": folder}));
    assert_eq!(
        summary["summary"],
        "7 files: 2 .wwav, 1 .swav, 3 plain audio, 1 other. Plain audio comes in as master only."
    );
    let r = ok(
        &core,
        "library.import",
        json!({"paths": [folder], "label": "import Music"}),
    );
    let clips = r["clips"].as_array().unwrap();
    assert_eq!(clips.len(), 6);
    assert_eq!(r["skipped"].as_array().unwrap().len(), 1, "{r}");
    // Get Info's verdict for a plain WAV is the reference tool's. None of
    // these three (mono, 48 kHz, 24-bit) is a master PRANA lists.
    for c in clips.iter().filter(|c| c["kind"] == "audio") {
        assert_eq!(
            c["verdict"],
            "not listed: the master isn't 44.1 kHz 16-bit stereo PCM"
        );
    }
    assert_eq!(clips.iter().filter(|c| c["kind"] == "audio").count(), 3);
    assert_eq!(
        ok(&core, "history.get", json!({"room": "library"}))["undo"],
        "Undo import Music"
    );
    // Pressing again picks up: nothing comes in twice.
    let again = ok(
        &core,
        "library.import",
        json!({"paths": [folder], "label": "import Music"}),
    );
    assert_eq!(again["clips"], json!([]));
    assert_eq!(
        ok(&core, "library.list", json!({}))["clips"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    let summary = ok(
        &core,
        "library.inspect",
        json!({"path": folder.join("sub/b.wwav")}),
    );
    assert_eq!(summary["summary"], "1 file: 1 .wwav.");
}

#[test]
fn tags_are_lowercase_twelve_at_most_and_pins_hold_four() {
    let setup = Setup::new();
    let core = setup.core();
    let id = import(&core, &[&corpus("original.wwav")])[0].clone();
    let r = ok(
        &core,
        "library.tag",
        json!({"ids": [id], "add": ["Live-Drums", "  Late Night "], "label": "tag clip"}),
    );
    assert_eq!(r["clips"][0]["tags"], json!(["late night", "live-drums"]));
    let more: Vec<String> = (0..10).map(|n| format!("t{n}")).collect();
    ok(
        &core,
        "library.tag",
        json!({"ids": [id], "add": more, "label": "tag clip"}),
    );
    let thirteenth = core
        .invoke(
            "library.tag",
            json!({"ids": [id], "add": ["one-more"], "label": "tag clip"}),
        )
        .unwrap_err();
    assert_eq!(
        thirteenth.message,
        "A clip holds 12 tags. Remove one first."
    );
    let tags = ok(&core, "library.get", json!({"id": id}))["clip"]["tags"].clone();
    assert_eq!(tags.as_array().unwrap().len(), 12);

    let five: Vec<String> = (0..5)
        .map(|_| {
            let p = setup.dir.path().join(format!("{}.txt", wwav_ids::ulid()));
            std::fs::write(&p, wwav_ids::ulid()).unwrap();
            import(&core, &[&p])[0].clone()
        })
        .collect();
    for c in &five[..4] {
        ok(
            &core,
            "library.pin",
            json!({"id": c, "slot": 0, "label": "pin"}),
        );
    }
    let fifth = core
        .invoke(
            "library.pin",
            json!({"id": five[4], "slot": 0, "label": "pin"}),
        )
        .unwrap_err();
    assert_eq!(fifth.message, "Pins hold 4. Unpin one first.");
    let pins = ok(
        &core,
        "library.pin",
        json!({"id": five[0], "slot": null, "label": "unpin"}),
    )["pins"]
        .clone();
    assert_eq!(
        pins,
        json!([{"clip": five[1]}, {"clip": five[2]}, {"clip": five[3]}, null])
    );
    ok(
        &core,
        "library.pin",
        json!({"id": five[4], "slot": 3, "label": "pin"}),
    );

    let colour = ok(
        &core,
        "library.colour",
        json!({"ids": [id], "colour": "purple", "label": "colour"}),
    );
    assert_eq!(colour["clips"][0]["colour"], "purple");
    let bad = core.invoke(
        "library.colour",
        json!({"ids": [id], "colour": "teal", "label": "colour"}),
    );
    assert_eq!(bad.unwrap_err().code, "bad_args");
}

#[test]
fn a_delete_keeps_the_file_until_the_trash_is_emptied() {
    let setup = Setup::new();
    let core = setup.core();
    let id = import(&core, &[&corpus("original.wwav")])[0].clone();
    let file = core.library().join("media").join(media_name(&core, &id));
    ok(
        &core,
        "library.delete",
        json!({"ids": [id], "label": "delete clip"}),
    );
    assert!(file.exists(), "a delete removed the file");
    assert_eq!(ok(&core, "library.list", json!({}))["clips"], json!([]));
    // Undo brings it back with its sound.
    assert_eq!(
        ok(&core, "history.undo", json!({"room": "library"}))["label"],
        "delete clip"
    );
    assert_eq!(ok(&core, "library.list", json!({}))["clips"][0]["id"], id);
    ok(&core, "history.redo", json!({"room": "library"}));

    // Clean up media: a file no row and no journal entry names. The journal
    // still names this one, so it stays.
    let preview = ok(&core, "library.cleanup.preview", json!({}));
    assert_eq!(preview["sentence"], "Nothing to clean up.");
    std::fs::write(core.library().join("media/stray.wav"), vec![0u8; 2048]).unwrap();
    let preview = ok(&core, "library.cleanup.preview", json!({}));
    assert_eq!(preview["sentence"], "2.0 KB in 1 file");
    let moved = ok(&core, "library.cleanup.run", json!({}));
    assert_eq!(moved["moved"], json!(["media/stray.wav"]));
    assert!(file.exists());
    let emptied = ok(&core, "library.trash.empty", json!({}));
    assert_eq!(emptied["deleted"], json!(["trash/stray.wav"]));
}

#[test]
fn search_lists_and_smart_folders_filter_the_library() {
    let setup = Setup::new();
    let core = setup.core();
    let ids = import(&core, &[&corpus("original.wwav"), &corpus("mono.wav")]);
    ok(
        &core,
        "library.tag",
        json!({"ids": [ids[0]], "add": ["live-drums"], "label": "tag"}),
    );
    let found = ok(
        &core,
        "library.search",
        json!({"q": "tést tag:live-drums key:minor bpm:118-122"}),
    );
    assert_eq!(found["clips"].as_array().unwrap().len(), 1);
    assert_eq!(found["clips"][0]["id"], ids[0]);
    assert_eq!(
        ok(&core, "library.search", json!({"q": "bpm:90-100"}))["clips"],
        json!([])
    );

    let folder = ok(
        &core,
        "library.smart.save",
        json!({"name": "Minor", "rules": {"key": "minor", "kind": "wwav"}, "label": "save smart folder"}),
    )["folder"]
        .clone();
    let listed = ok(&core, "library.list", json!({"smart": folder["id"]}));
    assert_eq!(listed["clips"].as_array().unwrap().len(), 1);
    let page = ok(&core, "library.list", json!({"limit": 1}));
    assert_eq!(page["clips"].as_array().unwrap().len(), 1);
    let next = page["next"].as_str().unwrap();
    let rest = ok(&core, "library.list", json!({"after": next}));
    assert_eq!(rest["clips"].as_array().unwrap().len(), 1);
    assert_eq!(rest["next"], Value::Null);
    let clip = &rest["clips"][0];
    for key in [
        "id",
        "kind",
        "title",
        "artist",
        "bpm",
        "key",
        "duration",
        "colour",
        "tags",
        "pinned",
        "verdict",
        "published",
        "sha256",
        "bytes",
        "created",
    ] {
        assert!(clip.get(key).is_some(), "Clip has no {key}");
    }
}
