//! A task's home and its type, in the store.
//!
//! A task can belong to one home that tells it more about itself: a course
//! (`courseId`) or a project (`projectId`). It never has to. Whatever made a
//! task (Brightspace, mail, ⌘⇧N, a sheet, a syllabus), it comes through
//! [`inherit`]: a home is matched if one fits, then a type (home, then space,
//! then the defaults of `wi_heat::model::types`), then the type fills the
//! minutes and the difficulty. A field the person set is theirs and is never
//! refilled: `typeBy`, `estBy` and `difficultyBy` say who made each.
//!
//! Also here: the courses a sync finds in mail, calendars and old groups
//! ([`tidy`]); a syllabus read into a draft, previewed, and accepted as one
//! journal entry; and the batch Claude scores when no type matched.

use serde_json::{json, Map, Value};
use std::collections::HashMap;
use wi_heat::brightspace::{classes_types, same_title, task_type, School, DEFAULT_COURSE_PATTERN};
use wi_heat::homes::{
    self as rules, course_label, kept_code, offering_in, squash, Score, Syllabus, Unscored,
};
use wi_heat::model::grades;
use wi_heat::model::records::{GroupKind, Task};
use wi_heat::model::types::{self, Level, Levels, Sample, TypeDef, OTHER};
use wi_store::{Actor, Store};

use crate::derive::World;
use crate::feed::term_name;
use crate::{
    all, commit, kind, num, one, put, refused, set_setting, setting, ulid, Clock, Outcome, Result,
};

/// The setting that says the library has been through [`tidy`] once.
const TIDIED: &str = "homes";
const TIDY_VERSION: i64 = 1;

/// A syllabus item due longer ago than this is left out, as the feed leaves
/// last month's homework out.
const PAST_MS: f64 = 2.0 * 3_600_000.0;

/// Claude is asked about a task again only after this long.
const ASK_AGAIN_MS: f64 = 24.0 * 3_600_000.0;

fn text<'a>(m: &'a Map<String, Value>, k: &str) -> &'a str {
    m.get(k).and_then(Value::as_str).unwrap_or("")
}

/// The types a record keeps under `field`. One that doesn't read is left out.
pub(crate) fn defs_of(record: &Value, field: &str) -> Vec<TypeDef> {
    record[field]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| serde_json::from_value::<TypeDef>(d.clone()).ok())
        .filter(|d| !d.name.trim().is_empty())
        .collect()
}

#[derive(Clone, Debug)]
struct SpaceInfo {
    id: String,
    name: String,
    persona: String,
    by_course: bool,
    defs: Vec<TypeDef>,
}

#[derive(Clone, Debug)]
struct CourseInfo {
    id: String,
    code: String,
    defs: Vec<TypeDef>,
}

#[derive(Clone, Debug)]
struct ProjectInfo {
    id: String,
    space_id: String,
    title: String,
    open: bool,
    defs: Vec<TypeDef>,
}

/// What [`inherit`] reads: every home and its types, the defaults, and what
/// finished tasks say about how long each type really takes.
#[derive(Clone, Debug)]
pub(crate) struct Ctx {
    school: School,
    spaces: Vec<SpaceInfo>,
    courses: Vec<CourseInfo>,
    projects: Vec<ProjectInfo>,
    global: Vec<TypeDef>,
    samples: Vec<Sample>,
}

/// The school as these rules read it: its course pattern, the default when
/// the one in Settings doesn't read.
pub(crate) fn school_of(world: &World) -> School {
    School::new("", &world.course_pattern, jiff::tz::TimeZone::UTC)
        .or_else(|_| School::new("", DEFAULT_COURSE_PATTERN, jiff::tz::TimeZone::UTC))
        .expect("the default pattern reads")
}

impl Ctx {
    pub fn load(world: &World) -> Ctx {
        let mut ctx = Ctx {
            school: school_of(world),
            spaces: Vec::new(),
            courses: Vec::new(),
            projects: Vec::new(),
            global: types::defaults(),
            samples: Vec::new(),
        };
        world.raw_spaces.iter().for_each(|r| ctx.set_space(r));
        world.raw_courses.iter().for_each(|r| ctx.set_course(r));
        world.raw_projects.iter().for_each(|r| ctx.set_project(r));
        ctx.samples = ctx.samples_of(world);
        ctx
    }

    /// A space as it is about to be written, in place of the one stored.
    pub fn set_space(&mut self, r: &Value) {
        let info = SpaceInfo {
            id: r["id"].as_str().unwrap_or_default().to_string(),
            name: r["name"].as_str().unwrap_or_default().to_string(),
            persona: r["persona"].as_str().unwrap_or_default().to_string(),
            by_course: r["groupKind"] == "course",
            defs: defs_of(r, "typeDefs"),
        };
        match self.spaces.iter_mut().find(|s| s.id == info.id) {
            Some(s) => *s = info,
            None => self.spaces.push(info),
        }
    }

    pub fn set_course(&mut self, r: &Value) {
        let info = CourseInfo {
            id: r["id"].as_str().unwrap_or_default().to_string(),
            code: r["code"].as_str().unwrap_or_default().to_string(),
            defs: defs_of(r, "types"),
        };
        match self.courses.iter_mut().find(|c| c.id == info.id) {
            Some(c) => *c = info,
            None => self.courses.push(info),
        }
    }

    pub fn set_project(&mut self, r: &Value) {
        let info = ProjectInfo {
            id: r["id"].as_str().unwrap_or_default().to_string(),
            space_id: r["spaceId"].as_str().unwrap_or_default().to_string(),
            title: r["title"].as_str().unwrap_or_default().to_string(),
            open: matches!(r["status"].as_str(), Some("active") | None),
            defs: defs_of(r, "types"),
        };
        match self.projects.iter_mut().find(|p| p.id == info.id) {
            Some(p) => *p = info,
            None => self.projects.push(info),
        }
    }

