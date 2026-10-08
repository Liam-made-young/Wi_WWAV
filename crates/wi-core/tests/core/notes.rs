//! Notes through the commands and the folder (docs/NOTES.md). What a fail
//! looks like:
//! - a note isn't a file the moment it is made, or its file and the library
//!   say different things after an edit, a rename, a delete or an undo;
//! - an edit made outside the app is lost, or can't be undone;
//! - a rename leaves a link to the old name;
//! - a checkbox made a task isn't linked to it, or ticking one doesn't mark
//!   the other;
//! - a daily note from before Notes is lost when it is moved over.

use std::path::PathBuf;
use std::time::Duration;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

const DAY: &str = "2026-10-07";
const NOW: &str = "2026-10-07 09:00";
const T: Duration = Duration::from_secs(20);

fn notes_core(setup: &Setup) -> (Core, PathBuf) {
    let dir = setup.dir.path().join("Notes");
    let mut config = setup.config(NO_SERVER, no_browser());
    config.now = Some(ny(NOW));
    config.notes_dir = Some(dir.clone());
    let core = Core::open(&setup.library(), config).unwrap();
    ok(
        &core,
        "app.settings.set",
        json!({"patch": {"heat": {"timeZone": "America/New_York"}}}),
    );
    (core, dir)
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("no file at {}", path.display()))
}

fn note(core: &Core, id: &Value) -> Value {
    records(&snap(core, DAY), "note")
        .into_iter()
        .find(|n| &n["id"] == id)
        .unwrap_or(Value::Null)
}

fn index(core: &Core, id: &Value) -> Value {
    snap(core, DAY)["notes"]["index"][id.as_str().unwrap()].clone()
}

#[test]
fn a_note_is_a_file_the_moment_it_is_made_and_stays_the_same_as_the_library() {
    let setup = Setup::new();
    let (core, dir) = notes_core(&setup);
    let made = ok(
        &core,
        "heat.note.create",
        json!({"title": "Te-form", "markdown": "Group one verbs change their ending.\n"}),
    );
    assert_eq!(made["undo"], "Undo add note");
    let id = made["note"]["id"].clone();
    let text = read(dir.join("Te-form.md"));
    assert_eq!(
        text,
        format!(
            "---\nid: {}\n---\n\nGroup one verbs change their ending.\n",
            id.as_str().unwrap()
        )
    );
    assert_eq!(snap(&core, DAY)["notes"]["folder"], json!(dir));
    // An edit is in the file at once.
    ok(
        &core,
        "heat.note.save",
        json!({"id": id, "markdown": "Group one verbs change their ending.\n\nSee [[Verb groups]].\n"}),
    );
    assert!(read(dir.join("Te-form.md")).ends_with("See [[Verb groups]].\n"));
    // No two notes share a name.
    assert_eq!(
        refused(&core, "heat.note.create", json!({"title": "te-form"})).1,
        "A note is already called te-form."
    );
    let same = ok(
        &core,
        "heat.note.create",
        json!({"title": "te-form", "ifMissing": true}),
    );
    assert_eq!(
        (
            same["note"]["id"].clone(),
            same["existed"].clone(),
            same["undo"].clone()
        ),
        (id.clone(), json!(true), Value::Null)
    );
    // A second note links to the first; renaming the first moves the link and the file.
    let drills = ok(
        &core,
        "heat.note.create",
        json!({"title": "Drills", "markdown": "Practice [[Te-form]] and [[te-form|again]].\n"}),
    );
    let renamed = ok(
        &core,
        "heat.note.save",
        json!({"id": id, "title": "Te-form: the rule"}),
    );
    assert_eq!(
        (renamed["undo"].as_str(), renamed["renamed"].as_i64()),
        (Some("Undo rename note"), Some(1))
    );
    assert!(!dir.join("Te-form.md").exists());
    // A colon can't be in a file's name; the note keeps its own.
    assert!(read(dir.join("Te-form- the rule.md")).contains("Group one verbs"));
    assert_eq!(note(&core, &id)["title"], "Te-form: the rule");
    assert_eq!(
        note(&core, &drills["note"]["id"])["markdown"],
        "Practice [[Te-form: the rule]] and [[Te-form: the rule|again]].\n"
    );
    assert!(read(dir.join("Drills.md")).contains("[[Te-form: the rule|again]]"));
    // One undo puts the name, the link and both files back.
    ok(&core, "history.undo", json!({"room": "heat"}));
    ok(&core, "heat.note.sync", json!({}));
    assert!(dir.join("Te-form.md").exists() && !dir.join("Te-form- the rule.md").exists());
    assert!(read(dir.join("Drills.md")).contains("Practice [[Te-form]]"));
    // A deleted note's file goes to the trash, never away; undo brings it back.
    ok(&core, "heat.delete", json!({"kind": "note", "id": id}));
    ok(&core, "heat.note.sync", json!({}));
    assert!(!dir.join("Te-form.md").exists() && dir.join(".trash/Te-form.md").exists());
    ok(&core, "history.undo", json!({"room": "heat"}));
    assert!(
        eventually(T, || dir.join("Te-form.md").exists()),
        "the worker writes an undone note's file by itself"
    );
    assert_eq!(
        ok(&core, "heat.note.sync", json!({}))["changed"],
        0,
        "the two agree, so there is nothing to do"
    );
    // Where the file is, for the Finder.
    assert_eq!(
        ok(&core, "heat.note.reveal", json!({"id": id, "quiet": true}))["path"],
        json!(dir.join("Te-form.md"))
    );
}

