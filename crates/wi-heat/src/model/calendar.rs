//! Calendar (`docs/SPEC.md` 3.1 and 3.7). The month grid stays as Heat has it:
//! from Sunday, 3 pills a cell with a heat border, "N more", today circled.
//! Week and Day views sit on the PKM's grid (44 px an hour, the whole day):
//! Google events grey behind blocks, and each deadline a small heat-coloured
//! flag at its time. Brightspace items are tasks, so they never draw as
//! events. A port of `calendar.ts` (the title of each view is display and
//! stays in TypeScript).

use super::heat::{by_heat, by_heat_order, heat_of, order_of, HeatLevel};
use super::plan::HOUR_PX;
use super::records::{ser, CalendarEvent, Habit, Id, Milestone, Task, TaskOccurrence, TimeBlock};
use super::recurrence::{open_tasks, recurs, task_occurrences, with_effective_due};
use super::zone::{
    add_days, at_minute, day_key, key_of, key_parts, minute_of_day, start_of_day, weekday_of,
    DayKey,
};
use super::{copy, format, js};
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;

pub const GRID_HOUR_PX: f64 = HOUR_PX;
const MIN_ITEM_PX: f64 = 16.0;
const PILLS_PER_CELL: usize = 3;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CalendarData {
    pub tasks: Vec<Task>,
    pub occurrences: Vec<TaskOccurrence>,
    pub blocks: Vec<TimeBlock>,
    pub events: Vec<CalendarEvent>,
    pub milestones: Vec<Milestone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub habits: Option<Vec<Habit>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pill {
    pub task_id: Id,
    pub title: String,
    /// None for a past occurrence nobody ticked: it is left behind, not overdue.
    pub level: Option<HeatLevel>,
    pub colour: Option<String>,
}

/// A heat level's colour, from the token file (`heat.*`); a done task has none.
fn colour_of(level: Option<HeatLevel>) -> Option<String> {
    let c = match level? {
        HeatLevel::Overdue => wwav_tokens::heat::OVERDUE,
        HeatLevel::Hot => wwav_tokens::heat::HOT,
        HeatLevel::Warm => wwav_tokens::heat::WARM,
        HeatLevel::Cool => wwav_tokens::heat::COOL,
        HeatLevel::Done => return None,
    };
    Some(format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b))
}

struct DueItem {
    task: Task,
    date: DayKey,
    level: Option<HeatLevel>,
}

