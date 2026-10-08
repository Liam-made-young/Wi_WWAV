//! The Focus layout's rules (docs/FOCUS.md): a port of
//! `app/ui/src/focus/model.ts`, which is the reference. Nothing here reads
//! the clock, the library or the network: every function works on the
//! snapshot's JSON (`heat.snapshot`), so the same snapshot gives the same
//! answer here and in the TypeScript.
//!
//! Three things are decided, each from the snapshot alone:
//!
//! - [`entropy`]: how out of order things are, 0 to 1;
//! - [`now_of`]: what Focus shows: a task, the fix, or nothing;
//! - [`should_interrupt`]: whether an event may put one line under the Now task.
//!
//! Every number they turn on is in [`Config`].

use std::collections::{BTreeMap, HashSet};

use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wi_heat::model::estimate::format_minutes;
use wi_heat::model::format::clock_at;
use wi_heat::model::heat::due_phrase;
use wi_heat::model::records::ser;
use wi_heat::model::{copy, js, zone};
use wi_store::DocChange;

const MIN: f64 = 60_000.0;
const HOUR: f64 = 60.0 * MIN;
const DAY: f64 = 24.0 * HOUR;

/// What one of each open loop weighs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Weights {
    #[serde(serialize_with = "ser::num")]
    pub overdue: f64,
    #[serde(serialize_with = "ser::num")]
    pub unplanned: f64,
    #[serde(serialize_with = "ser::num")]
    pub mail: f64,
    #[serde(serialize_with = "ser::num")]
    pub grades: f64,
}

/// Every threshold of the Focus layout, in one place: the one config. The
/// snapshot carries it as `focus.config`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// "This week": a task due within this many days counts when it has no planned time.
    #[serde(serialize_with = "ser::num")]
    pub horizon_days: f64,
    pub weights: Weights,
    /// The weighted sum at which entropy is 1.
    #[serde(serialize_with = "ser::num")]
    pub full: f64,
    /// Entropy at or above this is "busy"; below it, and above 0, "calm".
    #[serde(serialize_with = "ser::num")]
    pub busy_at: f64,
    /// Entropy at or above this is "high": Focus shows the fix before the next task.
    #[serde(serialize_with = "ser::num")]
    pub high_at: f64,
    /// Deadlines this near are checked for risk.
    #[serde(serialize_with = "ser::num")]
    pub risk_horizon_days: f64,
    /// The stretch of a day that can be planned, in minutes after midnight.
    #[serde(serialize_with = "ser::num")]
    pub day_starts_min: f64,
    #[serde(serialize_with = "ser::num")]
    pub day_ends_min: f64,
    /// No day gives more than this much working time, however empty it is.
    #[serde(serialize_with = "ser::num")]
    pub plannable_per_day_min: f64,
    /// Work shorter than this is never "at risk".
    #[serde(serialize_with = "ser::num")]
    pub risk_floor_min: f64,
    /// A due date that moved interrupts when it now falls within this many days.
    #[serde(serialize_with = "ser::num")]
    pub due_changed_within_days: f64,
    /// A task from mail is urgent when it is due within this many hours.
    #[serde(serialize_with = "ser::num")]
    pub urgent_within_hours: f64,
    /// "Time to leave" shows this many minutes before a commitment starts.
    #[serde(serialize_with = "ser::num")]
    pub leave_lead_min: f64,
    /// A queued event older than this is dropped unseen.
    #[serde(serialize_with = "ser::num")]
    pub queue_keep_days: f64,
    /// The hint under the task shows for this many days or launches, whichever ends first.
    #[serde(serialize_with = "ser::num")]
    pub hint_days: f64,
    #[serde(serialize_with = "ser::num")]
    pub hint_launches: f64,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            horizon_days: 7.0,
            weights: Weights {
                overdue: 3.0,
                unplanned: 2.0,
                mail: 1.5,
                grades: 0.5,
            },
            full: 12.0,
            busy_at: 0.2,
            high_at: 0.5,
            risk_horizon_days: 7.0,
            day_starts_min: 7.0 * 60.0,
            day_ends_min: 22.0 * 60.0,
            plannable_per_day_min: 6.0 * 60.0,
            risk_floor_min: 15.0,
            due_changed_within_days: 7.0,
            urgent_within_hours: 48.0,
            leave_lead_min: 15.0,
            queue_keep_days: 7.0,
            hint_days: 7.0,
            hint_launches: 20.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Clear,
    Calm,
    Busy,
    High,
}

/// How many of each open loop there are.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parts {
    pub unplanned: u32,
    pub overdue: u32,
    pub mail: u32,
    pub grades: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entropy {
    /// 0 when every loop is closed, 1 at `full` and beyond.
    #[serde(serialize_with = "ser::num")]
    pub score: f64,
    pub level: Level,
    pub parts: Parts,
}

/// What an [`Action`] does. `current`: make the task the Now task. `task`:
/// show it in Tasks. `view`: open a tool. `break`: start the break. `plan`:
/// Plan my day. `command`: run one of Learn's own commands (`cmd`, `args`),
/// for a view that raises a line with an answer of its own ("Skip it").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Do {
    Current,
    Task,
    View,
    Break,
    Plan,
    Command,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub label: String,
    #[serde(rename = "do")]
    pub act: Do,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    /// A `command` action's command, one of `heat.*`, and what it is given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Value>,
}