#[test]
fn what_changes_in_the_folder_reaches_the_library_and_one_undo_takes_it_back() {
    let setup = Setup::new();
    let (core, dir) = notes_core(&setup);
    let made = ok(
        &core,
        "heat.note.create",
        json!({"title": "Circuits", "markdown": "Kirchhoff's voltage law.\n"}),
    );
    let id = made["note"]["id"].clone();
    let path = dir.join("Circuits.md");
    // Edited in another app, with a line of front matter of the person's own.
    let edited = read(path.clone())
        .replace("---\n\n", "aliases: [KVL]\n---\n\n")
        .replace(
            "voltage law.",
            "voltage law: the sum around a loop is zero. #ele",
        );
    std::fs::write(&path, &edited).unwrap();
    // The worker may have seen the edit first: either way it is in the library now.
    ok(&core, "heat.note.sync", json!({}));
    assert_eq!(
        note(&core, &id)["markdown"],
        "Kirchhoff's voltage law: the sum around a loop is zero. #ele\n"
    );
    assert_eq!(
        ok(&core, "history.get", json!({"room": "heat"}))["undo"],
        "Undo notes from disk"
    );
    assert_eq!(
        read(path.clone()),
        edited,
        "the file is left as the person wrote it"
    );
    // The app's own edit keeps their line.
    ok(
        &core,
        "heat.note.save",
        json!({"id": id, "markdown": "Kirchhoff's two laws.\n"}),
    );
    assert!(read(path.clone()).contains("aliases: [KVL]\n---\n\nKirchhoff's two laws.\n"));
    ok(&core, "history.undo", json!({"room": "heat"}));
    ok(&core, "history.undo", json!({"room": "heat"}));
    ok(&core, "heat.note.sync", json!({}));
    assert_eq!(note(&core, &id)["markdown"], "Kirchhoff's voltage law.\n");
    assert!(read(path.clone()).ends_with("Kirchhoff's voltage law.\n"));

    // A file dropped in the folder is a note, named for the file.
    std::fs::write(
        dir.join("Lab 3 prep.md"),
        "Bring the breadboard.\n\n- [ ] Read the handout\n",
    )
    .unwrap();
    ok(&core, "heat.note.sync", json!({}));
    let s = snap(&core, DAY);
    let lab = records(&s, "note")
        .into_iter()
        .find(|n| n["title"] == "Lab 3 prep")
        .expect("a note for the new file");
    assert_eq!(
        lab["markdown"],
        "Bring the breadboard.\n\n- [ ] Read the handout\n"
    );
    assert_eq!(
        read(dir.join("Lab 3 prep.md")),
        "Bring the breadboard.\n\n- [ ] Read the handout\n",
        "a file Learn didn't write isn't rewritten"
    );
    // Renamed in the Finder: the same note, with its new name.
    std::fs::rename(dir.join("Lab 3 prep.md"), dir.join("Lab 3.md")).unwrap();
    // It has no id in it, so it is a new file and the old one is gone: one entry.
    ok(&core, "heat.note.sync", json!({}));
    let titles: Vec<String> = records(&snap(&core, DAY), "note")
        .iter()
        .map(|n| n["title"].as_str().unwrap().to_string())
        .collect();
    assert!(
        titles.contains(&"Lab 3".to_string()) && !titles.contains(&"Lab 3 prep".to_string()),
        "{titles:?}"
    );
    // A note Learn wrote carries its id, so renaming its file renames the note.
    std::fs::rename(&path, dir.join("Circuits 1.md")).unwrap();
    ok(&core, "heat.note.sync", json!({}));
    assert_eq!(note(&core, &id)["title"], "Circuits 1");
    // Deleted in the Finder: the note goes, and undo puts the note and the file back.
    std::fs::remove_file(dir.join("Circuits 1.md")).unwrap();
    ok(&core, "heat.note.sync", json!({}));
    assert!(note(&core, &id).is_null());
    ok(&core, "history.undo", json!({"room": "heat"}));
    ok(&core, "heat.note.sync", json!({}));
    assert_eq!(note(&core, &id)["title"], "Circuits 1");
    assert!(dir.join("Circuits 1.md").exists());
    // The worker looks at the folder by itself, every few seconds.
    std::fs::write(dir.join("Found.md"), "By the worker.\n").unwrap();
    assert!(
        eventually(T, || records(&snap(&core, DAY), "note")
            .iter()
            .any(|n| n["title"] == "Found")),
        "a new file was never noticed"
    );
}

