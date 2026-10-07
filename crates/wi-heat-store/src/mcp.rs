//! The store side of the eight MCP tools (docs/SPEC.md 3.13): one function
//! per tool, each one transaction by Claude, with the tool's name and
//! Claude's reason on the journal entry. The helper has already checked the
//! arguments against the tool's schema; the rules that need the library
//! (clamps, dedupe, ids that must exist) live here.

use serde_json::{json, Map, Value};
use wi_store::{Actor, Store};

use crate::derive::{self, World};
use crate::{change, kind, num, parse_instant, put, refused, round, Clock, Result};

/// The tools, in 3.13's order.
pub const TOOLS: [&str; 8] = [
    "list_tasks",
    "add_task",
    "update_task",
    "plan_day",
    "get_grades",
    "add_pending_grade",
    "log_focus",
    "record_mail_thread",
];

/// Minutes Claude may estimate, clamped (3.12).
pub const MINUTES: (f64, f64) = (5.0, 600.0);

/// How hard a new task is until someone says otherwise: the middle of 1–5.
pub const DEFAULT_DIFFICULTY: f64 = 3.0;

/// Runs one tool. `args` passed the tool's schema in `wi-mcp`.
pub fn call(store: &mut Store, clock: &Clock, tool: &str, args: &Map<String, Value>) -> Result<Value> {
    match tool {
        "list_tasks" => list_tasks(store, clock, args),
        "add_task" => add_task(store, clock, args),
        "update_task" => update_task(store, clock, args),
        "plan_day" => plan_day(store, clock, args),
        "get_grades" => get_grades(store, args),
        "add_pending_grade" => add_pending_grade(store, args),
        "log_focus" => log_focus(store, clock, args),
        "record_mail_thread" => record_mail_thread(store, args),
        _ => refused(format!("There's no tool called {tool}.")),
    }
}

fn text<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}

fn whole(args: &Map<String, Value>, key: &str) -> Option<i64> {
    args.get(key).and_then(Value::as_i64)
}

fn claude(tool: &str, args: &Map<String, Value>) -> Actor {
    Actor::Claude {
        tool: tool.to_string(),
        reason: text(args, "reason").unwrap_or_default().to_string(),
    }
}

/// `list_tasks`: tasks in heat order with Heat's numbers, the person's
/// averages by type, and each space's persona.
pub fn list_tasks(store: &Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let world = World::load(store)?;
    let status = text(args, "status").unwrap_or("open");
    let space = match text(args, "space") {
        Some(q) => Some(world.space(q)?.id.clone()),
        None => None,
    };
    let before = text(args, "due_before").map(parse_instant).transpose()?;
    let limit = whole(args, "limit").unwrap_or(50).clamp(1, 200) as usize;
    let ctx = world.estimates();
    let mut tasks = Vec::new();
    for i in derive::heat_order(&world, clock) {
        let task = &world.tasks[i];
        let keep = match status {
            "done" => task.done,
            "all" => true,
            _ => !task.done,
        } && space.as_ref().map_or(true, |s| &task.space_id == s)
            && before.map_or(true, |b| task.due.is_some_and(|d| d < b));
        if !keep {
            continue;
        }
        if tasks.len() == limit {
            break;
        }
        tasks.push(derive::task_view(&world, &ctx, i, clock));
    }
    Ok(json!({
        "tasks": tasks,
        "averages": derive::averages(&world, &ctx),
        "spaces": world.spaces.iter().map(|s| json!({"name": s.name, "persona": s.persona})).collect::<Vec<_>>(),
    }))
}

