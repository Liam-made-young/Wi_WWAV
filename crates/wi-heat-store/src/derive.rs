//! Everything Heat works out, from `wi_heat::model`: heat order, the
//! estimate chain, Plan my day and grades. The tools and the snapshot read
//! these, so Claude and the window never disagree.

use serde_json::{json, Value};
use wi_heat::model::records::{
    CalendarEvent, Capture, Course, FocusSession, Grade, Habit, Milestone, Project, Space, Task,
    TaskOccurrence, Term, TimeBlock,
};
use wi_heat::model::{estimate, grades, heat, plan, recurrence, zone};
use wi_store::Store;

use crate::{all, kind, num, refused, round, Clock, Result};

/// "Day ends at", 11 PM by default (3.5).
pub const DAY_ENDS_MIN: f64 = 23.0 * 60.0;

/// Where the time column starts, 7 AM: a day ahead is planned from here.
const DAY_STARTS_MIN: f64 = 7.0 * 60.0;

/// Heat's records, read once per call. The raw JSON is kept beside the
/// typed record so fields the model doesn't read (who estimated, Claude's
/// reason, the Public switch) survive a write.
pub(crate) struct World {
    pub spaces: Vec<Space>,
    /// Beside `spaces`, `projects` and `courses`: the types each one keeps.
    pub raw_spaces: Vec<Value>,
    pub raw_projects: Vec<Value>,
    pub raw_courses: Vec<Value>,
    pub raw_tasks: Vec<Value>,
    pub tasks: Vec<Task>,
    pub occurrences: Vec<TaskOccurrence>,
    pub blocks: Vec<TimeBlock>,
    pub sessions: Vec<FocusSession>,
    pub habits: Vec<Habit>,
    pub projects: Vec<Project>,
    pub milestones: Vec<Milestone>,
    pub captures: Vec<Capture>,
    pub terms: Vec<Term>,
    pub courses: Vec<Course>,
    pub raw_grades: Vec<Value>,
    pub grades: Vec<Grade>,
    pub mail: Vec<Value>,
    pub events: Vec<CalendarEvent>,
    /// Commitments, breaks and sleep: what a plan steps around
    /// (`commit::Fixed`).
    pub fixed: crate::commit::Fixed,
    /// The School sheet's course pattern, for reading a course out of a title.
    pub course_pattern: String,
}

/// The records of one kind that read as their type, beside their raw JSON.
/// A record that doesn't read (one from a newer app, one half-written by
/// another view) is left out of the maths, and said so on stderr, rather than
/// stopping every command that reads Heat.
fn typed<T: serde::de::DeserializeOwned>(kind: &str, raw: Vec<Value>) -> (Vec<T>, Vec<Value>) {
    let mut out = Vec::with_capacity(raw.len());
    let mut kept = Vec::with_capacity(raw.len());
    for v in raw {
        match serde_json::from_value(v.clone()) {
            Ok(t) => {
                out.push(t);
                kept.push(v);
            }
            Err(e) => {
                let id = v.get("id").and_then(Value::as_str).unwrap_or("?");
                eprintln!(
                    "wi-heat-store: Learn's {kind} record {id} doesn't read, so it is left out: {e}"
                );
            }
        }
    }
    (out, kept)
}