    fn levels(&self, course: &str, project: &str, space: &str) -> Levels<'_> {
        let home = self
            .courses
            .iter()
            .find(|c| !course.is_empty() && c.id == course)
            .map(|c| (Level::Course, c.defs.as_slice()))
            .or_else(|| {
                self.projects
                    .iter()
                    .find(|p| !project.is_empty() && p.id == project)
                    .map(|p| (Level::Project, p.defs.as_slice()))
            });
        Levels {
            home,
            space: self
                .spaces
                .iter()
                .find(|s| s.id == space)
                .map_or(&[], |s| s.defs.as_slice()),
            global: &self.global,
        }
    }

    fn levels_of(&self, t: &Task) -> Levels<'_> {
        self.levels(
            t.course_id.as_deref().unwrap_or(""),
            t.project_id.as_deref().unwrap_or(""),
            &t.space_id,
        )
    }

    /// Each finished task with measured time, against what its type says now.
    fn samples_of(&self, world: &World) -> Vec<Sample> {
        let mut focus: HashMap<&str, f64> = HashMap::new();
        for s in &world.sessions {
            if let Some(id) = s.task_id.as_deref() {
                *focus.entry(id).or_insert(0.0) += s.focus_min;
            }
        }
        world
            .tasks
            .iter()
            .filter(|t| t.done)
            .filter_map(|t| {
                let actual = focus.get(t.id.as_str()).copied().unwrap_or(0.0) + t.adjust_min;
                let fill = types::fill(&t.r#type, &t.title, &self.levels_of(t));
                (actual > 0.0 && fill.from.is_some()).then(|| Sample {
                    home: types::home_key(t),
                    space_id: t.space_id.clone(),
                    kind: t.r#type.trim().to_lowercase(),
                    base_min: fill.est_min,
                    actual_min: actual,
                    done_at: t.done_at.unwrap_or(0.0),
                })
            })
            .collect()
    }

    /// Where a task's numbers come from as it stands: the level its type is
    /// found at, or None when nothing matches.
    pub fn level_of(&self, t: &Task) -> Option<Level> {
        types::fill(&t.r#type, &t.title, &self.levels_of(t)).from
    }

    /// The type names a picker may offer.
    pub fn type_names(&self) -> Value {
        let names = |defs: &[TypeDef]| defs.iter().map(|d| json!(d.name)).collect::<Vec<_>>();
        let by = |pairs: Vec<(&String, &Vec<TypeDef>)>| -> Value {
            Value::Object(
                pairs
                    .into_iter()
                    .filter(|(_, d)| !d.is_empty())
                    .map(|(id, d)| (id.clone(), Value::Array(names(d))))
                    .collect(),
            )
        };
        json!({
            "global": names(&self.global),
            "spaces": by(self.spaces.iter().map(|s| (&s.id, &s.defs)).collect()),
            "courses": by(self.courses.iter().map(|c| (&c.id, &c.defs)).collect()),
            "projects": by(self.projects.iter().map(|p| (&p.id, &p.defs)).collect()),
        })
    }
}

/// A project a title names: an open one of the same space whose whole title
/// is in the task's, the longest such winning. One word isn't a name unless
/// the task leads with it ("WWAV: order the PCBs").
fn project_in<'a>(ctx: &'a Ctx, space: &str, title: &str) -> Option<&'a ProjectInfo> {
    let title = types::words(title);
    ctx.projects
        .iter()
        .filter(|p| p.open && p.space_id == space)
        .filter_map(|p| {
            let name = types::words(&p.title);
            let found = match name.len() {
                0 => false,
                1 => title.first() == name.first() && title.len() > 1,
                n => title.windows(n).any(|w| w == name.as_slice()),
            };
            found.then_some((name.len(), p))
        })
        .max_by_key(|(n, _)| *n)
        .map(|(_, p)| p)
}

/// Brings a task record to what its home and its type say, in place. `first`
/// is for a task that has never been through here (a new one, or one from
/// before types): it is also matched to a home. `force` gives the person's
/// own minutes and difficulty back to the type. Returns whether it changed.
pub(crate) fn inherit(m: &mut Map<String, Value>, ctx: &Ctx, first: bool, force: bool) -> bool {
    let before = m.clone();
    let title = text(m, "title").to_string();
    let space_id = text(m, "spaceId").to_string();
    let by_course = ctx.spaces.iter().any(|s| s.id == space_id && s.by_course);

    if first && text(m, "courseId").is_empty() && text(m, "projectId").is_empty() {
        // A group that is a course's code is that course, in any space; a
        // code in a title is one only where tasks are grouped by course, so a
        // course is never forced onto work that isn't school.
        let group = text(m, "group").to_string();
        let by_group = offering_in(&ctx.school.course_pattern, &group)
            .filter(|o| squash(&o.code) == squash(&group) || o.is_whole());
        let named = by_group.or_else(|| {
            by_course
                .then(|| offering_in(&ctx.school.course_pattern, &title))
                .flatten()
        });
        let course = named.and_then(|o| {
            ctx.courses
                .iter()
                .find(|c| squash(&c.code) == squash(&o.code))
        });
        if let Some(c) = course {
            m.insert("courseId".into(), json!(c.id));
        } else if let Some(p) = project_in(ctx, &space_id, &title) {
            m.insert("projectId".into(), json!(p.id));
        }
    }

    // Who made each number, for a task from before anyone wrote it down: an
    // estimate that is there is the person's, and so is a difficulty that
    // isn't the old default of 3.
    if text(m, "estBy").is_empty() {
        let has = m.get("estMin").and_then(Value::as_f64).is_some_and(|x| x > 0.0);
        m.insert("estBy".into(), json!(if has { "you" } else { "default" }));
    }
    if text(m, "difficultyBy").is_empty() {
        let set = m
            .get("difficulty")
            .and_then(Value::as_f64)
            .is_some_and(|d| d != 3.0);
        m.insert(
            "difficultyBy".into(),
            json!(if set { "you" } else { "default" }),
        );
    }
    // The feed's own rule made a Brightspace task's type, unless it has been changed since.
    if first
        && text(m, "typeBy").is_empty()
        && text(m, "source") == "ical"
        && text(m, "type") == task_type(&title, &classes_types())
    {
        m.insert("typeBy".into(), json!("rule"));
    }

    let levels = ctx.levels(text(m, "courseId"), text(m, "projectId"), &space_id);
    if text(m, "type").trim().is_empty() || text(m, "typeBy") == "rule" {
        let picked = levels
            .match_title(&title)
            .map(|(_, d)| d.name.clone())
            // Where tasks are grouped by course, the words a class uses.
            .or_else(|| {
                by_course
                    .then(|| task_type(&title, &classes_types()))
                    .filter(|t| t != OTHER)
            })
            .unwrap_or_else(|| OTHER.to_string());
        m.insert("type".into(), json!(picked));
        m.insert("typeBy".into(), json!("rule"));
    }

    let kind_of = text(m, "type").to_string();
    let fill = types::fill(&kind_of, &title, &levels);
    let made = if fill.from.is_some() { "type" } else { "default" };
    if force || matches!(text(m, "estBy"), "type" | "default") {
        let home = match (text(m, "courseId"), text(m, "projectId")) {
            ("", "") => None,
            ("", p) => Some(format!("p:{p}")),
            (c, _) => Some(format!("c:{c}")),
        };
        let ratio = match fill.from {
            Some(_) => types::ratio(&ctx.samples, home.as_deref(), &space_id, &kind_of),
            None => 1.0,
        };
        let minutes = num(types::tidy_minutes(fill.est_min * ratio));
        if m.get("estMin") != Some(&minutes) || text(m, "estBy") != made {
            m.insert("estMin".into(), minutes);
            m.insert("estBy".into(), json!(made));
            m.remove("estReason");
        }
    }
    if force || matches!(text(m, "difficultyBy"), "type" | "default") {
        m.insert("difficulty".into(), num(fill.difficulty));
        m.insert("difficultyBy".into(), json!(made));
    }
    *m != before
}

