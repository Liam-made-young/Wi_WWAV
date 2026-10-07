//! Export everything, the core's half (docs/PLAN.md S1.10; docs/SPEC.md
//! 2.9). The offline page itself is played in Chromium by
//! app/ui/e2e/export-offline.spec.ts. What a fail looks like:
//! - the export misses a library file, a session or a record;
//! - any file's sha256 differs from the library's, unsaid;
//! - it needs a sign-in;
//! - index.html reaches for the network.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

fn sha256(path: &Path) -> String {
    hex::encode(Sha256::digest(std::fs::read(path).unwrap()))
}

/// A library holding one of everything, signed out.
fn full_library(setup: &Setup) -> (Core, Vec<String>) {
    let core = setup.core();
    let note = setup.dir.path().join("Ideas.md");
    std::fs::write(&note, "fix the snare at 1:32").unwrap();
    let ids = import(
        &core,
        &[
            &corpus("original.wwav"),
            &corpus("mono.wav"),
            &corpus("large.swav"),
            &note,
        ],
    );
    // A Console session, as the store keeps it.
    let mut store = wi_store::Store::open(core.library()).unwrap();
    let mut tx = store.begin(wi_store::Room::Console, "new session").unwrap();
    let seq = tx.add_sequence("Sketch").unwrap();
    tx.commit().unwrap();
    let package = core.library().join(&seq.package);
    std::fs::create_dir_all(package.join("plugin-state")).unwrap();
    std::fs::write(package.join("session.json"), "{\"title\":\"Sketch\"}\n").unwrap();
    std::fs::write(package.join("plugin-state/01J.state"), vec![7u8; 300_000]).unwrap();
    drop(store);
    for (kind, id, value) in [
        ("space", "sp1", json!({"name": "WWAV", "hue": 210})),
        (
            "project",
            "p1",
            json!({"spaceId": "sp1", "title": "EP", "status": "active"}),
        ),
        (
            "milestone",
            "m1",
            json!({"projectId": "p1", "title": "EP v1 mixed", "date": "2026-11-02", "done": false}),
        ),
        (
            "task",
            "t1",
            json!({"projectId": "p1", "title": "Bounce stems", "done": true}),
        ),
        (
            "dailyNote",
            "2026-10-07",
            json!({"date": "2026-10-07", "markdown": "Mixed the bridge."}),
        ),
        (
            "habit",
            "h1",
            json!({"title": "Scales", "log": {"2026-10-07": true}}),
        ),
    ] {
        ok(
            &core,
            "records.mutate",
            json!({"label": "add", "room": "heat",
            "ops": [{"op": "put", "kind": kind, "id": id, "value": value}]}),
        );
    }
    (core, ids)
}

