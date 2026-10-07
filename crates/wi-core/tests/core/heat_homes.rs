//! Homes and types (docs/HEAT.md, "Homes and types"). What a fail looks like:
//! - a library from before courses had names or tasks had types loses a
//!   record, a typed estimate or a hand-set difficulty when it is brought
//!   over, or can't be put back with one undo;
//! - a course Mail names doesn't reach Grades, or reaches it twice, once for
//!   each section; a subject that only looks like a code makes a course;
//! - a task outside any course or project gets no estimate, or a course is
//!   forced onto work that isn't school;
//! - a syllabus changes anything before it is accepted, or takes more than
//!   one undo to take back, or invents a weight the syllabus didn't give;
//! - a number the person set is overwritten by a type; a moved ratio rewrites
//!   tasks that are already there;
//! - Claude is started without being asked, or its estimate lands on a task
//!   that has one.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;
use wi_store::Store;

const DAY: &str = "2026-10-07";
const NOW: &str = "2026-10-07 09:00";
const T: Duration = Duration::from_secs(20);

fn by<'a>(list: &'a [Value], field: &str, value: &str) -> &'a Value {
    list.iter()
        .find(|r| r[field] == value)
        .unwrap_or_else(|| panic!("nothing with {field} = {value}"))
}

fn task(core: &Core, id: &str) -> Value {
    by(&records(&snap(core, DAY), "task"), "id", id).clone()
}

fn numbers(t: &Value) -> (Option<&str>, Option<f64>, Option<f64>, Option<&str>) {
    (
        t["type"].as_str(),
        t["estMin"].as_f64(),
        t["difficulty"].as_f64(),
        t["estBy"].as_str(),
    )
}

/// The library as an earlier build left it on the founder's Mac: three
/// courses the feed named by their codes, tasks at difficulty 3 with no
/// estimate, and mail Claude recorded for seven courses.
fn old_library(setup: &Setup) {
    let mut store = Store::open(&setup.library()).unwrap();
    wi_heat_store::ops::ensure_defaults(&mut store).unwrap();
    let mut put = |kind: &str, record: Value| {
        let key = record["id"].as_str().unwrap().to_string();
        store.set_doc(kind, &key, &record, "").unwrap();
    };
    put("term", json!({"id": "fall", "name": "Fall 2026"}));
    for code in ["JPN 101", "ELE 202", "ELE 201"] {
        let id = code.to_lowercase().replace(' ', "");
        put(
            "course",
            json!({"id": id, "termId": "fall", "code": code, "name": code, "categories": [], "notes": "", "public": false}),
        );
    }
    let feed = |id: &str, title: &str, kind: &str, course: &str| {
        json!({
            "id": id, "spaceId": "classes", "title": title, "type": kind, "courseId": course,
            "due": ny("2026-10-16 23:59"), "difficulty": 3, "estMin": null, "estBy": "default", "adjustMin": 0,
            "notes": "", "done": false, "doneAt": null, "source": "ical", "sourceId": id, "public": false,
        })
    };
    put("task", feed("t-quiz", "Online Vocabulary Quiz (第2課)", "Quiz", "jpn101"));
    put("task", feed("t-lab", "lab3", "Other", "ele202"));
    put("task", feed("t-sheet", "Worksheet 3", "Homework", "jpn101"));
    put("task", feed("t-hw", "Homework 3", "Homework", "ele201"));
    put("task", feed("t-async", "Asynchronous Task (1%)  = October 12th class", "Other", "jpn101"));
    // The person's own numbers, and a type they picked against the title.
    let mut mine = feed("t-mine", "Video Project 1 (draft)", "Homework", "jpn101");
    mine["estMin"] = json!(75);
    mine["estBy"] = json!("you");
    mine["difficulty"] = json!(5);
    put("task", mine);
    let mut done = feed("t-done", "Listening Comprehension Task_Ch.1", "Listening", "jpn101");
    done["done"] = json!(true);
    done["doneAt"] = json!(ny("2026-10-01 20:00"));
    put("task", done);
    // Outside any course: mail Claude turned into an errand, and a group that is a course's code.
    put("task", json!({
        "id": "t-wifi", "spaceId": "personal", "title": "Update device for URI Wi-Fi", "type": "Errand",
        "due": ny("2026-10-21 23:59"), "difficulty": 3, "estMin": null, "estBy": "default", "adjustMin": 0,
        "notes": "From mail: IT asks everyone to update.", "done": false, "doneAt": null, "source": "claude", "public": false,
    }));
    put("task", json!({
        "id": "t-group", "spaceId": "personal", "title": "Edfinity 7.4", "type": "Other", "group": "MTH 142",
        "due": null, "difficulty": 3, "estMin": null, "estBy": "default", "adjustMin": 0,
        "notes": "", "done": false, "doneAt": null, "source": "you", "public": false,
    }));
    let mail = |id: &str, subject: &str, course: Option<&str>, category: &str| {
        let mut m = json!({
            "id": id, "gmailThreadId": id, "subject": subject, "from": "noreply@uri.brightspace.com",
            "receivedAt": ny("2026-10-07 06:01"), "state": "nothing", "reason": "r", "recordedBy": "claude",
            "account": "liam.young@uri.edu", "priority": "low", "category": category,
        });
        if let Some(c) = course {
            m["course"] = json!(c);
        }
        m
    };
    for (id, subject, course) in [
        ("m1", "Activity summary for EGR101: Intro to Engineering Design_0011_FALL26 on Oct 7, 2026", "EGR 101"),
        ("m2", "Activity summary for EGR101: Intro to Engineering Design_R01_FALL26 on Oct 7, 2026", "EGR 101"),
        ("m3", "ELE209: Intro. to Computer Systems Lab_0001_FALL26 – \"ELE209_Recitation4\" has been created", "ELE 209"),
        ("m4", "Activity summary for JPN101: Beginning Japanese I_0001_FALL26 on Oct 7, 2026", "JPN 101"),
        ("m5", "Activity summary for ELE202: Digital Circuit Design Lab_0005_FALL26 on Oct 7, 2026", "ELE 202"),
        ("m6", "Submission receipt", "ELE 208"),
        ("m7", "JPN101: FYI. First-year students with Pell grants: apply for a grant to get your passport", "JPN 101"),
    ] {
        put("mailThread", mail(id, subject, Some(course), "school"));
    }
    put("mailThread", mail("m8", "Carothers Library  Booking Confirmation", None, "updates"));
    put("mailThread", mail("m9", "Re: USB 300 hub order", None, "money"));
}

