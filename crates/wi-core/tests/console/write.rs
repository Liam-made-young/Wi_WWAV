use super::*;

fn read(app: &App, d: &Value) -> Value {
    app.call("console.write.read", json!({"id":d["id"]}))
}
fn edit(app: &App, d: &Value, action: Value) -> Value {
    app.call(
        "console.write.edit",
        json!({"id":d["id"],"base":d["head"],"action":action}),
    )["document"]
        .clone()
}
fn section(app: &App, d: &Value) -> Value {
    read(app, d)["manuscript"]["sections"][0]["id"].clone()
}
fn exported(app: &App, d: &Value, format: &str) -> Vec<u8> {
    let result = app.call(
        "console.write.export",
        json!({"id":d["id"],"version":d["head"],"format":format}),
    );
    let bytes = STANDARD.decode(result["base64"].as_str().unwrap()).unwrap();
    assert_eq!(fs::read(result["path"].as_str().unwrap()).unwrap(), bytes);
    assert_eq!(result["version"], d["head"]);
    bytes
}

#[test]
fn multi_section_bundle_is_readable_versioned_and_variations_keep_everything() {
    let app = App::new();
    let first = app.create("write", "Essay");
    let first_id = section(&app, &first);
    let d = edit(
        &app,
        &first,
        json!({"type":"update","section":first_id,"title":"Beginning","text":"# Light\n\nThree small words."}),
    );
    let d = edit(
        &app,
        &d,
        json!({"type":"add","title":"Ending","parent":null}),
    );
    let mut m = read(&app, &d)["manuscript"].clone();
    m["sections"][1]["text"] = json!("Another two.");
    m["notes"] = json!("Private reminder");
    let d = app.call(
        "console.write.save",
        json!({"id":d["id"],"base":d["head"],"manuscript":m}),
    )["document"]
        .clone();
    let data = read(&app, &d);
    assert_eq!(data["stats"]["words"], 6);
    let old = app.call(
        "console.write.read",
        json!({"id":d["id"],"version":first["head"]}),
    );
    assert_eq!(old["manuscript"]["sections"].as_array().unwrap().len(), 1);
    assert_eq!(old["manuscript"]["sections"][0]["text"], "");
    let raw = app.read(&d);
    let manifest: Value = serde_json::from_slice(
        &fs::read(
            std::path::Path::new(raw["path"].as_str().unwrap())
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let record = &manifest["versions"].as_array().unwrap().last().unwrap()["write"];
    assert_eq!(record["sections"].as_array().unwrap().len(), 2);
    let folder = std::path::Path::new(raw["path"].as_str().unwrap())
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for s in record["sections"].as_array().unwrap() {
        assert!(folder.join(s["asset"]["file"].as_str().unwrap()).is_file());
    }
    let fork = app.call(
        "console.variation",
        json!({"id":d["id"],"base":d["head"],"title":"Essay variation"}),
    )["document"]
        .clone();
    assert_eq!(read(&app, &fork)["manuscript"], data["manuscript"]);
    assert_eq!(fork["parent"]["versionId"], d["head"]);
    let md = String::from_utf8(exported(&app, &d, "markdown")).unwrap();
    assert!(md.contains("# Beginning"));
    assert!(md.contains("# Ending"));
    assert!(!md.contains("Private reminder"));
    let txt = String::from_utf8(exported(&app, &d, "text")).unwrap();
    assert!(txt.contains("Three small words."));
    assert!(!txt.contains("# Light"));
}

#[test]
fn binder_moves_whole_subtrees_and_rejects_cycles_invalid_orders_and_last_deletion() {
    let app = App::new();
    let d = app.create("write", "Binder");
    let a = section(&app, &d);
    let d = edit(&app, &d, json!({"type":"add","title":"Child","parent":a}));
    let child = read(&app, &d)["manuscript"]["sections"][1]["id"].clone();
    let d = edit(
        &app,
        &d,
        json!({"type":"add","title":"Other","parent":null}),
    );
    let other = read(&app, &d)["manuscript"]["sections"][2]["id"].clone();
    let moved = edit(
        &app,
        &d,
        json!({"type":"move","section":a,"parent":null,"before":null}),
    );
    let rows = read(&app, &moved)["manuscript"]["sections"].clone();
    assert_eq!(rows[0]["id"], other);
    assert_eq!(rows[1]["id"], a);
    assert_eq!(rows[2]["id"], child);
    for action in [
        json!({"type":"move","section":a,"parent":child,"before":null}),
        json!({"type":"reorder","order":[a,other,child]}),
        json!({"type":"reorder","order":[a,a,child]}),
    ] {
        assert!(app
            .core
            .invoke(
                "console.write.edit",
                json!({"id":moved["id"],"base":moved["head"],"action":action})
            )
            .is_err());
        assert_eq!(read(&app, &moved)["base"], moved["head"]);
    }
    let nested = edit(
        &app,
        &moved,
        json!({"type":"move","section":a,"parent":other,"before":null}),
    );
    assert!(app.core.invoke("console.write.edit",json!({"id":nested["id"],"base":nested["head"],"action":{"type":"delete","section":other}})).is_err());
    let stale = app
        .core
        .invoke(
            "console.write.edit",
            json!({"id":d["id"],"base":d["head"],"action":{"type":"notes","text":"Stale"}}),
        )
        .unwrap_err();
    assert_eq!(stale.code, "conflict");
    let restored = app.call(
        "console.undo",
        json!({"id":nested["id"],"base":nested["head"]}),
    )["document"]
        .clone();
    assert_eq!(read(&app, &restored)["manuscript"]["sections"], rows);
}

#[test]
fn selections_use_utf16_and_claude_edits_are_scoped_undoable_and_honestly_marked() {
    let app = App::new();
    let d = app.create("write", "Unicode");
    let id = section(&app, &d);
    let d = edit(
        &app,
        &d,
        json!({"type":"update","section":id,"text":"A 😀 café."}),
    );
    let bad=app.core.invoke("console.write.edit",json!({"id":d["id"],"base":d["head"],"action":{"type":"replace","section":id,"from":3,"to":4,"text":"x"}}));
    assert!(bad.is_err());
    let next=app.call("console.tool.call",json!({"name":"console_write_edit","args":{"id":d["id"],"base":d["head"],"action":{"type":"replace","section":id,"from":5,"to":9,"text":"story"},"actor":"hand"}}))["document"].clone();
    assert_eq!(
        read(&app, &next)["manuscript"]["sections"][0]["text"],
        "A 😀 story."
    );
    assert_eq!(next["marker"], "Claude assisted");
    let undo =
        app.call("console.undo", json!({"id":next["id"],"base":next["head"]}))["document"].clone();
    assert_eq!(
        read(&app, &undo)["manuscript"]["sections"][0]["text"],
        "A 😀 café."
    );
    let h = app.call("console.history", json!({"id":d["id"]}));
    assert_eq!(h["versions"][2]["actor"], "claude");
    assert!(h["versions"][2]["changes"]
        .to_string()
        .contains("text edited"));
}

#[test]
fn markdown_preview_is_safe_and_never_embeds_remote_images() {
    let app = App::new();
    let r=app.call("console.write.render",json!({"mode":"prose","text":"# Heading\n\n**Strong** [bad](javascript:alert%281%29)\n\n<script>alert(1)</script>\n\n![alt](https://example.com/private.png)"}));
    let html = r["html"].as_str().unwrap();
    assert!(html.contains("<h1>Heading</h1>"));
    assert!(html.contains("<strong>Strong</strong>"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("href=\"javascript:"));
    assert!(!html.contains("<img"));
    assert!(!html.contains("src="));
    assert_eq!(r["stats"]["outline"][0]["title"], "Heading");
}

#[test]
fn lyrics_counts_lines_and_estimated_syllables_without_counting_blank_lines() {
    let app = App::new();
    let r = app.call(
        "console.write.render",
        json!({"mode":"lyrics","text":"We shine\n\nMake a little light\n"}),
    );
    assert_eq!(r["stats"]["lineCount"], 2);
    assert_eq!(r["stats"]["lines"][0]["syllables"], 2);
    assert_eq!(r["stats"]["lines"][1]["syllables"], 5);
    assert_eq!(r["stats"]["words"], 6);
    assert!(
        r["html"].as_str().unwrap().contains("<br />")
            || r["html"].as_str().unwrap().contains("</p>")
    );
}

const SCRIPT:&str="Title: Small Hours\nAuthor: Liam\n\nINT. STUDIO - NIGHT\n\nA lamp clicks on.\n\nLIAM\n(quietly)\nOne line.\nA second line.\n\nEXT. STREET - DAWN\n\nThe city wakes.\n\nCUT TO:\n";

#[test]
fn fountain_import_preview_export_preserves_source_and_six_standard_elements() {
    let app = App::new();
    let d = app.call(
        "console.import",
        json!({"tool":"write","name":"script.fountain","base64":STANDARD.encode(SCRIPT)}),
    )["document"]
        .clone();
    let m = read(&app, &d);
    assert_eq!(m["manuscript"]["mode"], "screenplay");
    assert_eq!(m["manuscript"]["sections"][0]["text"], SCRIPT);
    let r = app.call(
        "console.write.render",
        json!({"mode":"screenplay","text":SCRIPT}),
    );
    let html = r["html"].as_str().unwrap();
    for kind in [
        "scene",
        "action",
        "character",
        "dialogue",
        "parenthetical",
        "transition",
    ] {
        assert!(
            html.contains(&format!("write-script-{kind}")),
            "missing {kind}: {html}"
        );
    }
    assert!(html.contains("write-script-dialogue\"><p>A second line."));
    assert_eq!(exported(&app, &d, "fountain"), SCRIPT.as_bytes());
    let forced=app.call("console.write.render",json!({"mode":"screenplay","text":".A CUSTOM SCENE\n\n!LOUD ACTION\n\n@Someone\n(softly)\nHello there.\n"}));
    assert!(forced["html"]
        .as_str()
        .unwrap()
        .contains("write-script-character\"><p>Someone"));
}

#[test]
fn linked_learn_research_is_readable_and_private_sources_never_leave_space_package() {
    let app = App::new();
    let n = app.call(
        "heat.note.create",
        json!({"title":"Light research","markdown":"A secret research paragraph."}),
    );
    let d = app.create("write", "Public poem");
    let d = app.save(&d, "Public words.");
    let d = edit(&app, &d, json!({"type":"linkNote","note":n["note"]["id"]}));
    let d = edit(&app, &d, json!({"type":"notes","text":"A private plan."}));
    let m = read(&app, &d);
    assert_eq!(m["manuscript"]["research"][0]["noteId"], n["note"]["id"]);
    let hits = app.call(
        "console.write.research.search",
        json!({"query":"Light research"}),
    );
    assert!(!hits["hits"].as_array().unwrap().is_empty());
    assert_eq!(
        app.call(
            "console.write.research.read",
            json!({"note":n["note"]["id"]})
        )["markdown"],
        "A secret research paragraph."
    );
    let package = app.call(
        "console.post.prepare",
        json!({"id":d["id"],"base":d["head"]}),
    );
    let record = fs::read_to_string(
        std::path::Path::new(package["path"].as_str().unwrap()).join("space.json"),
    )
    .unwrap();
    assert!(!record.contains("A secret research paragraph"));
    assert!(!record.contains("A private plan"));
    assert!(!record.contains("research\":"));
    assert!(!record.contains("\"write\"") || record.contains("\"tool\": \"write\""));
    assert!(record.contains("Document notes edited"));
    assert!(app.core.invoke("console.write.edit",json!({"id":d["id"],"base":d["head"],"action":{"type":"researchAdd","title":"Bad","url":"file:///etc/passwd"}})).is_err());
}

#[test]
fn pdf_embeds_fonts_paginates_and_retains_unicode_and_screenplay_indents() {
    let app = App::new();
    let d = app.create("write", "Café essay");
    let d = app.save(
        &d,
        "# A heading\n\nCafé, naïve, office.\n\nA paragraph of **bold words**.",
    );
    let bytes = exported(&app, &d, "pdf");
    assert!(bytes.starts_with(b"%PDF-1.7"));
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 1);
    let text = pdf.extract_text(&[1]).unwrap();
    assert!(text.contains("Café"), "{text}");
    assert!(text.contains("naïve"), "{text}");
    assert!(pdf
        .objects
        .values()
        .any(|o| o.as_dict().is_ok_and(|d| d.has(b"FontFile2"))));
    let d = app.save(&d, &"A paragraph with room to grow.\n\n".repeat(160));
    let pdf = lopdf::Document::load_mem(&exported(&app, &d, "pdf")).unwrap();
    assert!(pdf.get_pages().len() > 2);
    let script = app.call(
        "console.import",
        json!({"tool":"write","name":"small.fountain","base64":STANDARD.encode(SCRIPT)}),
    )["document"]
        .clone();
    let pdf = lopdf::Document::load_mem(&exported(&app, &script, "pdf")).unwrap();
    assert_eq!(pdf.get_pages().len(), 2);
    let content =
        lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&2]).unwrap())
            .unwrap();
    for x in [108.0, 180.0, 216.0, 266.0] {
        assert!(
            content.operations.iter().any(|o| o.operator == "Tm"
                && o.operands[4].as_float().is_ok_and(|n| (n - x).abs() < 0.01)),
            "missing indent {x}"
        );
    }
    let d = app.save(&d, "An emoji 😀.");
    let error = app
        .core
        .invoke("console.write.export", json!({"id":d["id"],"format":"pdf"}))
        .unwrap_err();
    assert!(error.message.contains("cannot display"));
}

