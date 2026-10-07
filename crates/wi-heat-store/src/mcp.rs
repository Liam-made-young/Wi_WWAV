//! The store side of the MCP tools (docs/SPEC.md 3.13): one function per
//! tool, each one transaction by Claude, with the tool's name and Claude's
//! reason on the journal entry. The first eight are the mail and planning
//! tools; the ten after them read the rest of the view and draft into it;
//! the last two read the mail accounts and what is recorded from each. The helper has already checked the
//! arguments against the tool's schema; the rules that need the library
//! (clamps, dedupe, ids that must exist) live here.

use serde_json::{json, Map, Value};
use wi_heat::model::{habits, plan, zone};
use wi_store::{Actor, Store};

use crate::derive::{self, World};
use crate::homes::{self, Sighting};
use crate::{
    all, change, kind, mail, num, one, parse_instant, put, refused, round, schema, Clock, Result,
};

/// The tools, in 3.13's order.
pub const TOOLS: [&str; 21] = [
    "list_tasks",
    "add_task",
    "update_task",
    "plan_day",
    "get_grades",
    "add_pending_grade",
    "log_focus",
    "record_mail_thread",
    "get_schedule",
    "draft_block",
    "list_habits",
    "list_projects",
    "add_project",
    "add_milestone",
    "get_notes",
    "add_note",
    "list_inbox",
    "add_capture",
    "list_mail_accounts",
    "list_mail",
    "save_mail_text",
];

/// The most days `get_schedule` reads at once.
pub const SCHEDULE_DAYS: f64 = 31.0;

/// A block Claude drafts: 15 minutes to 4 hours, on the 15-minute grid.
pub const DRAFT_MINUTES: (f64, f64) = (15.0, 240.0);

/// How much of a note Claude reads.
pub const NOTE_CHARS: usize = 4000;

/// Minutes Claude may estimate, clamped (3.12).
pub const MINUTES: (f64, f64) = (5.0, 600.0);

/// How hard a new task is until someone says otherwise: the middle of 1–5.
pub const DEFAULT_DIFFICULTY: f64 = 3.0;

/// Runs one tool. `args` passed the tool's schema in `wi-mcp`.
pub fn call(
    store: &mut Store,
    clock: &Clock,
    tool: &str,
    args: &Map<String, Value>,
) -> Result<Value> {
    match tool {
        "list_tasks" => list_tasks(store, clock, args),
        "add_task" => add_task(store, clock, args),
        "update_task" => update_task(store, clock, args),
        "plan_day" => plan_day(store, clock, args),
        "get_grades" => get_grades(store, args),
        "add_pending_grade" => add_pending_grade(store, args),
        "log_focus" => log_focus(store, clock, args),
        "record_mail_thread" => record_mail_thread(store, args),
        "get_schedule" => get_schedule(store, clock, args),
        "draft_block" => draft_block(store, clock, args),
        "list_habits" => list_habits(store, clock),
        "list_projects" => list_projects(store, args),
        "add_project" => add_project(store, clock, args),
        "add_milestone" => add_milestone(store, clock, args),
        "get_notes" => get_notes(store, clock, args),
        "add_note" => add_note(store, clock, args),
        "list_inbox" => list_inbox(store),
        "add_capture" => add_capture(store, clock, args),
        "list_mail_accounts" => list_mail_accounts(store),
        "list_mail" => list_mail(store, clock, args),
        "save_mail_text" => mail::save_text(
            store,
            clock,
            text(args, "thread_id").unwrap_or_default(),
            args.get("messages")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice),
        ),
        _ => refused(format!("There's no tool called {tool}.")),
    }
}

