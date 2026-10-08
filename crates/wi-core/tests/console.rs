use std::fs;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};
use tempfile::TempDir;
use wi_core::{Config, Core, MemorySecrets};

#[path = "console/write.rs"]
mod write;

struct App {
    core: Core,
    dir: TempDir,
}
impl App {
    fn new() -> Self {
        Self::with_claude(None)
    }
    fn with_claude(answer: Option<Value>) -> Self {
        let dir = TempDir::new().unwrap();
        let mut config = Config::new(
            dir.path().join("no-engine"),
            "http://127.0.0.1:1",
            Arc::new(MemorySecrets::default()),
            Arc::new(|_| Ok(())),
        );
        config.tmp_dir = dir.path().join("engine");
        if let Some(answer) = answer {
            use std::os::unix::fs::PermissionsExt;
            let path = dir.path().join("claude-test");
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{}'\n",
                    json!({"structured_output":answer})
                ),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            config.claude = Some(path);
        }
        let core = Core::open(dir.path(), config).unwrap();
        Self { core, dir }
    }
    fn call(&self, cmd: &str, a: Value) -> Value {
        self.core
            .invoke(cmd, a)
            .unwrap_or_else(|e| panic!("{cmd}: {e}"))
    }
    fn create(&self, tool: &str, title: &str) -> Value {
        self.call("console.create", json!({"tool":tool,"title":title}))["document"].clone()
    }
    fn save(&self, d: &Value, text: &str) -> Value {
        self.call(
            "console.save",
            json!({"id":d["id"],"base":d["head"],"text":text}),
        )["document"]
            .clone()
    }
    fn read(&self, d: &Value) -> Value {
        self.call("console.read", json!({"id":d["id"]}))
    }
}