#[test]
fn write_claude_proposes_only_selected_edits_and_read_only_intents_never_mutate() {
    let app = App::with_claude(Some(
        json!({"summary":"Tighter.","text":"clear","order":null}),
    ));
    let d = app.create("write", "Selection");
    let id = section(&app, &d);
    let d = app.save(&d, "A muddled line.");
    let ask=app.call("console.write.assist",json!({"id":d["id"],"base":d["head"],"section":id,"selection":{"from":2,"to":9},"intent":"tighten","prompt":"Keep my meaning."}));
    assert_eq!(read(&app, &d)["base"], d["head"]);
    assert_eq!(ask["proposal"]["before"], "muddled");
    let next = app.call(
        "console.claude.apply",
        json!({"proposalId":ask["proposal"]["id"]}),
    )["document"]
        .clone();
    assert_eq!(
        read(&app, &next)["manuscript"]["sections"][0]["text"],
        "A clear line."
    );
    assert_eq!(next["marker"], "Claude assisted");
    for intent in ["rhymes", "alternatives", "summarize"] {
        let answer = app.call(
            "console.write.assist",
            json!({"id":next["id"],"intent":intent}),
        );
        assert!(answer["proposal"].is_null());
        assert_eq!(read(&app, &next)["base"], next["head"]);
    }
    let undo =
        app.call("console.undo", json!({"id":next["id"],"base":next["head"]}))["document"].clone();
    assert_eq!(
        read(&app, &undo)["manuscript"]["sections"][0]["text"],
        "A muddled line."
    );
    assert!(app
        .core
        .invoke(
            "console.claude.apply",
            json!({"proposalId":ask["proposal"]["id"]})
        )
        .is_err());
}