fn docs(setup_library: &Path) -> Vec<(String, String, Value)> {
    let store = Store::open(setup_library).unwrap();
    let mut out = Vec::new();
    for kind in ["term", "course", "task", "grade", "mailThread", "space", "project"] {
        for d in store.docs(kind).unwrap() {
            out.push((kind.to_string(), d.key, d.json));
        }
    }
    out.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    out
}

#[test]
fn an_old_library_is_brought_over_once_and_one_undo_puts_it_back() {
    let setup = Setup::new();
    old_library(&setup);
    let before = docs(&setup.library());
    let core = heat_core(&setup, NOW);
    let shown = snap(&core, DAY);

    // Every course Mail names is in Grades, once, under its real name.
    let courses = records(&shown, "course");
    let mut labels: Vec<String> = courses
        .iter()
        .map(|c| shown["derived"]["courses"][c["id"].as_str().unwrap()]["label"].as_str().unwrap().to_string())
        .collect();
    labels.sort();
    assert_eq!(
        labels,
        [
            "EGR 101 · Intro to Engineering Design",
            "ELE 201",
            "ELE 202 · Digital Circuit Design Lab",
            "ELE 208",
            "ELE 209 · Intro. to Computer Systems Lab",
            "JPN 101 · Beginning Japanese I",
            "MTH 142",
        ],
        "two sections are one course; a library booking and a USB hub are none"
    );
    for c in &courses {
        let d = &shown["derived"]["courses"][c["id"].as_str().unwrap()];
        assert_eq!(
            (c["status"].as_str(), d["needsSyllabus"].as_bool(), d["weights"].is_null(), c["termId"].as_str()),
            (Some("stub"), Some(true), true, Some("fall")),
            "{}",
            c["code"]
        );
    }
    assert_eq!(records(&shown, "term").len(), 1, "FALL26 is the Fall 2026 already there");

    // Class tasks stop being difficulty 3 and no time at all.
    let tasks = records(&shown, "task");
    let t = |id: &str| by(&tasks, "id", id);
    assert_eq!(numbers(t("t-quiz")), (Some("Quiz"), Some(20.0), Some(2.0), Some("type")));
    assert_eq!(numbers(t("t-lab")), (Some("Lab"), Some(120.0), Some(3.0), Some("type")), "lab3 is a lab");
    assert_eq!(numbers(t("t-sheet")), (Some("Worksheet"), Some(45.0), Some(2.0), Some("type")));
    assert_eq!(numbers(t("t-hw")), (Some("Homework"), Some(90.0), Some(3.0), Some("type")));
    // Nothing matched: the catch-all's numbers, marked for Claude to better.
    assert_eq!(numbers(t("t-async")), (Some("Other"), Some(45.0), Some(2.0), Some("default")));
    assert_eq!(shown["derived"]["unscored"], 2, "that one, and the Edfinity task");
    // What the person set stays theirs: the type, the minutes, the difficulty.
    assert_eq!(numbers(t("t-mine")), (Some("Homework"), Some(75.0), Some(5.0), Some("you")));
    assert_eq!(t("t-mine")["difficultyBy"], "you");
    // A finished task is history, and is left as it was.
    assert_eq!(t("t-done"), &before.iter().find(|d| d.1 == "t-done").unwrap().2);
    // A task with no course at all still gets a sensible estimate, and no course.
    assert_eq!(numbers(t("t-wifi")), (Some("Errand"), Some(30.0), Some(1.0), Some("type")));
    assert!(t("t-wifi").get("courseId").is_none());
    assert_eq!(shown["derived"]["tasks"]["t-wifi"]["home"], Value::Null);
    assert_eq!(shown["derived"]["tasks"]["t-wifi"]["estimate"]["typeFrom"], "global");
    // A group that was a course's code is that course now; the group is kept.
    let mth = by(&courses, "code", "MTH 142");
    assert_eq!((t("t-group")["courseId"].clone(), t("t-group")["group"].clone()), (mth["id"].clone(), json!("MTH 142")));
    assert_eq!(
        shown["derived"]["tasks"]["t-quiz"]["home"],
        json!({"kind": "course", "id": "jpn101", "label": "JPN 101 · Beginning Japanese I"})
    );

    // One entry; opening again makes no second one; undo puts every record back.
    assert_eq!(
        ok(&core, "history.get", json!({"room": "heat"}))["undo"],
        "Undo course and type setup"
    );
    let after = docs(&setup.library());
    assert_eq!(after.len(), before.len() + 4, "four courses made, nothing lost");
    snap(&core, DAY);
    assert_eq!(docs(&setup.library()), after);
    ok(&core, "history.undo", json!({"room": "heat"}));
    assert_eq!(docs(&setup.library()), before, "one undo is the old library, record for record");
}