/// A task's home as the views show it: its course ("ELE 209 · Intro to
/// Computer Systems Lab") or its project. None: the task shows its space.
pub(crate) fn home_of(world: &World, task: &Task) -> Option<Value> {
    if let Some(c) = task
        .course_id
        .as_deref()
        .and_then(|id| world.courses.iter().find(|c| c.id == id))
    {
        return Some(
            json!({"kind": "course", "id": c.id, "label": course_label(&c.code, &c.name)}),
        );
    }
    task.project_id
        .as_deref()
        .and_then(|id| world.projects.iter().find(|p| p.id == id))
        .map(|p| json!({"kind": "project", "id": p.id, "label": p.title}))
}

/// The category a grade's title suggests: the course's assignment type the
/// title picks says which category it counts toward; else the categories'
/// own keywords.
pub(crate) fn guess_category(course: &Value, title: &str) -> Value {
    let categories: Vec<wi_heat::model::records::GradeCategory> = course["categories"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| serde_json::from_value(c.clone()).ok())
        .collect();
    let by_type = types::match_title(title, &defs_of(course, "types"))
        .and_then(|d| d.category.as_deref())
        .and_then(|name| {
            categories
                .iter()
                .find(|c| c.name.trim().eq_ignore_ascii_case(name.trim()))
        })
        .map(|c| c.id.clone());
    by_type
        .or_else(|| grades::guess_category(title, &categories))
        .map_or(Value::Null, Value::String)
}

// ----- courses a sync finds --------------------------------------------------

/// A course something names.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Sighting {
    pub code: String,
    pub name: Option<String>,
    pub term: Option<String>,
    /// Enough to make a course: an offering's full name, the feed's own
    /// course, or Claude's word on a school thread. A bare code in a subject
    /// isn't: it only finds a course that is there.
    pub sure: bool,
}

/// What a thread says of a course: its subject, and the code Claude gave it.
pub(crate) fn mail_sightings(school: &School, thread: &Value) -> Vec<Sighting> {
    let mut out = Vec::new();
    let subject = thread["subject"].as_str().unwrap_or("");
    if let Some(o) = offering_in(&school.course_pattern, subject).filter(|o| o.is_whole()) {
        out.push(Sighting { code: o.code, name: o.name, term: o.term, sure: true });
    }
    // Claude's word makes a course only for mail it sorted as school, or left unsorted.
    let school_mail = matches!(thread["category"].as_str(), None | Some("school"));
    if let Some(code) = thread["course"].as_str().map(str::trim).filter(|c| !c.is_empty()) {
        let found = offering_in(&school.course_pattern, &code.to_uppercase());
        // What Claude wrote has to be a code and nothing else.
        if let Some(o) = found.filter(|o| squash(&o.code) == squash(code)) {
            out.push(Sighting { code: o.code, name: None, term: None, sure: school_mail });
        }
    }
    out
}

/// What [`courses_from`] decided: records to write, whole.
#[derive(Clone, Debug, Default)]
pub(crate) struct Found {
    pub terms: Vec<Value>,
    /// New courses, and courses that gained their name.
    pub courses: Vec<Value>,
    pub new: usize,
}

impl Found {
    /// The id of the course with this code, among the world's and the new.
    pub fn id_of(&self, world: &World, code: &str) -> Option<String> {
        let want = squash(code);
        self.courses
            .iter()
            .chain(world.raw_courses.iter())
            .find(|c| squash(c["code"].as_str().unwrap_or("")) == want)
            .and_then(|c| c["id"].as_str().map(str::to_string))
    }

    pub fn writes(&self) -> Vec<(&'static str, Value)> {
        self.terms
            .iter()
            .map(|t| (kind::TERM, t.clone()))
            .chain(self.courses.iter().map(|c| (kind::COURSE, c.clone())))
            .collect()
    }
}