#[test]
fn four_independent_tools_keep_documents_and_selections_after_reopen() {
    let app = App::new();
    let mut docs = Vec::new();
    for tool in ["write", "image", "audiovisual", "three"] {
        let d = app.create(tool, &format!("My {tool}"));
        app.call(
            "console.selection",
            json!({"tool":tool,"selection":{"object":tool}}),
        );
        docs.push(d);
    }
    app.call("console.selectTool", json!({"tool":"write"}));
    let state = app.call("console.workspace", json!({}));
    for d in &docs {
        let t = d["tool"].as_str().unwrap();
        assert_eq!(state["workspace"]["tools"][t]["active"], d["id"]);
        assert_eq!(state["workspace"]["tools"][t]["selection"]["object"], t);
    }
    assert_eq!(
        app.call("console.library", json!({}))["documents"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    let path = app.dir.path().to_path_buf();
    drop(app.core);
    let cfg = Config::new(
        path.join("no-engine"),
        "http://127.0.0.1:1",
        Arc::new(MemorySecrets::default()),
        Arc::new(|_| Ok(())),
    );
    let reopened = Core::open(&path, cfg).unwrap();
    assert_eq!(
        reopened.invoke("console.workspace", json!({})).unwrap(),
        state
    );
}

#[test]
fn saves_are_immutable_versions_and_ordinary_files_and_stale_saves_fail() {
    let app = App::new();
    let first = app.create("write", "Song");
    let next = app.save(&first, "First line\nSecond line\n");
    assert_ne!(first["head"], next["head"]);
    assert_eq!(next["versions"], 2);
    let old = app.call(
        "console.read",
        json!({"id":first["id"],"version":first["head"]}),
    );
    assert_eq!(old["text"], "");
    let read = app.read(&next);
    assert_eq!(read["text"], "First line\nSecond line\n");
    assert_eq!(
        fs::read_to_string(read["path"].as_str().unwrap()).unwrap(),
        read["text"].as_str().unwrap()
    );
    let error = app
        .core
        .invoke(
            "console.save",
            json!({"id":first["id"],"base":first["head"],"text":"old overwrite"}),
        )
        .unwrap_err();
    assert_eq!(error.code, "conflict");
    assert_eq!(app.read(&next)["document"]["versions"], 2);
}

#[test]
fn claude_uses_same_save_and_cannot_forge_a_hand_edit_and_undo_keeps_authorship() {
    let app = App::new();
    let first = app.create("write", "My poem");
    let hand = app.save(&first, "By hand");
    assert_eq!(hand["marker"], "Made by hand");
    let claude=app.call("console.tool.call",json!({"name":"console_save","args":{"id":hand["id"],"base":hand["head"],"text":"Assisted line","actor":"hand"}}))["document"].clone();
    assert_eq!(claude["marker"], "Claude assisted");
    let undo = app.call(
        "console.undo",
        json!({"id":claude["id"],"base":claude["head"]}),
    )["document"]
        .clone();
    assert_eq!(app.read(&undo)["text"], "By hand");
    assert_eq!(undo["marker"], "Claude assisted");
    let redo =
        app.call("console.redo", json!({"id":undo["id"],"base":undo["head"]}))["document"].clone();
    assert_eq!(app.read(&redo)["text"], "Assisted line");
    let h = app.call("console.history", json!({"id":first["id"]}));
    assert_eq!(h["versions"][2]["actor"], "claude");
    assert_eq!(h["versions"][3]["action"], "undo");
    assert_eq!(h["versions"][3]["restoredFrom"], hand["head"]);
}

#[test]
fn variation_points_to_exact_parent_and_keeps_inherited_provenance() {
    let app = App::new();
    let d = app.create("write", "Original");
    let d=app.call("console.tool.call",json!({"name":"console_save","args":{"id":d["id"],"base":d["head"],"text":"First thought"}}))["document"].clone();
    let fork = app.call(
        "console.variation",
        json!({"id":d["id"],"base":d["head"],"title":"Another thought"}),
    )["document"]
        .clone();
    assert_ne!(d["id"], fork["id"]);
    assert_eq!(fork["parent"]["versionId"], d["head"]);
    assert_eq!(fork["parent"]["documentId"], d["id"]);
    assert_eq!(fork["marker"], "Claude assisted");
    let edited = app.save(&fork, "New ending");
    assert_eq!(app.read(&d)["text"], "First thought");
    assert_eq!(app.read(&edited)["text"], "New ending");
}

#[test]
fn library_search_filter_and_read_only_imports_keep_original_bytes() {
    let app = App::new();
    app.create("image", "Poster");
    let bytes = b"A pocket of quiet.\n";
    let d = app.call(
        "console.import",
        json!({"name":"poem.md","base64":STANDARD.encode(bytes)}),
    )["document"]
        .clone();
    assert_eq!(d["marker"], "Origin unverified");
    let got = app.call("console.library", json!({"tool":"write","query":"quiet"}));
    assert_eq!(got["documents"].as_array().unwrap().len(), 1);
    assert_eq!(
        app.call("console.library", json!({"tool":"image","query":"quiet"}))["documents"],
        json!([])
    );
    for _ in 0..3 {
        assert_eq!(
            app.read(&d)["text"],
            String::from_utf8_lossy(bytes).as_ref()
        );
    }
    assert_eq!(app.read(&d)["document"]["versions"], 1);
    assert_eq!(app.read(&d)["readOnly"], true);
}

#[test]
fn space_package_is_local_and_contains_exact_export_lineage_and_provenance() {
    let app = App::new();
    let d = app.create("write", "Original");
    let d = app.save(&d, "Unchanged bytes\n");
    let fork = app.call(
        "console.variation",
        json!({"id":d["id"],"base":d["head"],"title":"Variation"}),
    )["document"]
        .clone();
    let result = app.call(
        "console.post.prepare",
        json!({"id":fork["id"],"base":fork["head"]}),
    );
    assert_eq!(result["published"], false);
    let path = std::path::Path::new(result["path"].as_str().unwrap());
    assert_eq!(
        fs::read(path.join(fork["asset"]["name"].as_str().unwrap())).unwrap(),
        b"Unchanged bytes\n"
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(path.join("space.json")).unwrap()).unwrap();
    assert_eq!(manifest["lineage"]["versionId"], d["head"]);
    assert_eq!(manifest["provenance"]["marker"], "Made by hand");
}

#[test]
fn traversal_external_edits_and_corrupt_bundles_are_reported_without_losing_other_files() {
    let app = App::new();
    let d = app.create("write", "Good file");
    assert!(app
        .core
        .invoke("console.read", json!({"id":"../outside"}))
        .is_err());
    let read = app.read(&d);
    fs::write(read["path"].as_str().unwrap(), "outside edit").unwrap();
    assert!(app
        .core
        .invoke("console.read", json!({"id":d["id"]}))
        .unwrap_err()
        .message
        .contains("outside Console"));
    let bad = app.create("write", "Broken");
    let path = app
        .dir
        .path()
        .join("Wi-WWAV Library/documents")
        .join(format!(
            "{}.wwwork/manifest.json",
            bad["id"].as_str().unwrap()
        ));
    fs::write(path, "not json").unwrap();
    let list = app.call("console.library", json!({}));
    assert_eq!(list["documents"].as_array().unwrap().len(), 1);
    assert_eq!(list["issues"].as_array().unwrap().len(), 1);
}

#[test]
fn simultaneous_saves_accept_one_and_refuse_the_stale_one() {
    let app = App::new();
    let d = app.create("write", "Race");
    std::thread::scope(|s| {
        let c = &app.core;
        let d = &d;
        let handles: Vec<_> = (0..8)
            .map(|n| {
                s.spawn(move || {
                    c.invoke(
                        "console.save",
                        json!({"id":d["id"],"base":d["head"],"text":format!("{n}")}),
                    )
                })
            })
            .collect();
        let answers: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(answers.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(answers
            .iter()
            .filter_map(|r| r.as_ref().err())
            .all(|e| e.code == "conflict"));
    });
    assert_eq!(app.read(&d)["document"]["versions"], 2);
}

#[test]
fn claude_proposal_does_not_edit_until_applied_then_can_be_undone() {
    let app = App::with_claude(Some(
        json!({"summary":"Tighten the line","operation":"save","title":null,"text":"A clear line."}),
    ));
    let d = app.create("write", "A line");
    let d = app.save(&d, "A somewhat unclear line.");
    let answer = app.call(
        "console.claude.ask",
        json!({"tool":"write","id":d["id"],"prompt":"Tighten this line"}),
    );
    assert_eq!(app.read(&d)["document"]["head"], d["head"]);
    let applied = app.call(
        "console.claude.apply",
        json!({"proposalId":answer["proposal"]["id"]}),
    )["document"]
        .clone();
    assert_eq!(app.read(&d)["text"], "A clear line.");
    assert_eq!(applied["marker"], "Claude assisted");
    assert!(app
        .core
        .invoke(
            "console.claude.apply",
            json!({"proposalId":answer["proposal"]["id"]})
        )
        .is_err());
    app.call(
        "console.undo",
        json!({"id":applied["id"],"base":applied["head"]}),
    );
    assert_eq!(app.read(&d)["text"], "A somewhat unclear line.");
}

#[test]
fn stale_claude_proposal_is_refused_and_context_cannot_mix_tools() {
    let app = App::with_claude(Some(
        json!({"summary":"Rename","operation":"save","title":"New name","text":null}),
    ));
    let d = app.create("write", "Old name");
    assert!(app
        .core
        .invoke(
            "console.claude.context",
            json!({"tool":"image","id":d["id"]})
        )
        .is_err());
    let answer = app.call(
        "console.claude.ask",
        json!({"tool":"write","id":d["id"],"prompt":"Rename"}),
    );
    app.save(&d, "New work");
    assert_eq!(
        app.core
            .invoke(
                "console.claude.apply",
                json!({"proposalId":answer["proposal"]["id"]})
            )
            .unwrap_err()
            .code,
        "conflict"
    );
    assert_eq!(app.read(&d)["text"], "New work");
}

#[test]
fn claude_variation_cannot_silently_discard_a_proposed_text_edit() {
    let app = App::with_claude(Some(
        json!({"summary":"New ending","operation":"variation","title":"Alternate","text":"Different words"}),
    ));
    let d = app.create("write", "Original");
    assert!(app
        .core
        .invoke(
            "console.claude.ask",
            json!({"tool":"write","id":d["id"],"prompt":"Make a variation"})
        )
        .is_err());
    assert_eq!(
        app.call("console.library", json!({}))["documents"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(app.read(&d)["document"]["head"], d["head"]);
}

#[test]
fn local_import_is_available_to_tools_and_keeps_unknown_original_authorship() {
    let app = App::new();
    let path = app.dir.path().join("PRODUCT.OBJ");
    fs::write(&path, "v 0 0 0\n").unwrap();
    let d = app.call(
        "console.tool.call",
        json!({"name":"console_import","args":{"path":path}}),
    )["document"]
        .clone();
    assert_eq!(d["tool"], "three");
    assert_eq!(d["marker"], "Origin unverified");
    assert_eq!(app.read(&d)["text"], "v 0 0 0\n");
}