fn classes_core(setup: &Setup) -> Core {
    heat_core(setup, NOW)
}

fn course(core: &Core, code: &str) -> Value {
    ok(core, "heat.put", json!({"kind": "term", "record": {"id": "fall", "name": "Fall 2026"}}));
    ok(
        core,
        "heat.put",
        json!({"kind": "course", "record": {"termId": "fall", "code": code, "status": "stub"}}),
    )["record"]
        .clone()
}

fn syllabus(final_pct: u32) -> Value {
    json!({
        "course": {"code": "ELE 209", "name": "Intro to Computer Systems Lab", "term": "Fall 2026"},
        "weights": [
            {"category": "Labs", "percent": 40, "drop_lowest": 1},
            {"category": "Quizzes", "percent": 20, "drop_lowest": null},
            {"category": "Final", "percent": final_pct, "drop_lowest": null}
        ],
        "types": [
            {"name": "Lab", "title_patterns": ["lab"], "est_minutes": 150, "difficulty": 4, "category": "Labs"},
            {"name": "Quiz", "title_patterns": ["quiz", "quizzes"], "est_minutes": null, "difficulty": null, "category": "Quizzes"},
            {"name": "Recitation", "title_patterns": ["recitation"], "est_minutes": null, "difficulty": null, "category": null},
            {"name": "Final exam", "title_patterns": ["final exam", "final"], "est_minutes": null, "difficulty": null, "category": "Final"},
            {"name": "Prelab", "title_patterns": ["prelab"], "est_minutes": 30, "difficulty": 2, "category": "Labs"}
        ],
        "items": [
            {"title": "Lab 4", "type": "Lab", "due": "2026-10-23"},
            {"title": "Lab 5", "type": "Lab", "due": "2026-10-30"},
            {"title": "Lab 6", "type": "Lab", "due": "2026-11-06"},
            {"title": "Final exam", "type": "Final exam", "due": "2026-12-14T08:00"},
            {"title": "Quiz 2", "type": "Quiz", "due": "2026-10-20"},
            {"title": "Lab 1", "type": "Lab", "due": "2026-09-18"},
            {"title": "Office hours", "type": null, "due": null}
        ]
    })
}

