//! What the views read (docs/HEAT.md): the snapshot with every derived value
//! the model works out, so a view never sorts by heat, clamps, plans or does
//! grade arithmetic; what it would take to reach a grade; the weekly review's
//! facts; the public Heat view as someone opening your sun would see it; and
//! what Settings → Claude lists.

use serde_json::{json, Map, Value};
use wi_heat::model::estimate::{estimate_min, format_minutes};
use wi_heat::model::focus::{focus_lcd, focus_strip};
use wi_heat::model::heat::{self, HeatLevel};
use wi_heat::model::plan::{plan_sections, plan_subtitle, PlanData, PlanSection, RecurringKind};
use wi_heat::model::records::{Capture, Milestone, Project, Task};
use wi_heat::model::review::{fact_lines, last_week_facts, ReviewData};
use wi_heat::model::spaces::{library_lists, SidebarData};
use wi_heat::model::{copy, grades, habits, recurrence, zone};
use wi_store::{Actor, Store};

use crate::derive::World;
use crate::schema::is_day;
use crate::timer::{load_focus, public_state};
use crate::{all, kind, mcp, num, refused, state, tool_switches, Clock, Result};

/// The kinds the snapshot carries, in the order HEAT.md lists them.
pub const SNAPSHOT_KINDS: [&str; 17] = [
    kind::SPACE,
    kind::TASK,
    kind::OCCURRENCE,
    kind::BLOCK,
    kind::FOCUS,
    kind::PROJECT,
    kind::MILESTONE,
    kind::HABIT,
    kind::TERM,
    kind::COURSE,
    kind::GRADE,
    kind::MAIL,
    kind::CALENDAR,
    kind::CAPTURE,
    kind::DAILY_NOTE,
    kind::NOTE,
    kind::SHARE,
];

/// Which day the views are on, the stretch of days events and blocks are
/// wanted for, and when the account last synced.
#[derive(Clone, Debug, Default)]
pub struct Window {
    pub date: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    /// When Heat last reached mi-wwav.com, for "Saved on this Mac · Synced 3:41 PM".
    pub synced_at: Option<f64>,
}

fn ids(v: &[&Task]) -> Vec<Value> {
    v.iter().map(|t| json!(t.id)).collect()
}

fn task_ids(v: &[Task]) -> Vec<Value> {
    v.iter().map(|t| json!(t.id)).collect()
}

/// The shown line of a time: "3:41 PM", by the person's clock.
pub fn clock_text(clock: &Clock, ms: f64) -> String {
    wi_heat::model::format::clock_at(ms, &clock.zone)
}

/// A day's first moment and the next day's, for bounding events.
fn day_bounds(clock: &Clock, from: &str, to: &str) -> (f64, f64) {
    (zone::start_of_day(from, &clock.zone), zone::start_of_day(&zone::add_days(to, 1.0), &clock.zone))
}

/// A month around a day: from a week before the 1st to a week after the last.
fn default_range(date: &str) -> (String, String) {
    let (year, month, _) = zone::key_parts(date);
    let first = zone::key_of(year, month, 1.0);
    let last = zone::key_of(year, month, zone::days_in_month(year, month));
    (zone::add_days(&first, -7.0), zone::add_days(&last, 7.0))
}

