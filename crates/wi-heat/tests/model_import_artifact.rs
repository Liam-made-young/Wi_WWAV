//! `importArtifact.test.ts`, case for case.

mod common;

use common::*;
use serde_json::{json, Value};
use wi_heat::model::import_artifact::{
    import_artifact, parse_heat_export, Existing, ExportError, HeatExport,
};
use wi_heat::model::records::{GroupKind, TaskSource};
use wi_heat::model::spaces::default_spaces;

fn dump_json() -> Value {
    json!({
        "format": "heat-export",
        "version": 1,
        "exportedAt": "2026-10-06T12:40:00.000Z",
        "timeZone": "America/New_York",
        "workspaces": [
            {"key": "classes", "name": "Classes", "groupLabel": "Course", "types": ["Homework", "Quiz", "Other"], "persona": "a student"},
            {"key": "wwav", "name": "WWAV", "groupLabel": "Milestone", "types": ["Hardware", "Other"], "persona": "a founder"},
            {"key": "personal", "name": "Personal", "groupLabel": "Area", "types": ["Errand", "Other"], "persona": "a person"},
        ],
        "tasks": [
            {
                "id": "5h3k2j1abcdefg_20261008T035900Z", "workspace": "classes", "title": "Grammar quiz 4",
                "group": "JPN 201", "type": "Quiz", "due": "2026-10-08T03:59:00.000Z", "difficulty": 2,
                "estMin": 45, "actualMin": null, "notes": "", "done": false, "doneAt": null, "source": "calendar",
            },
            {
                "id": "em-3f2a9c71", "workspace": "classes", "title": "Read chapter 3", "group": "MTH 142",
                "type": "Reading", "due": "2026-10-10T03:59:00.000Z", "difficulty": 3, "estMin": null,
                "actualMin": 75, "notes": "From mail: …", "done": true, "doneAt": "2026-10-05T20:00:00.000Z",
                "source": "gmail",
            },
            {
                "id": "t-1001", "workspace": "wwav", "title": "Route the PCB", "group": "Enclosure v2",
                "type": "Hardware", "due": null, "difficulty": 4, "estMin": 120, "actualMin": null, "notes": "",
                "done": false, "doneAt": null, "source": "manual",
            },
            {
                "id": "t-1002", "workspace": "personal", "title": "Renew registration", "group": "Car",
                "type": "Errand", "due": null, "difficulty": 1, "estMin": null, "actualMin": null, "notes": "",
                "done": false, "doneAt": null, "source": "manual",
            },
        ],
        "milestones": [
            {"id": "ms-7", "workspace": "wwav", "title": "Enclosure v2", "date": "2026-10-20", "done": false, "order": 1},
        ],
        "habits": [
            {"id": "hb-1", "title": "Practise kanji", "log": {"2026-10-05": true, "2026-10-06": true}},
        ],
        "term": "Fall 2026",
        "courses": [
            {
                "code": "JPN 201", "name": "Intermediate Japanese",
                "categories": [
                    {"name": "Quizzes", "weight": 40, "keywords": ["quiz", "kanji"]},
                    {"name": "Exit tickets", "weight": 60, "keywords": ["exit ticket"]},
                ],
                "sticky": "Office hours Tue 2 PM",
            },
        ],
        "grades": [
            {
                "id": "gr-31", "course": "JPN 201", "title": "Kanji quiz 6", "category": "Quizzes", "score": 18,
                "outOf": 20, "dropped": false, "pending": false,
            },
            {
                "id": "gp-91be2c", "course": "JPN 201", "title": "Exit Ticket 12", "category": null, "score": null,
                "outOf": 10, "dropped": false, "pending": true, "link": "https://brightspace.uri.edu/d2l/home",
            },
        ],
        "processedMailIds": ["18f2a", "18f2b", "18f2c"],
        "lastSyncAt": "2026-10-06T12:41:00.000Z",
    })
}

fn dump() -> HeatExport {
    serde_json::from_value(dump_json()).expect("the dump is a Learn export")
}

fn utc(text: &str) -> f64 {
    text.parse::<jiff::Timestamp>()
        .expect("an instant")
        .as_millisecond() as f64
}