#[test]
fn a_syllabus_is_previewed_then_accepted_in_one_entry_that_one_undo_takes_back() {
    let setup = Setup::new();
    let core = classes_core(&setup);
    let space = records(&snap(&core, DAY), "space")[0]["id"].as_str().unwrap().to_string();
    let ele = course(&core, "ele209");
    let id = ele["id"].as_str().unwrap();
    assert_eq!((ele["code"].as_str(), ele["name"].as_str()), (Some("ELE 209"), Some("ELE 209")));
    // Tasks already in the course: one the syllabus dates differently, one it retimes,
    // one whose minutes are the person's, and a grade waiting for a category.
    let lab4 = add_task(&core, &space, "lab4", json!({"courseId": id, "due": ny("2026-10-16 23:59")}));
    let quiz = add_task(&core, &space, "Quiz 2", json!({"courseId": id, "due": ny("2026-10-20 23:59")}));
    let mine = add_task(&core, &space, "Lab 5", json!({"courseId": id, "due": ny("2026-10-30 23:59"), "estMin": 200}));
    assert_eq!(numbers(&lab4), (Some("Lab"), Some(120.0), Some(3.0), Some("type")));
    ok(&core, "heat.put", json!({"kind": "grade", "record": {"courseId": id, "title": "Lab 2 report", "score": 18, "outOf": 20}}));
    let stub = &snap(&core, DAY)["derived"]["courses"][id];
    assert_eq!((stub["needsSyllabus"].clone(), stub["label"].clone()), (json!(true), json!("ELE 209")));

    let before = docs(&setup.library());
    let events = core.events();
    let draft = ok(
        &core,
        "heat.course.importSyllabus",
        json!({"courseId": id, "json": syllabus(40), "fileName": "ELE209_syllabus.pdf"}),
    )["draft"]
        .clone();
    assert_eq!(wait_event(&events, "heat", T).payload["kinds"], json!(["syllabusDraft"]));
    assert_eq!(
        draft["line"],
        "ELE 209 · Intro to Computer Systems Lab. Labs 40%, Quizzes 20%, Final 40%. 5 types. 2 new tasks, 1 date changed."
    );
    assert_eq!((draft["state"].clone(), draft["weightsFlag"].clone(), draft["weightsTotal"].clone()), (json!("ready"), Value::Null, json!(100)));
    assert_eq!(draft["course"], json!({"code": "ELE 209", "name": "Intro to Computer Systems Lab", "term": "Fall 2026", "label": "ELE 209 · Intro to Computer Systems Lab", "isNew": false}));
    // New: Lab 6 and the final. Lab 5 is here on that day; Quiz 2 too; Lab 1 is long past; office hours have no date.
    let new: Vec<&str> = draft["newTasks"].as_array().unwrap().iter().map(|t| t["title"].as_str().unwrap()).collect();
    assert_eq!(new, ["Lab 6", "Final exam"]);
    assert_eq!(draft["newTasks"][0]["estMin"], 150);
    assert_eq!(
        draft["dateChanges"],
        json!([{"taskId": lab4["id"], "title": "lab4", "from": ny("2026-10-16 23:59"), "to": ny("2026-10-23 23:59")}])
    );
    // A type the syllabus gives no minutes takes them from the default of that name; one nothing names gets the catch-all's.
    let types = draft["types"].as_array().unwrap();
    assert_eq!((by(types, "name", "Quiz")["estMin"].clone(), by(types, "name", "Recitation")["estMin"].clone()), (json!(20), json!(45)));
    // The preview is in the snapshot, and nothing at all has been written.
    assert_eq!(snap(&core, DAY)["syllabus"]["drafts"][0]["line"], draft["line"]);
    assert_eq!(docs(&setup.library()), before, "a draft is not a change");

    let done = ok(&core, "heat.syllabus.accept", json!({"draftId": draft["id"]}));
    assert_eq!(done["undo"], "Undo import syllabus");
    assert_eq!(done["counts"], json!({"newTasks": 2, "dateChanges": 1, "retimed": 1}));
    let shown = snap(&core, DAY);
    assert_eq!(shown["syllabus"]["drafts"], json!([]));
    let c = by(&records(&shown, "course"), "id", id).clone();
    assert_eq!(
        (c["name"].as_str(), c["status"].as_str(), c["syllabusSource"]["name"].as_str()),
        (Some("Intro to Computer Systems Lab"), Some("confirmed"), Some("ELE209_syllabus.pdf"))
    );
    let weights: Vec<(String, f64, Option<f64>)> = c["categories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| (k["name"].as_str().unwrap().to_string(), k["weight"].as_f64().unwrap(), k["dropLowest"].as_f64()))
        .collect();
    assert_eq!(weights, [("Labs".into(), 40.0, Some(1.0)), ("Quizzes".into(), 20.0, None), ("Final".into(), 40.0, None)]);
    let d = &shown["derived"]["courses"][id];
    assert_eq!((d["needsSyllabus"].clone(), d["weights"].clone()), (json!(false), Value::Null));
    // Grades can answer again, and the grade already there found its category by its type.
    assert!(ok(&core, "heat.whatItWouldTake", json!({"courseId": id, "letter": "B"}))["text"].as_str().unwrap().contains("you need"));
    let labs = by(c["categories"].as_array().unwrap(), "name", "Labs")["id"].clone();
    assert_eq!(records(&shown, "grade")[0]["categoryId"], labs);
    // The course's own lab is 150 minutes at 4: the lab already here follows it and its new date.
    let moved = task(&core, lab4["id"].as_str().unwrap());
    assert_eq!(numbers(&moved), (Some("Lab"), Some(150.0), Some(4.0), Some("type")));
    assert_eq!(moved["due"], ny("2026-10-23 23:59"));
    assert_eq!(shown["derived"]["tasks"][lab4["id"].as_str().unwrap()]["estimate"]["typeFrom"], "course");
    // The person's 200 minutes are theirs; only the difficulty they never set follows.
    let kept = task(&core, mine["id"].as_str().unwrap());
    assert_eq!((kept["estMin"].clone(), kept["estBy"].clone(), kept["difficulty"].clone()), (json!(200), json!("you"), json!(4)));
    assert_eq!(task(&core, quiz["id"].as_str().unwrap())["estMin"], 20);
    let made = records(&shown, "task");
    let final_exam = by(&made, "title", "Final exam");
    assert_eq!(
        (final_exam["due"].clone(), final_exam["courseId"].clone(), final_exam["source"].clone(), final_exam["claudeReason"].clone()),
        (json!(ny("2026-12-14 08:00")), json!(id), json!("claude"), json!("From the syllabus, ELE209_syllabus.pdf."))
    );

    // One ⌘Z, and all of it is gone: weights, types, tasks, dates, the grade's category.
    ok(&core, "history.undo", json!({"room": "heat"}));
    assert_eq!(docs(&setup.library()), before);
    // Importing the same syllabus twice makes nothing twice.
    ok(&core, "history.redo", json!({"room": "heat"}));
    let again = ok(&core, "heat.course.importSyllabus", json!({"courseId": id, "json": syllabus(40)}))["draft"].clone();
    assert_eq!((again["newTasks"].clone(), again["dateChanges"].clone()), (json!([]), json!([])));
    ok(&core, "heat.syllabus.discard", json!({"draftId": again["id"]}));
    assert_eq!(snap(&core, DAY)["syllabus"]["drafts"], json!([]));
}

#[test]
fn a_syllabus_whose_weights_dont_add_up_says_so_and_one_that_names_no_course_is_refused() {
    let setup = Setup::new();
    let core = classes_core(&setup);
    // Dropped on Grades, not on a course: the syllabus's own code names it, and it is new.
    let draft = ok(&core, "heat.course.importSyllabus", json!({"json": syllabus(35)}))["draft"].clone();
    assert_eq!(draft["weightsFlag"], "Weights add to 95%. The other 5% is unassigned.");
    assert_eq!((draft["course"]["isNew"].clone(), draft["courseId"].clone()), (json!(true), Value::Null));
    assert!(records(&snap(&core, DAY), "course").is_empty(), "nothing is made by a preview");
    ok(&core, "heat.syllabus.accept", json!({"draftId": draft["id"]}));
    let shown = snap(&core, DAY);
    let made = records(&shown, "course");
    assert_eq!((made.len(), made[0]["code"].as_str(), made[0]["status"].as_str()), (1, Some("ELE 209"), Some("confirmed")));
    assert_eq!(records(&shown, "term")[0]["name"], "Fall 2026");
    assert_eq!(
        shown["derived"]["courses"][made[0]["id"].as_str().unwrap()]["weights"],
        "Weights add to 95%. The other 5% is unassigned."
    );
    ok(&core, "history.undo", json!({"room": "heat"}));
    assert!(records(&snap(&core, DAY), "course").is_empty() && records(&snap(&core, DAY), "term").is_empty());

    let mut nameless = syllabus(40);
    nameless["course"]["code"] = Value::Null;
    assert_eq!(
        refused(&core, "heat.course.importSyllabus", json!({"json": nameless})).1,
        "The syllabus doesn't say which course it is. Drop it on the course in Grades."
    );
    let mut invented = syllabus(40);
    invented["weights"][0]["curve"] = json!(true);
    assert!(refused(&core, "heat.course.importSyllabus", json!({"json": invented})).1.starts_with("The syllabus's answer doesn't read"));
    assert_eq!(snap(&core, DAY)["syllabus"]["drafts"], json!([]), "a refused syllabus leaves no draft");
    assert_eq!(refused(&core, "heat.course.importSyllabus", json!({"path": "/tmp/notes.txt"})).1, "Drop the syllabus as a PDF.");
}

#[test]
fn types_come_from_the_home_then_the_space_then_the_defaults_and_never_over_the_persons_own() {
    let setup = Setup::new();
    let core = classes_core(&setup);
    let spaces = records(&snap(&core, DAY), "space");
    let (wwav, personal) = (by(&spaces, "name", "WWAV")["id"].clone(), by(&spaces, "name", "Personal")["id"].clone());
    let (wwav, personal) = (wwav.as_str().unwrap(), personal.as_str().unwrap());

    // No home anywhere: the defaults still answer.
    assert_eq!(numbers(&add_task(&core, wwav, "Mix the second verse", json!({}))), (Some("Creative session"), Some(120.0), Some(3.0), Some("type")));
    assert_eq!(numbers(&add_task(&core, personal, "Reply to Dana about the venue", json!({}))), (Some("Email"), Some(10.0), Some(1.0), Some("type")));
    let form = add_task(&core, personal, "Renew passport", json!({}));
    assert_eq!(numbers(&form), (Some("Admin"), Some(20.0), Some(1.0), Some("type")));
    // A course is never forced onto work that isn't school, whatever its title says.
    course(&core, "MTH 142");
    let errand = add_task(&core, personal, "Buy a calculator for MTH 142", json!({}));
    assert!(errand.get("courseId").is_none());
    assert_eq!(errand["type"], "Errand");

    // A project is a home: a task that leads with its name joins it, and its own types win.
    let project = ok(&core, "heat.put", json!({"kind": "project", "record": {"spaceId": wwav, "title": "Mi-WWAV beta"}}))["record"].clone();
    let pid = project["id"].as_str().unwrap();
    let bug = add_task(&core, wwav, "Mi-WWAV beta: fix sign-in bug", json!({}));
    let fixed = add_task(&core, wwav, "Fix the export bug", json!({"projectId": pid, "estMin": 15}));
    assert_eq!((bug["projectId"].as_str(), bug["type"].as_str(), bug["estBy"].as_str()), (Some(pid), Some("Other"), Some("default")));
    let updated = ok(
        &core,
        "heat.project.update",
        json!({"id": pid, "set": {"types": [{"name": "Bug", "patterns": ["Bug", "fix"], "estMin": 50, "difficulty": 3}]}}),
    );
    assert_eq!((updated["undo"].as_str(), updated["retimed"].as_u64()), (Some("Undo edit project"), Some(2)));
    assert_eq!(updated["record"]["types"][0]["patterns"], json!(["bug", "fix"]));
    let bug = task(&core, bug["id"].as_str().unwrap());
    assert_eq!(numbers(&bug), (Some("Bug"), Some(50.0), Some(3.0), Some("type")));
    let shown = snap(&core, DAY);
    assert_eq!(shown["derived"]["tasks"][bug["id"].as_str().unwrap()]["estimate"]["typeFrom"], "project");
    assert_eq!(shown["derived"]["tasks"][bug["id"].as_str().unwrap()]["home"]["label"], "Mi-WWAV beta");
    assert_eq!(shown["derived"]["types"]["projects"][pid], json!(["Bug"]));
    // The person's 15 minutes stay; the type and the difficulty they never set follow.
    let fixed = task(&core, fixed["id"].as_str().unwrap());
    assert_eq!(numbers(&fixed), (Some("Bug"), Some(15.0), Some(3.0), Some("you")));
    // The same change undone takes the tasks back with it.
    ok(&core, "history.undo", json!({"room": "heat"}));
    assert_eq!(task(&core, bug["id"].as_str().unwrap())["type"], "Other");
    ok(&core, "history.redo", json!({"room": "heat"}));

    // A space's own type sits between the two.
    ok(&core, "heat.patch", json!({"kind": "space", "id": wwav, "set": {"typeDefs": [{"name": "Hardware", "patterns": ["pcb", "solder"], "estMin": 180, "difficulty": 4}]}}));
    assert_eq!(numbers(&add_task(&core, wwav, "Solder the PRANA board", json!({}))), (Some("Hardware"), Some(180.0), Some(4.0), Some("type")));

    // Picking a type by hand: the numbers nobody set follow it; back to automatic, the title picks again.
    let id = form["id"].as_str().unwrap();
    let picked = ok(&core, "heat.task.setType", json!({"taskId": id, "type": "Errand"}));
    assert_eq!((picked["undo"].as_str(), picked["task"]["typeBy"].as_str()), (Some("Undo set type"), Some("you")));
    assert_eq!(numbers(&picked["task"]), (Some("Errand"), Some(30.0), Some(1.0), Some("type")));
    assert_eq!(numbers(&ok(&core, "heat.task.setType", json!({"taskId": id, "type": null}))["task"]), (Some("Admin"), Some(20.0), Some(1.0), Some("type")));
    // The person's estimate and difficulty are flagged as theirs and survive a reapply; force gives them back.
    let est = ok(&core, "heat.estimate", json!({"taskId": id, "estMin": 75, "difficulty": 4}))["task"].clone();
    assert_eq!((est["estBy"].as_str(), est["difficultyBy"].as_str()), (Some("you"), Some("you")));
    assert_eq!(ok(&core, "heat.task.reapplyDefaults", json!({"all": true}))["changed"], 0);
    assert_eq!(task(&core, id)["estMin"], 75);
    let back = ok(&core, "heat.task.reapplyDefaults", json!({"taskId": id, "force": true}));
    assert_eq!((back["changed"].as_u64(), back["undo"].as_str()), (Some(1), Some("Undo reapply defaults")));
    assert_eq!(numbers(&task(&core, id)), (Some("Admin"), Some(20.0), Some(1.0), Some("type")));
    // Clearing an estimate hands it back to the type too.
    ok(&core, "heat.estimate", json!({"taskId": id, "estMin": 75}));
    assert_eq!(ok(&core, "heat.estimate", json!({"taskId": id, "estMin": null}))["task"]["estMin"], 20);
    assert_eq!(refused(&core, "heat.task.reapplyDefaults", json!({})).1, "Say which tasks: one task, a course, a project, a space, or all.");
}

#[test]
fn time_taken_calibrates_the_tasks_that_come_after_and_leaves_the_ones_already_here() {
    let setup = Setup::new();
    let core = classes_core(&setup);
    let space = records(&snap(&core, DAY), "space")[0]["id"].as_str().unwrap().to_string();
    let jpn = course(&core, "JPN 101");
    let id = jpn["id"].as_str().unwrap();
    let waiting = add_task(&core, &space, "Kanji quiz 5", json!({"courseId": id}));
    assert_eq!(waiting["estMin"], 20);
    // Two quizzes that each took twice what a quiz is said to take.
    for title in ["Vocabulary quiz 1", "Vocabulary quiz 2"] {
        let q = add_task(&core, &space, title, json!({"courseId": id}));
        ok(&core, "heat.tookTime", json!({"taskId": q["id"], "minutes": 40}));
        ok(&core, "heat.done", json!({"taskId": q["id"], "done": true, "date": DAY}));
    }
    assert_eq!(add_task(&core, &space, "Vocabulary quiz 3", json!({"courseId": id}))["estMin"], 40, "this course's quizzes take 40");
    // Another course has no quizzes of its own yet: the space's speak for it.
    let ele = ok(&core, "heat.put", json!({"kind": "course", "record": {"termId": "fall", "code": "ELE 201"}}))["record"].clone();
    assert_eq!(add_task(&core, &space, "Quiz 1", json!({"courseId": ele["id"]}))["estMin"], 40);
    // A lab is not a quiz, and the quiz that was already waiting keeps its 20.
    assert_eq!(add_task(&core, &space, "Lab 1", json!({"courseId": id}))["estMin"], 120);
    assert_eq!(task(&core, waiting["id"].as_str().unwrap())["estMin"], 20, "future tasks only");
}

/// A stand-in for the Claude Code command line: it keeps what it was asked
/// and answers with the envelope in `answer.json`.
fn fake_claude(dir: &Path, answer: &Value) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("claude");
    std::fs::write(dir.join("answer.json"), json!({"is_error": false, "result": "", "structured_output": answer}).to_string()).unwrap();
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/args'\ncat > '{d}/prompt'\ncat '{d}/answer.json'\n",
            d = dir.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn core_with_claude(setup: &Setup, claude: &Path, auto_score: bool) -> Core {
    let mut config = setup.config(NO_SERVER, no_browser());
    config.now = Some(ny(NOW));
    config.claude = Some(claude.to_path_buf());
    config.auto_score = auto_score;
    let core = Core::open(&setup.library(), config).unwrap();
    ok(&core, "app.settings.set", json!({"patch": {"heat": {"timeZone": "America/New_York"}}}));
    core
}

/// One page of text as a PDF, written by hand: a catalog, a page, a stream.
fn tiny_pdf(lines: &[&str]) -> Vec<u8> {
    let mut content = String::from("BT /F1 12 Tf 72 740 Td 14 TL\n");
    for l in lines {
        let l = l.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
        content.push_str(&format!("({l}) Tj T*\n"));
    }
    content.push_str("ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (n, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{o}\nendobj\n", n + 1).bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for off in offsets {
        out.extend(format!("{off:010} 00000 n \n").bytes());
    }
    out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).bytes());
    out
}