impl World {
    pub fn load(store: &Store) -> Result<World> {
        let (tasks, raw_tasks) = typed(kind::TASK, all(store, kind::TASK)?);
        let (grades, raw_grades) = typed(kind::GRADE, all(store, kind::GRADE)?);
        let (spaces, raw_spaces) = typed(kind::SPACE, all(store, kind::SPACE)?);
        let (projects, raw_projects) = typed(kind::PROJECT, all(store, kind::PROJECT)?);
        let (courses, raw_courses) = typed(kind::COURSE, all(store, kind::COURSE)?);
        Ok(World {
            spaces,
            raw_spaces,
            raw_projects,
            raw_courses,
            tasks,
            raw_tasks,
            occurrences: typed(kind::OCCURRENCE, all(store, kind::OCCURRENCE)?).0,
            blocks: typed(kind::BLOCK, all(store, kind::BLOCK)?).0,
            sessions: typed(kind::FOCUS, all(store, kind::FOCUS)?).0,
            habits: typed(kind::HABIT, all(store, kind::HABIT)?).0,
            projects,
            milestones: typed(kind::MILESTONE, all(store, kind::MILESTONE)?).0,
            captures: typed(kind::CAPTURE, all(store, kind::CAPTURE)?).0,
            terms: typed(kind::TERM, all(store, kind::TERM)?).0,
            courses,
            grades,
            raw_grades,
            mail: all(store, kind::MAIL)?,
            events: typed(kind::EVENT, all(store, kind::EVENT)?).0,
            fixed: crate::commit::Fixed::load(store)?,
            course_pattern: crate::setting(store, "school")?
                .and_then(|s| s["codePattern"].as_str().map(str::to_string))
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| wi_heat::brightspace::DEFAULT_COURSE_PATTERN.to_string()),
        })
    }

    pub fn task_index(&self, id: &str) -> Option<usize> {
        self.tasks.iter().position(|t| t.id == id)
    }

    /// A space by id, or by name without regard to case.
    pub fn space(&self, q: &str) -> Result<&Space> {
        self.spaces
            .iter()
            .find(|s| s.id == q)
            .or_else(|| self.spaces.iter().find(|s| s.name.eq_ignore_ascii_case(q)))
            .map_or_else(|| refused(format!("No space is called {q}.")), Ok)
    }

    /// A course by its code ("JPN 201", "jpn201") or id.
    pub fn course(&self, code: &str) -> Result<&Course> {
        let squash = |s: &str| {
            s.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .to_ascii_uppercase()
        };
        let want = squash(code);
        self.courses
            .iter()
            .find(|c| c.id == code)
            .or_else(|| self.courses.iter().find(|c| squash(&c.code) == want))
            .map_or_else(|| refused(format!("No course has the code {code}.")), Ok)
    }

    /// A day's busy time for a plan: other calendars' events, and every
    /// commitment with its travel time, and sleep.
    pub fn busy(&self, clock: &Clock, date: &str) -> Vec<CalendarEvent> {
        let mut events = self.events.clone();
        events.extend(self.fixed.busy_events(clock, date));
        events
    }

    pub fn estimates(&self) -> estimate::EstimateContext<'_> {
        estimate::estimate_context(&self.tasks, &self.sessions)
    }
}

/// Task indices in heat order: overdue and hot first, done last.
pub(crate) fn heat_order(world: &World, clock: &Clock) -> Vec<usize> {
    heat::by_heat_order(&world.tasks, clock.now_ms)
}

/// One task as `list_tasks` gives it (3.13).
pub(crate) fn task_view(
    world: &World,
    ctx: &estimate::EstimateContext<'_>,
    i: usize,
    clock: &Clock,
) -> Value {
    let task = &world.tasks[i];
    let raw = &world.raw_tasks[i];
    let space = world
        .spaces
        .iter()
        .find(|s| s.id == task.space_id)
        .map(|s| s.name.clone());
    let course = task
        .course_id
        .as_ref()
        .and_then(|id| world.courses.iter().find(|c| &c.id == id))
        .map(|c| c.code.clone());
    let h = heat::heat_of(task, clock.now_ms);
    let by = raw
        .get("estBy")
        .and_then(Value::as_str)
        .unwrap_or(if task.est_min.is_some() {
            "you"
        } else {
            "default"
        });
    let mut view = json!({
        "id": task.id,
        "title": task.title,
        "type": task.r#type,
        "space": space,
        "due": task.due.map(|d| Value::String(clock.iso(d))).unwrap_or(Value::Null),
        "difficulty": num(task.difficulty),
        "estimate_min": num(round(estimate::estimate_min(task, ctx), 0)),
        "estimate_by": by,
        "heat": {"v": h.v.map(|v| num(round(v, 2))).unwrap_or(Value::Null), "level": h.level.as_str()},
        "notes": task.notes,
        "source": raw.get("source").cloned().unwrap_or(Value::Null),
        "done": task.done,
    });
    if let Some(code) = course {
        view["course"] = json!(code);
    }
    if let Some(home) = crate::homes::home_of(world, task) {
        view["home"] = home["label"].clone();
    }
    if let Some(day) = &task.scheduled_date {
        view["scheduled"] = json!(day);
    }
    if let Some(reason) = raw.get("estReason").and_then(Value::as_str) {
        view["estimate_reason"] = json!(reason);
    }
    view
}

