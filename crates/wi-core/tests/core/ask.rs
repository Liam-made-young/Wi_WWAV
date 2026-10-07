//! The prompt box through the core (docs/ASK.md), with a stand-in for the
//! Claude Code command line that calls the core's tools over HTTP as the
//! real one does (fake_claude.mjs). What a fail looks like:
//! - typing finds nothing, or finds it only after Claude is asked;
//! - a change is made before the person applies it, or takes more than one
//!   ⌘Z to undo;
//! - something that leaves this Mac is made without its own click;
//! - Claude can reach anything but the tools named, or anyone but the run
//!   can reach the tools;
//! - Claude missing or signed out breaks the box instead of saying so.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::{Core, Event};

fn fake_claude() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/core/fake_claude.mjs")
}

/// A core on New York's clock whose Claude is the stand-in.
fn asking_core(setup: &Setup, wiki: Option<&str>) -> Core {
    let mut config = setup.config(NO_SERVER, no_browser());
    config.now = Some(ny("2026-10-07 09:00"));
    config.claude = Some(fake_claude());
    config.wiki_url = wiki.map(String::from);
    let core = Core::open(&setup.library(), config).unwrap();
    ok(
        &core,
        "app.settings.set",
        json!({"patch": {"heat": {"timeZone": "America/New_York"}}}),
    );
    core
}

struct World {
    kanji: String,
    lab: String,
    essay: String,
    laundry: String,
}