fn drafts(core: &Core) -> Vec<Value> {
    snap(core, DAY)["syllabus"]["drafts"].as_array().cloned().unwrap_or_default()
}

#[test]
fn a_dropped_pdf_is_read_in_rust_and_claude_is_asked_once_for_the_course() {
    let setup = Setup::new();
    let dir = tempfile::tempdir().unwrap();
    let claude = fake_claude(dir.path(), &syllabus(40));
    let core = core_with_claude(&setup, &claude, false);
    let ele = course(&core, "ELE 209");
    let pdf = dir.path().join("ELE209 Syllabus.PDF");
    std::fs::write(
        &pdf,
        tiny_pdf(&[
            "ELE 209 Introduction to Computer Systems Lab, Fall 2026",
            "Grading: Labs 40% (lowest lab dropped), Quizzes 20%, Final exam 40%.",
            "Lab 4 is due October 23. Lab 5 is due October 30. Lab 6 is due November 6.",
            "The final exam is on December 14 at 8:00 AM in Kelley Hall.",
        ]),
    )
    .unwrap();
    let draft = ok(&core, "heat.course.importSyllabus", json!({"path": pdf, "courseId": ele["id"]}))["draft"].clone();
    assert_eq!((draft["state"].as_str(), draft["fileName"].as_str()), (Some("reading"), Some("ELE209 Syllabus.PDF")));
    assert!(draft.get("path").is_none(), "where the file is stays in the core");
    assert!(
        eventually(T, || drafts(&core).first().is_some_and(|d| d["state"] != "reading")),
        "the draft never left reading"
    );
    let ready = drafts(&core)[0].clone();
    assert_eq!(ready["state"], "ready", "{}", ready["error"]);
    assert_eq!(ready["pages"], 1);
    assert_eq!(
        ready["line"],
        "ELE 209 · Intro to Computer Systems Lab. Labs 40%, Quizzes 20%, Final 40%. 5 types. 5 new tasks."
    );
    // What Claude was sent: the PDF's own text, the course it was dropped on, and no tools at all.
    let prompt = std::fs::read_to_string(dir.path().join("prompt")).unwrap();
    assert!(prompt.contains("Grading: Labs 40% (lowest lab dropped), Quizzes 20%, Final exam 40%."), "{prompt}");
    assert!(prompt.contains("The student dropped it on the course ELE 209."));
    assert!(prompt.contains("Use only what the syllabus says."));
    let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
    assert!(args.contains("--json-schema") && args.contains("sonnet"), "{args}");
    // The course is still a stub until the person accepts.
    assert_eq!(records(&snap(&core, DAY), "course")[0]["status"], "stub");
    assert_eq!(ok(&core, "heat.syllabus.accept", json!({"draftId": ready["id"]}))["counts"]["newTasks"], 5);

    // A PDF with nothing to read says so, and so does an answer that isn't the course asked for.
    let blank = dir.path().join("scan.pdf");
    std::fs::write(&blank, tiny_pdf(&["page 1"])).unwrap();
    ok(&core, "heat.course.importSyllabus", json!({"path": blank}));
    assert!(eventually(T, || drafts(&core).first().is_some_and(|d| d["state"] == "failed")));
    let failed = drafts(&core)[0].clone();
    assert!(failed["error"].as_str().unwrap().starts_with("That PDF has no text to read."));
    ok(&core, "heat.syllabus.discard", json!({"draftId": failed["id"]}));
    assert!(drafts(&core).is_empty());
}