impl Action {
    fn new(label: &str, act: Do) -> Action {
        Action {
            label: label.to_string(),
            act,
            task_id: None,
            view: None,
            cmd: None,
            args: None,
        }
    }

    fn on_task(label: &str, act: Do, task_id: &str) -> Action {
        Action {
            task_id: Some(task_id.to_string()),
            ..Action::new(label, act)
        }
    }

    fn open(label: &str, view: &str) -> Action {
        Action {
            view: Some(view.to_string()),
            ..Action::new(label, Do::View)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FixKind {
    Plan,
    Mail,
    Grades,
}

/// The one thing that would put the most back in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    pub kind: FixKind,
    /// "4 tasks this week have no plan."
    pub line: String,
    /// "Plan them?"
    pub ask: String,
    pub action: Action,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NowKind {
    Task,
    Fix,
    Clear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Why {
    Current,
    Heat,
}

/// What Focus shows. A task has `task_id` and `why`; the fix has `fix`;
/// clear has neither.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Now {
    pub kind: NowKind,
    pub task_id: Option<String>,
    pub why: Option<Why>,
    pub fix: Option<Fix>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
}

impl Priority {
    /// The most pressing sorts first.
    fn rank(self) -> u8 {
        match self {
            Priority::High => 0,
            Priority::Normal => 1,
            Priority::Low => 2,
        }
    }
}

/// Something that happened, or is so now, that might be worth one line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Event {
    DueChanged {
        task_id: String,
        #[serde(serialize_with = "ser::opt_num")]
        from: Option<f64>,
        #[serde(serialize_with = "ser::opt_num")]
        to: Option<f64>,
        #[serde(serialize_with = "ser::num")]
        at: f64,
    },
    AtRisk {
        task_id: String,
        #[serde(serialize_with = "ser::num")]
        due: f64,
        #[serde(serialize_with = "ser::num")]
        work_left_min: f64,
        #[serde(serialize_with = "ser::num")]
        plannable_min: f64,
    },
    MailUrgent {
        task_id: String,
        #[serde(serialize_with = "ser::num")]
        at: f64,
    },
    FocusEnded {
        key: String,
        note: Option<String>,
    },
    LeaveFor {
        event_id: String,
        title: String,
        #[serde(serialize_with = "ser::num")]
        start: f64,
    },
    GradeWaiting {
        grade_id: String,
        title: String,
        #[serde(serialize_with = "ser::num")]
        count: f64,
    },
    Custom {
        id: String,
        source: String,
        line: String,
        action: Option<Action>,
        changes_next: bool,
        priority: Priority,
        #[serde(serialize_with = "ser::num")]
        at: f64,
    },
}

impl Event {
    /// When a queued event happened; None for one that is simply so now.
    fn at(&self) -> Option<f64> {
        match self {
            Event::DueChanged { at, .. }
            | Event::MailUrgent { at, .. }
            | Event::Custom { at, .. } => Some(*at),
            _ => None,
        }
    }
}

/// The one line Focus may show under the Now task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interrupt {
    pub id: String,
    /// The event's kind, or a custom event's own source: what a view registers to be opened by.
    pub source: String,
    pub line: String,
    pub action: Option<Action>,
    pub priority: Priority,
}

/// What a commitment is until commitments have a record of their own: the next calendar event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NextCommitment {
    pub id: String,
    pub title: String,
    #[serde(serialize_with = "ser::num")]
    pub start: f64,
}

/// What the core remembers between snapshots; none of it is journaled.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Memory {
    /// Events that happened: a due date moved, mail made a task, something raised by a view.
    pub queue: Vec<Event>,
    /// Interrupts sent away, by id, with when.
    pub dismissed: BTreeMap<String, f64>,
    /// The day the fix was last put off, so it isn't offered again that day.
    pub fix_snoozed: Option<String>,
}

/// What [`should_interrupt`] decides against.
pub struct Ctx<'a> {
    pub snap: &'a Value,
    /// The task Focus shows, if it shows one.
    pub now_task_id: Option<&'a str>,
    pub cfg: &'a Config,
}

// ----- reading the snapshot, as the TypeScript reads it -----

fn list(v: &Value) -> &[Value] {
    v.as_array().map_or(&[], Vec::as_slice)
}

/// Whether JavaScript would take the value as true.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn snap_now(snap: &Value) -> f64 {
    snap["now"].as_f64().unwrap_or(0.0)
}

fn snap_date(snap: &Value) -> &str {
    snap["date"].as_str().unwrap_or("")
}

fn snap_zone(snap: &Value) -> TimeZone {
    snap["zone"]
        .as_str()
        .and_then(zone::zone)
        .unwrap_or(TimeZone::UTC)
}

/// `derived.lists.allOpen`: the open tasks, in heat order already.
fn open_ids(snap: &Value) -> Vec<&str> {
    list(&snap["derived"]["lists"]["allOpen"])
        .iter()
        .filter_map(Value::as_str)
        .collect()
}

fn task_of<'a>(snap: &'a Value, id: &str) -> Option<&'a Value> {
    list(&snap["records"]["task"])
        .iter()
        .find(|t| t["id"].as_str() == Some(id))
}