// Every task due in [from, to], one entry per occurrence for a recurring task.
fn due_items(data: &CalendarData, from: &str, to: &str, now: f64, tz: &TimeZone) -> Vec<DueItem> {
    let today = day_key(now, tz);
    let ticked: HashSet<(&str, &str)> = data
        .occurrences
        .iter()
        .map(|o| (o.task_id.as_str(), o.date.as_str()))
        .collect();
    let mut out = vec![];
    for t in &data.tasks {
        if !recurs(t) {
            let Some(due) = t.due else { continue };
            let date = day_key(due, tz);
            if js::cmp(&date, from) == Ordering::Less || js::cmp(&date, to) == Ordering::Greater {
                continue;
            }
            out.push(DueItem {
                task: t.clone(),
                date,
                level: Some(heat_of(t, now).level),
            });
            continue;
        }
        for o in task_occurrences(t, tz, from, to) {
            let done = ticked.contains(&(t.id.as_str(), o.date.as_str()));
            let level = if done {
                Some(HeatLevel::Done)
            } else if js::cmp(&o.date, &today) == Ordering::Less {
                None
            } else {
                Some(
                    heat_of(
                        &Task {
                            due: Some(o.at),
                            ..t.clone()
                        },
                        now,
                    )
                    .level,
                )
            };
            out.push(DueItem {
                task: Task {
                    due: Some(o.at),
                    done: done || level.is_none(),
                    ..t.clone()
                },
                date: o.date,
                level,
            });
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthCell {
    pub date: DayKey,
    pub in_month: bool,
    pub is_today: bool,
    pub pills: Vec<Pill>,
    pub more: Option<String>,
}

/// The month of `anchor`: 42 days from the Sunday before the 1st.
pub fn month_grid(anchor: &str, data: &CalendarData, now: f64, tz: &TimeZone) -> Vec<MonthCell> {
    let (year, month, _) = key_parts(anchor);
    let first = key_of(year, month, 1.0);
    let start = add_days(&first, -weekday_of(&first));
    let items = due_items(data, &start, &add_days(&start, 41.0), now, tz);
    let today = day_key(now, tz);
    (0..42)
        .map(|i| {
            let date = add_days(&start, f64::from(i));
            let here: Vec<&DueItem> = items.iter().filter(|x| x.date == date).collect();
            let tasks: Vec<Task> = here.iter().map(|x| x.task.clone()).collect();
            let sorted = by_heat_order(&tasks, now);
            let pills: Vec<Pill> = sorted
                .iter()
                .take(PILLS_PER_CELL)
                .map(|&i| Pill {
                    task_id: tasks[i].id.clone(),
                    title: tasks[i].title.clone(),
                    level: here[i].level,
                    colour: colour_of(here[i].level),
                })
                .collect();
            let rest = sorted.len() - pills.len();
            MonthCell {
                in_month: key_parts(&date).1 == month,
                is_today: date == today,
                more: (rest > 0).then(|| copy::calendar::more(rest as f64)),
                pills,
                date,
            }
        })
        .collect()
}

/// The week of `anchor`, Sunday first.
pub fn week_days(anchor: &str) -> Vec<DayKey> {
    let sunday = add_days(anchor, -weekday_of(anchor));
    (0..7).map(|i| add_days(&sunday, f64::from(i))).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaidOutKind {
    Event,
    Block,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LaidOut {
    pub kind: LaidOutKind,
    pub id: Id,
    pub title: String,
    #[serde(serialize_with = "ser::num")]
    pub top: f64,
    #[serde(serialize_with = "ser::num")]
    pub height: f64,
    /// A block's left border: its task's heat colour. Events are grey.
    pub colour: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DueFlag {
    pub task_id: Id,
    pub title: String,
    #[serde(serialize_with = "ser::num")]
    pub top: f64,
    pub label: String,
    pub colour: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllDayDue {
    pub task_id: Id,
    pub title: String,
    pub colour: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AllDay {
    pub due: Vec<AllDayDue>,
    pub beads: Vec<Milestone>,
    pub events: Vec<CalendarEvent>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayLayout {
    pub items: Vec<LaidOut>,
    pub due_flags: Vec<DueFlag>,
    pub all_day: AllDay,
}

fn to_y(min: f64) -> f64 {
    (min / 60.0) * GRID_HOUR_PX
}

/// One day column: events, then blocks drawn over them, then deadline flags; and the all-day strip.
pub fn day_layout(day: &str, data: &CalendarData, now: f64, tz: &TimeZone) -> DayLayout {
    let day_start = start_of_day(day, tz);
    let day_end = start_of_day(&add_days(day, 1.0), tz);
    let overlapping: Vec<&CalendarEvent> = data
        .events
        .iter()
        .filter(|e| e.end > day_start && e.start < day_end)
        .collect();
    let timed: Vec<&CalendarEvent> = overlapping.iter().copied().filter(|e| !e.all_day).collect();
    let events = js::sort_by(timed, |a, b| order_of(a.start - b.start))
        .into_iter()
        .map(|e| {
            let from = if e.start <= day_start {
                0.0
            } else {
                minute_of_day(e.start, tz)
            };
            let to = if e.end >= day_end {
                1440.0
            } else {
                minute_of_day(e.end, tz)
            };
            LaidOut {
                kind: LaidOutKind::Event,
                id: e.id.clone(),
                title: e.title.clone(),
                top: to_y(from),
                height: js::max2(MIN_ITEM_PX, to_y(to - from)),
                colour: None,
            }
        });
    let todays: Vec<&TimeBlock> = data.blocks.iter().filter(|b| b.date == day).collect();
    let blocks = js::sort_by(todays, |a, b| order_of(a.start - b.start))
        .into_iter()
        .filter_map(|b| {
            let task = b
                .task_id
                .as_ref()
                .and_then(|id| data.tasks.iter().find(|x| x.id == *id));
            let habit_title = || {
                let habits = data.habits.as_ref()?;
                let id = b.habit_id.as_ref()?;
                habits.iter().find(|h| h.id == *id).map(|h| h.title.clone())
            };
            let title = task.map(|t| t.title.clone()).or_else(habit_title)?;
            let colour = task.and_then(|t| {
                colour_of(Some(
                    heat_of(&with_effective_due(t, &data.occurrences, now, tz), now).level,
                ))
            });
            Some(LaidOut {
                kind: LaidOutKind::Block,
                id: b.id.clone(),
                title,
                top: to_y(b.start),
                height: js::max2(MIN_ITEM_PX, to_y(b.minutes)),
                colour,
            })
        });
    let items: Vec<LaidOut> = events.chain(blocks).collect();
    let due: Vec<DueItem> = due_items(data, day, day, now, tz)
        .into_iter()
        .filter(|x| x.level.is_some() && x.level != Some(HeatLevel::Done))
        .collect();
    let due_at = |x: &DueItem| minute_of_day(x.task.due.unwrap_or(f64::NAN), tz);
    DayLayout {
        items,
        due_flags: due
            .iter()
            .map(|x| DueFlag {
                task_id: x.task.id.clone(),
                title: x.task.title.clone(),
                top: to_y(due_at(x)),
                label: copy::calendar::due_flag(&format::clock(due_at(x))),
                colour: colour_of(x.level),
            })
            .collect(),
        all_day: AllDay {
            due: due
                .iter()
                .map(|x| AllDayDue {
                    task_id: x.task.id.clone(),
                    title: x.task.title.clone(),
                    colour: colour_of(x.level),
                })
                .collect(),
            beads: data
                .milestones
                .iter()
                .filter(|m| m.date == day)
                .cloned()
                .collect(),
            events: overlapping
                .iter()
                .filter(|e| e.all_day)
                .map(|e| (*e).clone())
                .collect(),
        },
    }
}

/// The tray left of the week: this week's open tasks with no block in it, in heat order.
pub fn unscheduled_tray(anchor: &str, data: &CalendarData, now: f64, tz: &TimeZone) -> Vec<Task> {
    let days = week_days(anchor);
    let (first, last) = (&days[0], &days[6]);
    let in_week =
        |d: &str| js::cmp(d, first) != Ordering::Less && js::cmp(d, last) != Ordering::Greater;
    let blocked: HashSet<&str> = data
        .blocks
        .iter()
        .filter(|b| in_week(&b.date))
        .filter_map(|b| b.task_id.as_deref())
        .collect();
    let candidates: Vec<Task> = open_tasks(&data.tasks, &data.occurrences, now, tz)
        .into_iter()
        .filter(|t| {
            !blocked.contains(t.id.as_str())
                && (t.due.is_some_and(|due| in_week(&day_key(due, tz)))
                    || t.scheduled_date.as_deref().is_some_and(in_week))
        })
        .collect();
    by_heat(&candidates, now).into_iter().cloned().collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CalendarMode {
    Month,
    Week,
    Day,
}

/// ← and →: a month (to its 1st), a week or a day.
pub fn page(mode: CalendarMode, anchor: &str, dir: f64) -> DayKey {
    match mode {
        CalendarMode::Day => add_days(anchor, dir),
        CalendarMode::Week => add_days(anchor, 7.0 * dir),
        CalendarMode::Month => {
            let (year, month, _) = key_parts(anchor);
            let m = month - 1.0 + dir;
            key_of(
                year + (m / 12.0).floor(),
                (((m % 12.0) + 12.0) % 12.0) + 1.0,
                1.0,
            )
        }
    }
}

/// Calendar's "+" adds a task due 11:59 PM on the selected day.
pub fn due_at_end_of_day(day: &str, tz: &TimeZone) -> f64 {
    at_minute(day, 23.0 * 60.0 + 59.0, tz)
}