fn not_export() -> Result<HeatExport, ExportError> {
    Err(ExportError {
        error: "This file isn’t a Learn export.".into(),
    })
}

// --- parsing the export ----------------------------------------------------------------------------

#[test]
fn takes_a_heat_export_and_refuses_anything_else_in_one_line() {
    let parsed = parse_heat_export(&dump_json()).expect("a Learn export");
    assert_eq!(serde_json::to_value(&parsed).unwrap(), dump_json());
    assert_eq!(parse_heat_export(&json!({"tasks": []})), not_export());
    assert_eq!(parse_heat_export(&Value::Null), not_export());
    let mut stray = dump_json();
    stray["tasks"][0]["workspace"] = json!("nowhere");
    assert_eq!(parse_heat_export(&stray), not_export());
    let mut newer = dump_json();
    newer["version"] = json!(2);
    assert_eq!(
        parse_heat_export(&newer),
        Err(ExportError {
            error: "This export is from a newer Learn. Update Wi_WWAV, then try again.".into()
        })
    );
}

// --- moving in (3.15) ----------------------------------------------------------------------------------

#[test]
fn keeps_every_id_event_ids_em_and_gp_hashes_and_the_processed_gmail_ids() {
    let result = import_artifact(&dump(), &Existing::default(), &mut ids("new"));
    assert_eq!(
        result
            .tasks
            .iter()
            .map(|t| t.id.as_str())
            .collect::<Vec<_>>(),
        [
            "5h3k2j1abcdefg_20261008T035900Z",
            "em-3f2a9c71",
            "t-1001",
            "t-1002"
        ]
    );
    assert_eq!(
        result
            .grades
            .iter()
            .map(|g| g.id.as_str())
            .collect::<Vec<_>>(),
        ["gr-31", "gp-91be2c"]
    );
    assert_eq!(
        result
            .milestones
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        ["ms-7"]
    );
    assert_eq!(
        result
            .habits
            .iter()
            .map(|h| h.id.as_str())
            .collect::<Vec<_>>(),
        ["hb-1"]
    );
    assert_eq!(result.sync.processed_mail_ids, ["18f2a", "18f2b", "18f2c"]);
    assert_eq!(
        result.sync.last_sync_at,
        Some(utc("2026-10-06T12:41:00.000Z"))
    );
}

#[test]
fn maps_each_workspace_to_a_space_with_its_group_kind() {
    let result = import_artifact(&dump(), &Existing::default(), &mut ids("new"));
    let got: Vec<(&str, GroupKind, &str, Vec<&str>, &str)> = result
        .spaces
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.group_kind,
                s.group_label.as_str(),
                s.types.iter().map(String::as_str).collect(),
                s.persona.as_str(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (
                "Classes",
                GroupKind::Course,
                "Course",
                vec!["Homework", "Quiz", "Other"],
                "a student"
            ),
            (
                "WWAV",
                GroupKind::Milestone,
                "Milestone",
                vec!["Hardware", "Other"],
                "a founder"
            ),
            (
                "Personal",
                GroupKind::Free,
                "Area",
                vec!["Errand", "Other"],
                "a person"
            ),
        ]
    );
    let (classes, wwav, personal) = (&result.spaces[0], &result.spaces[1], &result.spaces[2]);
    assert_eq!(
        result
            .tasks
            .iter()
            .map(|t| t.space_id.as_str())
            .collect::<Vec<_>>(),
        [
            classes.id.as_str(),
            classes.id.as_str(),
            wwav.id.as_str(),
            personal.id.as_str()
        ]
    );
    assert_eq!(result.milestones[0].space_id, wwav.id);
}