fn text<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
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
        if let Some(i) = world
            .raw_tasks
            .iter()
            .position(|t| t["sourceId"].as_str() == Some(source))
        {
            let ctx = world.estimates();
            return Ok(
                json!({"task": derive::task_view(&world, &ctx, i, clock), "created": false, "undo_label": null}),
            );
        }
    }
    let space = match text(args, "space") {
        Some(q) => world.space(q)?,
        None => match world.spaces.first() {
            Some(s) => s,
            None => return refused("There's no space yet. Open Learn in Wi_WWAV once."),
        },
    };
    // A thread Claude already recorded points at the task it made.
    let thread = text(args, "mail_thread_id").and_then(|t| {
        world
            .mail
            .iter()
            .find(|m| m["gmailThreadId"].as_str() == Some(t))
            .cloned()
    });
    // The task's course: the one Claude names, else the one its thread is
    // about, and that only where tasks are grouped by course: mail about
    // work or home never gets a course. A code Learn doesn't hold yet makes
    // the course, a stub until its syllabus comes.
    let school = homes::school_of(&world);
    let by_course = space.group_kind == wi_heat::model::records::GroupKind::Course;
    let named = text(args, "course").map(str::to_string).or_else(|| {
        thread
            .as_ref()
            .filter(|_| by_course)
            .and_then(|t| t["course"].as_str().map(str::to_string))
    });
    let mut found = homes::Found::default();
    let course = match &named {
        None => None,
        Some(code) => match world.course(code) {
            Ok(c) => Some(c.id.clone()),
            Err(e) => {
                let sighting = wi_heat::homes::offering_in(&school.course_pattern, &code.to_uppercase())
                    .filter(|o| wi_heat::homes::squash(&o.code) == wi_heat::homes::squash(code))
                    .map(|o| Sighting { code: o.code, name: None, term: None, sure: true });
                found = homes::courses_from(&world, clock, &sighting.into_iter().collect::<Vec<_>>());
                match found.id_of(&world, code) {
                    Some(id) => Some(id),
                    // What Claude passed isn't a course code: said, when it was asked for by name.
                    None if text(args, "course").is_some() => return Err(e),
                    None => None,
                }
            }
        },
    };
    let due = text(args, "due").map(parse_instant).transpose()?;
    let id = wwav_ids::ulid();
    let mut task = json!({
        "id": id,
        "spaceId": space.id,
        "title": text(args, "title").unwrap_or_default(),
        "due": due.map(num).unwrap_or(Value::Null),
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
    if let Some(kind_of) = text(args, "type") {
        task["type"] = json!(kind_of);
        task["typeBy"] = json!("claude");
    }
    // Its type, and the minutes and difficulty that type starts with: a task
    // from mail is estimated like any other, in whatever space it lands.
    let mut ctx = homes::Ctx::load(&world);
    found.courses.iter().for_each(|c| ctx.set_course(c));
    if let Some(m) = task.as_object_mut() {
        homes::inherit(m, &ctx, true, false);
    }
    let made = found.writes();
    let undo = change(store, "Claude's task", claude("add_task", args), |txn| {
        for (k, r) in &made {
            put(txn, k, r)?;
        }
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
    Ok(
        json!({"task": derive::task_view(&world, &ctx, i, clock), "created": true, "undo_label": undo}),
    )
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
        task["difficultyBy"] = json!("claude");
    }
    task["estReason"] = json!(text(args, "reason").unwrap_or_default());
    let undo = change(
        store,
        "Claude's estimate",
        claude("update_task", args),
        |txn| put(txn, kind::TASK, &task),
    )?;
    let world = World::load(store)?;
    let ctx = world.estimates();
    Ok(
        json!({"task": derive::task_view(&world, &ctx, i, clock), "clamped": clamped, "undo_label": undo}),
    )
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
    // Claude reads the first twenty that didn't fit; the window's footer counts them all.
    let unplanned: Vec<&Value> = plan.unplanned.iter().take(20).collect();
    Ok(
        json!({"drafts": plan.drafts, "unplanned": unplanned, "minutes_left": num(plan.minutes_left)}),
    )
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
    if let Some(existing) = world.raw_grades.iter().find(|g| {
        g["courseId"].as_str() == Some(&course.id)
            && g["title"]
                .as_str()
                .is_some_and(|t| t.trim().eq_ignore_ascii_case(item))
    }) {
        return Ok(json!({"grade": existing, "created": false, "undo_label": null}));
    }
    let mut grade = json!({
        "id": wwav_ids::ulid(),
        "courseId": course.id,
        "categoryId": homes::guess_category(
            world.raw_courses.iter().find(|c| c["id"] == json!(course.id)).unwrap_or(&Value::Null),
            item,
        ),
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
    let undo = change(
        store,
        "Claude's pending grade",
        claude("add_pending_grade", args),
        |txn| put(txn, kind::GRADE, &grade),
    )?;
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
    let undo = change(
        store,
        "Claude's focus log",
        claude("log_focus", args),
        |txn| put(txn, kind::FOCUS, &session),
    )?;
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
    let existing = world
        .mail
        .iter()
        .find(|m| m["gmailThreadId"].as_str() == Some(thread_id))
        .cloned();
    let created = existing.is_none();
    let mut thread = existing.unwrap_or_else(
        || json!({"id": wwav_ids::ulid(), "gmailThreadId": thread_id, "recordedBy": "claude"}),
    );
    thread["subject"] = json!(text(args, "subject").unwrap_or_default());
    thread["from"] = json!(text(args, "from").unwrap_or_default());
    thread["receivedAt"] = num(received);
    thread["state"] = json!(text(args, "state").unwrap_or("nothing"));
    thread["reason"] = json!(text(args, "reason").unwrap_or_default());
    // Which mailbox it is from, how pressing it is and what it is about.
    // A thread read again keeps what it had unless Claude says otherwise.
    let accounts = mail::accounts(store)?;
    if let Some(account) = mail::account_for(text(args, "account"), &accounts)? {
        if text(args, "account").is_some() || thread.get("account").is_none() {
            thread["account"] = json!(account);
        }
    }
    match text(args, "priority") {
        Some(p) => thread["priority"] = json!(p),
        None if thread.get("priority").is_none() => thread["priority"] = json!("normal"),
        None => {}
    }
    if let Some(category) = text(args, "category") {
        thread["category"] = json!(category);
    }
    if let Some(course) = text(args, "course") {
        thread["course"] = json!(course);
    }
    if let Some(task) = text(args, "task_id") {
        thread["taskId"] = json!(task);
    }
    // A course the thread names and Learn doesn't hold yet is made here, a
    // stub until its syllabus comes, so Grades lists every course Mail does;
    // one still named by its code takes the name the subject gives it.
    let courses = homes::courses_from(
        &world,
        &Clock::at(received, jiff::tz::TimeZone::UTC),
        &homes::mail_sightings(&homes::school_of(&world), &thread),
    )
    .writes();
    let undo = change(
        store,
        "Claude's mail note",
        claude("record_mail_thread", args),
        |txn| {
            for (k, r) in &courses {
                put(txn, k, r)?;
            }
            put(txn, kind::MAIL, &thread)
        },
    )?;
    Ok(json!({"thread": thread, "created": created, "undo_label": undo}))
}

/// A record Claude drafts, checked and completed the way a person's write
/// is (`schema::finish`), and marked as Claude's with the reason.
fn drafted(
    world: &World,
    clock: &Clock,
    k: &str,
    mut record: Map<String, Value>,
    args: &Map<String, Value>,
) -> Result<Value> {
    record.insert("source".into(), json!("claude"));
    if let Some(reason) = text(args, "reason") {
        record.insert("claudeReason".into(), json!(reason));
    }
    let sp = schema::writable(k)?;
    let done = schema::finish(sp, None, record, world, clock, &mut crate::ulid)?;
    Ok(Value::Object(done.record))
}

/// `HH:MM` as minutes after midnight.
fn minute_of(hhmm: &str) -> f64 {
    let (h, m) = hhmm.split_once(':').unwrap_or(("0", "0"));
    h.parse::<f64>().unwrap_or(0.0) * 60.0 + m.parse::<f64>().unwrap_or(0.0)
}

/// A draft as a tool answers it: the start as ISO 8601, with the title.
fn draft_view(world: &World, clock: &Clock, d: &plan::Draft) -> Value {
    let title = world
        .tasks
        .iter()
        .find(|t| t.id == d.task_id)
        .map(|t| t.title.as_str())
        .unwrap_or_default();
    json!({
        "task_id": d.task_id,
        "title": title,
        "start": clock.iso(zone::at_minute(&d.date, d.start, &clock.zone)),
        "minutes": num(d.minutes),
        "reason": d.reason,
    })
}

fn drafts_of(state: &Value) -> Vec<plan::Draft> {
    state["planDrafts"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|d| serde_json::from_value(d.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// `get_schedule`: what the time column and Calendar show between two days:
/// blocks, calendar events, tasks due, and the drafts that wait. Read only.
pub fn get_schedule(store: &Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let today = clock.today();
    let from = text(args, "from").unwrap_or(&today).to_string();
    let to = text(args, "to").unwrap_or(&from).to_string();
    if !schema::is_day(&from) || !schema::is_day(&to) {
        return refused("A day is written YYYY-MM-DD.");
    }
    if to < from {
        return refused("The last day comes before the first.");
    }
    let starts = zone::at_minute(&from, 0.0, &clock.zone);
    let ends = zone::at_minute(&to, 24.0 * 60.0, &clock.zone);
    if (ends - starts) / 86_400_000.0 > SCHEDULE_DAYS + 0.5 {
        return refused("Ask for 31 days or fewer at a time.");
    }
    let world = World::load(store)?;
    let in_days = |date: &str| date >= from.as_str() && date <= to.as_str();
    let mut blocks: Vec<_> = world.blocks.iter().filter(|b| in_days(&b.date)).collect();
    blocks.sort_by(|a, b| (&a.date, a.start).partial_cmp(&(&b.date, b.start)).unwrap());
    let blocks: Vec<Value> = blocks
        .iter()
        .map(|b| {
            let title = match (&b.task_id, &b.habit_id) {
                (Some(t), _) => world.tasks.iter().find(|x| &x.id == t).map(|x| &x.title),
                (None, Some(h)) => world.habits.iter().find(|x| &x.id == h).map(|x| &x.title),
                (None, None) => None,
            };
            let mut v = json!({
                "id": b.id,
                "title": title,
                "start": clock.iso(zone::at_minute(&b.date, b.start, &clock.zone)),
                "minutes": num(b.minutes),
            });
            if let Some(t) = &b.task_id {
                v["task_id"] = json!(t);
            }
            if let Some(h) = &b.habit_id {
                v["habit_id"] = json!(h);
            }
            v
        })
        .collect();
    let mut events: Vec<_> = world
        .events
        .iter()
        .filter(|e| e.start < ends && e.end > starts)
        .collect();
    events.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
    let events: Vec<Value> = events
        .iter()
        .map(|e| {
            json!({
                "title": e.title,
                "start": clock.iso(e.start),
                "end": clock.iso(e.end),
                "all_day": e.all_day,
            })
        })
        .collect();
    let mut due: Vec<_> = world
        .tasks
        .iter()
        .filter(|t| !t.done && t.due.is_some_and(|d| d >= starts && d < ends))
        .collect();
    due.sort_by(|a, b| a.due.partial_cmp(&b.due).unwrap());
    let due: Vec<Value> = due
        .iter()
        .map(|t| json!({"task_id": t.id, "title": t.title, "due": clock.iso(t.due.unwrap_or(0.0))}))
        .collect();
    let drafts: Vec<Value> = drafts_of(&crate::state(store)?)
        .iter()
        .filter(|d| in_days(&d.date))
        .map(|d| draft_view(&world, clock, d))
        .collect();
    Ok(
        json!({"from": from, "to": to, "blocks": blocks, "events": events, "due": due, "drafts": drafts}),
    )
}

/// `draft_block`: one block for a task, as a draft the person accepts or
/// clears. Written outside the journal, like `plan_day`'s: a draft isn't a
/// change (8.8). A second draft for the same task and day replaces the first.
pub fn draft_block(store: &mut Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let world = World::load(store)?;
    let task_id = text(args, "task_id").unwrap_or_default();
    if !world
        .task_index(task_id)
        .is_some_and(|i| !world.tasks[i].done)
    {
        return refused("No open task has that id.");
    }
    let today = clock.today();
    let date = text(args, "date").unwrap_or(&today).to_string();
    if !schema::is_day(&date) {
        return refused("A day is written YYYY-MM-DD.");
    }
    if date < today {
        return refused("That day is over. Plan today or a day ahead.");
    }
    let minutes = plan::snap(whole(args, "minutes").unwrap_or(0) as f64)
        .clamp(DRAFT_MINUTES.0, DRAFT_MINUTES.1);
    // The column runs from 7 AM to midnight in steps of 15 minutes (3.5).
    let start = plan::snap(minute_of(text(args, "start").unwrap_or_default()));
    if start < 7.0 * 60.0 || start + minutes > 24.0 * 60.0 {
        return refused("A block sits between 7 AM and midnight.");
    }
    let at = zone::at_minute(&date, start, &clock.zone);
    let until = at + minutes * 60_000.0;
    if until <= clock.now_ms {
        return refused("That time has passed.");
    }
    let mut state = crate::state(store)?;
    let mut drafts = drafts_of(&state);
    drafts.retain(|d| !(d.task_id == task_id && d.date == date));
    let taken = world
        .blocks
        .iter()
        .filter(|b| b.date == date && b.start < start + minutes && start < b.start + b.minutes)
        .map(|_| "a block")
        .chain(
            drafts
                .iter()
                .filter(|d| {
                    d.date == date && d.start < start + minutes && start < d.start + d.minutes
                })
                .map(|_| "a draft"),
        )
        .chain(
            world
                .events
                .iter()
                .filter(|e| !e.all_day && e.start < until && at < e.end)
                .map(|_| "a calendar event"),
        )
        .next();
    if let Some(what) = taken {
        return refused(format!(
            "{what} already holds that time. Read get_schedule and pick a free one.",
            what = what.replacen('a', "A", 1)
        ));
    }
    let draft = plan::Draft {
        task_id: task_id.to_string(),
        date,
        start,
        minutes,
        left_min: 0.0,
        reason: text(args, "reason").unwrap_or_default().to_string(),
        left_line: None,
    };
    let view = draft_view(&world, clock, &draft);
    drafts.push(draft);
    drafts.sort_by(|a, b| (&a.date, a.start).partial_cmp(&(&b.date, b.start)).unwrap());
    let waiting = drafts.len();
    state["planDrafts"] = Value::Array(
        drafts
            .iter()
            .filter_map(|d| serde_json::to_value(d).ok())
            .collect(),
    );
    crate::set_state(store, &state)?;
    Ok(json!({"draft": view, "drafts_waiting": waiting}))
}

/// `list_habits`: each habit, whether today is ticked, and its record line.
/// Read only: only the person ticks a habit.
pub fn list_habits(store: &Store, clock: &Clock) -> Result<Value> {
    let world = World::load(store)?;
    let today = clock.today();
    let list: Vec<Value> = world
        .habits
        .iter()
        .map(|h| {
            json!({
                "id": h.id,
                "title": h.title,
                "minutes": h.minutes.map(num),
                "done_today": h.log.get(&today).copied().unwrap_or(false),
                "record": habits::habit_line(h, &today),
            })
        })
        .collect();
    Ok(json!({"date": today, "habits": list}))
}

/// `list_projects`: projects with their milestones in order and how many
/// open tasks each holds, and the milestones that belong to no project.
pub fn list_projects(store: &Store, args: &Map<String, Value>) -> Result<Value> {
    let world = World::load(store)?;
    let space = match text(args, "space") {
        Some(q) => Some(world.space(q)?.id.clone()),
        None => None,
    };
    let status = text(args, "status").unwrap_or("active");
    let space_name = |id: &str| {
        world
            .spaces
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.name.clone())
    };
    let milestone = |m: &wi_heat::model::records::Milestone| json!({"id": m.id, "title": m.title, "date": m.date, "done": m.done});
    let mut ordered: Vec<_> = world.milestones.iter().collect();
    ordered.sort_by(|a, b| a.order.partial_cmp(&b.order).unwrap());
    let projects: Vec<Value> = world
        .projects
        .iter()
        .filter(|p| space.as_ref().map_or(true, |s| &p.space_id == s))
        .filter(|p| {
            status == "all"
                || serde_json::to_value(p.status)
                    .ok()
                    .as_ref()
                    .and_then(Value::as_str)
                    == Some(status)
        })
        .map(|p| {
            json!({
                "id": p.id,
                "title": p.title,
                "space": space_name(&p.space_id),
                "status": p.status,
                "target_date": p.target_date,
                "milestones": ordered
                    .iter()
                    .filter(|m| m.project_id.as_deref() == Some(&p.id))
                    .map(|m| milestone(m))
                    .collect::<Vec<_>>(),
                "open_tasks": world
                    .tasks
                    .iter()
                    .filter(|t| !t.done && t.project_id.as_deref() == Some(&p.id))
                    .count(),
            })
        })
        .collect();
    let loose: Vec<Value> = ordered
        .iter()
        .filter(|m| m.project_id.is_none())
        .filter(|m| space.as_ref().map_or(true, |s| &m.space_id == s))
        .map(|m| {
            let mut v = milestone(m);
            v["space"] = json!(space_name(&m.space_id));
            v
        })
        .collect();
    Ok(json!({"projects": projects, "milestones": loose}))
}

/// `add_project`: a project marked as Claude's, active, with nothing in it.
pub fn add_project(store: &mut Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let world = World::load(store)?;
    let space = match text(args, "space") {
        Some(q) => world.space(q)?,
        None => match world.spaces.first() {
            Some(s) => s,
            None => return refused("There's no space yet. Open Learn in Wi_WWAV once."),
        },
    };
    let title = text(args, "title").unwrap_or_default();
    if world
        .projects
        .iter()
        .any(|p| p.space_id == space.id && p.title.trim().eq_ignore_ascii_case(title))
    {
        return refused(format!(
            "{} already has a project called {title}.",
            space.name
        ));
    }
    let mut record = Map::new();
    record.insert("spaceId".into(), json!(space.id));
    record.insert("title".into(), json!(title));
    if let Some(day) = text(args, "target_date") {
        record.insert("targetDate".into(), json!(day));
    }
    let project = drafted(&world, clock, kind::PROJECT, record, args)?;
    let undo = change(
        store,
        "Claude's project",
        claude("add_project", args),
        |txn| put(txn, kind::PROJECT, &project),
    )?;
    Ok(json!({"project": project, "undo_label": undo}))
}

/// `add_milestone`: a milestone marked as Claude's, never done, on a project
/// or loose in a space.
pub fn add_milestone(store: &mut Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let world = World::load(store)?;
    let project = match text(args, "project_id") {
        Some(id) => match world.projects.iter().find(|p| p.id == id) {
            Some(p) => Some(p),
            None => return refused("No project has that id."),
        },
        None => None,
    };
    let space_id = match (text(args, "space"), project) {
        (Some(q), _) => world.space(q)?.id.clone(),
        (None, Some(p)) => p.space_id.clone(),
        (None, None) => match world.spaces.first() {
            Some(s) => s.id.clone(),
            None => return refused("There's no space yet. Open Learn in Wi_WWAV once."),
        },
    };
    let mut record = Map::new();
    record.insert("spaceId".into(), json!(space_id));
    record.insert(
        "title".into(),
        json!(text(args, "title").unwrap_or_default()),
    );
    record.insert("date".into(), json!(text(args, "date").unwrap_or_default()));
    if let Some(p) = project {
        record.insert("projectId".into(), json!(p.id));
    }
    let milestone = drafted(&world, clock, kind::MILESTONE, record, args)?;
    let undo = change(
        store,
        "Claude's milestone",
        claude("add_milestone", args),
        |txn| put(txn, kind::MILESTONE, &milestone),
    )?;
    Ok(json!({"milestone": milestone, "undo_label": undo}))
}

fn clipped(markdown: &str) -> (String, bool) {
    let clipped: String = markdown.chars().take(NOTE_CHARS).collect();
    let cut = clipped.len() < markdown.len();
    (clipped, cut)
}

/// `get_notes`: the notes, newest first, and one day's daily note. Read only.
pub fn get_notes(store: &Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let today = clock.today();
    let date = text(args, "date").unwrap_or(&today);
    if !schema::is_day(date) {
        return refused("A day is written YYYY-MM-DD.");
    }
    let limit = whole(args, "limit").unwrap_or(20).clamp(1, 100) as usize;
    let mut notes = all(store, kind::NOTE)?;
    // Ids are ULIDs, so the newest sorts last.
    notes.sort_by(|a, b| b["id"].as_str().cmp(&a["id"].as_str()));
    let notes: Vec<Value> = notes
        .iter()
        .take(limit)
        .map(|n| {
            let (text, cut) = clipped(n["markdown"].as_str().unwrap_or_default());
            json!({
                "id": n["id"],
                "title": n["title"],
                "text": text,
                "cut_short": cut,
                "project_id": n["projectId"],
                "by": n["source"].as_str().unwrap_or("you"),
            })
        })
        .collect();
    let daily = one(store, kind::DAILY_NOTE, date)?.map(|d| {
        let (text, cut) = clipped(d["markdown"].as_str().unwrap_or_default());
        json!({"date": date, "text": text, "cut_short": cut})
    });
    Ok(json!({"notes": notes, "daily_note": daily}))
}

/// `add_note`: a note marked as Claude's. It never touches a note the person
/// wrote, and never the daily note.
pub fn add_note(store: &mut Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let world = World::load(store)?;
    let mut record = Map::new();
    record.insert(
        "title".into(),
        json!(text(args, "title").unwrap_or_default()),
    );
    record.insert(
        "markdown".into(),
        json!(args.get("text").and_then(Value::as_str).unwrap_or("")),
    );
    if let Some(id) = text(args, "project_id") {
        if !world.projects.iter().any(|p| p.id == id) {
            return refused("No project has that id.");
        }
        record.insert("projectId".into(), json!(id));
    }
    let note = drafted(&world, clock, kind::NOTE, record, args)?;
    let undo = change(store, "Claude's note", claude("add_note", args), |txn| {
        put(txn, kind::NOTE, &note)
    })?;
    Ok(json!({"note": {"id": note["id"], "title": note["title"]}, "undo_label": undo}))
}

/// `list_inbox`: the captures that wait to be triaged. Read only: only the
/// person triages.
pub fn list_inbox(store: &Store) -> Result<Value> {
    let world = World::load(store)?;
    let raw = all(store, kind::CAPTURE)?;
    let waiting: Vec<Value> = world
        .captures
        .iter()
        .filter(|c| c.triaged_at.is_none())
        .map(|c| {
            let by = raw
                .iter()
                .find(|r| r["id"].as_str() == Some(&c.id))
                .and_then(|r| r["source"].as_str())
                .unwrap_or("you");
            json!({"id": c.id, "text": c.text, "by": by})
        })
        .collect();
    Ok(json!({"captures": waiting}))
}

/// `add_capture`: a line in the Inbox, marked as Claude's, for the person to
/// triage into a task, a note or a project, or to throw away.
pub fn add_capture(store: &mut Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let world = World::load(store)?;
    let mut record = Map::new();
    record.insert("text".into(), json!(text(args, "text").unwrap_or_default()));
    let capture = drafted(&world, clock, kind::CAPTURE, record, args)?;
    let undo = change(
        store,
        "Claude's capture",
        claude("add_capture", args),
        |txn| put(txn, kind::CAPTURE, &capture),
    )?;
    let waiting = world
        .captures
        .iter()
        .filter(|c| c.triaged_at.is_none())
        .count()
        + 1;
    Ok(json!({"capture": capture, "in_inbox": waiting, "undo_label": undo}))
}

/// `list_mail_accounts`: the accounts whose mail Claude reads, the Gmail
/// search that finds each one's mail, and how many threads are recorded
/// from each. Read only.
pub fn list_mail_accounts(store: &Store) -> Result<Value> {
    let accounts = mail::accounts(store)?;
    let threads = all(store, kind::MAIL)?;
    let list: Vec<Value> = accounts
        .iter()
        .map(|a| {
            let at = a["address"].as_str().unwrap_or_default();
            let forwarded = a["forwardTo"].as_str();
            json!({
                "address": at,
                "name": a["name"],
                "arrives": match forwarded {
                    Some(to) => format!("forwarded to {to}, in the mailbox your Gmail tools read"),
                    None => "in the mailbox your Gmail tools read".to_string(),
                },
                "gmail_query": mail::gmail_query(a, &accounts),
                "recorded": threads.iter().filter(|t| t["account"].as_str() == Some(at)).count(),
            })
        })
        .collect();
    let mut out = json!({
        "accounts": list,
        "priorities": mail::PRIORITIES,
        "categories": mail::CATEGORIES,
    });
    if accounts.is_empty() {
        out["note"] = json!("No mail account is set up yet. Mail can still be recorded with no account. The person adds accounts in Settings → Learn.");
    }
    Ok(out)
}

/// `list_mail`: the threads recorded so far, newest first, so Claude can see
/// what it has been through and how it sorted it. Read only.
pub fn list_mail(store: &Store, clock: &Clock, args: &Map<String, Value>) -> Result<Value> {
    let account = text(args, "account").map(mail::address);
    let limit = whole(args, "limit").unwrap_or(50).clamp(1, 200) as usize;
    let since = text(args, "since").map(parse_instant).transpose()?;
    let is = |t: &Value, field: &str, want: Option<&str>| {
        want.map_or(true, |w| t[field].as_str() == Some(w))
    };
    let mut threads: Vec<Value> = all(store, kind::MAIL)?
        .into_iter()
        .filter(|t| is(t, "account", account.as_deref()))
        .filter(|t| is(t, "state", text(args, "state")))
        .filter(|t| is(t, "category", text(args, "category")))
        .filter(|t| match text(args, "priority") {
            Some(p) => t["priority"].as_str().unwrap_or("normal") == p,
            None => true,
        })
        .filter(|t| since.map_or(true, |s| t["receivedAt"].as_f64().unwrap_or(0.0) >= s))
        .collect();
    threads.sort_by(|a, b| {
        b["receivedAt"]
            .as_f64()
            .partial_cmp(&a["receivedAt"].as_f64())
            .unwrap()
    });
    let total = threads.len();
    let saved = mail::with_text(store)?;
    let threads: Vec<Value> = threads
        .iter()
        .take(limit)
        .map(|t| {
            json!({
                "thread_id": t["gmailThreadId"],
                "account": t["account"],
                "subject": t["subject"],
                "from": t["from"],
                "received_at": clock.iso(t["receivedAt"].as_f64().unwrap_or(0.0)),
                "course": t["course"],
                "state": t["state"],
                "priority": t["priority"].as_str().unwrap_or("normal"),
                "category": t["category"],
                "reason": t["reason"],
                "task_id": t["taskId"],
                "has_text": t["gmailThreadId"].as_str().is_some_and(|id| saved.iter().any(|s| s == id)),
            })
        })
        .collect();
    Ok(json!({"threads": threads, "recorded": total}))
}