/// Whether a task has time set aside from today on: a block, or a day it is scheduled for.
fn planned(snap: &Value, t: &Value) -> bool {
    let date = snap_date(snap);
    if t["scheduledDate"].as_str().is_some_and(|d| d >= date) {
        return true;
    }
    let id = t["id"].as_str();
    id.is_some()
        && list(&snap["records"]["timeBlock"])
            .iter()
            .any(|b| b["taskId"].as_str() == id && b["date"].as_str().is_some_and(|d| d >= date))
}

/// A mail thread still asking for something: it made no task yet, or it is pressing and unread.
fn mail_needs_action(snap: &Value) -> u32 {
    list(&snap["records"]["mailThread"])
        .iter()
        .filter(|m| {
            let place = m["gmailThreadId"]
                .as_str()
                .and_then(|id| snap["mailState"].get(id));
            if place.is_some_and(|p| truthy(&p["archived"])) {
                return false;
            }
            if m["state"] == "task" && !truthy(&m["taskId"]) {
                return true;
            }
            place.is_some_and(|p| p["unread"] == true)
                && matches!(m["priority"].as_str(), Some("urgent" | "high"))
        })
        .count() as u32
}

fn pending_grades(snap: &Value) -> Vec<&Value> {
    list(&snap["records"]["grade"])
        .iter()
        .filter(|g| truthy(&g["pending"]))
        .collect()
}

fn round2(x: f64) -> f64 {
    js::round(x * 100.0) / 100.0
}

/// How out of order things are: overdue work, this week's tasks with no
/// planned time, mail that needs you and grades waiting for a score, each
/// weighed, over the sum at which things are fully out of order.
pub fn entropy(snap: &Value, cfg: &Config) -> Entropy {
    let now = snap_now(snap);
    let (mut overdue, mut unplanned) = (0u32, 0u32);
    for id in open_ids(snap) {
        let Some(t) = task_of(snap, id) else { continue };
        if snap["derived"]["tasks"][id]["heat"]["level"] == "Overdue" {
            overdue += 1;
            continue;
        }
        let Some(due) = t["due"].as_f64() else {
            continue;
        };
        if truthy(&t["rrule"]) {
            continue;
        }
        if due - now <= cfg.horizon_days * DAY && !planned(snap, t) {
            unplanned += 1;
        }
    }
    let parts = Parts {
        unplanned,
        overdue,
        mail: mail_needs_action(snap),
        grades: pending_grades(snap).len() as u32,
    };
    let w = &cfg.weights;
    let sum = f64::from(parts.overdue) * w.overdue
        + f64::from(parts.unplanned) * w.unplanned
        + f64::from(parts.mail) * w.mail
        + f64::from(parts.grades) * w.grades;
    let score = round2(js::min2(1.0, sum / cfg.full));
    let level = if sum == 0.0 {
        Level::Clear
    } else if score >= cfg.high_at {
        Level::High
    } else if score >= cfg.busy_at {
        Level::Busy
    } else {
        Level::Calm
    };
    Entropy {
        score,
        level,
        parts,
    }
}

fn count(n: u32, one: &str, many: &str) -> String {
    if n == 1 {
        one.to_string()
    } else {
        many.replacen('#', &n.to_string(), 1)
    }
}

/// The one thing that would put the most back in order: the heaviest part
/// that has a fix. Overdue work has none but doing it.
pub fn fix_of(e: &Entropy, cfg: &Config) -> Option<Fix> {
    let w = &cfg.weights;
    let p = &e.parts;
    let options = [
        (
            f64::from(p.unplanned) * w.unplanned,
            Fix {
                kind: FixKind::Plan,
                line: count(
                    p.unplanned,
                    "1 task this week has no plan.",
                    "# tasks this week have no plan.",
                ),
                ask: count(p.unplanned, "Plan it?", "Plan them?"),
                action: Action::new("Plan my day", Do::Plan),
            },
        ),
        (
            f64::from(p.mail) * w.mail,
            Fix {
                kind: FixKind::Mail,
                line: count(p.mail, "1 mail needs you.", "# mails need you."),
                ask: count(p.mail, "Read it?", "Read them?"),
                action: Action::open("Open Mail", "mail"),
            },
        ),
        (
            f64::from(p.grades) * w.grades,
            Fix {
                kind: FixKind::Grades,
                line: count(
                    p.grades,
                    "1 grade is waiting for its score.",
                    "# grades are waiting for their scores.",
                ),
                ask: count(p.grades, "Enter it?", "Enter them?"),
                action: Action::open("Open Grades", "grades"),
            },
        ),
    ];
    let mut best: Option<(f64, Fix)> = None;
    for (weight, fix) in options {
        if weight > 0.0 && best.as_ref().map_or(true, |b| weight > b.0) {
            best = Some((weight, fix));
        }
    }
    best.map(|b| b.1)
}