#[test]
fn turns_groups_into_course_milestone_and_area_links() {
    let result = import_artifact(&dump(), &Existing::default(), &mut ids("new"));
    let (quiz, reading, pcb, errand) = (
        &result.tasks[0],
        &result.tasks[1],
        &result.tasks[2],
        &result.tasks[3],
    );
    let jpn = result.courses.iter().find(|c| c.code == "JPN 201").unwrap();
    let mth = result.courses.iter().find(|c| c.code == "MTH 142").unwrap();
    assert_eq!(quiz.course_id.as_deref(), Some(jpn.id.as_str()));
    // A course only a task names is made once, so it is defined once.
    assert_eq!(reading.course_id.as_deref(), Some(mth.id.as_str()));
    assert_eq!(mth.name, "MTH 142");
    assert_eq!(mth.categories, vec![]);
    assert_eq!(pcb.milestone_id.as_deref(), Some("ms-7"));
    assert_eq!(errand.group.as_deref(), Some("Car"));
}

#[test]
fn carries_the_task_fields_over_with_typed_time_as_the_hand_adjustment() {
    let result = import_artifact(&dump(), &Existing::default(), &mut ids("new"));
    let (quiz, reading) = (&result.tasks[0], &result.tasks[1]);
    assert_eq!(
        serde_json::to_value(quiz).unwrap(),
        json!({
            "id": "5h3k2j1abcdefg_20261008T035900Z",
            "spaceId": result.spaces[0].id,
            "title": "Grammar quiz 4",
            "type": "Quiz",
            "courseId": result.courses[0].id,
            "due": ny("2026-10-07 23:59") as i64,
            "difficulty": 2,
            "estMin": 45,
            "adjustMin": 0,
            "notes": "",
            "done": false,
            "doneAt": null,
            "source": "calendar",
        })
    );
    assert_eq!(reading.adjust_min, 75.0);
    assert!(reading.done);
    assert_eq!(reading.done_at, Some(utc("2026-10-05T20:00:00.000Z")));
    assert_eq!(reading.source, TaskSource::Mail);
}

#[test]
fn brings_habits_with_their_whole_log_and_the_counter_off() {
    let result = import_artifact(&dump(), &Existing::default(), &mut ids("new"));
    assert_eq!(
        serde_json::to_value(&result.habits[0]).unwrap(),
        json!({
            "id": "hb-1",
            "title": "Practise kanji",
            "log": {"2026-10-05": true, "2026-10-06": true},
            "showCounter": false,
        })
    );
}

#[test]
fn brings_the_term_the_courses_with_their_categories_and_stickies_and_the_grades() {
    let result = import_artifact(&dump(), &Existing::default(), &mut ids("new"));
    assert_eq!(
        serde_json::to_value(&result.terms).unwrap(),
        json!([{"id": result.courses[0].term_id, "name": "Fall 2026"}])
    );
    let jpn = &result.courses[0];
    let cats: Vec<(&str, f64, Vec<&str>)> = jpn
        .categories
        .iter()
        .map(|c| {
            (
                c.name.as_str(),
                c.weight,
                c.keywords.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        cats,
        vec![
            ("Quizzes", 40.0, vec!["quiz", "kanji"]),
            ("Exit tickets", 60.0, vec!["exit ticket"])
        ]
    );
    assert_eq!(jpn.notes, "Office hours Tue 2 PM");
    let first = &result.grades[0];
    assert_eq!(first.course_id, jpn.id);
    assert_eq!(
        first.category_id.as_deref(),
        Some(jpn.categories[0].id.as_str())
    );
    assert_eq!((first.score, first.out_of), (Some(18.0), 20.0));
    assert_eq!(serde_json::to_value(first.source).unwrap(), json!("you"));
    let second = &result.grades[1];
    assert_eq!(second.category_id, None);
    assert_eq!(second.score, None);
    assert!(second.pending);
    assert_eq!(serde_json::to_value(second.source).unwrap(), json!("mail"));
}

#[test]
fn moves_into_spaces_that_already_exist_by_name_keeping_their_ids() {
    let existing = Existing {
        spaces: Some(default_spaces(&mut ids("space"))),
        ..Existing::default()
    };
    let again = import_artifact(&dump(), &existing, &mut ids("new"));
    assert_eq!(again.spaces, vec![]);
    assert_eq!(
        again
            .tasks
            .iter()
            .map(|t| t.space_id.as_str())
            .collect::<Vec<_>>(),
        ["space-1", "space-1", "space-2", "space-3"]
    );
}