/// The person's average minutes and count by space and type.
pub(crate) fn averages(world: &World, ctx: &estimate::EstimateContext<'_>) -> Vec<Value> {
    let mut out: Vec<Value> = ctx
        .averages
        .iter()
        .map(|(key, avg)| {
            let (space_id, kind_of) = key.split_once('\u{0}').unwrap_or(("", key.as_str()));
            let space = world.spaces.iter().find(|s| s.id == space_id).map(|s| s.name.clone());
            json!({"space": space, "type": kind_of, "minutes": num(round(avg.minutes, 0)), "count": num(avg.count)})
        })
        .collect();
    out.sort_by_key(|a| a.to_string());
    out
}

pub(crate) struct Plan {
    /// As the tool answers: start as ISO 8601, with the task's title.
    pub drafts: Vec<Value>,
    /// As `heatState.planDrafts` keeps them, for the time column.
    pub drafts_stored: Vec<Value>,
    /// Open tasks that got no draft and have no block that day, in heat order.
    pub unplanned: Vec<Value>,
    /// Minutes on the 15-minute grid still free between now and the day's end, after the drafts.
    pub minutes_left: f64,
}

/// Plan my day for `date`, from now (or 7 AM on a day ahead) to `day_ends`.
pub(crate) fn plan(world: &World, clock: &Clock, date: &str, day_ends: f64) -> Result<Plan> {
    let now = if date == clock.today() {
        clock.now_ms
    } else {
        zone::at_minute(date, DAY_STARTS_MIN, &clock.zone)
    };
    let data = plan::PlanData {
        tasks: world.tasks.clone(),
        occurrences: world.occurrences.clone(),
        blocks: world.blocks.clone(),
        events: world.busy(clock, date),
        sessions: world.sessions.clone(),
        habits: world.habits.clone(),
    };
    let options = plan::PlanOptions {
        day_ends_at: Some(day_ends),
        space_id: None,
    };
    let drafts = plan::plan_my_day(&data, now, &clock.zone, &options);
    let title = |id: &str| {
        world
            .tasks
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.title.clone())
            .unwrap_or_default()
    };
    let drafted: Vec<&str> = drafts.iter().map(|d| d.task_id.as_str()).collect();
    let blocked: Vec<&str> = world
        .blocks
        .iter()
        .filter(|b| b.date == date)
        .filter_map(|b| b.task_id.as_deref())
        .collect();
    // A parent is planned through its subtasks, and a recurring series by its open occurrence.
    let parents: Vec<&str> = world
        .tasks
        .iter()
        .filter(|t| !t.done)
        .filter_map(|t| t.parent_task_id.as_deref().filter(|p| !p.is_empty()))
        .collect();
    let open_tasks = recurrence::open_tasks(&world.tasks, &world.occurrences, now, &clock.zone);
    let open: Vec<&str> = open_tasks.iter().map(|t| t.id.as_str()).collect();
    let unplanned = heat_order(world, clock)
        .into_iter()
        .map(|i| &world.tasks[i])
        .filter(|t| {
            open.contains(&t.id.as_str())
                && !parents.contains(&t.id.as_str())
                && !drafted.contains(&t.id.as_str())
                && !blocked.contains(&t.id.as_str())
        })
        .map(|t| {
            json!({
                "task_id": t.id,
                "title": t.title,
                "due": t.due.map(|d| Value::String(clock.iso(d))).unwrap_or(Value::Null),
            })
        })
        .collect();
    let minutes_left = free_minutes(world, clock, date, now, day_ends, &drafts);
    Ok(Plan {
        drafts: drafts
            .iter()
            .map(|d| {
                json!({
                    "task_id": d.task_id,
                    "title": title(&d.task_id),
                    "start": clock.iso(zone::at_minute(&d.date, d.start, &clock.zone)),
                    "minutes": num(d.minutes),
                    "reason": d.reason,
                })
            })
            .collect(),
        drafts_stored: drafts
            .iter()
            .filter_map(|d| serde_json::to_value(d).ok())
            .collect(),
        unplanned,
        minutes_left: minutes_left.max(0.0),
    })
}