/// `heat.snapshot`: everything the views draw for a day.
pub fn snapshot(store: &Store, clock: &Clock, window: &Window) -> Result<Value> {
    let today = clock.today();
    let date = match window.date.as_deref() {
        Some(d) if is_day(d) => d.to_string(),
        Some(_) => return refused("A day is written YYYY-MM-DD."),
        None => today.clone(),
    };
    let (default_from, default_to) = default_range(&date);
    let from = window.from.clone().filter(|d| is_day(d)).unwrap_or(default_from);
    let to = window.to.clone().filter(|d| is_day(d)).unwrap_or(default_to);
    let world = World::load(store)?;
    let (now, tz) = (clock.now_ms, &clock.zone);

    // The records, as stored. Blocks and events are bounded to the stretch
    // asked for; a Now making line past its time is no longer there (3.15).
    let mut records = Map::new();
    for k in SNAPSHOT_KINDS {
        let mut list = all(store, k)?;
        match k {
            kind::BLOCK => list.retain(|b| {
                b["date"].as_str().is_some_and(|d| d >= from.as_str() && d <= to.as_str())
            }),
            kind::SHARE => list.retain(|s| s.get("clearsAt").and_then(Value::as_f64).map_or(true, |t| t > now)),
            _ => {}
        }
        records.insert(k.to_string(), Value::Array(list));
    }
    let (lo, hi) = day_bounds(clock, &from, &to);
    let events: Vec<Value> = all(store, kind::EVENT)?
        .into_iter()
        .filter(|e| {
            let (s, t) = (e["start"].as_f64().unwrap_or(0.0), e["end"].as_f64().unwrap_or(0.0));
            s < hi && t.max(s) >= lo
        })
        .collect();

    // Heat, task by task: as heat sees it (a recurring task by its next open
    // occurrence), what has been spent, and the estimate with who made it.
    let ctx = world.estimates();
    let mut tasks = Map::new();
    for (i, t) in world.tasks.iter().enumerate() {
        let effective = recurrence::with_effective_due(t, &world.occurrences, now, tz);
        let h = heat::heat_of(&effective, now);
        let raw = &world.raw_tasks[i];
        let computed = estimate_min(t, &ctx);
        let by = if t.est_min.filter(|m| *m > 0.0) == Some(computed) {
            raw.get("estBy").and_then(Value::as_str).unwrap_or("you")
        } else {
            "default"
        };
        let mut entry = json!({
            "heat": {"v": h.v.map_or(Value::Null, num), "level": h.level.as_str()},
            "actualMin": num(wi_heat::model::estimate::actual_min(t, &world.sessions)),
            "estimate": {"min": num(computed), "by": by, "reason": raw.get("estReason").cloned().unwrap_or(Value::Null)},
        });
        if recurrence::recurs(t) {
            let next = recurrence::next_open_occurrence(t, &world.occurrences, now, tz);
            entry["next"] = next.map_or(Value::Null, |n| json!(n.date));
        }
        tasks.insert(t.id.clone(), entry);
    }

    // Today: the plan list's sections as ids, and its subtitle.
    let data = PlanData {
        tasks: world.tasks.clone(),
        occurrences: world.occurrences.clone(),
        blocks: world.blocks.clone(),
        events: world.events.clone(),
        sessions: world.sessions.clone(),
        habits: world.habits.clone(),
    };
    let view_now = if date == today { now } else { zone::at_minute(&date, 0.0, tz) };
    let (mut planned, mut due_today, mut recurring, mut hot_unplanned) = (vec![], vec![], vec![], vec![]);
    for section in plan_sections(&data, view_now, tz, None) {
        match section {
            PlanSection::Planned { items, .. } => planned = items.iter().map(|r| json!(r.block.id)).collect::<Vec<_>>(),
            PlanSection::DueToday { items, .. } => due_today = task_ids(&items),
            PlanSection::Recurring { items, .. } => {
                recurring = items
                    .iter()
                    .map(|r| json!({"kind": if r.kind == RecurringKind::Task { "task" } else { "habit" }, "id": r.id}))
                    .collect()
            }
            PlanSection::Hot { items, .. } => hot_unplanned = task_ids(&items),
        }
    }
    let today_part = json!({
        "header": plan_subtitle(&data, view_now, tz, None),
        "planned": planned,
        "dueToday": due_today,
        "recurringToday": recurring,
        "hotUnplanned": hot_unplanned,
    });

    // Hot tasks: up to five Hot or Overdue, from every space (3.5).
    let open = recurrence::open_tasks(&world.tasks, &world.occurrences, now, tz);
    let hot: Vec<&Task> = heat::by_heat(&open, now)
        .into_iter()
        .filter(|t| matches!(heat::heat_of(*t, now).level, HeatLevel::Hot | HeatLevel::Overdue))
        .take(5)
        .collect();

    let side = SidebarData {
        tasks: world.tasks.clone(),
        occurrences: world.occurrences.clone(),
        captures: world.captures.clone(),
        projects: world.projects.clone(),
        milestones: world.milestones.clone(),
        courses: world.courses.clone(),
    };
    let lists = library_lists(&side, now, tz, None);
    let capture_ids = |v: &[Capture]| -> Vec<Value> { v.iter().map(|c| json!(c.id)).collect() };

    // "Your average time", by space and then by the space's own order of types.
    let mut averages = Vec::new();
    for space in &world.spaces {
        let mut kinds: Vec<&str> = space.types.iter().map(String::as_str).collect();
        let mut extra: Vec<&str> = ctx
            .averages
            .keys()
            .filter_map(|k| k.split_once('\u{0}'))
            .filter(|(s, t)| *s == space.id && !kinds.contains(t))
            .map(|(_, t)| t)
            .collect();
        extra.sort_unstable();
        kinds.extend(extra);
        for t in kinds {
            if let Some(a) = ctx.averages.get(&wi_heat::model::estimate::type_key(&space.id, t)) {
                averages.push(json!({
                    "space": space.id, "type": t, "minutes": num(a.minutes), "count": num(a.count),
                    "line": copy::average_time(t, &format_minutes(a.minutes), a.count),
                }));
            }
        }
    }

    let mut courses = Map::new();
    for c in &world.courses {
        let scale = c.scale.clone().unwrap_or_else(grades::default_scale);
        let current = grades::current_pct(c, &world.grades);
        courses.insert(
            c.id.clone(),
            json!({
                "currentPct": current.map_or(Value::Null, num),
                "decidedPct": num(grades::decided_pct(c, &world.grades)),
                "letter": current.map_or(Value::Null, |p| json!(grades::letter_for(p, &scale))),
                "weights": grades::weights_line(c).map_or(Value::Null, |w| json!(w)),
                "basedOn": grades::based_on_line(c, &world.grades),
            }),
        );
    }

    let mut habit_lines = Map::new();
    for h in &world.habits {
        habit_lines.insert(
            h.id.clone(),
            json!({
                "today": h.log.get(&today).copied().unwrap_or(false),
                "record": habits::habit_line(h, &today).unwrap_or_default(),
            }),
        );
    }

    let st = state(store)?;
    let fs = load_focus(&st);
    let lcd = focus_lcd(&fs, now);
    let status = match window.synced_at {
        Some(at) => format!("Saved on this Mac · Synced {}", clock_text(clock, at)),
        None => "Saved on this Mac".to_string(),
    };
    Ok(json!({
        "now": num(now),
        "date": date,
        "zone": clock.zone.iana_name().unwrap_or("UTC"),
        "records": records,
        "heatState": public_state(&st),
        "events": events,
        "derived": {
            "tasks": tasks,
            "today": today_part,
            "hotTasks": ids(&hot),
            "lists": {
                "inbox": capture_ids(&lists.inbox),
                "allOpen": task_ids(&lists.all_open),
                "hot": task_ids(&lists.hot),
                "dueThisWeek": task_ids(&lists.due_this_week),
                "scheduled": task_ids(&lists.scheduled),
                "someday": task_ids(&lists.someday),
                "done": task_ids(&lists.done),
            },
            "averages": averages,
            "courses": courses,
            "habits": habit_lines,
            "timer": {
                "digits": lcd.digits, "line": lcd.line, "note": lcd.note, "meter": num(lcd.meter), "paused": lcd.paused,
                "strip": focus_strip(&fs, now),
            },
            "status": status,
        },
    }))
}