/// `add_task`: a new task labelled as Claude's. The same `source_id` again
/// adds nothing and says so.
pub fn add_task(store: &mut Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let world = World::load(store)?;
    if let Some(source) = text(args, "source_id") {
        if let Some(i) = world.raw_tasks.iter().position(|t| t["sourceId"].as_str() == Some(source)) {
            let ctx = world.estimates();
            return Ok(json!({"task": derive::task_view(&world, &ctx, i, clock), "created": false, "undo_label": null}));
        }
    }
    let space = match text(args, "space") {
        Some(q) => world.space(q)?,
        None => match world.spaces.first() {
            Some(s) => s,
            None => return refused("There's no space yet. Open Heat in Wi_WWAV once."),
        },
    };
    let course = match text(args, "course") {
        Some(code) => Some(world.course(code)?.id.clone()),
        None => None,
    };
    let due = text(args, "due").map(parse_instant).transpose()?;
    let kind_of = text(args, "type")
        .map(str::to_string)
        .or_else(|| space.types.first().cloned())
        .unwrap_or_else(|| "Task".to_string());
    let id = wwav_ids::ulid();
    let mut task = json!({
        "id": id,
        "spaceId": space.id,
        "title": text(args, "title").unwrap_or_default(),
        "type": kind_of,
        "due": due.map(num).unwrap_or(Value::Null),
        "difficulty": num(DEFAULT_DIFFICULTY),
        "estMin": null,
        "estBy": "default",
        "adjustMin": 0,
        "notes": args.get("notes").and_then(Value::as_str).unwrap_or(""),
        "done": false,
        "doneAt": null,
        "source": "claude",
        "public": false,
    });
    if let Some(c) = &course {
        task["courseId"] = json!(c);
    }
    if let Some(source) = text(args, "source_id") {
        task["sourceId"] = json!(source);
    }
    if let Some(reason) = text(args, "reason") {
        task["claudeReason"] = json!(reason);
    }
    // A thread Claude already recorded points at the task it made.
    let thread = text(args, "mail_thread_id")
        .and_then(|t| world.mail.iter().find(|m| m["gmailThreadId"].as_str() == Some(t)).cloned());
    let undo = change(store, "Claude's task", claude("add_task", args), |txn| {
        put(txn, kind::TASK, &task)?;
        if let Some(mut thread) = thread {
            thread["taskId"] = json!(id);
            put(txn, kind::MAIL, &thread)?;
        }
        Ok(())
    })?;
    let world = World::load(store)?;
    let i = world.task_index(&id).expect("the task was just written");
    let ctx = world.estimates();
    Ok(json!({"task": derive::task_view(&world, &ctx, i, clock), "created": true, "undo_label": undo}))
}

/// `update_task`: Claude's difficulty, minutes or both, clamped, with the
/// reason Heat shows as the field's hint.
pub fn update_task(store: &mut Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let difficulty = whole(args, "difficulty");
    let minutes = whole(args, "estimate_min");
    if difficulty.is_none() && minutes.is_none() {
        return refused("Give a difficulty, an estimate in minutes, or both.");
    }
    let world = World::load(store)?;
    let id = text(args, "id").unwrap_or_default();
    let Some(i) = world.task_index(id).filter(|i| !world.tasks[*i].done) else {
        return refused("No open task has that id.");
    };
    let mut task = world.raw_tasks[i].clone();
    let mut clamped = false;
    if let Some(m) = minutes {
        let set = (m as f64).clamp(MINUTES.0, MINUTES.1);
        clamped = set != m as f64;
        task["estMin"] = num(set);
        task["estBy"] = json!("claude");
    }
    if let Some(d) = difficulty {
        task["difficulty"] = num(d as f64);
    }
    task["estReason"] = json!(text(args, "reason").unwrap_or_default());
    let undo = change(store, "Claude's estimate", claude("update_task", args), |txn| put(txn, kind::TASK, &task))?;
    let world = World::load(store)?;
    let ctx = world.estimates();
    Ok(json!({"task": derive::task_view(&world, &ctx, i, clock), "clamped": clamped, "undo_label": undo}))
}

/// `plan_day`: Plan my day's rule, as drafts the person accepts or clears.
/// Writes the drafts outside the journal: a draft isn't a change (8.8).
pub fn plan_day(store: &mut Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let today = clock.today();
    let date = text(args, "date").unwrap_or(&today).to_string();
    if date < today {
        return refused("That day is over. Plan today or a day ahead.");
    }
    let day_ends = match text(args, "day_ends") {
        Some(hhmm) => {
            let (h, m) = hhmm.split_once(':').unwrap_or(("23", "00"));
            h.parse::<f64>().unwrap_or(23.0) * 60.0 + m.parse::<f64>().unwrap_or(0.0)
        }
        None => derive::DAY_ENDS_MIN,
    };
    let world = World::load(store)?;
    let plan = derive::plan(&world, clock, &date, day_ends)?;
    let mut state = crate::state(store)?;
    state["planDrafts"] = Value::Array(plan.drafts_stored.clone());
    crate::set_state(store, &state)?;
    Ok(json!({"drafts": plan.drafts, "unplanned": plan.unplanned, "minutes_left": num(plan.minutes_left)}))
}

/// `get_grades`: the term's courses with their items and Heat's own
/// percentages and letter. Never written to.
pub fn get_grades(store: &Store, args: &Map<String, Value>) -> Result<Value> {
    let world = World::load(store)?;
    let only = match text(args, "course") {
        Some(code) => Some(world.course(code)?.id.clone()),
        None => None,
    };
    let Some(term) = world.terms.last() else {
        return Ok(json!({"term": null, "courses": []}));
    };
    let courses: Vec<Value> = world
        .courses
        .iter()
        .filter(|c| match &only {
            Some(id) => &c.id == id,
            None => c.term_id == term.id,
        })
        .map(|c| derive::course_view(&world, c))
        .collect();
    Ok(json!({"term": term.name, "courses": courses}))
}

