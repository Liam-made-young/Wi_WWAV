//! Moving in (`docs/SPEC.md` 3.16): the Heat artifact's JSON export becomes
//! the app's records. Each workspace maps to a space, and every id is kept
//! (event ids, the `em-` and `gp-` hashes, the processed Gmail ids), so the
//! first sync in the app finds nothing new instead of everything twice.
//!
//! The artifact has no export yet; step 0 is a "Download JSON" button on it.
//! This is the format that button should write: the plain dump of the records
//! 3.1 describes, one array per kind, instants as ISO 8601 strings with their
//! offset ("Z" or "-04:00") and days as "YYYY-MM-DD". [`parse_heat_export`]
//! checks every row against it and refuses the whole file if one row is off,
//! so nothing is half-read.
//!
//! ```text
//!   {
//!     "format": "heat-export", "version": 1,
//!     "exportedAt": "2026-10-06T12:40:00.000Z",
//!     "timeZone": "America/New_York",            // the browser's zone
//!     "workspaces": [{ "key": "classes", "name": "Classes", "groupLabel": "Course",
//!                      "types": ["Homework", …, "Other"], "persona": "…" }, …],
//!     "tasks": [{ "id": "<event id> | em-<hash> | <own id>", "workspace": "classes",
//!                 "title": "Grammar quiz 4", "group": "JPN 201" | null,
//!                 "type": "Quiz", "due": "<ISO>" | null, "difficulty": 2,
//!                 "estMin": 45 | null, "actualMin": 75 | null,   // "Time it took"
//!                 "notes": "", "done": false, "doneAt": "<ISO>" | null,
//!                 "source": "manual" | "calendar" | "gmail" }, …],
//!     "milestones": [{ "id": "…", "workspace": "wwav", "title": "…",
//!                      "date": "2026-10-20", "done": false, "order": 1 }, …],
//!     "habits": [{ "id": "…", "title": "…", "log": { "2026-10-05": true } }, …],
//!     "term": "Fall 2026",
//!     "courses": [{ "code": "JPN 201", "name": "…",
//!                   "categories": [{ "name": "Quizzes", "weight": 40,
//!                                    "keywords": ["quiz", "kanji"] }, …],
//!                   "scale": [{ "letter": "A", "min": 93 }, …],   // optional
//!                   "sticky": "…" }, …],                          // optional
//!     "grades": [{ "id": "<own id> | gp-<hash>", "course": "JPN 201",
//!                  "title": "…", "category": "Quizzes" | null,
//!                  "score": 18 | null, "outOf": 20, "dropped": false,
//!                  "pending": false, "link": "…" }, …],         // link optional
//!     "processedMailIds": ["18f2a…", …],                        // the last 400
//!     "lastSyncAt": "<ISO>" | null
//!   }
//! ```
//!
//! A port of `importArtifact.ts`. [`parse_heat_export`] reads the parsed JSON
//! as a [`serde_json::Value`] and hands back the typed export; an instant is
//! read as V8's `Date.parse` reads the ISO form ([`js::parse_iso_instant`]).