#[test]
fn claude_scores_what_no_type_matched_in_one_batch_when_asked_and_never_on_its_own() {
    let setup = Setup::new();
    let dir = tempfile::tempdir().unwrap();
    let claude = fake_claude(
        dir.path(),
        &json!({"scores": [
            {"n": 1, "estimate_min": 25, "difficulty": 2, "reason": "Two words to find and post."},
            {"n": 2, "estimate_min": 9000, "difficulty": 9, "reason": "A clinic visit."},
            {"n": 3, "estimate_min": 30, "difficulty": 1, "reason": "This one has a type."}
        ]}),
    );
    let core = core_with_claude(&setup, &claude, false);
    let spaces = records(&snap(&core, DAY), "space");
    let (classes, personal) = (spaces[0]["id"].as_str().unwrap(), by(&spaces, "name", "Personal")["id"].as_str().unwrap().to_string());
    let hunt = add_task(&core, classes, "Asynchronous Task (1%) = October 12th class", json!({"due": ny("2026-10-12 23:59")}));
    let shots = add_task(&core, &personal, "Finish outstanding immunization requirements", json!({}));
    let quiz = add_task(&core, classes, "Grammar quiz 4", json!({}));
    assert_eq!(snap(&core, DAY)["derived"]["unscored"], 2);
    // Nothing starts Claude until it is asked.
    std::thread::sleep(Duration::from_millis(400));
    assert!(!dir.path().join("prompt").exists(), "Claude was started without being asked");

    let asked = ok(&core, "heat.tasks.score", json!({}));
    assert_eq!((asked["asked"].as_u64(), asked["line"].as_str()), (Some(2), Some("Asking Claude to estimate 2 tasks.")));
    assert!(eventually(T, || snap(&core, DAY)["derived"]["unscored"] == 0), "the scores never arrived");
    let prompt = std::fs::read_to_string(dir.path().join("prompt")).unwrap();
    assert!(prompt.contains("1. Asynchronous Task (1%) = October 12th class [type: Other; space: Classes]"), "{prompt}");
    assert!(prompt.contains("2. Finish outstanding immunization requirements [type: Other; space: Personal]"));
    assert!(!prompt.contains("Grammar quiz 4"), "a task with a type isn't sent");
    assert!(std::fs::read_to_string(dir.path().join("args")).unwrap().contains("haiku"));

    let hunt = task(&core, hunt["id"].as_str().unwrap());
    assert_eq!(
        (hunt["estMin"].clone(), hunt["difficulty"].clone(), hunt["estBy"].clone(), hunt["difficultyBy"].clone(), hunt["estReason"].clone()),
        (json!(25), json!(2), json!("claude"), json!("claude"), json!("Two words to find and post."))
    );
    // Out of range is kept in range; a score for a task that wasn't in the batch lands nowhere.
    let shots = task(&core, shots["id"].as_str().unwrap());
    assert_eq!((shots["estMin"].clone(), shots["difficulty"].clone()), (json!(600), json!(5)));
    assert_eq!(numbers(&task(&core, quiz["id"].as_str().unwrap())), (Some("Quiz"), Some(20.0), Some(2.0), Some("type")));
    // One labelled entry of Claude's, in Settings → Claude, undone in one step.
    let recent = ok(&core, "heat.claude.get", json!({}))["recent"].clone();
    assert_eq!((recent[0]["label"].as_str(), recent.as_array().unwrap().len()), (Some("Claude's estimates"), 1));
    ok(&core, "history.undo", json!({"room": "heat"}));
    assert_eq!(snap(&core, DAY)["derived"]["unscored"], 2);
    assert_eq!(ok(&core, "heat.tasks.score", json!({}))["asked"], 2, "asking by hand asks again");
    assert!(eventually(T, || snap(&core, DAY)["derived"]["unscored"] == 0));
    assert_eq!(ok(&core, "heat.tasks.score", json!({}))["line"], "Every task has a type or an estimate.");
}