/// What Focus shows. The current task if one is set; when entropy is high,
/// the fix; else the top open task by heat; else the fix for what is left;
/// else nothing. A fix put off today stays put off until tomorrow.
pub fn now_of(snap: &Value, e: &Entropy, mem: &Memory, cfg: &Config) -> Now {
    let task = |id: &str, why: Why| Now {
        kind: NowKind::Task,
        task_id: Some(id.to_string()),
        why: Some(why),
        fix: None,
    };
    let the_fix = |fix: Fix| Now {
        kind: NowKind::Fix,
        task_id: None,
        why: None,
        fix: Some(fix),
    };
    let open = open_ids(snap);
    if let Some(current) = snap["heatState"]["currentTaskId"]
        .as_str()
        .filter(|c| !c.is_empty() && open.contains(c))
    {
        return task(current, Why::Current);
    }
    let fix = if mem.fix_snoozed.as_deref() == Some(snap_date(snap)) {
        None
    } else {
        fix_of(e, cfg)
    };
    match fix {
        Some(fix) if e.score >= cfg.high_at => the_fix(fix),
        // `allOpen` is in heat order already (wi-heat's model/spaces.rs).
        _ if !open.is_empty() => task(open[0], Why::Heat),
        Some(fix) => the_fix(fix),
        None => Now {
            kind: NowKind::Clear,
            task_id: None,
            why: None,
            fix: None,
        },
    }
}

/// The next calendar event that has a time and hasn't started.
pub fn next_commitment(snap: &Value) -> Option<NextCommitment> {
    let now = snap_now(snap);
    let mut next: Option<NextCommitment> = None;
    for e in list(&snap["events"]) {
        let Some(start) = e["start"].as_f64() else {
            continue;
        };
        if truthy(&e["allDay"]) || start <= now {
            continue;
        }
        if next.as_ref().map_or(true, |n| start < n.start) {
            next = Some(NextCommitment {
                id: e["id"].as_str().unwrap_or_default().to_string(),
                title: e["title"].as_str().unwrap_or_default().to_string(),
                start,
            });
        }
    }
    next
}

/// Minutes that can still be planned between now and `due`, with the
/// calendar and other tasks' blocks taken out.
pub fn plannable_min(snap: &Value, task_id: &str, due: f64, cfg: &Config) -> f64 {
    let tz = snap_zone(snap);
    let now = snap_now(snap);
    let last = zone::day_key(due, &tz);
    let mut total = 0.0;
    let mut day = snap_date(snap).to_string();
    let mut n = 0.0;
    while day <= last && n <= cfg.risk_horizon_days {
        let lo = js::max2(zone::at_minute(&day, cfg.day_starts_min, &tz), now);
        let hi = js::min2(zone::at_minute(&day, cfg.day_ends_min, &tz), due);
        if hi > lo {
            let mut busy: Vec<(f64, f64)> = Vec::new();
            for e in list(&snap["events"]) {
                if !truthy(&e["allDay"]) {
                    busy.push((
                        e["start"].as_f64().unwrap_or(f64::NAN),
                        e["end"].as_f64().unwrap_or(f64::NAN),
                    ));
                }
            }
            for b in list(&snap["records"]["timeBlock"]) {
                if b["date"].as_str() != Some(day.as_str()) || b["taskId"].as_str() == Some(task_id)
                {
                    continue;
                }
                let start = zone::at_minute(&day, b["start"].as_f64().unwrap_or(f64::NAN), &tz);
                busy.push((
                    start,
                    start + b["minutes"].as_f64().unwrap_or(f64::NAN) * MIN,
                ));
            }
            busy.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let mut taken = 0.0;
            let mut cursor = lo;
            for (s, t) in busy {
                let from = js::max2(s, cursor);
                let to = js::min2(t, hi);
                if to > from {
                    taken += to - from;
                    cursor = to;
                }
            }
            total += js::min2(cfg.plannable_per_day_min, ((hi - lo - taken) / MIN).floor());
        }
        day = zone::add_days(&day, 1.0);
        n += 1.0;
    }
    total
}

/// `timer.running ?? timer.endsAt !== null`.
fn timer_running(timer: &Value) -> bool {
    match timer.get("running") {
        Some(v) if !v.is_null() => truthy(v),
        _ => timer.get("endsAt").map_or(true, |v| !v.is_null()),
    }
}

/// `a === b` for two fields that may be missing.
fn same(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (Some(Value::Number(x)), Some(Value::Number(y))) => x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}