#[test]
fn links_backlinks_tags_search_and_a_checkbox_that_becomes_a_task() {
    let setup = Setup::new();
    let (core, _dir) = notes_core(&setup);
    let term = ok(
        &core,
        "heat.put",
        json!({"kind": "term", "record": {"name": "Fall 2026"}}),
    )["record"]["id"]
        .clone();
    let course = ok(&core, "heat.put", json!({"kind": "course", "record": {"termId": term, "code": "JPN 101", "name": "Beginning Japanese I"}}))["record"].clone();
    let space = records(&snap(&core, DAY), "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let quiz = add_task(&core, &space, "Kanji quiz", json!({}));
    let groups = ok(
        &core,
        "heat.note.create",
        json!({"title": "Verb groups", "markdown": "Three groups.\n"}),
    )["note"]
        .clone();
    let page = ok(
        &core,
        "heat.note.create",
        json!({"title": "Te-form", "course": "jpn101", "markdown": "See [[verb groups]] for [[JPN 101]], before the [[Kanji quiz]]. Also [[Particles]].\n\n- [ ] Do worksheet 4\n- [ ] \n\n#grammar #JPN101 `#not`\n"}),
    )["note"]
        .clone();
    assert_eq!(page["courseId"], course["id"]);
    let i = index(&core, &page["id"]);
    let kinds: Vec<(&str, &str, Value)> = i["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["target"].as_str().unwrap(),
                l["kind"].as_str().unwrap(),
                l["id"].clone(),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            ("verb groups", "note", groups["id"].clone()),
            ("JPN 101", "course", course["id"].clone()),
            ("Kanji quiz", "task", quiz["id"].clone()),
            ("Particles", "missing", Value::Null),
        ]
    );
    assert_eq!(i["tags"], json!(["grammar", "jpn101"]));
    assert_eq!(i["label"], "JPN 101 · Beginning Japanese I");
    let back = index(&core, &groups["id"])["backlinks"].clone();
    assert_eq!(
        back,
        json!([{"id": page["id"], "title": "Te-form", "line": "See verb groups for JPN 101, before the Kanji quiz. Also Particles."}])
    );
    let s = snap(&core, DAY);
    assert_eq!(
        s["notes"]["tags"],
        json!([{"tag": "grammar", "count": 1}, {"tag": "jpn101", "count": 1}])
    );
    assert_eq!(s["notes"]["order"].as_array().unwrap().len(), 2);

    // A link to a name nothing has makes the note.
    let linked = ok(
        &core,
        "heat.note.link",
        json!({"id": groups["id"], "to": "Particles"}),
    );
    assert_eq!(
        (
            linked["line"].as_str(),
            linked["created"]["title"].as_str(),
            linked["undo"].as_str()
        ),
        (
            Some("Linked to Particles"),
            Some("Particles"),
            Some("Undo link note")
        )
    );
    assert_eq!(index(&core, &page["id"])["links"][3]["kind"], "note");
    // To a course it writes the code, and makes nothing.
    let to_course = ok(
        &core,
        "heat.note.link",
        json!({"id": groups["id"], "to": "jpn 101"}),
    );
    assert!(to_course["created"].is_null());
    assert!(to_course["note"]["markdown"]
        .as_str()
        .unwrap()
        .ends_with("[[Particles]]\n\n[[JPN 101]]\n"));

    // The checkbox on line 2 becomes a task of the note's course.
    assert_eq!(
        refused(
            &core,
            "heat.note.taskFromLine",
            json!({"id": page["id"], "line": 0})
        )
        .1,
        "That line isn't a checkbox."
    );
    assert_eq!(
        refused(
            &core,
            "heat.note.taskFromLine",
            json!({"id": page["id"], "line": 3})
        )
        .1,
        "Write what the task is first."
    );
    let made = ok(
        &core,
        "heat.note.taskFromLine",
        json!({"id": page["id"], "line": 2}),
    );
    assert_eq!(made["undo"], "Undo task from note");
    let task = &made["task"];
    assert_eq!(
        (
            task["title"].as_str(),
            task["noteId"].clone(),
            task["courseId"].clone()
        ),
        (
            Some("Do worksheet 4"),
            page["id"].clone(),
            course["id"].clone()
        )
    );
    assert_eq!(task["notes"], "From the note [[Te-form]].");
    let line = made["note"]["markdown"]
        .as_str()
        .unwrap()
        .lines()
        .nth(2)
        .unwrap()
        .to_string();
    assert_eq!(
        line,
        format!("- [ ] Do worksheet 4 ^t-{}", task["id"].as_str().unwrap())
    );
    assert_eq!(
        index(&core, &page["id"])["boxes"][0],
        json!({"line": 2, "text": "Do worksheet 4", "done": false, "taskId": task["id"]})
    );
    assert_eq!(
        refused(
            &core,
            "heat.note.taskFromLine",
            json!({"id": page["id"], "line": 2})
        )
        .1,
        "That line is a task already."
    );
    // Ticking the box marks the task, in one entry; marking the task elsewhere shows in the box.
    let ticked = ok(
        &core,
        "heat.note.toggleBox",
        json!({"id": page["id"], "line": 2, "done": true}),
    );
    assert_eq!(
        (ticked["undo"].as_str(), ticked["task"]["done"].as_bool()),
        (Some("Undo check box"), Some(true))
    );
    assert!(ticked["note"]["markdown"]
        .as_str()
        .unwrap()
        .contains("- [x] Do worksheet 4 ^t-"));
    ok(
        &core,
        "heat.done",
        json!({"taskId": task["id"], "done": false, "date": DAY}),
    );
    assert_eq!(
        index(&core, &page["id"])["boxes"][0]["done"],
        false,
        "the box shows the task's own state"
    );
    // One undo of "task from note" takes the task and the mark away together.
    ok(&core, "history.undo", json!({"room": "heat"}));
    ok(&core, "history.undo", json!({"room": "heat"}));
    ok(&core, "history.undo", json!({"room": "heat"}));
    let s = snap(&core, DAY);
    assert!(!records(&s, "task")
        .iter()
        .any(|t| t["title"] == "Do worksheet 4"));
    assert_eq!(
        s["notes"]["index"][page["id"].as_str().unwrap()]["boxes"][0]["taskId"],
        Value::Null
    );

    // Search: every word, a title first, with the words around the match.
    let hits = ok(&core, "heat.note.search", json!({"q": "worksheet"}))["hits"].clone();
    assert_eq!(
        (hits[0]["title"].as_str(), hits.as_array().unwrap().len()),
        (Some("Te-form"), 1)
    );
    assert!(hits[0]["snippet"]
        .as_str()
        .unwrap()
        .contains("Do worksheet 4"));
    assert_eq!(
        ok(&core, "heat.note.search", json!({"q": "groups"}))["hits"][0]["title"],
        "Verb groups"
    );
    assert!(
        ok(&core, "heat.note.search", json!({"q": "worksheet nowhere"}))["hits"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    // Filing: to a course, to a space, to nothing.
    let filed = ok(
        &core,
        "heat.note.file",
        json!({"id": "Verb groups", "course": "JPN 101"}),
    );
    assert_eq!(
        (filed["line"].as_str(), filed["undo"].as_str()),
        (Some("Filed to JPN 101"), Some("Undo file note"))
    );
    let moved = ok(
        &core,
        "heat.note.file",
        json!({"id": groups["id"], "space": "Personal"}),
    );
    assert_eq!(moved["line"], "Filed to Personal");
    assert!(moved["note"].get("courseId").is_none());
    assert_eq!(
        ok(
            &core,
            "heat.note.file",
            json!({"id": groups["id"], "none": true})
        )["line"],
        "Filed with no course or space"
    );
    assert_eq!(
        refused(
            &core,
            "heat.note.file",
            json!({"id": groups["id"], "course": "MTH 999"})
        )
        .1,
        "No course has the code MTH 999."
    );
    // A course that is deleted leaves its notes, filed to nothing.
    ok(
        &core,
        "heat.delete",
        json!({"kind": "course", "id": course["id"]}),
    );
    assert!(note(&core, &page["id"]).get("courseId").is_none());
}

#[test]
fn a_daily_note_is_the_note_titled_with_its_day_and_old_ones_are_moved_over_once() {
    let setup = Setup::new();
    // A library from before Notes: two daily notes, one of them public.
    {
        let mut store = wi_store::Store::open(&setup.library()).unwrap();
        store
            .set_doc(
                "dailyNote",
                "2026-10-05",
                &json!({"date": "2026-10-05", "markdown": "Mixed the bridge.", "public": true}),
                "",
            )
            .unwrap();
        store.set_doc("dailyNote", "2026-10-06", &json!({"date": "2026-10-06", "markdown": "Kanji all evening. [[Te-form]]", "public": false}), "").unwrap();
        store
            .set_doc(
                "dailyNote",
                "2026-10-04",
                &json!({"date": "2026-10-04", "markdown": "  ", "public": false}),
                "",
            )
            .unwrap();
    }
    let (core, dir) = notes_core(&setup);
    let s = snap(&core, DAY);
    assert!(records(&s, "dailyNote").is_empty());
    let notes = records(&s, "note");
    let by_title = |t: &str| {
        notes
            .iter()
            .find(|n| n["title"] == t)
            .cloned()
            .unwrap_or(Value::Null)
    };
    assert_eq!(
        by_title("2026-10-06")["markdown"],
        "Kanji all evening. [[Te-form]]"
    );
    assert_eq!(
        by_title("2026-10-05")["public"],
        true,
        "one that was public stays public"
    );
    assert!(
        by_title("2026-10-04").is_null(),
        "an empty one is nothing to keep"
    );
    assert!(s["notes"]["daily"].is_null());
    ok(&core, "heat.note.sync", json!({}));
    assert!(read(dir.join("2026-10-06.md")).ends_with("Kanji all evening. [[Te-form]]"));
    assert_eq!(
        ok(&core, "history.get", json!({"room": "heat"}))["undo"],
        "Undo move daily notes into Notes"
    );
    // Today's: read (nothing yet), written, written again.
    assert!(ok(&core, "heat.note.daily", json!({"date": DAY}))["note"].is_null());
    assert!(ok(
        &core,
        "heat.note.daily",
        json!({"date": DAY, "markdown": " "})
    )["note"]
        .is_null());
    let made = ok(
        &core,
        "heat.note.daily",
        json!({"date": DAY, "markdown": "A good morning."}),
    );
    assert_eq!(
        (made["undo"].as_str(), made["note"]["title"].as_str()),
        (Some("Undo add daily note"), Some(DAY))
    );
    assert_eq!(snap(&core, DAY)["notes"]["daily"], made["note"]["id"]);
    let again = ok(
        &core,
        "heat.note.daily",
        json!({"date": DAY, "markdown": "A good morning.\nA better afternoon."}),
    );
    assert_eq!(
        (again["undo"].as_str(), again["note"]["id"].clone()),
        (Some("Undo edit note"), made["note"]["id"].clone())
    );
    assert_eq!(
        refused(&core, "heat.note.daily", json!({"date": "today"})).1,
        "A daily note needs a day, as YYYY-MM-DD."
    );
    // Claude's `get_notes` still finds the day's note.
    drop(core);
    let store = wi_store::Store::open(&setup.library()).unwrap();
    let clock = wi_heat_store::Clock::at(
        ny(NOW),
        jiff::tz::TimeZone::get("America/New_York").unwrap(),
    );
    let got =
        wi_heat_store::mcp::get_notes(&store, &clock, json!({"date": DAY}).as_object().unwrap())
            .unwrap();
    assert_eq!(
        got["daily_note"]["text"],
        "A good morning.\nA better afternoon."
    );
    // It is done once: a second opening moves nothing and writes no entry.
    drop(store);
    let (core, _) = notes_core(&setup);
    snap(&core, DAY);
    assert_eq!(
        ok(&core, "history.get", json!({"room": "heat"}))["undo"],
        "Undo edit note"
    );
}

#[test]
fn with_no_folder_notes_are_records_and_nothing_is_written() {
    let setup = Setup::new();
    let core = heat_core(&setup, NOW);
    let made = ok(
        &core,
        "heat.note.create",
        json!({"markdown": "Loose thought."}),
    );
    assert_eq!(made["note"]["title"], "Untitled");
    assert_eq!(
        ok(&core, "heat.note.create", json!({}))["note"]["title"],
        "Untitled 2"
    );
    assert_eq!(ok(&core, "heat.note.sync", json!({}))["changed"], 0);
    assert!(!setup.library().join("Notes").exists());
    let s = snap(&core, DAY);
    assert!(s["notes"]["folder"].is_null());
    assert_eq!(s["notes"]["capture"]["watching"], false);
    assert_eq!(
        refused(
            &core,
            "heat.note.attachment",
            json!({"path": "attachments/x.jpg"})
        )
        .1,
        "Notes aren't kept in a folder here."
    );
    assert_eq!(
        refused(&core, "heat.capture.process", json!({})).1,
        "Notes aren't kept in a folder here."
    );
    // A path out of the folder is never read.
    let (with, dir) = notes_core(&Setup::new());
    std::fs::create_dir_all(dir.join("attachments")).unwrap();
    std::fs::write(dir.join("attachments/p.png"), b"png").unwrap();
    assert_eq!(
        ok(
            &with,
            "heat.note.attachment",
            json!({"path": "attachments/p.png"})
        )["dataUrl"],
        "data:image/png;base64,cG5n"
    );
    for bad in ["../library.sqlite", "/etc/hosts", "attachments/../../x"] {
        assert_eq!(
            refused(&with, "heat.note.attachment", json!({"path": bad})).1,
            "That file isn't in the notes folder.",
            "{bad}"
        );
    }
}