#[test]
fn mail_makes_tasks_in_any_space_and_only_school_mail_gets_a_course() {
    let setup = Setup::new();
    let core = classes_core(&setup);
    snap(&core, DAY);
    let events = core.events();
    let mut helper = McpHelper::start(core.library());
    let record = |helper: &mut McpHelper, id: &str, subject: &str, course: Option<&str>, category: &str| {
        let mut args = json!({
            "thread_id": id, "subject": subject, "from": "noreply@uri.brightspace.com",
            "received_at": "2026-10-07T06:01:55-04:00", "state": "nothing", "category": category, "reason": "r",
        });
        if let Some(c) = course {
            args["course"] = json!(c);
        }
        helper.call("record_mail_thread", args).unwrap();
    };
    // Recording the thread is enough for Grades to list its course, under its real name.
    record(&mut helper, "g1", "Activity summary for EGR101: Intro to Engineering Design_R01_FALL26 on Oct 7, 2026", Some("EGR 101"), "school");
    record(&mut helper, "g2", "ELE209: Intro. to Computer Systems Lab_0001_FALL26 - Announcements: Quiz 3 moved", Some("ele209"), "school");
    record(&mut helper, "g3", "Your order of MTH 142 graph paper has shipped", Some("MTH 142"), "money");
    wait_event(&events, "heat", T);
    assert!(eventually(T, || records(&snap(&core, DAY), "course").len() == 2));
    let shown = snap(&core, DAY);
    let courses = records(&shown, "course");
    let ele = by(&courses, "code", "ELE 209");
    assert_eq!((ele["name"].as_str(), ele["status"].as_str()), (Some("Intro. to Computer Systems Lab"), Some("stub")));
    assert_eq!(by(&courses, "code", "EGR 101")["name"], "Intro to Engineering Design");
    assert_eq!(records(&shown, "term")[0]["name"], "Fall 2026");

    // School mail: the task lands in Classes, in the thread's course, typed and timed.
    let quiz = helper
        .call("add_task", json!({"title": "Study for Quiz 3", "mail_thread_id": "g2", "source_id": "msg-2", "reason": "The quiz moved to Friday."}))
        .unwrap();
    assert_eq!(
        (quiz["task"]["course"].as_str(), quiz["task"]["home"].as_str(), quiz["task"]["type"].as_str(), quiz["task"]["estimate_min"].as_i64()),
        (Some("ELE 209"), Some("ELE 209 · Intro. to Computer Systems Lab"), Some("Quiz"), Some(20))
    );
    // Work and personal mail land in their own space with a type, and never in a course,
    // even when the thread carries a course's code.
    let reply = helper
        .call("add_task", json!({"title": "Reply to the pressing plant about test lacquers", "space": "WWAV", "source_id": "msg-3", "reason": "They asked for a date."}))
        .unwrap();
    assert_eq!(
        (reply["task"]["space"].as_str(), reply["task"]["type"].as_str(), reply["task"]["estimate_min"].as_i64(), reply["task"].get("course")),
        (Some("WWAV"), Some("Email"), Some(10), None)
    );
    let paper = helper
        .call("add_task", json!({"title": "Pick up the graph paper", "space": "Personal", "mail_thread_id": "g3", "source_id": "msg-4", "reason": "It shipped."}))
        .unwrap();
    assert_eq!(
        (paper["task"]["type"].as_str(), paper["task"]["estimate_min"].as_i64(), paper["task"].get("course"), paper["task"].get("home")),
        (Some("Errand"), Some(30), None, None)
    );
    // Claude's own difficulty is marked as Claude's, so a type doesn't take it back.
    let id = paper["task"]["id"].as_str().unwrap();
    helper.call("update_task", json!({"id": id, "difficulty": 3, "reason": "It is across town."})).unwrap();
    assert!(eventually(T, || task(&core, id)["difficultyBy"] == "claude"));
    ok(&core, "heat.task.reapplyDefaults", json!({"all": true}));
    assert_eq!(task(&core, id)["difficulty"], 3);
}