/// `heat.whatItWouldTake`: "To finish with a B (83%), you need 78.4% on the
/// remaining 35%."
pub fn what_it_would_take(store: &Store, course_id: &str, letter: &str) -> Result<Value> {
    let world = World::load(store)?;
    let Some(course) = world.courses.iter().find(|c| c.id == course_id) else {
        return refused("No course has that id.");
    };
    match grades::what_it_would_take(course, &world.grades, letter) {
        Some(text) => Ok(json!({ "text": text })),
        None => refused(format!("This course has no grade called {letter}.")),
    }
}

/// `heat.review.week`: step 2's facts for the week starting `week_start`, and
/// step 5's headings with the facts beneath them. Facts only: no score, no
/// streak, no comparison, and no note (3.14).
pub fn review_week(store: &Store, clock: &Clock, week_start: &str) -> Result<Value> {
    if !is_day(week_start) {
        return refused("A week starts on a day, written YYYY-MM-DD.");
    }
    let world = World::load(store)?;
    let data = ReviewData {
        spaces: world.spaces.clone(),
        tasks: world.tasks.clone(),
        occurrences: world.occurrences.clone(),
        sessions: world.sessions.clone(),
        milestones: world.milestones.clone(),
    };
    // The facts are "the 7 days before today": a moment in the day after the week.
    let after = zone::at_minute(&zone::add_days(week_start, 7.0), 12.0 * 60.0, &clock.zone);
    let facts = last_week_facts(&data, after, &clock.zone);
    let lines = fact_lines(&facts);
    // What moved is the work and the time; what slipped is where an estimate and the time taken parted.
    let accuracy_from = lines.len() - facts.accuracy.len();
    let (moved, slipped) = lines.split_at(accuracy_from);
    Ok(json!({
        "weekStart": week_start,
        "facts": facts,
        "lines": lines,
        "headings": [
            {"title": "What moved", "lines": moved},
            {"title": "What slipped", "lines": slipped},
            {"title": "Next week's one thing", "lines": []},
        ],
        "complete": "Review complete ✓",
    }))
}