/// What is so right now that might be worth a line: deadlines at risk, a
/// break that waits, a commitment near, grades waiting.
pub fn standing_events(snap: &Value, cfg: &Config) -> Vec<Event> {
    let now = snap_now(snap);
    let mut events = Vec::new();
    for id in open_ids(snap) {
        let (Some(t), Some(d)) = (task_of(snap, id), snap["derived"]["tasks"].get(id)) else {
            continue;
        };
        let Some(due) = t["due"].as_f64() else {
            continue;
        };
        if truthy(&t["rrule"]) || due <= now || due - now > cfg.risk_horizon_days * DAY {
            continue;
        }
        let estimate = d["estimate"]["min"].as_f64().unwrap_or(f64::NAN);
        let actual = d["actualMin"].as_f64().unwrap_or(f64::NAN);
        let work_left_min = js::max2(0.0, js::round(estimate - actual));
        // A NaN (no estimate) is neither under the floor nor over what is free, as in the TypeScript.
        if work_left_min < cfg.risk_floor_min {
            continue;
        }
        let free = plannable_min(snap, id, due, cfg);
        if work_left_min > free {
            events.push(Event::AtRisk {
                task_id: id.to_string(),
                due,
                work_left_min,
                plannable_min: free,
            });
        }
    }

    let timer = &snap["heatState"]["timer"];
    let length = timer["lengthMs"].as_f64();
    let left = timer["leftMs"].as_f64().or(length);
    if timer["phase"] == "break" && !timer_running(timer) && left == length {
        let last = list(&snap["records"]["focusSession"])
            .iter()
            .fold(0.0, |m, s| {
                js::max2(m, s["endedAt"].as_f64().unwrap_or(f64::NAN))
            });
        let round = timer["round"]
            .as_f64()
            .map_or_else(|| "undefined".to_string(), js::num_to_string);
        events.push(Event::FocusEnded {
            key: format!("{round}:{}", js::num_to_string(last)),
            note: timer["note"].as_str().map(String::from),
        });
    }

    if let Some(next) = next_commitment(snap) {
        if next.start - now <= cfg.leave_lead_min * MIN {
            events.push(Event::LeaveFor {
                event_id: next.id,
                title: next.title,
                start: next.start,
            });
        }
    }

    let pending = pending_grades(snap);
    if let Some((first, rest)) = pending.split_first() {
        let posted = |g: &Value| g["postedAt"].as_f64().unwrap_or(0.0);
        let id = |g: &Value| g["id"].as_str().unwrap_or_default().to_string();
        let mut newest = *first;
        for b in rest {
            let later = posted(b) > posted(newest)
                || (same(b.get("postedAt"), newest.get("postedAt"))
                    && js::cmp(&id(b), &id(newest)).is_gt());
            if later {
                newest = b;
            }
        }
        events.push(Event::GradeWaiting {
            grade_id: id(newest),
            title: newest["title"].as_str().unwrap_or_default().to_string(),
            count: pending.len() as f64,
        });
    }
    events
}

/// A focus round is running: only what can't wait gets through.
fn focusing(snap: &Value) -> bool {
    let t = &snap["heatState"]["timer"];
    t["phase"] == "focus" && timer_running(t)
}

/// `${n}` for a number that may be null.
fn num_text(n: Option<f64>) -> String {
    n.map_or_else(|| "null".to_string(), js::num_to_string)
}

/// The line an event would show if nothing held it back, or None when it
/// changes nothing about what to do next. Only [`should_interrupt`] calls it.
fn line_for(event: &Event, ctx: &Ctx) -> Option<Interrupt> {
    let (snap, cfg) = (ctx.snap, ctx.cfg);
    let now = snap_now(snap);
    let tz = snap_zone(snap);
    let open: HashSet<&str> = open_ids(snap).into_iter().collect();
    let task = |id: &str| {
        if open.contains(id) {
            task_of(snap, id)
        } else {
            None
        }
    };
    let title = |t: &Value| t["title"].as_str().unwrap_or_default().to_string();
    let near = |due: Option<f64>, span: f64| due.is_some_and(|d| d - now <= span);
    match event {
        Event::DueChanged {
            task_id, from, to, ..
        } => {
            let t = task(task_id)?;
            // Done since, or moved again: the line would no longer be true.
            if t["due"].as_f64() != *to || to == from {
                return None;
            }
            let changes = ctx.now_task_id == Some(task_id.as_str())
                || near(*to, cfg.due_changed_within_days * DAY)
                || near(*from, cfg.urgent_within_hours * HOUR);
            if !changes {
                return None;
            }
            Some(Interrupt {
                id: format!("due:{task_id}:{}", num_text(*to)),
                source: "dueChanged".into(),
                line: match to {
                    None => format!("Due date removed: {} has no due date now.", title(t)),
                    Some(to) => format!(
                        "Due date moved: {} is now due {}.",
                        title(t),
                        due_phrase(*to, now, &tz)
                    ),
                },
                action: Some(Action::on_task("Show it", Do::Task, task_id)),
                priority: if near(*to, cfg.urgent_within_hours * HOUR) {
                    Priority::High
                } else {
                    Priority::Normal
                },
            })
        }
        Event::AtRisk {
            task_id,
            due,
            work_left_min,
            plannable_min,
        } => {
            let t = task(task_id)?;
            // Already the thing being done: nothing about what comes next changes.
            if t["due"].as_f64() != Some(*due) || ctx.now_task_id == Some(task_id.as_str()) {
                return None;
            }
            if work_left_min <= plannable_min {
                return None;
            }
            let free = if *plannable_min > 0.0 {
                format!("only {} is", format_minutes(*plannable_min))
            } else {
                "no time is".to_string()
            };
            Some(Interrupt {
                id: format!("risk:{task_id}:{}", js::num_to_string(*due)),
                source: "atRisk".into(),
                line: format!(
                    "At risk: {} needs {}, and {free} free before it is due.",
                    title(t),
                    format_minutes(*work_left_min)
                ),
                action: Some(Action::on_task("Do it now", Do::Current, task_id)),
                priority: Priority::High,
            })
        }
        Event::MailUrgent { task_id, .. } => {
            let t = task(task_id)?;
            if ctx.now_task_id == Some(task_id.as_str()) {
                return None;
            }
            let due = t["due"].as_f64();
            let urgent = list(&snap["records"]["mailThread"])
                .iter()
                .find(|m| m["taskId"].as_str() == Some(task_id.as_str()))
                .is_some_and(|m| m["priority"] == "urgent");
            if !near(due, cfg.urgent_within_hours * HOUR) && !urgent {
                return None;
            }
            Some(Interrupt {
                id: format!("mail:{task_id}"),
                source: "mailUrgent".into(),
                line: match due {
                    None => format!("From mail, and urgent: {}.", title(t)),
                    Some(due) => format!(
                        "From mail: {}, due {}.",
                        title(t),
                        due_phrase(due, now, &tz)
                    ),
                },
                action: Some(Action::on_task("Do it now", Do::Current, task_id)),
                priority: Priority::High,
            })
        }
        Event::FocusEnded { key, note } => Some(Interrupt {
            id: format!("focus:{key}"),
            source: "focusEnded".into(),
            line: note
                .clone()
                .unwrap_or_else(|| copy::focus::DONE_UNLOGGED.to_string()),
            action: Some(Action::new("Start break", Do::Break)),
            priority: Priority::High,
        }),
        Event::LeaveFor {
            event_id,
            title,
            start,
        } => {
            let lead = start - now;
            if lead <= 0.0 || lead > cfg.leave_lead_min * MIN {
                return None;
            }
            Some(Interrupt {
                id: format!("leave:{event_id}:{}", js::num_to_string(*start)),
                source: "leaveFor".into(),
                line: format!(
                    "Time to leave: {title} starts at {}.",
                    clock_at(*start, &tz)
                ),
                action: Some(Action::open("Open Calendar", "calendar")),
                priority: Priority::High,
            })
        }
        Event::GradeWaiting {
            grade_id,
            title,
            count,
        } => Some(Interrupt {
            id: format!("grade:{grade_id}:{}", js::num_to_string(*count)),
            source: "gradeWaiting".into(),
            line: if *count == 1.0 {
                format!("A grade is waiting for its score: {title}.")
            } else {
                format!(
                    "{} grades are waiting for their scores.",
                    js::num_to_string(*count)
                )
            },
            action: Some(Action::open("Open Grades", "grades")),
            priority: Priority::Low,
        }),
        Event::Custom {
            id,
            source,
            line,
            action,
            changes_next,
            priority,
            ..
        } => changes_next.then(|| Interrupt {
            id: id.clone(),
            source: source.clone(),
            line: line.clone(),
            action: action.clone(),
            priority: *priority,
        }),
    }
}