fn world(core: &Core) -> World {
    let space = records(&snap(core, "2026-10-07"), "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let term = ok(
        core,
        "heat.put",
        json!({"kind": "term", "record": {"name": "Fall 2026"}}),
    )["record"]["id"]
        .clone();
    let course = |code: &str, name: &str| {
        ok(
            core,
            "heat.put",
            json!({"kind": "course", "record": {"termId": term, "code": code, "name": name}}),
        )["record"]["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let (jpn, phy) = (
        course("JPN 101", "Elementary Japanese"),
        course("PHY 204", "Physics II"),
    );
    let task = |title: &str, extra: Value| {
        add_task(core, &space, title, extra)["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    World {
        kanji: task(
            "Kanji quiz",
            json!({"courseId": jpn, "due": ny("2026-10-09 23:59"), "estMin": 45}),
        ),
        lab: task(
            "Lab report",
            json!({"courseId": phy, "due": ny("2026-10-08 17:00"), "estMin": 120}),
        ),
        essay: task(
            "Essay draft",
            json!({"courseId": jpn, "due": ny("2026-10-12 23:59"), "estMin": 90}),
        ),
        laundry: task("Laundry", json!({})),
    }
}

fn due(core: &Core, id: &str) -> f64 {
    ok(core, "records.get", json!({"kind": "task", "id": id}))["record"]["due"]
        .as_f64()
        .unwrap()
}

fn undo_label(core: &Core) -> Value {
    ok(core, "history.get", json!({"room": "heat"}))["undo"].clone()
}

fn ask(core: &Core, prompt: &str) -> Value {
    ok(core, "ask.send", json!({"prompt": prompt}))
}

fn doing(events: &Receiver<Event>) -> Vec<String> {
    drain(events, "ask")
        .into_iter()
        .map(|e| e.payload["doing"].as_str().unwrap_or("").to_string())
        .collect()
}

#[test]
fn typing_finds_every_kind_of_record_without_claude() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let w = world(&core);
    ok(
        &core,
        "heat.put",
        json!({"kind": "note", "record": {"title": "Kanji radicals", "markdown": "Water, fire, tree."}}),
    );
    ok(
        &core,
        "heat.put",
        json!({"kind": "habit", "record": {"title": "Kanji practice"}}),
    );
    let table = ok(&core, "db.table.create", json!({"name": "Reading list"}))["table"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    ok(
        &core,
        "db.rows.add",
        json!({"table": table, "rows": [{"Name": "Remembering the Kanji"}]}),
    );
    ok(
        &core,
        "db.view.save",
        json!({"table": "Tasks", "name": "Kanji only", "spec": {"search": "kanji"}}),
    );

    let found = ok(&core, "ask.search", json!({"q": "kanji"}));
    let seen: Vec<(String, String)> = found["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["word"].as_str().unwrap().to_string(),
                r["title"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("Task".to_string(), "Kanji quiz".to_string()),
            ("Note".into(), "Kanji radicals".into()),
            ("Habit".into(), "Kanji practice".into()),
            ("View".into(), "Kanji only".into()),
            ("Row".into(), "Remembering the Kanji".into()),
        ]
    );
    let first = &found["results"][0];
    assert_eq!(
        (
            first["id"].as_str(),
            first["table"].as_str(),
            first["hint"].as_str()
        ),
        (
            Some(w.kanji.as_str()),
            Some("task"),
            Some("JPN 101 · Due 2026-10-09")
        )
    );
    assert_eq!(found["results"][4]["hint"], "Reading list");
    assert_eq!(found["results"][4]["table"], table);
    // A prefix of each word is enough; every word must be there.
    assert_eq!(
        ok(&core, "ask.search", json!({"q": "ess dra"}))["results"][0]["title"],
        "Essay draft"
    );
    assert_eq!(
        ok(&core, "ask.search", json!({"q": "jpn"}))["results"][0]["title"],
        "JPN 101"
    );
    assert_eq!(
        ok(&core, "ask.search", json!({"q": "kanji nothing"}))["results"],
        json!([])
    );
    assert_eq!(
        ok(&core, "ask.search", json!({"q": ""}))["results"],
        json!([])
    );
    assert_eq!(
        ok(
            &core,
            "ask.search",
            json!({"q": "kanji", "kinds": ["note"]})
        )["results"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn move_everything_due_this_week_to_the_day_before_previews_applies_and_undoes_as_one() {
    let setup = Setup::new();
    let core = asking_core(&setup, None);
    let w = world(&core);
    let before = undo_label(&core);
    let events = core.events();
    let r = ask(
        &core,
        "Move everything due this week in JPN 101 to the day before #day-before",
    );

    // An answer with the tasks as links, and a preview of what would change.
    assert_eq!(
        r["answer"],
        format!(
            "I've staged 2 moves: [Kanji quiz](learn://task/{}) and [Essay draft](learn://task/{}) each go to the day before.",
            w.kanji, w.essay
        )
    );
    assert_eq!(r["change"]["summary"], "2 tasks moved");
    assert_eq!(
        r["change"]["lines"],
        json!([
            "Kanji quiz: Due 2026-10-09 23:59 → 2026-10-08 23:59",
            "Essay draft: Due 2026-10-12 23:59 → 2026-10-11 23:59",
        ])
    );
    assert_eq!(r["tools"], json!(["list_rows", "update_rows"]));
    assert_eq!(
        doing(&events),
        ["Asking Claude", "Reading a table", "Staging changes"]
    );
    // Nothing has changed yet, and nothing is in the journal.
    assert_eq!(due(&core, &w.kanji), ny("2026-10-09 23:59"));
    assert_eq!(due(&core, &w.essay), ny("2026-10-12 23:59"));
    assert_eq!(undo_label(&core), before);

    // One click.
    let applied = ok(&core, "ask.apply", json!({"id": r["id"]}));
    assert_eq!(
        applied,
        json!({"applied": 2, "failed": [], "summary": "2 tasks moved", "undo": "Undo Claude: 2 tasks moved", "steps": 1})
    );
    assert_eq!(due(&core, &w.kanji), ny("2026-10-08 23:59"));
    assert_eq!(due(&core, &w.essay), ny("2026-10-11 23:59"));
    assert_eq!(
        due(&core, &w.lab),
        ny("2026-10-08 17:00"),
        "another course's task stays"
    );
    assert_eq!(undo_label(&core), "Undo Claude: 2 tasks moved");
    // The same click again does nothing.
    let (_, said) = refused(&core, "ask.apply", json!({"id": r["id"]}));
    assert_eq!(said, "Those changes aren't waiting any more. Ask again.");

    // One ⌘Z undoes it all.
    assert_eq!(
        ok(&core, "history.undo", json!({"room": "heat"}))["label"],
        "Claude: 2 tasks moved"
    );
    assert_eq!(due(&core, &w.kanji), ny("2026-10-09 23:59"));
    assert_eq!(due(&core, &w.essay), ny("2026-10-12 23:59"));
    assert_eq!(undo_label(&core), before);
}

#[test]
fn a_change_let_go_is_never_made() {
    let setup = Setup::new();
    let core = asking_core(&setup, None);
    let w = world(&core);
    let r = ask(&core, "move them #day-before");
    ok(&core, "ask.discard", json!({"id": r["id"]}));
    assert!(core.invoke("ask.apply", json!({"id": r["id"]})).is_err());
    assert_eq!(due(&core, &w.kanji), ny("2026-10-09 23:59"));
}

#[test]
fn a_general_knowledge_answer_carries_its_wikipedia_article() {
    let wikipedia = crate::wiki::Wikipedia::start();
    let setup = Setup::new();
    let core = asking_core(&setup, Some(&wikipedia.url));
    let r = ask(&core, "what is a Fourier transform? #fourier");
    assert!(r["answer"]
        .as_str()
        .unwrap()
        .starts_with("A Fourier transform breaks a signal"));
    assert!(r["answer"]
        .as_str()
        .unwrap()
        .ends_with("Wikipedia: [Fourier transform](wiki://Fourier_transform)"));
    assert_eq!(r["wiki"], json!(["Fourier transform"]));
    assert_eq!(r["change"], Value::Null);
    // The link opens the article in the Wiki tab: it reads.
    let article = ok(&core, "wiki.article", json!({"title": r["wiki"][0]}));
    assert_eq!(article["article"]["title"], "Fourier transform");
}

#[test]
fn in_the_wiki_tab_claude_reads_the_section_on_screen_and_cards_become_a_note() {
    let wikipedia = crate::wiki::Wikipedia::start();
    let setup = Setup::new();
    let core = asking_core(&setup, Some(&wikipedia.url));
    let r = ok(
        &core,
        "ask.send",
        json!({"prompt": "make flashcards from this #flashcards", "context": {"tab": "wiki", "wiki": {"title": "Fourier transform", "section": "History"}}}),
    );
    assert_eq!(r["change"]["summary"], "1 note created");
    assert_eq!(
        r["change"]["lines"][0],
        "New note: Title Flashcards: Fourier transform, Markdown Q: Who?"
    );
    assert_eq!(r["wiki"], json!(["Fourier transform"]));
    assert_eq!(
        ok(&core, "ask.apply", json!({"id": r["id"]}))["undo"],
        "Undo Claude: 1 note created"
    );
    // A normal Learn note: Notes lists it and search finds it.
    let notes = ok(&core, "records.list", json!({"kind": "note"}));
    assert_eq!(
        notes["records"][0]["title"],
        "Flashcards: Fourier transform"
    );
    assert_eq!(
        notes["records"][0]["markdown"],
        "Q: Who?\nA: Joseph Fourier, 1822."
    );
    assert_eq!(
        ok(&core, "ask.search", json!({"q": "flashcards"}))["results"][0]["word"],
        "Note"
    );
}

#[test]
fn a_row_made_and_used_in_the_same_answer() {
    let setup = Setup::new();
    let core = asking_core(&setup, None);
    world(&core);
    let r = ask(
        &core,
        "add vocabulary cards and put it at 2 #create-and-schedule",
    );
    assert_eq!(r["change"]["summary"], "1 task created, 1 block scheduled");
    assert_eq!(
        r["change"]["lines"],
        json!([
            "New task: Title Vocabulary cards, Course JPN 101, Estimate (min) 30",
            "Block for new:1: 2026-10-07 at 14:00, 30 min",
        ])
    );
    let applied = ok(&core, "ask.apply", json!({"id": r["id"]}));
    assert_eq!(
        (
            applied["applied"].clone(),
            applied["failed"].clone(),
            applied["steps"].clone()
        ),
        (json!(2), json!([]), json!(1))
    );
    // The block is on the task that was just made.
    let s = snap(&core, "2026-10-07");
    let task = records(&s, "task")
        .into_iter()
        .find(|t| t["title"] == "Vocabulary cards")
        .unwrap();
    let block = &records(&s, "timeBlock")[0];
    assert_eq!(
        (
            block["taskId"].clone(),
            block["start"].clone(),
            block["minutes"].clone()
        ),
        (task["id"].clone(), json!(840), json!(30))
    );
    ok(&core, "history.undo", json!({"room": "heat"}));
    let s = snap(&core, "2026-10-07");
    assert!(records(&s, "timeBlock").is_empty());
    assert!(records(&s, "task")
        .iter()
        .all(|t| t["title"] != "Vocabulary cards"));
}

#[test]
fn a_tool_says_what_is_wrong_so_claude_can_put_it_right() {
    let setup = Setup::new();
    let core = asking_core(&setup, None);
    world(&core);
    let r = ask(&core, "move the kanji quiz #wrong-then-right");
    let errors: Vec<String> = serde_json::from_str(r["answer"].as_str().unwrap()).unwrap();
    assert!(errors[0].starts_with("Tasks has no column called 'Deadline'. The ones that can be changed are: Space, Title, Type, Course,"), "{}", errors[0]);
    assert_eq!(
        errors[1],
        "'Heat' can't be changed. Learn works this out, so it can't be typed over."
    );
    assert!(
        errors[2].starts_with("Kanji quiz, Due: 'someday' isn't a date."),
        "{}",
        errors[2]
    );
    // Only the call that could be made was staged.
    assert_eq!(
        r["change"]["lines"],
        json!(["Kanji quiz: Due 2026-10-09 23:59 → 2026-10-08 23:59"])
    );
}

#[test]
fn what_leaves_this_mac_waits_for_its_own_click_every_time() {
    let setup = Setup::new();
    let core = asking_core(&setup, None);
    let w = world(&core);
    let public = |core: &Core| {
        ok(core, "records.get", json!({"kind": "task", "id": w.essay}))["record"]["public"].clone()
    };
    let r = ask(&core, "make my essay public #public");
    assert_eq!(
        r["outward"],
        json!([{"index": 0, "line": "Make the task 'Essay draft' public: anyone who opens your sun can see it."}])
    );
    assert_eq!(r["change"]["summary"], "1 task updated");
    // Applying the changes doesn't make it public.
    ok(&core, "ask.apply", json!({"id": r["id"]}));
    assert_ne!(public(&core), json!(true));
    // Its own click does, once.
    let done = ok(
        &core,
        "ask.applyOutward",
        json!({"id": r["id"], "index": 0}),
    );
    assert_eq!(
        (done["done"].clone(), done["undo"].clone()),
        (json!(true), json!("Undo make public"))
    );
    assert_eq!(public(&core), json!(true));
    assert!(core
        .invoke("ask.applyOutward", json!({"id": r["id"], "index": 0}))
        .is_err());
}

#[test]
fn every_kind_of_change_stages_and_applies_as_one_step() {
    let setup = Setup::new();
    let core = asking_core(&setup, None);
    let w = world(&core);
    let before = undo_label(&core);
    let r = ask(&core, "do a bit of everything #everything");
    let out: Value = serde_json::from_str(r["answer"].as_str().unwrap()).unwrap();
    assert_eq!(out["badFormula"]["error"], "That formula can't be used: Tasks has no column called 'Nope'. Fix it and call this again.");
    assert_eq!(out["formula"]["first_values"][0], 0.75);
    assert_eq!(
        out["total"]["groups"][0],
        json!({"keys": ["JPN 101"], "count": 2, "values": [135]})
    );
    assert_eq!(out["row"]["Title"], "Kanji quiz");
    assert_eq!(out["row"]["Course"], "JPN 101");
    assert_eq!(out["today"]["date"], "2026-10-07");
    assert!(out["plan"]["drafts"].as_array().unwrap().len() >= 2);
    assert_eq!(r["opens"], json!([{"what": "tab", "tab": "database"}]));
    assert_eq!(
        r["change"]["summary"],
        "1 task completed, 1 task deleted, 1 capture added, 1 column added, 1 view saved, 1 table created, 1 day planned"
    );
    assert_eq!(undo_label(&core), before);

    let applied = ok(&core, "ask.apply", json!({"id": r["id"]}));
    assert_eq!(applied["failed"], json!([]));
    assert_eq!(applied["steps"], 1);
    let tasks = ok(&core, "db.query", json!({"table": "Tasks"}));
    assert_eq!(tasks["total"], 3);
    assert!(tasks["columns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "Hours"));
    assert_eq!(
        ok(&core, "records.get", json!({"kind": "task", "id": w.lab}))["record"]["done"],
        true
    );
    let t = ok(&core, "db.tables", json!({}));
    assert_eq!(t["views"][0]["name"], "Open by course");
    assert_eq!(t["views"][0]["spec"]["charts"][0]["type"], "bar");
    assert_eq!(
        ok(&core, "db.query", json!({"table": "Reading list"}))["total"],
        1
    );
    let s = snap(&core, "2026-10-07");
    assert!(
        !records(&s, "timeBlock").is_empty(),
        "the plan's drafts became blocks"
    );
    assert_eq!(records(&s, "capture")[0]["text"], "Ask about office hours");

    // All of it is one ⌘Z.
    ok(&core, "history.undo", json!({"room": "heat"}));
    assert_eq!(undo_label(&core), before);
    let s = snap(&core, "2026-10-07");
    assert_eq!(records(&s, "task").len(), 4);
    assert!(records(&s, "timeBlock").is_empty() && records(&s, "capture").is_empty());
    assert_eq!(ok(&core, "db.tables", json!({}))["views"], json!([]));
    assert!(core
        .invoke("db.query", json!({"table": "Reading list"}))
        .is_err());
    let _ = w.laundry;
}

#[test]
fn claude_is_given_the_screen_the_tables_and_only_these_tools() {
    let setup = Setup::new();
    let core = asking_core(&setup, None);
    world(&core);
    let r = ok(
        &core,
        "ask.send",
        json!({
            "prompt": "what's due in this class #echo",
            "context": {"tab": "grades", "selection": [{"table": "course", "id": "c1", "title": "JPN 101"}]},
            "history": [{"prompt": "hello", "answer": "Hi.", "applied": true}],
        }),
    );
    let got: Value = serde_json::from_str(r["answer"].as_str().unwrap()).unwrap();
    let request = got["request"].as_str().unwrap();
    assert!(
        request.contains("\"title\": \"JPN 101\"") && request.contains("They asked: hello"),
        "{request}"
    );
    let system = got["system"].as_str().unwrap();
    assert!(
        system.contains("Today is 2026-10-07") && system.contains("(Wednesday)"),
        "{system}"
    );
    assert!(
        system.contains(
            "- Tasks [4 rows]: ID (text, read-only); Space (relation to Spaces); Title (text);"
        ),
        "{system}"
    );
    assert!(system.contains("- Mail [0 rows, read-only]"), "{system}");
    // No built-in tool, no other server, and exactly the tools the core offers.
    assert_eq!(got["builtIn"], "");
    let flags: Vec<&str> = got["flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    for needed in [
        "--strict-mcp-config",
        "--no-session-persistence",
        "--disable-slash-commands",
        "--system-prompt",
    ] {
        assert!(flags.contains(&needed), "{needed}");
    }
    let offered = ok(&core, "ask.tools", json!({}));
    let names: Vec<Value> = offered["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].clone())
        .collect();
    assert_eq!(got["tools"], json!(names));
    let allowed: Vec<String> = names
        .iter()
        .map(|n| format!("mcp__learn__{}", n.as_str().unwrap()))
        .collect();
    assert_eq!(got["allowed"], json!(allowed));
    assert_eq!(got["model"], "sonnet");
    assert_eq!(
        offered["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| t["effect"] == "leaves this Mac")
            .count(),
        1
    );
}

#[test]
fn nobody_but_the_run_reaches_the_tools() {
    let setup = Setup::new();
    let core = asking_core(&setup, None);
    let r = ask(&core, "try the doors #trespass");
    let got: Value = serde_json::from_str(r["answer"].as_str().unwrap()).unwrap();
    assert_eq!(
        got,
        json!({"none": 401, "wrong": 401, "page": 403, "right": 200, "get": 405, "unknown": -32601, "elsewhere": 404})
    );
}

#[test]
fn without_claude_the_box_says_so_and_still_searches() {
    let setup = Setup::new();
    let mut config = setup.config(NO_SERVER, no_browser());
    config.claude = Some(setup.dir.path().join("no-claude-here"));
    let core = Core::open(&setup.library(), config).unwrap();
    pin(&core, "2026-10-07 09:00");
    world(&core);
    let status = ok(&core, "ask.status", json!({}));
    assert_eq!(status["available"], false);
    let (code, said) = refused(&core, "ask.send", json!({"prompt": "hello"}));
    assert_eq!(code, "claude");
    assert!(said.contains("isn't there"), "{said}");
    assert_eq!(
        ok(&core, "ask.search", json!({"q": "kanji"}))["results"][0]["title"],
        "Kanji quiz"
    );

    // Signed out is a sentence too, and nothing is left waiting.
    let core = asking_core(&Setup::new(), None);
    assert_eq!(ok(&core, "ask.status", json!({}))["available"], true);
    let (code, said) = refused(&core, "ask.send", json!({"prompt": "hello #signed-out"}));
    assert_eq!(
        (code.as_str(), said.as_str()),
        (
            "claude",
            "Claude Code is signed out. Open a terminal, run claude, sign in, then try again."
        )
    );
    let (code, _) = refused(&core, "ask.send", json!({"prompt": "  "}));
    assert_eq!(code, "bad_args");
}

#[test]
fn a_run_can_be_stopped() {
    let setup = Setup::new();
    let core = std::sync::Arc::new(asking_core(&setup, None));
    let events = core.events();
    let asking = {
        let core = core.clone();
        std::thread::spawn(move || {
            core.invoke("ask.send", json!({"prompt": "take your time #slow"}))
        })
    };
    let started = wait_event(&events, "ask", Duration::from_secs(10));
    ok(&core, "ask.cancel", json!({"id": started.payload["id"]}));
    let e = asking.join().unwrap().expect_err("stopped");
    assert_eq!(
        (e.code.as_str(), e.message.as_str()),
        ("claude", "Stopped.")
    );
}