/// What a public record shows of itself, by kind: 3.15's table, and nothing
/// else. The same fields the server's `/api/heat/public/:userId` shows.
const SHOWN: [(&str, &[&str]); 9] = [
    (kind::TASK, &["title", "due", "done"]),
    (kind::PROJECT, &["title", "status", "targetDate"]),
    (kind::MILESTONE, &["title", "date", "done"]),
    (kind::HABIT, &["title", "log"]),
    (kind::NOTE, &["title", "markdown"]),
    (kind::DAILY_NOTE, &["date", "markdown"]),
    (kind::COURSE, &["code", "name"]),
    (kind::GRADE, &["title", "score", "outOf"]),
    (kind::FOCUS, &["startedAt", "focusMin"]),
];

/// `heat.publicView`: exactly what someone opening your sun would see
/// (3.15), built from the local records in the shape the server answers
/// with, so Settings → Privacy previews the real thing. Never a count or a total.
pub fn public_view(store: &Store, clock: &Clock) -> Result<Value> {
    let now = clock.now_ms;
    let shares = all(store, kind::SHARE)?;
    let live = |s: &Value| s.get("clearsAt").and_then(Value::as_f64).map_or(true, |t| t > now);
    let now_line = shares
        .iter()
        .find(|s| s["kind"] == "now" && live(s) && s["text"].is_string())
        .map(|s| json!({"text": s["text"]}))
        .unwrap_or(Value::Null);
    let projects: Vec<Project> = all(store, kind::PROJECT)?.iter().filter_map(|p| serde_json::from_value(p.clone()).ok()).collect();
    let milestones: Vec<Milestone> = all(store, kind::MILESTONE)?.iter().filter_map(|m| serde_json::from_value(m.clone()).ok()).collect();
    let mut timelines = Vec::new();
    for s in shares.iter().filter(|s| s["kind"] == "timeline") {
        let Some(project) = projects.iter().find(|p| Some(p.id.as_str()) == s["sourceId"].as_str()) else {
            continue;
        };
        let mut beads: Vec<&Milestone> = milestones.iter().filter(|m| m.project_id.as_deref() == Some(project.id.as_str())).collect();
        beads.sort_by(|a, b| a.date.cmp(&b.date).then(a.order.total_cmp(&b.order)));
        timelines.push(json!({
            "projectId": project.id,
            "targetId": s["targetId"],
            "title": project.title,
            "milestones": beads.iter().map(|m| json!({"id": m.id, "title": m.title, "date": m.date, "done": m.done})).collect::<Vec<_>>(),
        }));
    }
    let courses = all(store, kind::COURSE)?;
    let tasks = all(store, kind::TASK)?;
    let mut items = Map::new();
    for (k, fields) in SHOWN {
        let mut list = Vec::new();
        for d in store.docs_oldest_first(k)? {
            if d.json.get("public") != Some(&json!(true)) {
                continue;
            }
            let mut item = Map::new();
            item.insert("id".into(), json!(d.key));
            for f in fields {
                if let Some(v) = d.json.get(*f) {
                    item.insert((*f).into(), v.clone());
                }
            }
            // A grade names its course, and a focus record its task, by the
            // course's code and the task's title: nothing else of theirs.
            if k == kind::GRADE {
                let code = courses.iter().find(|c| c["id"] == d.json["courseId"]).map(|c| c["code"].clone());
                item.insert("course".into(), code.unwrap_or(Value::Null));
            }
            if k == kind::FOCUS {
                let title = tasks.iter().find(|t| t["id"] == d.json["taskId"]).map(|t| t["title"].clone());
                item.insert("task".into(), title.unwrap_or(Value::Null));
            }
            list.push(Value::Object(item));
        }
        if !list.is_empty() {
            items.insert(k.to_string(), Value::Array(list));
        }
    }
    Ok(json!({"now": now_line, "timelines": timelines, "items": items}))
}

/// Settings → Claude's lists: the eight tools with their switches, and
/// Claude's recent changes, each with its reason.
pub fn claude_data(store: &Store) -> Result<Value> {
    let switches = tool_switches(store)?;
    let tools: Vec<Value> = mcp::TOOLS
        .iter()
        .map(|t| json!({"name": t, "on": switches.get(*t).copied().unwrap_or(true)}))
        .collect();
    let recent: Vec<Value> = store
        .entries(true, 50)?
        .into_iter()
        .map(|e| {
            let reason = match &e.actor {
                Actor::Claude { reason, .. } => reason.clone(),
                Actor::You => String::new(),
            };
            json!({"txnId": e.id, "label": e.label, "reason": reason, "at": e.at_ms, "undone": !e.done})
        })
        .collect();
    Ok(json!({ "tools": tools, "recent": recent }))
}