/// The one decision: may this event interrupt? The rule: interrupt only if
/// it changes what the person should do next. Answers the line to show, or
/// None. While a focus round is running, only a `high` line gets through.
/// An event that gets None is not lost: a standing one is asked about again
/// at the next snapshot, and a queued one waits its turn.
pub fn should_interrupt(event: &Event, ctx: &Ctx) -> Option<Interrupt> {
    let made = line_for(event, ctx)?;
    if focusing(ctx.snap) && made.priority != Priority::High {
        return None;
    }
    Some(made)
}

/// At most one interrupt, and how many wait behind it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picked {
    pub interrupt: Option<Interrupt>,
    pub queued: usize,
}

/// At most one interrupt, and how many wait behind it: the most pressing
/// first, then the oldest.
pub fn pick(events: &[Event], ctx: &Ctx, dismissed: &BTreeMap<String, f64>) -> Picked {
    let mut seen: HashSet<String> = HashSet::new();
    let mut live: Vec<Interrupt> = Vec::new();
    for e in events {
        let Some(i) = should_interrupt(e, ctx) else {
            continue;
        };
        if dismissed.contains_key(&i.id) || !seen.insert(i.id.clone()) {
            continue;
        }
        live.push(i);
    }
    // A stable sort: within a priority, events keep the order they came in.
    live.sort_by_key(|i| i.priority.rank());
    let queued = live.len().saturating_sub(1);
    Picked {
        interrupt: live.into_iter().next(),
        queued,
    }
}

/// Everything `snapshot.focus` holds: `{entropy, now, next, interrupt,
/// queued, config}`.
pub fn focus_state(snap: &Value, mem: &Memory, cfg: &Config) -> Value {
    let e = entropy(snap, cfg);
    let now = now_of(snap, &e, mem, cfg);
    let keep = snap_now(snap) - cfg.queue_keep_days * DAY;
    let mut events: Vec<Event> = mem
        .queue
        .iter()
        .filter(|e| e.at().map_or(true, |at| at >= keep))
        .cloned()
        .collect();
    events.extend(standing_events(snap, cfg));
    let picked = pick(
        &events,
        &Ctx {
            snap,
            now_task_id: now.task_id.as_deref(),
            cfg,
        },
        &mem.dismissed,
    );
    json!({
        "entropy": e,
        "now": now,
        "next": next_commitment(snap),
        "interrupt": picked.interrupt,
        "queued": picked.queued,
        "config": cfg,
    })
}

