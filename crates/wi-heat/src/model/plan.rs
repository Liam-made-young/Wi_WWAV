//! Today: the time column, Plan my day, and the plan list (`docs/SPEC.md` 3.5).
//!
//! The column reuses the PKM calendar's scale (`portfolio/src/pkm/calendarUtils.js`):
//! 44 px an hour, a 15-minute snap and 30-minute default blocks, here from 7 AM
//! to midnight. Plan my day is a written rule, not Claude: unplanned open tasks
//! in heat order, each a block of its estimate rounded up to 15 minutes and
//! capped at 90, in the first gap that fits between now and "Day ends at".
//! A port of `plan.ts` (the header and the "Block ends" line are display and
//! stay in TypeScript).

use super::estimate::{estimate_context, estimate_min, format_minutes, EstimateContext};
use super::heat::{by_heat, due_phrase, heat_of, HeatLevel};
use super::records::{
    ser, CalendarEvent, FocusSession, Habit, Id, Origin, Task, TaskOccurrence, TimeBlock,
};
use super::recurrence::{open_tasks, task_occurrences};
use super::spaces::in_space;
use super::zone::{add_days, day_key, minute_of_day, start_of_day, DayKey};
use super::{copy, format, js};
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const HOUR_PX: f64 = 44.0;
pub const SNAP_MIN: f64 = 15.0;
pub const DEFAULT_BLOCK_MIN: f64 = 30.0;
const COLUMN_START: f64 = 7.0 * 60.0;
const COLUMN_END: f64 = 24.0 * 60.0;
pub const COLUMN_HEIGHT: f64 = ((COLUMN_END - COLUMN_START) / 60.0) * HOUR_PX;
const PLAN_CAP_MIN: f64 = 90.0;
pub const DAY_ENDS_AT: f64 = 23.0 * 60.0;
const HOT_SUGGESTIONS: usize = 5;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlanData {
    pub tasks: Vec<Task>,
    pub occurrences: Vec<TaskOccurrence>,
    pub blocks: Vec<TimeBlock>,
    pub events: Vec<CalendarEvent>,
    pub sessions: Vec<FocusSession>,
    pub habits: Vec<Habit>,
}

/// What a block is for: a task or a habit with a length.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockTarget {
    Task {
        #[serde(rename = "taskId")]
        task_id: Id,
    },
    Habit {
        #[serde(rename = "habitId")]
        habit_id: Id,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub task_id: Id,
    pub date: DayKey,
    #[serde(serialize_with = "ser::num")]
    pub start: f64,
    #[serde(serialize_with = "ser::num")]
    pub minutes: f64,
    /// Estimate past the 90-minute cap, still to plan.
    #[serde(serialize_with = "ser::num")]
    pub left_min: f64,
    pub reason: String,
    pub left_line: Option<String>,
}

/// What Plan my day is asked: when the day ends, and which space to plan.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlanOptions {
    pub day_ends_at: Option<f64>,
    pub space_id: Option<Id>,
}

fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    js::min2(hi, js::max2(lo, x))
}

pub fn minutes_to_y(min: f64) -> f64 {
    ((min - COLUMN_START) / 60.0) * HOUR_PX
}
pub fn y_to_minutes(y: f64) -> f64 {
    COLUMN_START + (y / HOUR_PX) * 60.0
}
/// The nearest 15-minute mark, as the PKM snaps a drag.
pub fn snap(min: f64) -> f64 {
    js::round(min / SNAP_MIN) * SNAP_MIN
}
fn round_up(min: f64) -> f64 {
    (min / SNAP_MIN).ceil() * SNAP_MIN
}

/// Scroll the column so now sits a third of the way down.
pub fn initial_scroll_top(now_min: f64, viewport_px: f64) -> f64 {
    clamp(
        minutes_to_y(now_min) - viewport_px / 3.0,
        0.0,
        js::max2(0.0, COLUMN_HEIGHT - viewport_px),
    )
}