/// The id of the term called `name`, made if it isn't there. With no name:
/// the newest term, or one named for today.
fn term_for(world: &World, made: &mut Vec<Value>, clock: &Clock, name: Option<&str>) -> String {
    let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => match (made.last(), world.terms.last()) {
            (Some(t), _) => return t["id"].as_str().unwrap_or_default().to_string(),
            (None, Some(t)) => return t.id.clone(),
            (None, None) => term_name(&clock.today()),
        },
    };
    if let Some(t) = world.terms.iter().find(|t| t.name.trim().eq_ignore_ascii_case(&name)) {
        return t.id.clone();
    }
    if let Some(t) = made
        .iter()
        .find(|t| t["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(&name)))
    {
        return t["id"].as_str().unwrap_or_default().to_string();
    }
    let id = ulid();
    made.push(json!({"id": id, "name": name}));
    id
}

/// The courses `sightings` call for. A sure sighting of a code Learn doesn't
/// hold makes a stub course, flagged as needing its syllabus; a sighting that
/// carries a name gives it to a course still named by its code. Nothing a
/// person named or weighted is touched.
pub(crate) fn courses_from(world: &World, clock: &Clock, sightings: &[Sighting]) -> Found {
    let mut found = Found::default();
    for s in sightings {
        let want = squash(&s.code);
        if want.is_empty() {
            continue;
        }
        let nameless = |c: &Value| {
            let name = c["name"].as_str().unwrap_or("");
            name.trim().is_empty() || squash(name) == squash(c["code"].as_str().unwrap_or(""))
        };
        if let Some(c) = found
            .courses
            .iter_mut()
            .find(|c| squash(c["code"].as_str().unwrap_or("")) == want)
        {
            if let (true, Some(name)) = (nameless(c), &s.name) {
                c["name"] = json!(name);
            }
            continue;
        }
        if let Some(c) = world
            .raw_courses
            .iter()
            .find(|c| squash(c["code"].as_str().unwrap_or("")) == want)
        {
            if let (true, Some(name)) = (nameless(c), &s.name) {
                let mut c = c.clone();
                c["name"] = json!(name);
                found.courses.push(c);
            }
            continue;
        }
        if !s.sure {
            continue;
        }
        let term = term_for(world, &mut found.terms, clock, s.term.as_deref());
        let code = kept_code(&school_of(world).course_pattern, &s.code);
        found.courses.push(json!({
            "id": ulid(), "termId": term, "code": code,
            "name": s.name.clone().unwrap_or_else(|| code.clone()),
            "categories": [], "notes": "", "status": "stub", "public": false,
        }));
        found.new += 1;
    }
    found
}

/// Everything a library holds that names a course: its mail, its other
/// calendars' events, and old groups that are a course's code.
fn sightings_in(world: &World, ctx: &Ctx) -> Vec<Sighting> {
    let pattern = &ctx.school.course_pattern;
    let mut out: Vec<Sighting> = Vec::new();
    for thread in &world.mail {
        out.extend(mail_sightings(&ctx.school, thread));
    }
    for e in &world.events {
        if let Some(o) = offering_in(pattern, &e.title).filter(|o| o.is_whole()) {
            out.push(Sighting { code: o.code, name: o.name, term: o.term, sure: true });
        }
    }
    for t in &world.tasks {
        let group = t.group.as_deref().unwrap_or("").trim();
        if let Some(o) = offering_in(pattern, group)
            .filter(|o| o.is_whole() || squash(&o.code) == squash(group))
        {
            out.push(Sighting { code: o.code, name: o.name, term: o.term, sure: true });
        }
    }
    out
}

/// Brings a library up to date with homes and types: courses its mail and
/// calendars name are made (stubs, "needs syllabus") or given their names,
/// old groups that are course codes become courses with their tasks linked,
/// and every open task that has never had a type gets one, with its minutes
/// and difficulty. One journal entry, so ⌘Z takes it back; nothing is
/// deleted, and nothing a person typed is changed. Run after every sync, it
/// finds nothing more to do than what the sync brought.
pub fn tidy(store: &mut Store, clock: &Clock) -> Result<Outcome> {
    let world = World::load(store)?;
    let mut ctx = Ctx::load(&world);
    let mut found = courses_from(&world, clock, &sightings_in(&world, &ctx));
    // A course from before courses had a status: one nobody has weighted is a stub.
    for c in &world.raw_courses {
        if c.get("status").is_some() {
            continue;
        }
        let id = c["id"].as_str().unwrap_or_default();
        let weighted = world
            .courses
            .iter()
            .find(|k| k.id == id)
            .is_some_and(|k| grades::weights_total(k) > 0.0);
        let status = json!(if weighted { "confirmed" } else { "stub" });
        match found.courses.iter_mut().find(|k| k["id"] == c["id"]) {
            Some(k) => k["status"] = status,
            None => {
                let mut k = c.clone();
                k["status"] = status;
                found.courses.push(k);
            }
        }
    }
    found.courses.iter().for_each(|c| ctx.set_course(c));

    let mut tasks: Vec<Value> = Vec::new();
    for (i, t) in world.tasks.iter().enumerate() {
        let raw = &world.raw_tasks[i];
        if t.done || raw.get("difficultyBy").is_some() {
            continue;
        }
        let mut m = raw.as_object().cloned().unwrap_or_default();
        if inherit(&mut m, &ctx, true, false) {
            tasks.push(Value::Object(m));
        }
    }
    let writes = found.writes();
    let c = commit(store, "course and type setup", Actor::You, |txn| {
        for (k, r) in &writes {
            put(txn, k, r)?;
        }
        for t in &tasks {
            put(txn, kind::TASK, t)?;
        }
        Ok(())
    })?;
    set_setting(store, TIDIED, &json!(TIDY_VERSION))?;
    Ok(Outcome::new(
        json!({"courses": found.new, "tasks": tasks.len()}),
        c,
    ))
}

/// Whether this library has been through [`tidy`] yet.
pub fn tidied(store: &Store) -> Result<bool> {
    Ok(setting(store, TIDIED)?.and_then(|v| v.as_i64()) == Some(TIDY_VERSION))
}

// ----- when a home's types change -------------------------------------------

/// The tasks and grades a write to a home moves, to go in the same entry:
/// when its types changed, its open tasks that nobody estimated by hand are
/// filled again; when a course's types or categories changed, its grades
/// without a category are sorted. `old` is None for a new home.
pub(crate) fn after_home_write(
    world: &World,
    k: &str,
    old: Option<&Value>,
    new: &Value,
) -> Vec<(&'static str, Value)> {
    let field = if k == kind::SPACE { "typeDefs" } else { "types" };
    let changed = |f: &str| old.map_or(new.get(f).is_some(), |o| o.get(f) != new.get(f));
    let id = new["id"].as_str().unwrap_or_default();
    let mut out: Vec<(&'static str, Value)> = Vec::new();
    if changed(field) {
        let mut ctx = Ctx::load(world);
        match k {
            kind::SPACE => ctx.set_space(new),
            kind::COURSE => ctx.set_course(new),
            _ => ctx.set_project(new),
        }
        for (i, t) in world.tasks.iter().enumerate() {
            let mine = match k {
                kind::SPACE => t.space_id == id,
                kind::COURSE => t.course_id.as_deref() == Some(id),
                _ => t.project_id.as_deref() == Some(id),
            };
            if !mine || t.done {
                continue;
            }
            let mut m = world.raw_tasks[i].as_object().cloned().unwrap_or_default();
            if inherit(&mut m, &ctx, false, false) {
                out.push((kind::TASK, Value::Object(m)));
            }
        }
    }
    if k == kind::COURSE && (changed("types") || changed("categories")) {
        out.extend(regraded(world, new));
    }
    out
}

/// A course's grades that have no category, given the one their title picks.
fn regraded(world: &World, course: &Value) -> Vec<(&'static str, Value)> {
    world
        .raw_grades
        .iter()
        .filter(|g| g["courseId"] == course["id"] && g["categoryId"].is_null())
        .filter_map(|g| {
            let category = guess_category(course, g["title"].as_str().unwrap_or(""));
            (!category.is_null()).then(|| {
                let mut g = g.clone();
                g["categoryId"] = category;
                (kind::GRADE, g)
            })
        })
        .collect()
}

/// `heat.course.update`: a course's fields changed by hand, which confirms
/// it, and its types applied again to its tasks in the same entry.
pub fn course_update(
    store: &mut Store,
    clock: &Clock,
    id: &str,
    set: &Map<String, Value>,
) -> Result<Outcome> {
    let mut set = set.clone();
    set.entry("status").or_insert(json!("confirmed"));
    crate::ops::patch_record(store, clock, kind::COURSE, id, &set)
}

/// `heat.project.update`: the same for a project and its own types.
pub fn project_update(
    store: &mut Store,
    clock: &Clock,
    id: &str,
    set: &Map<String, Value>,
) -> Result<Outcome> {
    crate::ops::patch_record(store, clock, kind::PROJECT, id, set)
}

/// `heat.task.setType`: the person picks a task's type, and what they
/// haven't set by hand follows it. None gives the choice back to the title.
pub fn set_type(
    store: &mut Store,
    _clock: &Clock,
    task_id: &str,
    kind_of: Option<&str>,
) -> Result<Outcome> {
    let world = World::load(store)?;
    let Some(i) = world.task_index(task_id) else {
        return refused("No task has that id.");
    };
    let mut m = world.raw_tasks[i].as_object().cloned().unwrap_or_default();
    match kind_of.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => {
            m.insert("type".into(), json!(t));
            m.insert("typeBy".into(), json!("you"));
        }
        None => {
            m.insert("typeBy".into(), json!("rule"));
        }
    }
    inherit(&mut m, &Ctx::load(&world), false, false);
    let record = Value::Object(m);
    let c = commit(store, "set type", Actor::You, |txn| {
        put(txn, kind::TASK, &record)
    })?;
    Ok(Outcome::new(json!({ "task": record }), c))
}

/// `heat.task.reapplyDefaults`: types applied again to one task, a home's
/// tasks, a space's, or all. Only what nobody set by hand moves, unless
/// `force` gives the person's own numbers back to the type. Open tasks only,
/// but for a task asked for by id.
pub fn reapply(
    store: &mut Store,
    _clock: &Clock,
    scope: &Map<String, Value>,
    force: bool,
) -> Result<Outcome> {
    let world = World::load(store)?;
    let ctx = Ctx::load(&world);
    let named = |k: &str| scope.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
    let (one_task, course, project, space) = (
        named("taskId"),
        named("courseId"),
        named("projectId"),
        named("spaceId"),
    );
    let everything = scope.get("all") == Some(&json!(true));
    if let Some(id) = one_task {
        if world.task_index(id).is_none() {
            return refused("No task has that id.");
        }
    } else if !everything && course.is_none() && project.is_none() && space.is_none() {
        return refused("Say which tasks: one task, a course, a project, a space, or all.");
    }
    let mut writes: Vec<Value> = Vec::new();
    for (i, t) in world.tasks.iter().enumerate() {
        let mine = match one_task {
            Some(id) => t.id == id,
            None => {
                !t.done
                    && (everything
                        || course.is_some_and(|c| t.course_id.as_deref() == Some(c))
                        || project.is_some_and(|p| t.project_id.as_deref() == Some(p))
                        || space.is_some_and(|s| t.space_id == s))
            }
        };
        if !mine {
            continue;
        }
        let mut m = world.raw_tasks[i].as_object().cloned().unwrap_or_default();
        if inherit(&mut m, &ctx, false, force) {
            writes.push(Value::Object(m));
        }
    }
    let c = commit(store, "reapply defaults", Actor::You, |txn| {
        for t in &writes {
            put(txn, kind::TASK, t)?;
        }
        Ok(())
    })?;
    Ok(Outcome::new(json!({ "changed": writes.len() }), c))
}

// ----- Claude's batch --------------------------------------------------------

/// Open tasks no type matched, soonest due first, for one call to Claude. A
/// task asked about in the last day waits, so a call that fails or is
/// answered in part isn't made again on every sync; `all` asks regardless.
pub fn unscored(store: &Store, clock: &Clock, all_of_them: bool) -> Result<Vec<Unscored>> {
    let world = World::load(store)?;
    let ctx = Ctx::load(&world);
    let asked = setting(store, "score.asked")?.unwrap_or_else(|| json!({}));
    let parents: Vec<&str> = world
        .tasks
        .iter()
        .filter(|t| !t.done)
        .filter_map(|t| t.parent_task_id.as_deref())
        .collect();
    let mut open: Vec<(usize, &Task)> = world
        .tasks
        .iter()
        .enumerate()
        .filter(|(i, t)| {
            let raw = &world.raw_tasks[*i];
            !t.done
                && raw["estBy"] == "default"
                && raw.get("difficultyBy").is_some()
                && !parents.contains(&t.id.as_str())
                && (all_of_them
                    || asked[&t.id]
                        .as_f64()
                        .map_or(true, |at| clock.now_ms - at >= ASK_AGAIN_MS))
        })
        .collect();
    open.sort_by(|a, b| {
        a.1.due
            .unwrap_or(f64::MAX)
            .total_cmp(&b.1.due.unwrap_or(f64::MAX))
    });
    Ok(open
        .into_iter()
        .take(rules::SCORE_BATCH)
        .map(|(_, t)| {
            let space = ctx.spaces.iter().find(|s| s.id == t.space_id);
            Unscored {
                id: t.id.clone(),
                title: t.title.clone(),
                kind: t.r#type.clone(),
                space: space.map(|s| s.name.clone()).unwrap_or_default(),
                persona: space.map(|s| s.persona.clone()).unwrap_or_default(),
                home: home_of(&world, t).and_then(|h| h["label"].as_str().map(str::to_string)),
            }
        })
        .collect())
}

/// How many open tasks wait for an estimate better than the catch-all.
pub(crate) fn unscored_count(world: &World) -> usize {
    world
        .tasks
        .iter()
        .enumerate()
        .filter(|(i, t)| !t.done && world.raw_tasks[*i]["estBy"] == "default")
        .count()
}

/// Notes that Claude was asked about these tasks now. Outside the journal.
pub fn mark_asked(store: &mut Store, clock: &Clock, ids: &[String]) -> Result<()> {
    let mut asked: Map<String, Value> = setting(store, "score.asked")?
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    // Tasks asked about long ago are asked again anyway: forget them.
    asked.retain(|_, at| at.as_f64().is_some_and(|at| clock.now_ms - at < ASK_AGAIN_MS));
    for id in ids {
        asked.insert(id.clone(), num(clock.now_ms));
    }
    set_setting(store, "score.asked", &Value::Object(asked))
}

/// Claude's scores for a batch, as one entry, "Claude's estimates". A task
/// that got a type, an estimate of the person's or was finished while Claude
/// thought is left as it is.
pub fn apply_scores(store: &mut Store, _clock: &Clock, scores: &[Score]) -> Result<Outcome> {
    let world = World::load(store)?;
    let mut writes: Vec<Value> = Vec::new();
    for s in scores {
        let Some(i) = world.task_index(&s.id) else {
            continue;
        };
        let mut t = world.raw_tasks[i].clone();
        if world.tasks[i].done || t["estBy"] != "default" {
            continue;
        }
        t["estMin"] = num(s.est_min);
        t["estBy"] = json!("claude");
        t["estReason"] = json!(s.reason);
        if t["difficultyBy"] != "you" {
            t["difficulty"] = num(s.difficulty);
            t["difficultyBy"] = json!("claude");
        }
        writes.push(t);
    }
    let actor = Actor::Claude {
        tool: "score_tasks".into(),
        reason: "No type matched these tasks, so Claude estimated them.".into(),
    };
    let c = commit(store, "Claude's estimates", actor, |txn| {
        for t in &writes {
            put(txn, kind::TASK, t)?;
        }
        Ok(())
    })?;
    Ok(Outcome::new(json!({ "scored": writes.len() }), c))
}

// ----- the syllabus ----------------------------------------------------------

fn save_draft(store: &mut Store, draft: &Value) -> Result<()> {
    let id = draft["id"].as_str().unwrap_or_default();
    store.set_doc(kind::SYLLABUS, id, draft, "")?;
    Ok(())
}

/// A syllabus dropped on Learn, waiting to be read: its file, and the course
/// it was dropped on, if any. Nothing of the course changes. The draft is a
/// record of this Mac alone, so the file's path stays here.
pub fn draft_begin(
    store: &mut Store,
    clock: &Clock,
    course_id: Option<&str>,
    file_name: &str,
    path: Option<&str>,
) -> Result<Value> {
    if let Some(id) = course_id {
        if one(store, kind::COURSE, id)?.is_none() {
            return refused("That course isn't in Learn any more.");
        }
    }
    let mut draft = json!({
        "id": ulid(), "courseId": course_id, "fileName": file_name, "pages": 0,
        "createdAt": num(clock.now_ms), "state": "reading",
    });
    if let Some(path) = path {
        draft["path"] = json!(path);
    }
    save_draft(store, &draft)?;
    Ok(draft)
}

/// The drafts whose file hasn't been read yet, as stored, oldest first.
pub fn drafts_to_read(store: &Store) -> Result<Vec<Value>> {
    Ok(all(store, kind::SYLLABUS)?
        .into_iter()
        .filter(|d| d["state"] == "reading" && d["path"].is_string())
        .collect())
}

/// How many pages a draft's file turned out to have.
pub fn draft_pages(store: &mut Store, id: &str, pages: usize) -> Result<()> {
    if let Some(mut draft) = one(store, kind::SYLLABUS, id)? {
        draft["pages"] = json!(pages);
        save_draft(store, &draft)?;
    }
    Ok(())
}

/// What a draft's course is called, for telling Claude where it was dropped.
pub fn draft_course_label(store: &Store, draft: &Value) -> Result<Option<String>> {
    let Some(id) = draft["courseId"].as_str() else {
        return Ok(None);
    };
    Ok(one(store, kind::COURSE, id)?.map(|c| {
        course_label(
            c["code"].as_str().unwrap_or(""),
            c["name"].as_str().unwrap_or(""),
        )
    }))
}

/// The reading didn't work: the draft says why, until it is dismissed.
pub fn draft_fail(store: &mut Store, id: &str, sentence: &str) -> Result<()> {
    if let Some(mut draft) = one(store, kind::SYLLABUS, id)? {
        draft["state"] = json!("failed");
        draft["error"] = json!(sentence);
        save_draft(store, &draft)?;
    }
    Ok(())
}

/// Claude's answer for a draft, checked and kept. The draft is ready to
/// preview; still nothing of the course has changed.
pub fn draft_ready(store: &mut Store, clock: &Clock, id: &str, answer: &Value) -> Result<Value> {
    let Some(mut draft) = one(store, kind::SYLLABUS, id)? else {
        return refused("That syllabus was dismissed.");
    };
    let syllabus = rules::parse_syllabus(answer).map_err(crate::Error::Refused)?;
    draft["answer"] = serde_json::to_value(&syllabus).unwrap_or(Value::Null);
    draft["state"] = json!("ready");
    if let Some(m) = draft.as_object_mut() {
        m.remove("error");
    }
    // It has to plan before it is offered: a syllabus that names no course
    // and was dropped on none can't be previewed.
    let world = World::load(store)?;
    plan(&world, &Ctx::load(&world), clock, &draft)?;
    save_draft(store, &draft)?;
    view(&world, clock, &draft)
}

/// `heat.course.importSyllabus {json}`: a syllabus that is JSON already, as a
/// ready draft in one step.
pub fn draft_from_json(
    store: &mut Store,
    clock: &Clock,
    course_id: Option<&str>,
    file_name: &str,
    answer: &Value,
) -> Result<Value> {
    let draft = draft_begin(store, clock, course_id, file_name, None)?;
    let id = draft["id"].as_str().unwrap_or_default().to_string();
    match draft_ready(store, clock, &id, answer) {
        Ok(v) => Ok(v),
        Err(e) => {
            store.remove_doc(kind::SYLLABUS, &id)?;
            Err(e)
        }
    }
}

/// `heat.syllabus.discard`
pub fn draft_discard(store: &mut Store, id: &str) -> Result<Outcome> {
    store.remove_doc(kind::SYLLABUS, id)?;
    Ok(Outcome::outside(json!({}), &[kind::SYLLABUS]))
}

/// What accepting a draft would write, worked out against the library as it
/// stands now.
struct Plan {
    course: Value,
    is_new: bool,
    term: Option<Value>,
    term_name: String,
    syllabus: Syllabus,
    types: Vec<Value>,
    new_tasks: Vec<Value>,
    /// Tasks already here that the import moves: (as it would be, due before, due after).
    changed: Vec<(Value, Option<(Option<f64>, f64)>)>,
    grades: Vec<(&'static str, Value)>,
}

fn plan(world: &World, ctx: &Ctx, clock: &Clock, draft: &Value) -> Result<Plan> {
    let syllabus: Syllabus =
        rules::parse_syllabus(&draft["answer"]).map_err(crate::Error::Refused)?;
    let code = syllabus.course.code.as_deref().map(|c| {
        offering_in(&ctx.school.course_pattern, c)
            .map(|o| o.code)
            .unwrap_or_else(|| kept_code(&ctx.school.course_pattern, c))
    });
    let existing = match (draft["courseId"].as_str(), &code) {
        (Some(id), _) => match world.raw_courses.iter().find(|c| c["id"] == id) {
            Some(c) => Some(c.clone()),
            None => return refused("That course isn't in Learn any more."),
        },
        (None, Some(code)) => world
            .raw_courses
            .iter()
            .find(|c| squash(c["code"].as_str().unwrap_or("")) == squash(code))
            .cloned(),
        (None, None) => {
            return refused(
                "The syllabus doesn't say which course it is. Drop it on the course in Grades.",
            )
        }
    };
    let is_new = existing.is_none();
    let mut made_terms: Vec<Value> = Vec::new();
    let mut course = match existing {
        Some(c) => c,
        None => {
            let code = code.clone().unwrap_or_default();
            let term = term_for(world, &mut made_terms, clock, syllabus.course.term.as_deref());
            json!({"id": ulid(), "termId": term, "code": code, "name": code, "categories": [], "notes": "", "public": false})
        }
    };
    let term_name = world
        .terms
        .iter()
        .find(|t| course["termId"] == json!(t.id))
        .map(|t| t.name.clone())
        .or_else(|| made_terms.first().and_then(|t| t["name"].as_str().map(str::to_string)))
        .unwrap_or_default();
    if let Some(name) = &syllabus.course.name {
        course["name"] = json!(name);
    }
    let course_id = course["id"].as_str().unwrap_or_default().to_string();

    // The syllabus's categories, keeping the id of one already here by that
    // name so its grades stay in it. A category the syllabus doesn't name
    // stays only if a grade is in it: nothing a grade rests on is lost.
    if !syllabus.weights.is_empty() {
        let old: Vec<Value> = course["categories"].as_array().cloned().unwrap_or_default();
        let same = |c: &Value, name: &str| {
            c["name"].as_str().is_some_and(|n| n.trim().eq_ignore_ascii_case(name))
        };
        let mut categories: Vec<Value> = syllabus
            .weights
            .iter()
            .map(|w| {
                let was = old.iter().find(|c| same(c, &w.category));
                let mut c = json!({
                    "id": was.and_then(|c| c["id"].as_str()).map_or_else(ulid, str::to_string),
                    "name": w.category,
                    "weight": num(w.percent),
                    "keywords": was.map_or(json!([]), |c| c["keywords"].clone()),
                });
                if let Some(n) = w.drop_lowest {
                    c["dropLowest"] = num(n);
                }
                c
            })
            .collect();
        for c in &old {
            let named = syllabus.weights.iter().any(|w| same(c, &w.category));
            let holds = world
                .raw_grades
                .iter()
                .any(|g| g["courseId"] == json!(course_id) && g["categoryId"] == c["id"]);
            if !named && holds {
                categories.push(c.clone());
            }
        }
        course["categories"] = Value::Array(categories);
    }
    if !syllabus.types.is_empty() {
        let defs: Vec<Value> = syllabus
            .types
            .iter()
            .map(|t| {
                let patterns = if t.title_patterns.is_empty() {
                    vec![t.name.to_lowercase()]
                } else {
                    t.title_patterns.clone()
                };
                let mut d = json!({
                    "name": t.name, "patterns": patterns,
                    "estMin": t.est_minutes.map_or(Value::Null, num),
                    "difficulty": t.difficulty.map_or(Value::Null, num),
                });
                if let Some(c) = &t.category {
                    d["category"] = json!(c);
                }
                d
            })
            .collect();
        course["types"] = Value::Array(defs);
    }
    course["status"] = json!("confirmed");
    course["syllabusSource"] = json!({
        "name": draft["fileName"], "pages": draft["pages"], "importedAt": num(clock.now_ms),
    });

    let mut ctx = ctx.clone();
    ctx.set_course(&course);
    // The types as the preview shows them: with the minutes and difficulty a
    // task of each would get, wherever those numbers come from.
    let levels = ctx.levels(&course_id, "", "");
    let types: Vec<Value> = defs_of(&course, "types")
        .iter()
        .map(|d| {
            let fill = types::fill(&d.name, &d.name, &levels);
            json!({
                "name": d.name, "patterns": d.patterns, "estMin": num(fill.est_min),
                "difficulty": num(fill.difficulty), "category": d.category,
            })
        })
        .collect();

    // Dated items: a task already in the course by that title follows the
    // syllabus's date, unless Brightspace keeps its date or it is done; one
    // that isn't here is made.
    let space = world
        .spaces
        .iter()
        .find(|s| s.group_kind == GroupKind::Course)
        .or_else(|| world.spaces.first());
    let mine: Vec<usize> = (0..world.tasks.len())
        .filter(|i| world.tasks[*i].course_id.as_deref() == Some(course_id.as_str()))
        .collect();
    let mut moved: HashMap<usize, (Option<f64>, f64)> = HashMap::new();
    let mut new_tasks: Vec<Value> = Vec::new();
    let course_code = course["code"].as_str().unwrap_or_default().to_string();
    for item in &syllabus.items {
        let Some(due) = item.due.as_deref().and_then(|d| rules::due_ms(d, &clock.zone)) else {
            continue;
        };
        let known = mine
            .iter()
            .copied()
            .filter(|i| same_title(&world.tasks[*i].title, &item.title))
            .min_by_key(|i| world.tasks[*i].done);
        if let Some(i) = known {
            let t = &world.tasks[i];
            let same_day = t.due.is_some_and(|d| clock.date_of(d) == clock.date_of(due));
            if !t.done && world.raw_tasks[i]["source"] != "ical" && !same_day {
                moved.insert(i, (t.due, due));
            }
            continue;
        }
        let source_id = format!(
            "syl-{:016x}",
            wwav_ids::fnv1a64(
                format!("{}|{}", squash(&course_code), item.title.to_lowercase()).as_bytes()
            )
        );
        let seen = world.raw_tasks.iter().any(|t| t["sourceId"] == json!(source_id))
            || new_tasks.iter().any(|t| t["sourceId"] == json!(source_id));
        if seen || due < clock.now_ms - PAST_MS {
            continue;
        }
        let Some(space) = space else {
            return refused("There's no space yet. Open Learn in Wi_WWAV once.");
        };
        let mut m = json!({
            "id": ulid(), "spaceId": space.id, "title": item.title, "courseId": course_id,
            "due": num(due), "adjustMin": 0, "notes": "", "done": false, "doneAt": null,
            "source": "claude", "sourceId": source_id,
            "claudeReason": format!("From the syllabus, {}.", draft["fileName"].as_str().unwrap_or("a PDF")),
            "public": false,
        })
        .as_object()
        .cloned()
        .unwrap_or_default();
        if let Some(kind_of) = &item.kind {
            m.insert("type".into(), json!(kind_of));
            m.insert("typeBy".into(), json!("claude"));
        }
        inherit(&mut m, &ctx, true, false);
        new_tasks.push(Value::Object(m));
    }
    let mut changed: Vec<(Value, Option<(Option<f64>, f64)>)> = Vec::new();
    for i in mine {
        if world.tasks[i].done {
            continue;
        }
        let mut m = world.raw_tasks[i].as_object().cloned().unwrap_or_default();
        let date = moved.get(&i).copied();
        if let Some((_, to)) = date {
            m.insert("due".into(), num(to));
        }
        if inherit(&mut m, &ctx, false, false) || date.is_some() {
            changed.push((Value::Object(m), date));
        }
    }
    Ok(Plan {
        grades: regraded(world, &course),
        course,
        is_new,
        term: made_terms.into_iter().next(),
        term_name,
        syllabus,
        types,
        new_tasks,
        changed,
    })
}

/// A draft as the views read it: with its preview when it is ready. A ready
/// draft whose course has gone since reads as failed, with why.
fn view(world: &World, clock: &Clock, draft: &Value) -> Result<Value> {
    let mut out = json!({
        "id": draft["id"], "courseId": draft["courseId"], "fileName": draft["fileName"],
        "pages": draft["pages"], "createdAt": draft["createdAt"], "state": draft["state"],
    });
    if let Some(e) = draft["error"].as_str() {
        out["error"] = json!(e);
    }
    if draft["state"] != "ready" {
        return Ok(out);
    }
    let p = match plan(world, &Ctx::load(world), clock, draft) {
        Ok(p) => p,
        Err(crate::Error::Refused(why)) => {
            out["state"] = json!("failed");
            out["error"] = json!(why);
            return Ok(out);
        }
        Err(e) => return Err(e),
    };
    let (code, name) = (
        p.course["code"].as_str().unwrap_or(""),
        p.course["name"].as_str().unwrap_or(""),
    );
    let label = course_label(code, name);
    let date_changes: Vec<Value> = p
        .changed
        .iter()
        .filter_map(|(t, date)| {
            date.map(|(from, to)| {
                json!({"taskId": t["id"], "title": t["title"], "from": from.map_or(Value::Null, num), "to": num(to)})
            })
        })
        .collect();
    let weights = &p.syllabus.weights;
    out["courseId"] = if p.is_new { Value::Null } else { p.course["id"].clone() };
    out["course"] = json!({"code": code, "name": name, "term": p.term_name, "label": label, "isNew": p.is_new});
    out["weights"] = weights
        .iter()
        .map(|w| json!({"category": w.category, "percent": num(w.percent), "dropLowest": w.drop_lowest.map_or(Value::Null, num)}))
        .collect();
    out["weightsTotal"] = num(crate::round(weights.iter().map(|w| w.percent).sum(), 1));
    out["weightsFlag"] = rules::weights_flag(weights).map_or(Value::Null, Value::String);
    out["line"] = json!(rules::preview_line(
        &label,
        weights,
        p.types.len(),
        p.new_tasks.len(),
        date_changes.len()
    ));
    out["types"] = Value::Array(p.types);
    out["newTasks"] = p
        .new_tasks
        .iter()
        .map(|t| json!({"title": t["title"], "type": t["type"], "due": t["due"], "estMin": t["estMin"]}))
        .collect();
    out["dateChanges"] = Value::Array(date_changes);
    Ok(out)
}

/// The syllabus drafts that wait, oldest first, for the snapshot.
pub(crate) fn drafts(store: &Store, world: &World, clock: &Clock) -> Result<Vec<Value>> {
    all(store, kind::SYLLABUS)?
        .iter()
        .map(|d| view(world, clock, d))
        .collect()
}

/// One draft as the views read it.
pub fn draft(store: &Store, clock: &Clock, id: &str) -> Result<Option<Value>> {
    let Some(d) = one(store, kind::SYLLABUS, id)? else {
        return Ok(None);
    };
    Ok(Some(view(&World::load(store)?, clock, &d)?))
}

/// `heat.syllabus.accept`: the draft becomes the course, as one entry,
/// "import syllabus": its weights and types, the new tasks, the dates that
/// moved, and the estimates its types change. One ⌘Z takes all of it back.
pub fn draft_accept(store: &mut Store, clock: &Clock, id: &str) -> Result<Outcome> {
    let Some(d) = one(store, kind::SYLLABUS, id)? else {
        return refused("That syllabus was dismissed.");
    };
    if d["state"] != "ready" {
        return refused("That syllabus isn't read yet.");
    }
    let world = World::load(store)?;
    let p = plan(&world, &Ctx::load(&world), clock, &d)?;
    let dates = p.changed.iter().filter(|(_, d)| d.is_some()).count();
    let c = commit(store, "import syllabus", Actor::You, |txn| {
        if let Some(t) = &p.term {
            put(txn, kind::TERM, t)?;
        }
        put(txn, kind::COURSE, &p.course)?;
        for t in p.new_tasks.iter().chain(p.changed.iter().map(|(t, _)| t)) {
            put(txn, kind::TASK, t)?;
        }
        for (k, g) in &p.grades {
            put(txn, k, g)?;
        }
        Ok(())
    })?;
    store.remove_doc(kind::SYLLABUS, id)?;
    Ok(Outcome::new(
        json!({
            "course": p.course,
            "counts": {"newTasks": p.new_tasks.len(), "dateChanges": dates, "retimed": p.changed.len() - dates},
        }),
        c,
    )
    .also(&[kind::SYLLABUS]))
}