/// What someone other than the person changed (Claude, reading mail), turned
/// into events for the queue: a task's due date moved, or mail made a task.
pub fn events_from_changes(docs: &[DocChange], at: f64) -> Vec<Event> {
    let mut events = Vec::new();
    for c in docs {
        if c.kind != wi_heat_store::kind::TASK {
            continue;
        }
        let Some(after) = c.after.as_ref().filter(|a| !a.is_null()) else {
            continue;
        };
        let Some(id) = after["id"].as_str() else {
            continue;
        };
        if truthy(&after["done"]) {
            continue;
        }
        let Some(before) = c.before.as_ref().filter(|b| !b.is_null()) else {
            if after["source"] == "mail" {
                events.push(Event::MailUrgent {
                    task_id: id.to_string(),
                    at,
                });
            }
            continue;
        };
        let (from, to) = (before["due"].as_f64(), after["due"].as_f64());
        if from != to {
            events.push(Event::DueChanged {
                task_id: id.to_string(),
                from,
                to,
                at,
            });
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    const NY: &str = "America/New_York";

    /// An instant on a day in New York, `minutes` after midnight.
    fn at(day: &str, minutes: f64) -> f64 {
        zone::at_minute(day, minutes, &zone::zone(NY).unwrap())
    }

    /// A snapshot of Wednesday 2026-10-07, 9:00 AM, holding these open tasks.
    fn snapshot(tasks: Value, events: Value, blocks: Value) -> Value {
        let open: Vec<Value> = list(&tasks).iter().map(|t| t["id"].clone()).collect();
        let derived: serde_json::Map<String, Value> = list(&tasks)
            .iter()
            .map(|t| {
                (
                    t["id"].as_str().unwrap().to_string(),
                    json!({"heat": {"v": 0.2, "level": "Cool"}, "actualMin": 0, "estimate": {"min": t["estMin"]}}),
                )
            })
            .collect();
        json!({
            "now": at("2026-10-07", 9.0 * 60.0),
            "date": "2026-10-07",
            "zone": NY,
            "records": {"task": tasks, "timeBlock": blocks, "mailThread": [], "grade": [], "focusSession": []},
            "heatState": {"currentTaskId": null, "timer": {"phase": "idle", "round": 1, "endsAt": null, "running": false}, "planDrafts": []},
            "events": events,
            "derived": {"tasks": derived, "lists": {"allOpen": open}},
        })
    }

    #[test]
    fn the_config_is_the_typescripts_with_its_keys() {
        assert_eq!(
            serde_json::to_value(Config::default()).unwrap(),
            json!({
                "horizonDays": 7,
                "weights": {"overdue": 3, "unplanned": 2, "mail": 1.5, "grades": 0.5},
                "full": 12,
                "busyAt": 0.2,
                "highAt": 0.5,
                "riskHorizonDays": 7,
                "dayStartsMin": 420,
                "dayEndsMin": 1320,
                "plannablePerDayMin": 360,
                "riskFloorMin": 15,
                "dueChangedWithinDays": 7,
                "urgentWithinHours": 48,
                "leaveLeadMin": 15,
                "queueKeepDays": 7,
                "hintDays": 7,
                "hintLaunches": 20,
            })
        );
    }

    #[test]
    fn an_event_is_written_as_the_typescript_writes_it() {
        let e = Event::Custom {
            id: "notes:1".into(),
            source: "notes".into(),
            line: "A note is ready.".into(),
            action: Some(Action::open("Open Notes", "notes")),
            changes_next: true,
            priority: Priority::Normal,
            at: 5.0,
        };
        let written = json!({
            "kind": "custom", "id": "notes:1", "source": "notes", "line": "A note is ready.",
            "action": {"label": "Open Notes", "do": "view", "view": "notes"},
            "changesNext": true, "priority": "normal", "at": 5,
        });
        assert_eq!(serde_json::to_value(&e).unwrap(), written);
        assert_eq!(serde_json::from_value::<Event>(written).unwrap(), e);
        assert_eq!(
            serde_json::to_value(Event::DueChanged {
                task_id: "t".into(),
                from: None,
                to: Some(7.0),
                at: 9.0
            })
            .unwrap(),
            json!({"kind": "dueChanged", "taskId": "t", "from": null, "to": 7, "at": 9})
        );
    }

    #[test]
    fn plannable_minutes_are_the_day_less_the_calendar_and_other_tasks_blocks() {
        let cfg = Config::default();
        let five = at("2026-10-07", 17.0 * 60.0);
        // Nothing in the way: 9 AM to 5 PM is 8 hours, and no day gives more than 6.
        let empty = snapshot(json!([]), json!([]), json!([]));
        assert_eq!(plannable_min(&empty, "t1", five, &cfg), 360.0);

        // A two-hour class and another task's hour come out; the task's own
        // block, an all-day event and a block on another day don't.
        let busy = snapshot(
            json!([]),
            json!([
                {"id": "e1", "title": "JPN 201", "start": at("2026-10-07", 600.0), "end": at("2026-10-07", 720.0), "allDay": false},
                {"id": "e2", "title": "Fall break", "start": at("2026-10-07", 0.0), "end": at("2026-10-08", 0.0), "allDay": true},
            ]),
            json!([
                {"id": "b1", "taskId": "t2", "date": "2026-10-07", "start": 780, "minutes": 60},
                {"id": "b2", "taskId": "t1", "date": "2026-10-07", "start": 900, "minutes": 60},
                {"id": "b3", "taskId": "t2", "date": "2026-10-08", "start": 780, "minutes": 60},
            ]),
        );
        assert_eq!(plannable_min(&busy, "t1", five, &cfg), 300.0);

        // Due tomorrow at 8 AM: today's six hours, and tomorrow's one from 7 AM.
        let tomorrow = at("2026-10-08", 8.0 * 60.0);
        assert_eq!(plannable_min(&empty, "t1", tomorrow, &cfg), 420.0);
        // Due before the day starts, or already past: nothing.
        assert_eq!(
            plannable_min(&empty, "t1", at("2026-10-07", 8.0 * 60.0), &cfg),
            0.0
        );
        // Overlapping events are counted once.
        let overlap = snapshot(
            json!([]),
            json!([
                {"id": "e1", "title": "A", "start": at("2026-10-07", 600.0), "end": at("2026-10-07", 720.0), "allDay": false},
                {"id": "e2", "title": "B", "start": at("2026-10-07", 660.0), "end": at("2026-10-07", 780.0), "allDay": false},
            ]),
            json!([]),
        );
        assert_eq!(plannable_min(&overlap, "t1", five, &cfg), 300.0);
    }

    #[test]
    fn a_deadline_at_risk_interrupts_unless_it_is_already_the_now_task() {
        let cfg = Config::default();
        let due = at("2026-10-07", 12.0 * 60.0);
        let snap = snapshot(
            json!([{"id": "t1", "title": "Essay draft", "due": due, "estMin": 600, "done": false}]),
            json!([]),
            json!([]),
        );
        // Ten hours of work, three hours until noon.
        let standing = standing_events(&snap, &cfg);
        assert_eq!(
            standing,
            [Event::AtRisk {
                task_id: "t1".into(),
                due,
                work_left_min: 600.0,
                plannable_min: 180.0
            }]
        );
        let ctx = |now_task_id| Ctx {
            snap: &snap,
            now_task_id,
            cfg: &cfg,
        };
        let shown = should_interrupt(&standing[0], &ctx(None)).unwrap();
        assert_eq!(
            serde_json::to_value(&shown).unwrap(),
            json!({
                "id": format!("risk:t1:{}", js::num_to_string(due)),
                "source": "atRisk",
                "line": "At risk: Essay draft needs 10h, and only 3h is free before it is due.",
                "action": {"label": "Do it now", "do": "current", "taskId": "t1"},
                "priority": "high",
            })
        );
        // Already the thing being done: nothing about what comes next changes.
        assert_eq!(should_interrupt(&standing[0], &ctx(Some("t1"))), None);
        // Another task being the Now task doesn't hold it back.
        assert!(should_interrupt(&standing[0], &ctx(Some("t2"))).is_some());
        // No time at all reads differently; work that fits is no risk.
        let none = Event::AtRisk {
            task_id: "t1".into(),
            due,
            work_left_min: 45.0,
            plannable_min: 0.0,
        };
        assert_eq!(
            should_interrupt(&none, &ctx(None)).unwrap().line,
            "At risk: Essay draft needs 45m, and no time is free before it is due."
        );
        let fits = Event::AtRisk {
            task_id: "t1".into(),
            due,
            work_left_min: 45.0,
            plannable_min: 45.0,
        };
        assert_eq!(should_interrupt(&fits, &ctx(None)), None);
        // The due date moved since: the line would no longer be true.
        let stale = Event::AtRisk {
            task_id: "t1".into(),
            due: due + 1.0,
            work_left_min: 600.0,
            plannable_min: 180.0,
        };
        assert_eq!(should_interrupt(&stale, &ctx(None)), None);
    }

    #[test]
    fn only_someone_elses_change_to_an_open_tasks_due_date_is_an_event() {
        let doc = |before: Option<Value>, after: Option<Value>| DocChange {
            kind: "task".into(),
            key: "t1".into(),
            before,
            after,
        };
        let task = |due: Value, done: bool, source: &str| json!({"id": "t1", "due": due, "done": done, "source": source});
        let docs = [
            // Moved.
            doc(
                Some(task(json!(10), false, "you")),
                Some(task(json!(20), false, "you")),
            ),
            // Not moved, done, and deleted: nothing.
            doc(
                Some(task(json!(10), false, "you")),
                Some(task(json!(10), false, "you")),
            ),
            doc(
                Some(task(json!(10), false, "you")),
                Some(task(json!(20), true, "you")),
            ),
            doc(Some(task(json!(10), false, "you")), None),
            // New: only mail's own.
            doc(None, Some(task(json!(10), false, "claude"))),
            doc(None, Some(task(Value::Null, false, "mail"))),
            // Another kind.
            DocChange {
                kind: "grade".into(),
                key: "g1".into(),
                before: None,
                after: Some(json!({"id": "g1", "due": 5})),
            },
        ];
        assert_eq!(
            events_from_changes(&docs, 99.0),
            [
                Event::DueChanged {
                    task_id: "t1".into(),
                    from: Some(10.0),
                    to: Some(20.0),
                    at: 99.0
                },
                Event::MailUrgent {
                    task_id: "t1".into(),
                    at: 99.0
                },
            ]
        );
    }
}