/// Where the 1 px red line marks the current minute; None outside the column.
pub fn now_line_y(now_min: f64) -> Option<f64> {
    // Not `!(START..END).contains(..)`, which would turn a NaN into None as well.
    #[allow(clippy::manual_range_contains)]
    if now_min < COLUMN_START || now_min >= COLUMN_END {
        None
    } else {
        Some(minutes_to_y(now_min))
    }
}

/// A block as long as an estimate, rounded up to 15 minutes.
pub fn block_length(estimate: f64) -> f64 {
    js::max2(SNAP_MIN, round_up(estimate))
}

type Span = (f64, f64);

// Today's busy minutes: every block and every timed event, whatever space it is in.
fn busy_spans(
    blocks: &[TimeBlock],
    events: &[CalendarEvent],
    today: &str,
    tz: &TimeZone,
) -> Vec<Span> {
    let day_start = start_of_day(today, tz);
    let day_end = start_of_day(&add_days(today, 1.0), tz);
    let mut spans: Vec<Span> = blocks
        .iter()
        .filter(|b| b.date == today)
        .map(|b| (b.start, b.start + b.minutes))
        .collect();
    for e in events {
        if e.all_day || e.end <= day_start || e.start >= day_end {
            continue;
        }
        spans.push((
            if e.start <= day_start {
                0.0
            } else {
                minute_of_day(e.start, tz)
            },
            if e.end >= day_end {
                1440.0
            } else {
                minute_of_day(e.end, tz)
            },
        ));
    }
    spans
}

// The first start on the 15-minute grid, from `from` on, where `length` fits before `until`.
fn first_gap(spans: &[Span], from: f64, until: f64, length: f64) -> Option<f64> {
    let mut s = round_up(from);
    while s + length <= until {
        if spans.iter().all(|(a, b)| s + length <= *a || s >= *b) {
            return Some(s);
        }
        s += SNAP_MIN;
    }
    None
}

fn target_length(target: &BlockTarget, data: &PlanData, ctx: &EstimateContext) -> Option<f64> {
    match target {
        BlockTarget::Task { task_id } => {
            let t = data.tasks.iter().find(|x| x.id == *task_id)?;
            Some(block_length(estimate_min(t, ctx)))
        }
        BlockTarget::Habit { habit_id } => {
            let h = data.habits.iter().find(|x| x.id == *habit_id)?;
            Some(block_length(h.minutes.unwrap_or(DEFAULT_BLOCK_MIN)))
        }
    }
}

fn new_block(
    target: &BlockTarget,
    date: &str,
    start: f64,
    minutes: f64,
    new_id: &mut dyn FnMut() -> Id,
) -> TimeBlock {
    let (task_id, habit_id) = match target {
        BlockTarget::Task { task_id } => (Some(task_id.clone()), None),
        BlockTarget::Habit { habit_id } => (None, Some(habit_id.clone())),
    };
    TimeBlock {
        id: new_id(),
        task_id,
        habit_id,
        date: date.to_string(),
        start,
        minutes,
        origin: Origin::You,
    }
}

// Where a search for a gap starts: now, but never before the column's 7 AM,
// so a block made after midnight still lands where the column can show it.
fn search_from(now: f64, tz: &TimeZone) -> f64 {
    js::max2(COLUMN_START, minute_of_day(now, tz))
}

/// P: the selected task or habit into the next free gap after now (from 7 AM).
pub fn plan_next(
    target: &BlockTarget,
    data: &PlanData,
    now: f64,
    tz: &TimeZone,
    new_id: &mut dyn FnMut() -> Id,
) -> Option<TimeBlock> {
    let length = target_length(target, data, &estimate_context(&data.tasks, &data.sessions))?;
    let today = day_key(now, tz);
    let start = first_gap(
        &busy_spans(&data.blocks, &data.events, &today, tz),
        search_from(now, tz),
        COLUMN_END,
        length,
    )?;
    Some(new_block(target, &today, start, length, new_id))
}