#[test]
fn claude_continuation_is_explicit_and_stale_proposals_cannot_overwrite_new_work() {
    let app = App::with_claude(Some(
        json!({"summary":"Continue at cursor.","text":" then","order":null}),
    ));
    let d = app.create("write", "Continuation");
    let d = app.save(&d, "Now.");
    let id = section(&app, &d);
    let ask = app.call(
        "console.write.assist",
        json!({"id":d["id"],"intent":"continue","selection":{"section":id,"from":3,"to":3}}),
    );
    let next = app.call(
        "console.claude.apply",
        json!({"proposalId":ask["proposal"]["id"]}),
    )["document"]
        .clone();
    assert_eq!(
        read(&app, &next)["manuscript"]["sections"][0]["text"],
        "Now then."
    );
    let ask = app.call(
        "console.write.assist",
        json!({"id":next["id"],"intent":"rewrite"}),
    );
    let latest = app.save(&next, "I changed my mind.");
    assert_eq!(
        app.core
            .invoke(
                "console.claude.apply",
                json!({"proposalId":ask["proposal"]["id"]})
            )
            .unwrap_err()
            .code,
        "conflict"
    );
    assert_eq!(
        read(&app, &latest)["manuscript"]["sections"][0]["text"],
        "I changed my mind."
    );
}