#[test]
fn everything_comes_out_byte_for_byte_signed_out() {
    let setup = Setup::new();
    let (core, _) = full_library(&setup);
    assert_eq!(ok(&core, "account.status", json!({}))["signedIn"], false);
    let to = setup.dir.path().join("Export");
    let r = ok(&core, "export.everything", json!({"to": to, "zip": false}));
    assert_eq!(r["mismatched"], json!([]));
    assert!(r["files"].as_u64().unwrap() >= 14, "{r}");
    assert_eq!(
        r["sentence"],
        format!("Exported everything to {}.", to.display())
    );

    // Every library file, under its own name, hashed as the library has it.
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(to.join("manifest.json")).unwrap()).unwrap();
    let files = manifest["files"].as_array().unwrap();
    let library = ok(&core, "library.list", json!({}))["clips"].clone();
    let library: BTreeMap<&str, &Value> = library
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["id"].as_str().unwrap(), c))
        .collect();
    assert_eq!(files.len(), library.len());
    for f in files {
        let clip = library[f["id"].as_str().unwrap()];
        let path = to.join(f["path"].as_str().unwrap());
        assert_eq!(
            sha256(&path),
            clip["sha256"].as_str().unwrap(),
            "{}",
            f["path"]
        );
        assert_eq!(f["sha256"], clip["sha256"]);
        assert_eq!(f["verified"], true);
    }
    // Nothing was bought, so there is no purchases folder and no receipts.
    assert!(files
        .iter()
        .all(|f| f["path"].as_str().unwrap().starts_with("media/")));
    assert!(!to.join("purchases").exists() && !to.join("receipts.json").exists());

    // The session, with the plugin state it saved.
    let sessions: Vec<_> = std::fs::read_dir(to.join("sessions"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        std::fs::read(sessions[0].join("plugin-state/01J.state")).unwrap(),
        vec![7u8; 300_000]
    );

    // Every record, by kind.
    let heat: Value =
        serde_json::from_str(&std::fs::read_to_string(to.join("heat.json")).unwrap()).unwrap();
    assert_eq!(heat["version"], 1);
    for kind in [
        "space",
        "project",
        "milestone",
        "task",
        "dailyNote",
        "habit",
    ] {
        assert_eq!(
            heat["records"][kind].as_array().map(Vec::len),
            Some(1),
            "{kind}"
        );
    }

    // Notes Obsidian opens, linked by name.
    let note = |name: &str| std::fs::read_to_string(to.join("notes").join(name)).unwrap();
    assert!(note("Ideas.md").starts_with("---\nid: \""));
    assert!(note("Ideas.md").ends_with("fix the snare at 1:32"));
    assert!(note("2026-10-07.md").contains("Mixed the bridge."));
    assert!(note("WWAV.md").contains("- [[EP]]"));
    let ep = note("EP.md");
    assert!(ep.contains("space: \"[[WWAV]]\""), "{ep}");
    assert!(
        ep.contains("- [ ] EP v1 mixed (2026-11-02)") && ep.contains("- [x] Bounce stems"),
        "{ep}"
    );

    let galaxy: Value =
        serde_json::from_str(&std::fs::read_to_string(to.join("galaxy.json")).unwrap()).unwrap();
    assert_eq!(galaxy["galaxy"], Value::Null);

    // The page: every song and film, from disk, and nothing from a network.
    let page = std::fs::read_to_string(to.join("index.html")).unwrap();
    for needle in [
        "Tést \\\"Song\\\"",
        "media/",
        "That's everything.",
        "Open this folder",
        "webkitdirectory",
    ] {
        assert!(page.contains(needle), "index.html has no {needle}");
    }
    for far in ["http://", "https://", "src=\"//", "@import", "fetch("] {
        assert!(!page.contains(far), "index.html reaches out: {far}");
    }
    assert!(!page.contains("/*TOKENS*/") && !page.contains("/*LIBRARY*/"));
}

#[test]
fn a_zip_holds_the_same_and_a_changed_file_is_named() {
    let setup = Setup::new();
    let (core, ids) = full_library(&setup);
    // A file changed on disk after it came in.
    let clip = ok(&core, "library.get", json!({"id": ids[1]}))["clip"].clone();
    let media: Vec<_> = std::fs::read_dir(core.library().join("media"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&ids[1])
        })
        .collect();
    std::fs::write(&media[0], b"changed").unwrap();

    let to = setup.dir.path().join("Everything.zip");
    let r = ok(&core, "export.everything", json!({"to": to, "zip": true}));
    assert_eq!(
        r["mismatched"],
        json!([format!("'{}'", clip["title"].as_str().unwrap())])
    );
    assert!(r["sentence"]
        .as_str()
        .unwrap()
        .starts_with("Exported, but 1 file has changed on disk"));
    let mut z = zip::ZipArchive::new(std::fs::File::open(&to).unwrap()).unwrap();
    let names: Vec<String> = (0..z.len())
        .map(|i| z.by_index(i).unwrap().name().to_string())
        .collect();
    for want in ["manifest.json", "heat.json", "galaxy.json", "index.html"] {
        assert!(
            names.iter().any(|n| n == want),
            "the zip has no {want}: {names:?}"
        );
    }
    let manifest: Value = {
        let mut f = z.by_name("manifest.json").unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut f, &mut s).unwrap();
        serde_json::from_str(&s).unwrap()
    };
    let changed = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == ids[1].as_str())
        .unwrap();
    assert_eq!(changed["verified"], false);
    assert!(!setup.dir.path().join("Everything.zip.part").exists());

    let full = setup.dir.path().join("Full");
    std::fs::create_dir_all(&full).unwrap();
    std::fs::write(full.join("x"), b"x").unwrap();
    let e = core
        .invoke("export.everything", json!({"to": full}))
        .unwrap_err();
    assert_eq!(e.code, "not_empty");
}
