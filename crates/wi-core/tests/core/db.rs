//! The Database tab's commands through the core (docs/ASK.md). What a fail
//! looks like:
//! - a kind of record Learn keeps is not a table, or a field is not a column;
//! - an edit in the grid changes something other than the real record, skips
//!   one of Learn's rules, or a paste takes more than one ⌘Z to undo;
//! - a locked cell can be typed over;
//! - a formula, a pivot, a chart or a saved view is wrong, or is gone after
//!   the app is opened again;
//! - a CSV file doesn't come back as it went out;
//! - any of it leaves this Mac.

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

fn space(core: &Core) -> String {
    records(&snap(core, "2026-10-07"), "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn undo_label(core: &Core) -> Value {
    ok(core, "history.get", json!({"room": "heat"}))["undo"].clone()
}

fn undo(core: &Core) -> Value {
    ok(core, "history.undo", json!({"room": "heat"}))["label"].clone()
}

/// A term, two courses, four tasks and three focus sessions.
struct World {
    jpn: String,
    phy: String,
    kanji: String,
    lab: String,
    essay: String,
    laundry: String,
}

fn world(core: &Core) -> World {
    let space = space(core);
    let term = ok(
        core,
        "heat.put",
        json!({"kind": "term", "record": {"name": "Fall 2026"}}),
    )["record"]["id"]
        .as_str()
        .unwrap()
        .to_string();
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
    let kanji = task(
        "Kanji quiz",
        json!({"courseId": jpn, "due": ny("2026-10-09 23:59"), "estMin": 45}),
    );
    let lab = task(
        "Lab report",
        json!({"courseId": phy, "due": ny("2026-10-06 17:00"), "estMin": 120}),
    );
    let essay = task(
        "Essay draft",
        json!({"courseId": jpn, "due": ny("2026-10-12 23:59"), "estMin": 90}),
    );
    let laundry = task("Laundry", json!({}));
    for (task, at, minutes) in [
        (&kanji, "2026-10-05 10:00", 25),
        (&kanji, "2026-10-06 10:00", 50),
        (&lab, "2026-10-06 14:00", 25),
        (&essay, "2026-09-28 10:00", 25),
    ] {
        ok(
            core,
            "heat.put",
            json!({"kind": "focusSession", "record": {
                "taskId": task, "startedAt": ny(at), "endedAt": ny(at) + minutes as f64 * 60_000.0,
                "focusMin": minutes, "interruptions": 0, "room": "heat",
            }}),
        );
    }
    World {
        jpn,
        phy,
        kanji,
        lab,
        essay,
        laundry,
    }
}

fn query(core: &Core, table: &str, spec: Value) -> Value {
    ok(core, "db.query", json!({"table": table, "spec": spec}))
}

/// The column's place in a query's answer, by id or by name.
fn col(q: &Value, name: &str) -> usize {
    q["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c["id"] == name || c["name"] == name)
        .unwrap_or_else(|| panic!("no column {name}"))
}

/// One column's cells, top to bottom.
fn cells(q: &Value, name: &str) -> Vec<Value> {
    let c = col(q, name);
    q["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["cells"][c].clone())
        .collect()
}

fn cell(q: &Value, row: &str, name: &str) -> Value {
    let c = col(q, name);
    q["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == row)
        .unwrap_or_else(|| panic!("no row {row}"))["cells"][c]
        .clone()
}

fn set(core: &Core, table: &str, row: &str, column: &str, value: Value) -> Value {
    ok(
        core,
        "db.cells.set",
        json!({"table": table, "edits": [{"row": row, "column": column, "value": value}]}),
    )
}

#[test]
fn every_kind_learn_keeps_is_a_table() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let w = world(&core);
    let t = ok(&core, "db.tables", json!({}));
    let count = |id: &str| {
        t["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == id)
            .unwrap_or_else(|| panic!("no table {id}"))["count"]
            .clone()
    };
    assert_eq!(count("task"), 4);
    assert_eq!(count("course"), 2);
    assert_eq!(count("focusSession"), 4);
    assert_eq!(count("space"), 3);
    for id in [
        "grade",
        "habit",
        "habitLog",
        "mailThread",
        "project",
        "milestone",
        "note",
        "term",
        "dailyNote",
        "timeBlock",
        "capture",
        "calendar",
        "gradeCategory",
    ] {
        count(id);
    }
    assert_eq!(t["tables"][0]["name"], "Tasks");
    assert!(t["functions"].as_array().unwrap().len() > 80);

    // Every field of the record is a column, and a relation shows the row it points at.
    let q = query(&core, "Tasks", json!({}));
    assert_eq!(q["total"], 4);
    assert_eq!(cell(&q, &w.kanji, "title"), "Kanji quiz");
    assert_eq!(
        cell(&q, &w.kanji, "Course"),
        json!({"id": w.jpn, "label": "JPN 101"})
    );
    assert_eq!(cell(&q, &w.kanji, "due"), "2026-10-09 23:59");
    assert_eq!(cell(&q, &w.kanji, "Estimate (min)"), 45);
    assert_eq!(cell(&q, &w.kanji, "done"), false);
    assert_eq!(cell(&q, &w.laundry, "Course"), Value::Null);
    // What Learn works out is there, and locked.
    assert_eq!(cell(&q, &w.kanji, "Logged (min)"), 75);
    let heat = &q["columns"][col(&q, "Heat")];
    assert_eq!(heat["locked"], true);
    assert!(heat["why"].as_str().unwrap().contains("works this out"));
    assert_eq!(q["columns"][col(&q, "title")]["locked"], false);
    assert_eq!(q["columns"][col(&q, "id")]["locked"], true);
    // A table by its id, its name, or its plural.
    for name in ["task", "tasks", "TASKS", "Tasks"] {
        assert_eq!(
            query(&core, name, json!({}))["table"]["id"],
            "task",
            "{name}"
        );
    }
    let (code, said) = refused(&core, "db.query", json!({"table": "Nowhere"}));
    assert_eq!(
        (code.as_str(), said.as_str()),
        ("refused", "There is no table called 'Nowhere'.")
    );
}

#[test]
fn an_edit_in_the_grid_changes_the_real_task_and_undoes() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let w = world(&core);
    let r = set(&core, "task", &w.kanji, "due", json!("2026-10-08"));
    assert_eq!(r["changed"], 1);
    assert_eq!(r["failed"], json!([]));
    assert_eq!(r["undo"], "Undo edit task");
    // The real record changed: a due date with no time is due at 11:59 PM.
    let task = ok(&core, "records.get", json!({"kind": "task", "id": w.kanji}))["record"].clone();
    assert_eq!(task["due"].as_f64().unwrap(), ny("2026-10-08 23:59"));
    assert_eq!(undo_label(&core), "Undo edit task");
    assert_eq!(undo(&core), "edit task");
    let task = ok(&core, "records.get", json!({"kind": "task", "id": w.kanji}))["record"].clone();
    assert_eq!(task["due"].as_f64().unwrap(), ny("2026-10-09 23:59"));

    // A time can be typed; a title renames; a course is picked by its code.
    assert_eq!(
        set(
            &core,
            "task",
            &w.kanji,
            "due",
            json!("Oct 10, 2026 5:00 PM")
        )["failed"],
        json!([])
    );
    assert_eq!(
        set(&core, "task", &w.kanji, "title", json!("Kanji quiz 4"))["undo"],
        "Undo rename task"
    );
    set(&core, "task", &w.laundry, "Course", json!("phy 204"));
    let q = query(&core, "task", json!({}));
    assert_eq!(cell(&q, &w.kanji, "due"), "2026-10-10 17:00");
    assert_eq!(cell(&q, &w.kanji, "title"), "Kanji quiz 4");
    assert_eq!(
        cell(&q, &w.laundry, "Course"),
        json!({"id": w.phy, "label": "PHY 204"})
    );
    // Emptying a cell clears the field.
    set(&core, "task", &w.laundry, "Course", json!(""));
    assert_eq!(
        cell(&query(&core, "task", json!({})), &w.laundry, "Course"),
        Value::Null
    );
    // Checking Done is the task's own command: it stamps the time.
    assert_eq!(
        set(&core, "task", &w.lab, "done", json!(true))["undo"],
        "Undo mark done"
    );
    let lab = ok(&core, "records.get", json!({"kind": "task", "id": w.lab}))["record"].clone();
    assert_eq!(lab["done"], true);
    assert!(lab["doneAt"].is_number());
}

#[test]
fn learns_rules_hold_in_the_grid_and_locked_cells_stay() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let w = world(&core);
    let fail = |column: &str, value: Value| {
        let r = set(&core, "task", &w.kanji, column, value);
        assert_eq!(r["changed"], 0, "{column}");
        assert_eq!(r["undo"], Value::Null, "{column}");
        r["failed"][0]["message"].as_str().unwrap().to_string()
    };
    assert_eq!(fail("title", json!("")), "Give the task a name first.");
    assert_eq!(
        fail("Estimate (min)", json!("soon")),
        "'soon' isn't a number."
    );
    assert!(fail("due", json!("whenever")).starts_with("'whenever' isn't a date."));
    assert_eq!(
        fail("Course", json!("CHM 101")),
        "No course is called 'CHM 101'."
    );
    assert_eq!(
        fail("Heat", json!(1)),
        "Learn works this out, so it can't be typed over."
    );
    assert_eq!(fail("id", json!("x")), "A record's own id never changes.");
    assert_eq!(fail("source", json!("claude")), "Learn keeps this itself.");
    assert_eq!(fail("Nope", json!(1)), "Tasks has no column called 'Nope'.");
    // Minutes are clamped as everywhere else in Learn.
    set(&core, "task", &w.kanji, "estMin", json!(9000));
    assert_eq!(
        cell(&query(&core, "task", json!({})), &w.kanji, "estMin"),
        600
    );
    // Nothing reaches a table nobody edits.
    let (_, said) = refused(
        &core,
        "db.rows.add",
        json!({"table": "Mail", "rows": [{"subject": "x"}]}),
    );
    assert!(
        said.contains("Mail lists only what Claude recorded"),
        "{said}"
    );
    let (_, said) = refused(
        &core,
        "db.rows.delete",
        json!({"table": "Habit log", "rows": ["x"]}),
    );
    assert!(said.contains("ticked in Habits"), "{said}");
}

#[test]
fn a_paste_is_one_undo_and_says_what_it_couldnt_do() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let w = world(&core);
    let before = undo_label(&core);
    let r = ok(
        &core,
        "db.cells.set",
        json!({"table": "task", "label": "paste", "edits": [
            {"row": w.kanji, "column": "estMin", "value": "30"},
            {"row": w.kanji, "column": "notes", "value": "Chapters 3 and 4"},
            {"row": w.lab, "column": "estMin", "value": "60"},
            {"row": w.essay, "column": "estMin", "value": "not a number"},
            {"row": w.essay, "column": "scheduledDate", "value": "2026-10-10"},
            {"row": w.laundry, "column": "Heat", "value": "9"},
        ]}),
    );
    assert_eq!(r["changed"], 4);
    assert_eq!(r["undo"], "Undo paste");
    let failed: Vec<&str> = r["failed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["column"].as_str().unwrap())
        .collect();
    assert_eq!(failed, ["estMin", "Heat"]);
    let q = query(&core, "task", json!({}));
    assert_eq!(cell(&q, &w.kanji, "estMin"), 30);
    assert_eq!(cell(&q, &w.kanji, "notes"), "Chapters 3 and 4");
    assert_eq!(cell(&q, &w.lab, "estMin"), 60);
    assert_eq!(cell(&q, &w.essay, "Scheduled"), "2026-10-10");

    // One ⌘Z takes all of it back, and the next reaches what came before.
    assert_eq!(undo(&core), "paste");
    let q = query(&core, "task", json!({}));
    assert_eq!(cell(&q, &w.kanji, "estMin"), 45);
    assert_eq!(cell(&q, &w.kanji, "notes"), "");
    assert_eq!(cell(&q, &w.lab, "estMin"), 120);
    assert_eq!(cell(&q, &w.essay, "Scheduled"), Value::Null);
    assert_eq!(undo_label(&core), before);
    // And one redo puts all of it back.
    assert_eq!(
        ok(&core, "history.redo", json!({"room": "heat"}))["label"],
        "paste"
    );
    assert_eq!(cell(&query(&core, "task", json!({})), &w.lab, "estMin"), 60);
}

#[test]
fn filters_sorts_groups_and_the_summary_row() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let w = world(&core);
    let titles = |spec: Value| cells(&query(&core, "task", spec), "title");
    assert_eq!(
        titles(json!({"sorts": [{"column": "due", "dir": "asc"}]})),
        ["Lab report", "Kanji quiz", "Essay draft", "Laundry"],
        "blanks sort last"
    );
    assert_eq!(
        titles(json!({"sorts": [{"column": "due", "dir": "desc"}]})),
        ["Essay draft", "Kanji quiz", "Lab report", "Laundry"]
    );
    assert_eq!(
        titles(json!({"sorts": [{"column": "Course"}, {"column": "estMin", "dir": "desc"}]})),
        ["Essay draft", "Kanji quiz", "Lab report", "Laundry"]
    );
    assert_eq!(
        titles(json!({"filters": [{"column": "Course", "op": "eq", "value": "JPN 101"}]})).len(),
        2
    );
    assert_eq!(
        titles(json!({"filters": [{"column": "courseId", "op": "eq", "value": w.jpn}]})).len(),
        2
    );
    assert_eq!(
        titles(json!({"filters": [{"column": "due", "op": "lt", "value": "2026-10-07"}]})),
        ["Lab report"]
    );
    assert_eq!(
        titles(json!({"filters": [{"column": "due", "op": "eq", "value": "2026-10-09"}]})),
        ["Kanji quiz"]
    );
    assert_eq!(
        titles(json!({"filters": [{"column": "due", "op": "blank"}]})),
        ["Laundry"]
    );
    assert_eq!(
        titles(json!({"filters": [{"column": "estMin", "op": "ge", "value": "90"}]})).len(),
        2
    );
    assert_eq!(
        titles(json!({"filters": [{"column": "title", "op": "contains", "value": "QUIZ"}]})),
        ["Kanji quiz"]
    );
    assert_eq!(
        titles(json!({"filters": [{"column": "done", "op": "is", "value": false}]})).len(),
        4
    );
    assert_eq!(titles(json!({"search": "physics"})).len(), 0);
    assert_eq!(titles(json!({"search": "phy 204"})), ["Lab report"]);
    assert_eq!(
        titles(json!({"match": "any", "filters": [
            {"column": "title", "op": "starts", "value": "lab"},
            {"column": "title", "op": "ends", "value": "draft"},
        ]})),
        ["Lab report", "Essay draft"]
    );
    assert_eq!(
        titles(
            json!({"filters": [{"column": "Course", "op": "in", "value": ["PHY 204", "nothing"]}]})
        ),
        ["Lab report"]
    );
    let (_, said) = refused(
        &core,
        "db.query",
        json!({"table": "task", "spec": {"filters": [{"column": "Nope", "op": "eq", "value": 1}]}}),
    );
    assert_eq!(said, "Tasks has no column called 'Nope'.");

    let q = query(&core, "task", json!({"group": "Course"}));
    let groups: Vec<(String, u64)> = q["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            (
                g["label"].as_str().unwrap().to_string(),
                g["count"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        groups,
        [("JPN 101".into(), 2), ("PHY 204".into(), 1), ("".into(), 1)]
    );

    let s = &q["summary"]["estMin"];
    assert_eq!(
        (
            s["sum"].clone(),
            s["average"].clone(),
            s["count"].clone(),
            s["filled"].clone()
        ),
        (json!(255), json!(85), json!(4), json!(3))
    );
    assert_eq!(
        (s["min"].clone(), s["max"].clone(), s["median"].clone()),
        (json!(45), json!(120), json!(90))
    );
    assert_eq!(q["summary"]["due"]["min"], "2026-10-06 17:00");
    assert_eq!(q["summary"]["due"]["sum"], Value::Null);
    assert_eq!(q["summary"]["courseId"]["unique"], 2);
    // Paging, for Claude's reads.
    let page = ok(
        &core,
        "db.query",
        json!({"table": "task", "limit": 2, "offset": 1, "spec": {"sorts": [{"column": "title"}]}}),
    );
    assert_eq!(cells(&page, "title"), ["Kanji quiz", "Lab report"]);
    assert_eq!(page["total"], 4);
}

#[test]
fn a_formula_column_reads_this_row_and_other_tables() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let w = world(&core);
    // On Tasks: days until due, and a lookup into Courses.
    let added = ok(
        &core,
        "db.column.add",
        json!({
            "table": "Tasks", "name": "Days left", "type": "formula",
            "formula": "IF(ISBLANK([Due]), \"\", INT([Due]) - TODAY())",
        }),
    );
    assert_eq!(added["undo"], "Undo add column");
    ok(
        &core,
        "db.column.add",
        json!({
            "table": "Tasks", "name": "Course name", "type": "formula",
            "formula": "LOOKUP([Course], Courses[Code], Courses[Name], \"None\")",
        }),
    );
    let q = query(&core, "task", json!({}));
    assert_eq!(cell(&q, &w.kanji, "Days left"), 2);
    assert_eq!(cell(&q, &w.lab, "Days left"), -1);
    assert_eq!(cell(&q, &w.laundry, "Days left"), "");
    assert_eq!(cell(&q, &w.kanji, "Course name"), "Elementary Japanese");
    assert_eq!(cell(&q, &w.laundry, "Course name"), "None");
    let c = &q["columns"][col(&q, "Days left")];
    assert_eq!(
        (c["type"].clone(), c["locked"].clone(), c["added"].clone()),
        (json!("formula"), json!(true), json!(true))
    );

    // "Hours logged per course this week", on Courses.
    ok(
        &core,
        "db.column.add",
        json!({
            "table": "Courses", "name": "Hours this week", "type": "formula",
            "formula": "ROUND(SUMIFS(Focus sessions[Minutes], Focus sessions[Course], [Code], Focus sessions[Date], \">=\" & STARTOFWEEK(TODAY())) / 60, 2)",
        }),
    );
    let q = query(&core, "course", json!({}));
    assert_eq!(cell(&q, &w.jpn, "Hours this week"), 1.25);
    assert_eq!(cell(&q, &w.phy, "Hours this week"), 0.42);
    // A formula can read another formula, and a view can filter and sort by one.
    ok(
        &core,
        "db.column.add",
        json!({"table": "Courses", "name": "Busy", "type": "formula", "formula": "[Hours this week] > 1"}),
    );
    let q = query(
        &core,
        "course",
        json!({"filters": [{"column": "Busy", "op": "is", "value": true}]}),
    );
    assert_eq!(cells(&q, "code"), ["JPN 101"]);
    let q = query(
        &core,
        "task",
        json!({"sorts": [{"column": "Days left"}], "filters": [{"column": "Days left", "op": "ge", "value": 0}]}),
    );
    assert_eq!(cells(&q, "title"), ["Kanji quiz", "Essay draft"]);

    // A formula that fails is a value in the cell, with its sentence.
    ok(
        &core,
        "db.column.add",
        json!({"table": "Tasks", "name": "Bad", "type": "formula", "formula": "1 / 0"}),
    );
    assert_eq!(
        cell(&query(&core, "task", json!({})), &w.kanji, "Bad"),
        json!({"error": "#DIV/0!", "message": "Divided by zero."})
    );
    // Two formulas that read each other say so and don't hang.
    ok(
        &core,
        "db.column.add",
        json!({"table": "Tasks", "name": "A", "type": "formula", "formula": "1"}),
    );
    ok(
        &core,
        "db.column.add",
        json!({"table": "Tasks", "name": "B", "type": "formula", "formula": "[A] + 1"}),
    );
    ok(
        &core,
        "db.column.update",
        json!({"table": "Tasks", "column": "A", "formula": "[B] + 1"}),
    );
    assert_eq!(
        cell(&query(&core, "task", json!({})), &w.kanji, "A")["error"],
        "#CYCLE!"
    );

    // What can't be read isn't saved, and says why.
    let (_, said) = refused(
        &core,
        "db.column.add",
        json!({"table": "Tasks", "name": "X", "type": "formula", "formula": "SUM(1,"}),
    );
    assert!(said.starts_with("That formula can't be read."), "{said}");
    let (_, said) = refused(
        &core,
        "db.column.add",
        json!({"table": "Tasks", "name": "title", "type": "formula", "formula": "1"}),
    );
    assert_eq!(said, "Tasks already has a column called 'title'.");
    let (_, said) = refused(
        &core,
        "db.column.add",
        json!({"table": "Tasks", "name": "Plain", "type": "text"}),
    );
    assert!(
        said.contains("A formula column can be added to it"),
        "{said}"
    );
    let (_, said) = refused(
        &core,
        "db.column.delete",
        json!({"table": "Tasks", "column": "title"}),
    );
    assert!(said.contains("stays as it is"), "{said}");

    // The editor's check: what it gives, or where it goes wrong.
    let check = ok(
        &core,
        "db.formula.check",
        json!({"table": "Tasks", "formula": "[Estimate (min)] / 60"}),
    );
    assert_eq!(check["ok"], true);
    assert_eq!(check["sample"][0], 0.75);
    let check = ok(
        &core,
        "db.formula.check",
        json!({"table": "Tasks", "formula": "[Nope] + 1"}),
    );
    assert_eq!(
        (check["ok"].clone(), check["message"].clone()),
        (json!(false), json!("Tasks has no column called 'Nope'."))
    );
    let check = ok(
        &core,
        "db.formula.check",
        json!({"table": "Tasks", "formula": "1 +"}),
    );
    assert_eq!(
        (check["ok"].clone(), check["at"].clone()),
        (json!(false), json!(3))
    );

    // A formula column is undone like any change.
    ok(
        &core,
        "db.column.delete",
        json!({"table": "Tasks", "column": "Bad"}),
    );
    assert_eq!(undo(&core), "delete column");
    assert!(query(&core, "task", json!({}))["columns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "Bad"));
}

#[test]
fn a_table_of_ones_own_with_rows_columns_and_undo() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let made = ok(&core, "db.table.create", json!({"name": "Reading list"}));
    assert_eq!(made["undo"], "Undo add table");
    let table = made["table"]["id"].as_str().unwrap().to_string();
    ok(
        &core,
        "db.column.add",
        json!({"table": table, "name": "Pages", "type": "number"}),
    );
    ok(
        &core,
        "db.column.add",
        json!({"table": "Reading list", "name": "Finished", "type": "date"}),
    );
    ok(
        &core,
        "db.column.add",
        json!({"table": table, "name": "Read", "type": "checkbox"}),
    );
    let added = ok(
        &core,
        "db.rows.add",
        json!({"table": table, "rows": [
            {"Name": "Genki I", "Pages": 384, "Read": true, "Finished": "Sep 30, 2026"},
            {"Name": "University Physics", "Pages": "1,596"},
            {},
        ]}),
    );
    assert_eq!(added["undo"], "Undo add 3 rows");
    let ids: Vec<String> = added["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i.as_str().unwrap().to_string())
        .collect();
    let q = query(&core, "Reading list", json!({}));
    assert_eq!(q["table"]["origin"], "user");
    assert_eq!(
        cells(&q, "Name"),
        [json!("Genki I"), json!("University Physics"), Value::Null]
    );
    assert_eq!(cells(&q, "Pages"), [json!(384), json!(1596), Value::Null]);
    assert_eq!(cells(&q, "Finished")[0], "2026-09-30");
    assert_eq!(cells(&q, "Read"), [json!(true), json!(false), json!(false)]);
    assert_eq!(
        q["summary"][q["columns"][col(&q, "Pages")]["id"].as_str().unwrap()]["sum"],
        1980
    );

    // Edits, a formula over its own columns, and one over the whole column.
    let r = ok(
        &core,
        "db.cells.set",
        json!({"table": table, "label": "fill down", "edits": [
            {"row": ids[2], "column": "Name", "value": "Remembering the Kanji"},
            {"row": ids[2], "column": "Pages", "value": 460},
            {"row": ids[1], "column": "Read", "value": "yes"},
            {"row": ids[1], "column": "Pages", "value": "many"},
        ]}),
    );
    assert_eq!(
        (r["changed"].clone(), r["undo"].clone()),
        (json!(3), json!("Undo fill down"))
    );
    assert_eq!(r["failed"][0]["message"], "'many' isn't a number.");
    ok(
        &core,
        "db.column.add",
        json!({"table": table, "name": "Share", "type": "formula", "formula": "ROUND(Pages / SUM(Reading list[Pages]) * 100)"}),
    );
    let q = query(&core, &table, json!({}));
    assert_eq!(cells(&q, "Share"), [json!(16), json!(65), json!(19)]);
    assert_eq!(undo(&core), "add column");
    assert_eq!(undo(&core), "fill down");
    let q = query(&core, &table, json!({}));
    assert_eq!(cells(&q, "Pages"), [json!(384), json!(1596), Value::Null]);
    assert_eq!(cells(&q, "Read"), [json!(true), json!(false), json!(false)]);

    // Rows go and come back; so does a column with what its cells held.
    ok(
        &core,
        "db.rows.delete",
        json!({"table": table, "rows": [ids[0], ids[1]]}),
    );
    assert_eq!(query(&core, &table, json!({}))["total"], 1);
    assert_eq!(undo(&core), "delete 2 rows");
    ok(
        &core,
        "db.column.delete",
        json!({"table": table, "column": "Pages"}),
    );
    assert!(query(&core, &table, json!({}))["columns"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["name"] != "Pages"));
    assert_eq!(undo(&core), "delete column");
    assert_eq!(cells(&query(&core, &table, json!({})), "Pages")[1], 1596);
    ok(
        &core,
        "db.column.update",
        json!({"table": table, "column": "Pages", "name": "Page count"}),
    );
    assert_eq!(
        cells(&query(&core, &table, json!({})), "Page count")[0],
        384
    );

    // Names are a table's own.
    let (_, said) = refused(&core, "db.table.create", json!({"name": "reading LIST"}));
    assert_eq!(said, "There is already a table called 'reading LIST'.");
    let (_, said) = refused(&core, "db.table.create", json!({"name": "Tasks"}));
    assert_eq!(said, "There is already a table called 'Tasks'.");
    let (_, said) = refused(&core, "db.table.create", json!({"name": "A[1]"}));
    assert!(said.contains("can't hold [ or ]"), "{said}");
    let (_, said) = refused(
        &core,
        "db.table.rename",
        json!({"table": "Tasks", "name": "Chores"}),
    );
    assert!(said.contains("one of Learn's own tables"), "{said}");
    ok(
        &core,
        "db.table.rename",
        json!({"table": table, "name": "Books"}),
    );
    assert_eq!(query(&core, "books", json!({}))["table"]["name"], "Books");

    // Deleting the table is one entry: ⌘Z brings back the rows and columns with it.
    ok(&core, "db.table.delete", json!({"table": "Books"}));
    assert!(core.invoke("db.query", json!({"table": "Books"})).is_err());
    assert_eq!(undo(&core), "delete table");
    assert_eq!(query(&core, "Books", json!({}))["total"], 3);
}

#[test]
fn a_pivot_and_a_charts_numbers() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    world(&core);
    let p = ok(
        &core,
        "db.pivot",
        json!({
            "table": "Tasks", "rows": ["Course"],
            "values": [{"column": "Estimate (min)", "agg": "sum"}, {"column": "Estimate (min)", "agg": "average"}],
        }),
    );
    let groups: Vec<(Value, Value, Value)> = p["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            (
                g["keys"][0].clone(),
                g["count"].clone(),
                g["values"].clone(),
            )
        })
        .collect();
    assert_eq!(
        groups,
        [
            (json!("JPN 101"), json!(2), json!([135, 67.5])),
            (json!("PHY 204"), json!(1), json!([120, 120])),
            (json!(""), json!(1), json!([null, null])),
        ]
    );
    assert_eq!(p["total"], json!({"count": 4, "values": [255, 85]}));
    // Two columns deep, through a filter.
    let p = ok(
        &core,
        "db.pivot",
        json!({
            "table": "Focus sessions", "rows": ["Course", "Date"],
            "values": [{"column": "Minutes", "agg": "sum"}],
            "spec": {"filters": [{"column": "Date", "op": "ge", "value": "2026-10-01"}]},
        }),
    );
    let keys: Vec<Value> = p["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| json!([g["keys"], g["values"][0]]))
        .collect();
    assert_eq!(
        keys,
        [
            json!([["JPN 101", "2026-10-05"], 25]),
            json!([["JPN 101", "2026-10-06"], 50]),
            json!([["PHY 204", "2026-10-06"], 25]),
        ]
    );
    let (_, said) = refused(&core, "db.pivot", json!({"table": "Tasks", "rows": []}));
    assert_eq!(said, "A pivot groups by one or two columns.");
    let (_, said) = refused(
        &core,
        "db.pivot",
        json!({"table": "Tasks", "rows": ["Course"], "values": [{"column": "estMin", "agg": "mode"}]}),
    );
    assert!(
        said.starts_with("There is no total called 'mode'."),
        "{said}"
    );

    // A bar per course; a point per session.
    let c = ok(
        &core,
        "db.chart",
        json!({"table": "Focus sessions", "x": "Course", "y": ["Minutes"], "agg": "sum"}),
    );
    assert_eq!(c["labels"], json!(["JPN 101", "PHY 204"]));
    assert_eq!(
        c["series"],
        json!([{"name": "Minutes", "values": [100, 25]}])
    );
    assert_eq!(c["x"]["kind"], "category");
    let c = ok(
        &core,
        "db.chart",
        json!({"table": "Focus sessions", "x": "Date", "y": ["Minutes"], "agg": "sum"}),
    );
    assert_eq!(
        c["labels"],
        json!(["2026-09-28", "2026-10-05", "2026-10-06"])
    );
    assert_eq!(c["series"][0]["values"], json!([25, 25, 75]));
    assert_eq!(c["x"]["kind"], "date");
    let c = ok(
        &core,
        "db.chart",
        json!({"table": "Tasks", "x": "Estimate (min)", "y": ["Logged (min)"]}),
    );
    assert_eq!(c["x"]["kind"], "number");
    assert_eq!(c["xs"], json!([45, 90, 120, null]));
    assert_eq!(c["series"][0]["values"], json!([75, 25, 25, 0]));
    let c = ok(
        &core,
        "db.chart",
        json!({"table": "Tasks", "x": "Course", "y": [], "agg": "count"}),
    );
    assert_eq!(c["series"], json!([{"name": "Count", "values": [2, 1, 1]}]));
}

#[test]
fn a_formula_a_pivot_a_chart_and_a_view_are_there_after_a_restart() {
    let setup = Setup::new();
    let spec = json!({
        "filters": [{"column": "done", "op": "is", "value": false}],
        "sorts": [{"column": "due", "dir": "asc"}],
        "hidden": ["notes", "id"], "order": ["title", "due"], "widths": {"title": 260}, "frozen": 1,
        "pivot": {"rows": ["courseId"], "values": [{"column": "estMin", "agg": "sum"}]},
        "charts": [{"id": "c1", "type": "bar", "x": "courseId", "y": ["estMin"], "agg": "sum"}],
    });
    let view_id;
    {
        let core = heat_core(&setup, "2026-10-07 09:00");
        world(&core);
        ok(
            &core,
            "db.column.add",
            json!({"table": "Tasks", "name": "Hours", "type": "formula", "formula": "[Estimate (min)] / 60"}),
        );
        let saved = ok(
            &core,
            "db.view.save",
            json!({"table": "Tasks", "name": "Open by due date", "spec": spec}),
        );
        assert_eq!(saved["undo"], "Undo add view");
        view_id = saved["view"]["id"].as_str().unwrap().to_string();
        ok(
            &core,
            "db.layout.set",
            json!({"table": "task", "spec": {"widths": {"due": 140}}}),
        );
        let (_, said) = refused(
            &core,
            "db.view.save",
            json!({"table": "Tasks", "name": "open by due date", "spec": {}}),
        );
        assert_eq!(
            said,
            "This table already has a view called 'open by due date'."
        );
        let (_, said) = refused(
            &core,
            "db.view.save",
            json!({"table": "Tasks", "name": "Broken", "spec": {"sorts": [{"column": "nope"}]}}),
        );
        assert_eq!(said, "Tasks has no column called 'nope'.");
    }
    // The app is opened again.
    let core = heat_core(&setup, "2026-10-07 09:00");
    let t = ok(&core, "db.tables", json!({}));
    assert_eq!(t["views"][0]["name"], "Open by due date");
    assert_eq!(t["views"][0]["spec"], spec);
    assert_eq!(t["layouts"]["task"], json!({"widths": {"due": 140}}));
    let q = ok(
        &core,
        "db.query",
        json!({"table": "Tasks", "view": "Open by due date"}),
    );
    assert_eq!(
        cells(&q, "title"),
        ["Lab report", "Kanji quiz", "Essay draft", "Laundry"]
    );
    // An empty cell counts as zero in arithmetic, as in a spreadsheet.
    assert_eq!(
        cells(&q, "Hours"),
        [json!(2), json!(0.75), json!(1.5), json!(0)]
    );
    let p = ok(
        &core,
        "db.pivot",
        json!({"table": "Tasks", "view": view_id, "rows": spec["pivot"]["rows"], "values": spec["pivot"]["values"]}),
    );
    assert_eq!(
        p["groups"][0],
        json!({"keys": ["JPN 101"], "count": 2, "values": [135]})
    );
    let c = ok(
        &core,
        "db.chart",
        json!({"table": "Tasks", "view": view_id, "x": "courseId", "y": ["estMin"], "agg": "sum"}),
    );
    assert_eq!(c["series"][0]["values"], json!([135, 120, null]));
    // A view is saved over, and deleted, and the delete undoes.
    ok(
        &core,
        "db.view.save",
        json!({"id": view_id, "table": "Tasks", "name": "Open", "spec": {}}),
    );
    assert_eq!(
        ok(&core, "db.tables", json!({}))["views"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    ok(&core, "db.view.delete", json!({"id": view_id}));
    assert_eq!(ok(&core, "db.tables", json!({}))["views"], json!([]));
    assert_eq!(undo(&core), "delete view");
    assert_eq!(
        ok(&core, "db.tables", json!({}))["views"][0]["name"],
        "Open"
    );
    let (_, said) = refused(&core, "db.query", json!({"table": "Tasks", "view": "Gone"}));
    assert_eq!(said, "There is no saved view called 'Gone'.");
}

#[test]
fn csv_goes_out_and_comes_back() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    world(&core);
    let out = ok(
        &core,
        "db.csv.export",
        json!({"table": "Tasks", "spec": {
            "sorts": [{"column": "title"}], "order": ["title", "courseId", "due", "estMin", "done"],
            "hidden": ["id", "spaceId", "type", "projectId", "milestoneId", "group", "parentTaskId", "scheduledDate",
                       "rrule", "difficulty", "estBy", "estReason", "adjustMin", "notes", "link", "doneAt", "source",
                       "sourceId", "claudeReason", "tag", "public", "~heat", "~level", "~planned", "~logged", "~next"],
        }}),
    );
    assert_eq!(out["name"], "Tasks.csv");
    assert_eq!(out["rows"], 4);
    assert_eq!(
        out["csv"],
        "Title,Course,Due,Estimate (min),Done\r\n\
         Essay draft,JPN 101,2026-10-12 23:59,90,FALSE\r\n\
         Kanji quiz,JPN 101,2026-10-09 23:59,45,FALSE\r\n\
         Lab report,PHY 204,2026-10-06 17:00,120,FALSE\r\n\
         Laundry,,,,FALSE\r\n"
    );
    // To a file, when one is named.
    let path = setup.dir.path().join("tasks.csv");
    let written = ok(
        &core,
        "db.csv.export",
        json!({"table": "Tasks", "to": path, "spec": {"filters": [{"column": "title", "op": "eq", "value": "Laundry"}]}}),
    );
    assert_eq!(written["rows"], 1);
    assert!(std::fs::read_to_string(&path).unwrap().contains("Laundry"));

    // In: the first line names the columns, and each column's type is read from it.
    let csv = "Book,Pages,Finished,Read,Note\r\n\"Genki, vol. I\",384,2026-09-30,yes,\"He said \"\"good\"\"\"\r\nUniversity Physics,\"1,596\",,no,\r\n";
    let before = undo_label(&core);
    let made = ok(
        &core,
        "db.csv.import",
        json!({"name": "books.csv", "csv": csv}),
    );
    assert_eq!(
        (
            made["rows"].clone(),
            made["columns"].clone(),
            made["undo"].clone()
        ),
        (json!(2), json!(5), json!("Undo import CSV"))
    );
    assert_eq!(made["table"]["name"], "books");
    let q = query(&core, "books", json!({}));
    let types: Vec<Value> = q["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["type"].clone())
        .collect();
    assert_eq!(types, ["text", "number", "date", "bool", "text"]);
    assert_eq!(cells(&q, "Book"), ["Genki, vol. I", "University Physics"]);
    assert_eq!(cells(&q, "Pages"), [384, 1596]);
    assert_eq!(cells(&q, "Read"), [true, false]);
    assert_eq!(cells(&q, "Note")[0], "He said \"good\"");
    // What went in comes out the same.
    let back = ok(&core, "db.csv.export", json!({"table": "books"}));
    assert_eq!(
        back["csv"],
        "Book,Pages,Finished,Read,Note\r\n\"Genki, vol. I\",384,2026-09-30,TRUE,\"He said \"\"good\"\"\"\r\nUniversity Physics,1596,,FALSE,\r\n"
    );
    // A second file of the same name gets its own; one ⌘Z takes an import away whole.
    assert_eq!(
        ok(
            &core,
            "db.csv.import",
            json!({"name": "books", "csv": "A\n1\n"})
        )["table"]["name"],
        "books 2"
    );
    assert_eq!(undo(&core), "import CSV");
    assert_eq!(undo(&core), "import CSV");
    assert!(core.invoke("db.query", json!({"table": "books"})).is_err());
    assert_eq!(undo_label(&core), before);
    let (_, said) = refused(&core, "db.csv.import", json!({"name": "x", "csv": ""}));
    assert_eq!(said, "That file has no rows.");
}

#[test]
fn rows_of_learns_tables_are_added_and_deleted_by_learns_rules() {
    let setup = Setup::new();
    let core = heat_core(&setup, "2026-10-07 09:00");
    let w = world(&core);
    let added = ok(
        &core,
        "db.rows.add",
        json!({"table": "Tasks", "rows": [
            {"Title": "Vocabulary cards", "Course": "JPN 101", "Due": "2026-10-11", "Estimate (min)": 30},
            {"Title": ""},
        ]}),
    );
    assert_eq!(added["ids"].as_array().unwrap().len(), 1);
    assert_eq!(added["failed"][0]["message"], "Give the task a name first.");
    assert_eq!(added["undo"], "Undo add task");
    let id = added["ids"][0].as_str().unwrap();
    let task = ok(&core, "records.get", json!({"kind": "task", "id": id}))["record"].clone();
    assert_eq!(task["courseId"], w.jpn);
    assert_eq!(task["due"].as_f64().unwrap(), ny("2026-10-11 23:59"));
    assert_eq!(task["spaceId"], space(&core));
    // Deleting rows is Learn's delete: a course with tasks takes its pointer out of them.
    let gone = ok(
        &core,
        "db.rows.delete",
        json!({"table": "Tasks", "rows": [id, w.laundry]}),
    );
    assert_eq!(
        (gone["deleted"].clone(), gone["undo"].clone()),
        (json!(2), json!("Undo delete 2 rows"))
    );
    assert_eq!(query(&core, "Tasks", json!({}))["total"], 3);
    assert_eq!(undo(&core), "delete 2 rows");
    assert_eq!(query(&core, "Tasks", json!({}))["total"], 5);
}

#[test]
fn the_tabs_own_records_never_leave_this_mac() {
    let server = MockServer::start();
    let (a, b) = (Setup::new(), Setup::new());
    let mac = sign_in(&a, &server);
    let air = sign_in(&b, &server);
    let made = ok(&mac, "db.table.create", json!({"name": "Private list"}));
    ok(
        &mac,
        "db.rows.add",
        json!({"table": made["table"]["id"], "rows": [{"Name": "a secret"}]}),
    );
    ok(
        &mac,
        "db.view.save",
        json!({"table": "Tasks", "name": "Mine", "spec": {}}),
    );
    ok(
        &mac,
        "db.column.add",
        json!({"table": "Tasks", "name": "Twice", "type": "formula", "formula": "2"}),
    );
    let space = space(&mac);
    add_task(&mac, &space, "This one syncs", json!({}));
    mac.sync_heat().unwrap();
    air.sync_heat().unwrap();
    // The task crossed; nothing of the Database tab's did.
    let tasks = ok(&air, "records.list", json!({"kind": "task"}));
    assert!(tasks["records"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["title"] == "This one syncs"));
    for kind in ["dbTable", "dbRow", "dbColumn", "dbView", "dbLayout"] {
        assert_eq!(
            ok(&air, "records.list", json!({"kind": kind}))["records"],
            json!([]),
            "{kind}"
        );
    }
    assert_eq!(
        ok(&mac, "records.list", json!({"kind": "dbRow"}))["records"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