/// The 15-minute marks from now (never before 7 AM) to the day's end that no
/// block, timed event or draft covers, in minutes.
fn free_minutes(
    world: &World,
    clock: &Clock,
    date: &str,
    now: f64,
    day_ends: f64,
    drafts: &[plan::Draft],
) -> f64 {
    let mut spans = plan::busy_spans(&world.blocks, &world.busy(clock, date), date, &clock.zone);
    spans.extend(drafts.iter().map(|d| (d.start, d.start + d.minutes)));
    let mut mark =
        ((zone::minute_of_day(now, &clock.zone).max(DAY_STARTS_MIN)) / 15.0).ceil() * 15.0;
    let mut free = 0.0;
    while mark + 15.0 <= day_ends {
        if spans.iter().all(|(a, b)| mark + 15.0 <= *a || mark >= *b) {
            free += 15.0;
        }
        mark += 15.0;
    }
    free
}

/// One course as `get_grades` gives it (3.13).
pub(crate) fn course_view(world: &World, course: &Course) -> Value {
    let mine: Vec<Grade> = world
        .grades
        .iter()
        .filter(|g| g.course_id == course.id)
        .cloned()
        .collect();
    let scale = course.scale.clone().unwrap_or_else(grades::default_scale);
    let current = grades::current_pct(course, &mine);
    let decided = grades::decided_pct(course, &mine);
    let category = |id: &Option<String>| {
        id.as_ref()
            .and_then(|id| course.categories.iter().find(|c| &c.id == id))
            .map(|c| Value::String(c.name.clone()))
            .unwrap_or(Value::Null)
    };
    json!({
        "code": course.code,
        "name": course.name,
        "scale": scale.iter().map(|s| json!({"letter": s.letter, "min": num(s.min)})).collect::<Vec<_>>(),
        "categories": course.categories.iter().map(|c| json!({"name": c.name, "weight": num(c.weight)})).collect::<Vec<_>>(),
        "items": mine.iter().map(|g| json!({
            "name": g.title,
            "category": category(&g.category_id),
            "score": g.score.map(num).unwrap_or(Value::Null),
            "out_of": num(g.out_of),
            "dropped": g.dropped,
            "pending": g.pending,
        })).collect::<Vec<_>>(),
        "current_pct": current.map(|p| num(round(p, 1))).unwrap_or(Value::Null),
        "decided_pct": num(round(decided, 1)),
        "letter": current.map(|p| Value::String(grades::letter_for(p, &scale))).unwrap_or(Value::Null),
    })
}

/// A task's measured minutes: its focus sessions plus Get Info's "Took".
pub(crate) fn actual_min(world: &World, task_id: &str) -> f64 {
    world
        .tasks
        .iter()
        .find(|t| t.id == task_id)
        .map(|t| estimate::actual_min(t, &world.sessions))
        .unwrap_or(0.0)
}