/// A task or habit dropped on the column at `y` becomes a block as long as its estimate.
pub fn drag_onto_column(
    target: &BlockTarget,
    y: f64,
    date: &str,
    data: &PlanData,
    new_id: &mut dyn FnMut() -> Id,
) -> Option<TimeBlock> {
    let length = target_length(target, data, &estimate_context(&data.tasks, &data.sessions))?;
    let start = clamp(snap(y_to_minutes(y)), COLUMN_START, COLUMN_END - length);
    Some(new_block(target, date, start, length, new_id))
}

/// Dragging a block's bottom edge. It changes the block, never the task's estimate.
pub fn resize_block(block: &TimeBlock, bottom_y: f64) -> TimeBlock {
    TimeBlock {
        minutes: clamp(
            snap(y_to_minutes(bottom_y) - block.start),
            SNAP_MIN,
            COLUMN_END - block.start,
        ),
        ..block.clone()
    }
}

/// Dragging a block's body to a new top.
pub fn move_block(block: &TimeBlock, top_y: f64) -> TimeBlock {
    TimeBlock {
        start: clamp(
            snap(y_to_minutes(top_y)),
            COLUMN_START,
            COLUMN_END - block.minutes,
        ),
        ..block.clone()
    }
}

/// A draft's reason: "Due tomorrow 11:59 PM, Hot."
pub fn plan_reason(task: &Task, now: f64, tz: &TimeZone) -> String {
    let Some(due) = task.due else {
        return copy::draft::NO_DUE.to_string();
    };
    let phrase = due_phrase(due, now, tz);
    let level = heat_of(task, now).level;
    if level == HeatLevel::Overdue {
        copy::draft::overdue(&phrase)
    } else {
        copy::draft::reason(&phrase, level.as_str())
    }
}

/// Plan my day: dashed drafts, nothing saved until they are accepted. Before 7 AM it plans from 7.
pub fn plan_my_day(data: &PlanData, now: f64, tz: &TimeZone, options: &PlanOptions) -> Vec<Draft> {
    let today = day_key(now, tz);
    let ctx = estimate_context(&data.tasks, &data.sessions);
    let planned_today: HashSet<&str> = data
        .blocks
        .iter()
        .filter(|b| b.date == today)
        .filter_map(|b| b.task_id.as_deref())
        .collect();
    // A parent's estimate is its open children's; plan the children instead.
    let parents: HashSet<&str> = data
        .tasks
        .iter()
        .filter(|t| !t.done)
        .filter_map(|t| t.parent_task_id.as_deref().filter(|p| !p.is_empty()))
        .collect();
    let in_scope = in_space(options.space_id.as_deref());
    let candidates: Vec<Task> = open_tasks(&data.tasks, &data.occurrences, now, tz)
        .into_iter()
        .filter(|t| {
            in_scope(t)
                && !planned_today.contains(t.id.as_str())
                && !parents.contains(t.id.as_str())
        })
        .collect();
    let mut spans = busy_spans(&data.blocks, &data.events, &today, tz);
    let from = search_from(now, tz);
    let mut drafts = vec![];
    for t in by_heat(&candidates, now) {
        let full = block_length(estimate_min(t, &ctx));
        let minutes = js::min2(PLAN_CAP_MIN, full);
        let Some(start) = first_gap(
            &spans,
            from,
            options.day_ends_at.unwrap_or(DAY_ENDS_AT),
            minutes,
        ) else {
            continue;
        };
        spans.push((start, start + minutes));
        let left_min = full - minutes;
        drafts.push(Draft {
            task_id: t.id.clone(),
            date: today.clone(),
            start,
            minutes,
            left_min,
            reason: plan_reason(t, now, tz),
            left_line: (left_min > 0.0)
                .then(|| copy::draft::left_to_plan(&format_minutes(left_min))),
        });
    }
    drafts
}