/// `add_pending_grade`: a grade notice with no score, which only the person
/// types. The same course and item again adds nothing.
pub fn add_pending_grade(store: &mut Store, args: &Map<String, Value>) -> Result<Value> {
    let world = World::load(store)?;
    let course = world.course(text(args, "course").unwrap_or_default())?;
    let item = text(args, "item").unwrap_or_default();
    if let Some(existing) = world
        .raw_grades
        .iter()
        .find(|g| g["courseId"].as_str() == Some(&course.id) && g["title"].as_str().is_some_and(|t| t.trim().eq_ignore_ascii_case(item)))
    {
        return Ok(json!({"grade": existing, "created": false, "undo_label": null}));
    }
    let mut grade = json!({
        "id": wwav_ids::ulid(),
        "courseId": course.id,
        "categoryId": derive::guess_category(item, course),
        "title": item,
        "score": null,
        "outOf": 100,
        "dropped": false,
        "pending": true,
        "source": "claude",
        "public": false,
    });
    if let Some(link) = text(args, "link") {
        grade["link"] = json!(link);
    }
    if let Some(at) = text(args, "posted_at") {
        grade["postedAt"] = num(parse_instant(at)?);
    }
    let undo = change(store, "Claude's pending grade", claude("add_pending_grade", args), |txn| {
        put(txn, kind::GRADE, &grade)
    })?;
    Ok(json!({"grade": grade, "created": true, "undo_label": undo}))
}

/// `log_focus`: minutes the person spent, as a focus record on the task.
pub fn log_focus(store: &mut Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let world = World::load(store)?;
    let task_id = text(args, "task_id").unwrap_or_default();
    if world.task_index(task_id).is_none() {
        return refused("No task has that id.");
    }
    let minutes = whole(args, "minutes").unwrap_or(0) as f64;
    let started = match text(args, "started_at") {
        Some(at) => parse_instant(at)?,
        None => clock.now_ms - minutes * 60_000.0,
    };
    let session = json!({
        "id": wwav_ids::ulid(),
        "taskId": task_id,
        "startedAt": num(started),
        "endedAt": num(started + minutes * 60_000.0),
        "focusMin": num(minutes),
        "interruptions": 0,
        "room": "heat",
        "source": "claude",
        "public": false,
    });
    let undo = change(store, "Claude's focus log", claude("log_focus", args), |txn| {
        put(txn, kind::FOCUS, &session)
    })?;
    let world = World::load(store)?;
    let actual = derive::actual_min(&world, task_id);
    Ok(json!({"focus_record": session, "actual_min": num(round(actual, 0)), "undo_label": undo}))
}

/// `record_mail_thread`: one row per Gmail thread, never its body. The same
/// thread again updates its state and reason.
pub fn record_mail_thread(store: &mut Store, args: &Map<String, Value>) -> Result<Value> {
    let world = World::load(store)?;
    let thread_id = text(args, "thread_id").unwrap_or_default();
    if let Some(task) = text(args, "task_id") {
        if world.task_index(task).is_none() {
            return refused("No task has that id.");
        }
    }
    let received = parse_instant(text(args, "received_at").unwrap_or_default())?;
    let existing = world.mail.iter().find(|m| m["gmailThreadId"].as_str() == Some(thread_id)).cloned();
    let created = existing.is_none();
    let mut thread = existing.unwrap_or_else(|| json!({"id": wwav_ids::ulid(), "gmailThreadId": thread_id, "recordedBy": "claude"}));
    thread["subject"] = json!(text(args, "subject").unwrap_or_default());
    thread["from"] = json!(text(args, "from").unwrap_or_default());
    thread["receivedAt"] = num(received);
    thread["state"] = json!(text(args, "state").unwrap_or("nothing"));
    thread["reason"] = json!(text(args, "reason").unwrap_or_default());
    if let Some(course) = text(args, "course") {
        thread["course"] = json!(course);
    }
    if let Some(task) = text(args, "task_id") {
        thread["taskId"] = json!(task);
    }
    let undo = change(store, "Claude's mail note", claude("record_mail_thread", args), |txn| {
        put(txn, kind::MAIL, &thread)
    })?;
    Ok(json!({"thread": thread, "created": created, "undo_label": undo}))
}