use super::records::{
    ser, Course, Grade, GradeCategory, GradeSource, GroupKind, Habit, Id, LetterStep, Milestone,
    Space, SyncState, Task, TaskSource, Term,
};
use super::spaces::default_spaces;
use super::{copy, js};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportWorkspace {
    pub key: String,
    pub name: String,
    pub group_label: String,
    pub types: Vec<String>,
    pub persona: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportSource {
    Manual,
    Calendar,
    Gmail,
}

impl ExportSource {
    fn task_source(self) -> TaskSource {
        match self {
            ExportSource::Manual => TaskSource::You,
            ExportSource::Calendar => TaskSource::Calendar,
            ExportSource::Gmail => TaskSource::Mail,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTask {
    pub id: String,
    pub workspace: String,
    pub title: String,
    pub group: Option<String>,
    #[serde(rename = "type")]
    pub r#type: String,
    pub due: Option<String>,
    #[serde(serialize_with = "ser::num")]
    pub difficulty: f64,
    #[serde(serialize_with = "ser::opt_num")]
    pub est_min: Option<f64>,
    #[serde(serialize_with = "ser::opt_num")]
    pub actual_min: Option<f64>,
    pub notes: String,
    pub done: bool,
    pub done_at: Option<String>,
    pub source: ExportSource,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportMilestone {
    pub id: String,
    pub workspace: String,
    pub title: String,
    pub date: String,
    pub done: bool,
    #[serde(serialize_with = "ser::num")]
    pub order: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportHabit {
    pub id: String,
    pub title: String,
    pub log: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportCategory {
    pub name: String,
    #[serde(serialize_with = "ser::num")]
    pub weight: f64,
    pub keywords: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportCourse {
    pub code: String,
    pub name: String,
    pub categories: Vec<ExportCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<Vec<LetterStep>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticky: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportGrade {
    pub id: String,
    pub course: String,
    pub title: String,
    pub category: Option<String>,
    #[serde(serialize_with = "ser::opt_num")]
    pub score: Option<f64>,
    #[serde(serialize_with = "ser::num")]
    pub out_of: f64,
    pub dropped: bool,
    pub pending: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatExport {
    pub format: String,
    #[serde(serialize_with = "ser::num")]
    pub version: f64,
    pub exported_at: String,
    pub time_zone: String,
    pub workspaces: Vec<ExportWorkspace>,
    pub tasks: Vec<ExportTask>,
    pub milestones: Vec<ExportMilestone>,
    pub habits: Vec<ExportHabit>,
    pub term: String,
    pub courses: Vec<ExportCourse>,
    pub grades: Vec<ExportGrade>,
    pub processed_mail_ids: Vec<String>,
    pub last_sync_at: Option<String>,
}

/// What the app takes in from an export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Imported {
    /// Spaces to add; a workspace whose name matches an existing space moves into it.
    pub spaces: Vec<Space>,
    pub tasks: Vec<Task>,
    pub milestones: Vec<Milestone>,
    pub habits: Vec<Habit>,
    /// Terms and courses to add; a term already there by name, and a course by code in it, are used as they are.
    pub terms: Vec<Term>,
    pub courses: Vec<Course>,
    pub grades: Vec<Grade>,
    pub sync: SyncState,
}

/// What the app already holds, so a second import (a fresher download) defines nothing twice.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Existing {
    pub spaces: Option<Vec<Space>>,
    pub terms: Option<Vec<Term>>,
    pub courses: Option<Vec<Course>>,
}

/// Why a file was refused: one sentence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportError {
    pub error: String,
}

// --- Checking the file, row by row -------------------------------------------

type Row = Map<String, Value>;
type Field<'a> = Option<&'a Value>;

fn is_string(v: Field) -> bool {
    matches!(v, Some(Value::String(_)))
}
fn is_number(v: Field) -> bool {
    matches!(v, Some(Value::Number(n)) if n.as_f64().is_some_and(f64::is_finite))
}
fn number(v: Field) -> Option<f64> {
    v.and_then(Value::as_f64)
}
fn is_null(v: Field) -> bool {
    matches!(v, Some(Value::Null))
}
fn is_bool(v: Field) -> bool {
    matches!(v, Some(Value::Bool(_)))
}
fn is_minutes(v: Field) -> bool {
    is_null(v) || (is_number(v) && number(v).is_some_and(|n| n >= 0.0))
}
fn is_strings(v: Field) -> bool {
    matches!(v, Some(Value::Array(a)) if a.iter().all(Value::is_string))
}
fn is_instant(v: Field) -> bool {
    matches!(v, Some(Value::String(s)) if js::parse_iso_instant(s).is_some())
}
/// `/^\d{4}-\d{2}-\d{2}$/`: the shape of a day, not a check that it is one.
fn is_day_text(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
}
fn is_day(v: Field) -> bool {
    matches!(v, Some(Value::String(s)) if is_day_text(s))
}
fn optional(v: Field, check: impl Fn(Field) -> bool) -> bool {
    v.is_none() || check(v)
}
fn or_null(v: Field, check: impl Fn(Field) -> bool) -> bool {
    is_null(v) || check(v)
}
fn every(v: Field, check: impl Fn(&Row) -> bool) -> bool {
    matches!(v, Some(Value::Array(rows)) if rows.iter().all(|r| r.as_object().is_some_and(&check)))
}

fn workspace_ok(w: &Row) -> bool {
    is_string(w.get("key"))
        && is_string(w.get("name"))
        && is_string(w.get("groupLabel"))
        && is_strings(w.get("types"))
        && is_string(w.get("persona"))
}

fn task_ok<'a>(keys: &'a [&'a str]) -> impl Fn(&Row) -> bool + 'a {
    move |t| {
        is_string(t.get("id"))
            && matches!(t.get("workspace"), Some(Value::String(w)) if keys.contains(&w.as_str()))
            && is_string(t.get("title"))
            && or_null(t.get("group"), is_string)
            && is_string(t.get("type"))
            && or_null(t.get("due"), is_instant)
            && is_number(t.get("difficulty"))
            && number(t.get("difficulty")).is_some_and(|d| d >= 1.0)
            && number(t.get("difficulty")).is_some_and(|d| d <= 5.0)
            && is_minutes(t.get("estMin"))
            && is_minutes(t.get("actualMin"))
            && is_string(t.get("notes"))
            && is_bool(t.get("done"))
            && or_null(t.get("doneAt"), is_instant)
            && matches!(t.get("source"), Some(Value::String(s)) if matches!(s.as_str(), "manual" | "calendar" | "gmail"))
    }
}

fn milestone_ok<'a>(keys: &'a [&'a str]) -> impl Fn(&Row) -> bool + 'a {
    move |m| {
        is_string(m.get("id"))
            && matches!(m.get("workspace"), Some(Value::String(w)) if keys.contains(&w.as_str()))
            && is_string(m.get("title"))
            && is_day(m.get("date"))
            && is_bool(m.get("done"))
            && is_number(m.get("order"))
    }
}

fn habit_ok(h: &Row) -> bool {
    is_string(h.get("id"))
        && is_string(h.get("title"))
        && matches!(h.get("log"), Some(Value::Object(log))
            if log.iter().all(|(k, v)| is_day_text(k) && matches!(v, Value::Bool(true))))
}

fn course_ok(c: &Row) -> bool {
    is_string(c.get("code"))
        && is_string(c.get("name"))
        && every(c.get("categories"), |k| {
            is_string(k.get("name")) && is_number(k.get("weight")) && is_strings(k.get("keywords"))
        })
        && optional(c.get("scale"), |v| {
            every(v, |x| is_string(x.get("letter")) && is_number(x.get("min")))
        })
        && optional(c.get("sticky"), is_string)
}

fn grade_ok(g: &Row) -> bool {
    is_string(g.get("id"))
        && is_string(g.get("course"))
        && is_string(g.get("title"))
        && or_null(g.get("category"), is_string)
        && or_null(g.get("score"), is_number)
        && is_number(g.get("outOf"))
        && is_bool(g.get("dropped"))
        && is_bool(g.get("pending"))
        && optional(g.get("link"), is_string)
}

/// Checks that a parsed file is a Heat export this version can read, every row of it.
pub fn parse_heat_export(json: &Value) -> Result<HeatExport, ExportError> {
    let not_export = || ExportError {
        error: copy::moving::NOT_EXPORT.to_string(),
    };
    let Some(x) = json.as_object() else {
        return Err(not_export());
    };
    if x.get("format").and_then(Value::as_str) != Some("heat-export") {
        return Err(not_export());
    }
    let version = number(x.get("version"));
    if matches!(x.get("version"), Some(Value::Number(_)))
        && version.is_some_and(|v| js::is_integer(v) && v > 1.0)
    {
        return Err(ExportError {
            error: copy::moving::NEWER.to_string(),
        });
    }
    if version != Some(1.0) {
        return Err(not_export());
    }
    if !is_instant(x.get("exportedAt"))
        || !is_string(x.get("timeZone"))
        || !is_string(x.get("term"))
        || !or_null(x.get("lastSyncAt"), is_instant)
    {
        return Err(not_export());
    }
    if !every(x.get("workspaces"), workspace_ok) {
        return Err(not_export());
    }
    let keys: Vec<&str> = match x.get("workspaces") {
        Some(Value::Array(ws)) => ws
            .iter()
            .filter_map(|w| w.get("key").and_then(Value::as_str))
            .collect(),
        _ => vec![],
    };
    let ok = every(x.get("tasks"), task_ok(&keys))
        && every(x.get("milestones"), milestone_ok(&keys))
        && every(x.get("habits"), habit_ok)
        && every(x.get("courses"), course_ok)
        && every(x.get("grades"), grade_ok)
        && is_strings(x.get("processedMailIds"));
    if !ok {
        return Err(not_export());
    }
    serde_json::from_value(json.clone()).map_err(|_| not_export())
}

/// `Date.parse` of an instant the export holds; None for none. [`import_artifact`]
/// is for an export [`parse_heat_export`] accepted, whose instants all read. One
/// it would have refused reads as no date here, where the TypeScript's
/// `Date.parse` makes NaN of it, or a time in the machine's own zone.
fn parse_time(iso: &Option<String>) -> Option<f64> {
    iso.as_deref().and_then(js::parse_iso_instant)
}

/// A span of the course list the import searches: the course in the term already,
/// then the ones this import has made.
fn find_course<'a>(in_term: &'a [Course], made: &'a [Course], code: &str) -> Option<&'a Course> {
    let wanted = js::normalise_name(code);
    in_term
        .iter()
        .chain(made.iter())
        .find(|c| js::normalise_name(&c.code) == wanted)
}

pub fn import_artifact(
    dump: &HeatExport,
    existing: &Existing,
    new_id: &mut dyn FnMut() -> Id,
) -> Imported {
    let defaults = default_spaces(&mut String::new);
    let mut spaces: Vec<Space> = vec![];
    let mut space_for: HashMap<&str, Space> = HashMap::new();
    for w in &dump.workspaces {
        let same = |s: &Space| s.name.to_lowercase() == w.name.to_lowercase();
        let found = existing
            .spaces
            .as_ref()
            .and_then(|all| all.iter().find(|s| same(s)));
        let space = match found {
            Some(space) => space.clone(),
            None => {
                let group_kind = match w.group_label.as_str() {
                    "Course" => GroupKind::Course,
                    "Milestone" => GroupKind::Milestone,
                    _ => GroupKind::Free,
                };
                let hue = defaults
                    .iter()
                    .find(|s| same(s))
                    .map_or_else(|| (spaces.len() as f64 * 120.0) % 360.0, |s| s.hue);
                let space = Space {
                    id: new_id(),
                    name: w.name.clone(),
                    hue,
                    group_kind,
                    group_label: w.group_label.clone(),
                    types: w.types.clone(),
                    persona: w.persona.clone(),
                };
                spaces.push(space.clone());
                space
            }
        };
        space_for.insert(w.key.as_str(), space);
    }

    let known = existing.terms.as_ref().and_then(|all| {
        all.iter()
            .find(|t| js::normalise_name(&t.name) == js::normalise_name(&dump.term))
    });
    let term = match known {
        Some(term) => term.clone(),
        None => Term {
            id: new_id(),
            name: dump.term.clone(),
        },
    };
    let terms = if known.is_some() {
        vec![]
    } else {
        vec![term.clone()]
    };
    // Each course is defined once (3.4): one already in the term is used as it is.
    let in_term: Vec<Course> = existing
        .courses
        .iter()
        .flatten()
        .filter(|c| c.term_id == term.id)
        .cloned()
        .collect();
    let mut courses: Vec<Course> = vec![];
    for c in &dump.courses {
        if find_course(&in_term, &courses, &c.code).is_some() {
            continue;
        }
        let id = new_id();
        let categories = c
            .categories
            .iter()
            .map(|k| GradeCategory {
                id: new_id(),
                name: k.name.clone(),
                weight: k.weight,
                keywords: k.keywords.clone(),
                drop_lowest: None,
            })
            .collect();
        courses.push(Course {
            id,
            term_id: term.id.clone(),
            code: c.code.clone(),
            name: c.name.clone(),
            categories,
            scale: c.scale.clone(),
            notes: c.sticky.clone().unwrap_or_default(),
            public: false,
        });
    }
    // A course only a task names is made here, once.
    let mut course_for = |code: &str, courses: &mut Vec<Course>| -> Course {
        if let Some(c) = find_course(&in_term, courses, code) {
            return c.clone();
        }
        let c = Course {
            id: new_id(),
            term_id: term.id.clone(),
            code: code.to_string(),
            name: code.to_string(),
            categories: vec![],
            scale: None,
            notes: String::new(),
            public: false,
        };
        courses.push(c.clone());
        c
    };

    let milestones: Vec<Milestone> = dump
        .milestones
        .iter()
        .map(|m| Milestone {
            id: m.id.clone(),
            space_id: space_for
                .get(m.workspace.as_str())
                .map(|s| s.id.clone())
                .unwrap_or_default(),
            project_id: None,
            title: m.title.clone(),
            date: m.date.clone(),
            done: m.done,
            order: m.order,
            link: None,
            public: false,
        })
        .collect();

    let mut tasks: Vec<Task> = vec![];
    for t in &dump.tasks {
        let space = space_for
            .get(t.workspace.as_str())
            .cloned()
            .unwrap_or_else(|| defaults[0].clone());
        let (mut course_id, mut milestone_id, mut group) = (None, None, None);
        if let Some(g) = &t.group {
            let milestone = milestones
                .iter()
                .find(|m| m.space_id == space.id && m.title == *g);
            if space.group_kind == GroupKind::Course {
                course_id = Some(course_for(g, &mut courses).id);
            } else if let (GroupKind::Milestone, Some(m)) = (space.group_kind, milestone) {
                milestone_id = Some(m.id.clone());
            } else {
                group = Some(g.clone());
            }
        }
        tasks.push(Task {
            id: t.id.clone(),
            space_id: space.id.clone(),
            title: t.title.clone(),
            r#type: t.r#type.clone(),
            course_id,
            project_id: None,
            milestone_id,
            group,
            parent_task_id: None,
            due: parse_time(&t.due),
            scheduled_date: None,
            rrule: None,
            difficulty: t.difficulty,
            est_min: t.est_min,
            est_by: None,
            est_reason: None,
            // The artifact has no focus sessions, so its typed time is all hand adjustment.
            adjust_min: t.actual_min.unwrap_or(0.0),
            notes: t.notes.clone(),
            link: None,
            done: t.done,
            done_at: parse_time(&t.done_at),
            source: t.source.task_source(),
            // The artifact's `em-` and `gp-` ids stay in `Task.id` here, as the TypeScript import leaves them.
            source_id: None,
            public: false,
        });
    }

    let mut grades: Vec<Grade> = vec![];
    for g in &dump.grades {
        let course = course_for(&g.course, &mut courses);
        grades.push(Grade {
            id: g.id.clone(),
            course_id: course.id.clone(),
            category_id: g
                .category
                .as_ref()
                .and_then(|name| course.categories.iter().find(|k| k.name == *name))
                .map(|k| k.id.clone()),
            title: g.title.clone(),
            score: g.score,
            out_of: g.out_of,
            dropped: g.dropped,
            pending: g.pending,
            link: g.link.clone().filter(|l| !l.is_empty()),
            // Pending grades come from grade notices in mail, under gp- hashes.
            source: if g.pending || g.id.starts_with("gp-") {
                GradeSource::Mail
            } else {
                GradeSource::You
            },
            public: false,
        });
    }

    Imported {
        spaces,
        tasks,
        milestones,
        habits: dump
            .habits
            .iter()
            .map(|h| Habit {
                id: h.id.clone(),
                title: h.title.clone(),
                minutes: None,
                log: h.log.clone(),
                show_counter: false,
                public: false,
            })
            .collect(),
        terms,
        courses,
        grades,
        sync: SyncState {
            processed_mail_ids: dump.processed_mail_ids.clone(),
            last_sync_at: parse_time(&dump.last_sync_at),
        },
    }
}