/// A click on a draft accepts that one.
pub fn accept_draft(d: &Draft, new_id: &mut dyn FnMut() -> Id) -> TimeBlock {
    TimeBlock {
        id: new_id(),
        task_id: Some(d.task_id.clone()),
        habit_id: None,
        date: d.date.clone(),
        start: d.start,
        minutes: d.minutes,
        origin: Origin::Plan,
    }
}

/// What a key does to the drafts: the drafts left, and the blocks made.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DraftKeyResult {
    pub drafts: Vec<Draft>,
    pub blocks: Vec<TimeBlock>,
}

/// Return accepts all the drafts and Esc clears them; any other key, or no drafts, does nothing here.
pub fn draft_key(
    drafts: &[Draft],
    key: &str,
    new_id: &mut dyn FnMut() -> Id,
) -> Option<DraftKeyResult> {
    if drafts.is_empty() {
        return None;
    }
    match key {
        "Enter" => Some(DraftKeyResult {
            drafts: vec![],
            blocks: drafts.iter().map(|d| accept_draft(d, new_id)).collect(),
        }),
        "Escape" => Some(DraftKeyResult {
            drafts: vec![],
            blocks: vec![],
        }),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlannedRow {
    pub block: TimeBlock,
    pub title: String,
    pub time: String,
    pub length: String,
    /// The block now is in: it wears the selection ring.
    pub current: bool,
    /// A block whose time is over: it dims.
    pub finished: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecurringKind {
    Task,
    Habit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecurringRow {
    pub kind: RecurringKind,
    pub id: Id,
    pub title: String,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlanSection {
    Planned {
        title: String,
        items: Vec<PlannedRow>,
    },
    DueToday {
        title: String,
        items: Vec<Task>,
    },
    Recurring {
        title: String,
        items: Vec<RecurringRow>,
    },
    Hot {
        title: String,
        items: Vec<Task>,
    },
}

impl PlanSection {
    pub fn len(&self) -> usize {
        match self {
            PlanSection::Planned { items, .. } => items.len(),
            PlanSection::DueToday { items, .. } | PlanSection::Hot { items, .. } => items.len(),
            PlanSection::Recurring { items, .. } => items.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// Today's blocks in the space filter: a task's only if its task is in the
// space, a habit's always, since habits belong to the person (3.4).
fn blocks_today<'a>(data: &'a PlanData, today: &str, space_id: Option<&str>) -> Vec<&'a TimeBlock> {
    let filter = in_space(space_id);
    let tasks: HashSet<&str> = data
        .tasks
        .iter()
        .filter(|t| filter(t))
        .map(|t| t.id.as_str())
        .collect();
    data.blocks
        .iter()
        .filter(|b| {
            b.date == today
                && (b.habit_id.is_some()
                    || b.task_id.as_deref().is_some_and(|id| tasks.contains(id)))
        })
        .collect()
}

fn is_due_on(t: &Task, day: &str, tz: &TimeZone) -> bool {
    t.due.is_some_and(|due| day_key(due, tz) == day)
}

fn in_scope_tasks(tasks: &[Task], space_id: Option<&str>) -> Vec<Task> {
    let filter = in_space(space_id);
    tasks.iter().filter(|t| filter(t)).cloned().collect()
}

/// The plan list's four sections, in order; an empty one is left out.
pub fn plan_sections(
    data: &PlanData,
    now: f64,
    tz: &TimeZone,
    space_id: Option<&str>,
) -> Vec<PlanSection> {
    let today = day_key(now, tz);
    let now_min = minute_of_day(now, tz);
    let blocks = js::sort_by(blocks_today(data, &today, space_id), |a, b| {
        super::heat::order_of(a.start - b.start)
    });
    let planned: HashSet<&str> = blocks.iter().filter_map(|b| b.task_id.as_deref()).collect();
    let scoped = in_scope_tasks(&data.tasks, space_id);
    let open = open_tasks(&scoped, &data.occurrences, now, tz);

    let recurring_tasks: Vec<&Task> = scoped
        .iter()
        .filter(|t| !t.done && !task_occurrences(t, tz, &today, &today).is_empty())
        .collect();
    let recurring_ids: HashSet<&str> = recurring_tasks.iter().map(|t| t.id.as_str()).collect();
    let ticked_today: HashSet<&str> = data
        .occurrences
        .iter()
        .filter(|o| o.date == today)
        .map(|o| o.task_id.as_str())
        .collect();
    let mut recurring: Vec<RecurringRow> = recurring_tasks
        .iter()
        .map(|t| RecurringRow {
            kind: RecurringKind::Task,
            id: t.id.clone(),
            title: t.title.clone(),
            done: ticked_today.contains(t.id.as_str()),
        })
        .collect();
    recurring.extend(
        data.habits
            .iter()
            .filter(|h| h.minutes.is_some())
            .map(|h| RecurringRow {
                kind: RecurringKind::Habit,
                id: h.id.clone(),
                title: h.title.clone(),
                done: h.log.get(&today).copied().unwrap_or(false),
            }),
    );

    let candidates: Vec<Task> = open
        .iter()
        .filter(|t| !planned.contains(t.id.as_str()) && !recurring_ids.contains(t.id.as_str()))
        .cloned()
        .collect();
    let unplanned: Vec<&Task> = by_heat(&candidates, now);
    let due_today: Vec<Task> = unplanned
        .iter()
        .filter(|t| is_due_on(t, &today, tz))
        .map(|t| (*t).clone())
        .collect();
    let hot: Vec<Task> = unplanned
        .iter()
        .filter(|t| {
            !is_due_on(t, &today, tz)
                && matches!(heat_of(**t, now).level, HeatLevel::Hot | HeatLevel::Overdue)
        })
        .take(HOT_SUGGESTIONS)
        .map(|t| (*t).clone())
        .collect();

    let title_of = |b: &TimeBlock| -> Option<String> {
        if b.habit_id.is_some() {
            data.habits
                .iter()
                .find(|h| Some(&h.id) == b.habit_id.as_ref())
                .map(|h| h.title.clone())
        } else {
            data.tasks
                .iter()
                .find(|t| Some(&t.id) == b.task_id.as_ref())
                .map(|t| t.title.clone())
        }
    };
    let rows: Vec<PlannedRow> = blocks
        .iter()
        .filter_map(|b| {
            let title = title_of(b)?;
            let end = b.start + b.minutes;
            Some(PlannedRow {
                block: (*b).clone(),
                title,
                time: format::clock(b.start),
                length: format_minutes(b.minutes),
                current: b.start <= now_min && now_min < end,
                finished: end <= now_min,
            })
        })
        .collect();

    vec![
        PlanSection::Planned {
            title: copy::today::PLANNED.to_string(),
            items: rows,
        },
        PlanSection::DueToday {
            title: copy::today::DUE_TODAY.to_string(),
            items: due_today,
        },
        PlanSection::Recurring {
            title: copy::today::RECURRING.to_string(),
            items: recurring,
        },
        PlanSection::Hot {
            title: copy::today::HOT.to_string(),
            items: hot,
        },
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect()
}

/// "4 blocks · 3h 10m planned · 2 due today"
pub fn plan_subtitle(data: &PlanData, now: f64, tz: &TimeZone, space_id: Option<&str>) -> String {
    let today = day_key(now, tz);
    let blocks = blocks_today(data, &today, space_id);
    let minutes = blocks.iter().fold(0.0, |sum, b| sum + b.minutes);
    let scoped = in_scope_tasks(&data.tasks, space_id);
    let due = open_tasks(&scoped, &data.occurrences, now, tz)
        .iter()
        .filter(|t| is_due_on(t, &today, tz))
        .count();
    copy::today::subtitle(blocks.len() as f64, &format_minutes(minutes), due as f64)
}