#[test]
fn invalid_claude_reorders_are_refused_before_proposals_are_written() {
    let app = App::with_claude(Some(
        json!({"summary":"Bad order","text":null,"order":["invented"]}),
    ));
    let d = app.create("write", "Order");
    assert!(app
        .core
        .invoke(
            "console.write.assist",
            json!({"id":d["id"],"intent":"reorder"})
        )
        .is_err());
    assert_eq!(read(&app, &d)["base"], d["head"]);
}

#[test]
fn view_preferences_persist_and_legacy_versions_upgrade_only_when_saved() {
    let app = App::new();
    let d = app.create("write", "Legacy");
    let d = app.save(&d, "Old source.");
    let id = section(&app, &d);
    app.call(
        "console.write.view",
        json!({"id":d["id"],"view":{"section":id,"focus":true,"typewriter":true,"outline":true}}),
    );
    assert_eq!(read(&app, &d)["view"]["focus"], true);
    let raw = app.read(&d);
    let path = std::path::Path::new(raw["path"].as_str().unwrap())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for v in manifest["versions"].as_array_mut().unwrap() {
        v.as_object_mut().unwrap().remove("write");
    }
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let before = fs::read(&path).unwrap();
    let virtual_m = read(&app, &d);
    assert_eq!(
        virtual_m["manuscript"]["sections"][0]["text"],
        "Old source."
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    let next = edit(&app, &d, json!({"type":"notes","text":"A new note"}));
    assert_eq!(read(&app, &next)["manuscript"]["notes"], "A new note");
    let manifest: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert!(manifest["versions"][0]["write"].is_null());
    assert_eq!(
        manifest["versions"].as_array().unwrap().last().unwrap()["write"]["format"],
        "wi-write/1"
    );
}

#[test]
fn write_limits_refuse_invalid_manuscripts_and_keep_large_imports_readable() {
    let app = App::new();
    let d = app.create("write", "Boundaries");
    let m = read(&app, &d)["manuscript"].clone();
    for invalid in [
        {
            let mut x = m.clone();
            x["sections"] = json!([]);
            x
        },
        {
            let mut x = m.clone();
            x["notes"] = json!("n".repeat(1024 * 1024));
            x
        },
        {
            let mut x = m.clone();
            x["sections"][0]["parent"] = x["sections"][0]["id"].clone();
            x
        },
    ] {
        assert!(app
            .core
            .invoke(
                "console.write.save",
                json!({"id":d["id"],"base":d["head"],"manuscript":invalid})
            )
            .is_err());
        assert_eq!(read(&app, &d)["base"], d["head"]);
    }
    let context = app.call(
        "console.claude.context",
        json!({"tool":"write","id":d["id"]}),
    );
    assert_eq!(
        context["manuscript"]["sections"][0]["id"],
        m["sections"][0]["id"]
    );
    let body = "x".repeat(1024 * 1024 + 1);
    let large = app.call(
        "console.import",
        json!({"name":"large.md","base64":STANDARD.encode(&body)}),
    )["document"]
        .clone();
    assert_eq!(app.read(&large)["text"], body);
    assert!(app
        .core
        .invoke("console.write.read", json!({"id":large["id"]}))
        .is_err());
    assert!(app.call(
        "console.claude.context",
        json!({"tool":"write","id":large["id"]})
    )["manuscript"]
        .is_null());
}

#[test]
fn consecutive_fountain_lines_render_and_corrupt_sections_cannot_create_partial_forks() {
    let app = App::new();
    let script = format!(
        "INT. ROOM - NIGHT\n\nLIAM\n{}",
        "The thought continues.\n".repeat(2000)
    );
    let rendered = app.call(
        "console.write.render",
        json!({"mode":"screenplay","text":script}),
    );
    assert_eq!(
        rendered["html"]
            .as_str()
            .unwrap()
            .matches("write-script-dialogue")
            .count(),
        2000
    );
    let d = app.create("write", "Corrupt source");
    let d = app.save(&d, "A public body.");
    let raw = app.read(&d);
    let folder = std::path::Path::new(raw["path"].as_str().unwrap())
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    let source =
        &manifest["versions"].as_array().unwrap().last().unwrap()["write"]["notes"]["file"];
    fs::write(folder.join(source.as_str().unwrap()), "An external change").unwrap();
    let count = app.call("console.library", json!({}))["documents"]
        .as_array()
        .unwrap()
        .len();
    assert!(app
        .core
        .invoke(
            "console.variation",
            json!({"id":d["id"],"base":d["head"],"title":"Must not exist"})
        )
        .is_err());
    assert_eq!(
        app.call("console.library", json!({}))["documents"]
            .as_array()
            .unwrap()
            .len(),
        count
    );
}
