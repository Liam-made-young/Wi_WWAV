//! The Now strip's task half (`docs/SPEC.md` 2.2), which is Heat's LCD moved
//! into the title bar (3.1, 3.3). It shows the current task, or else the
//! hottest open task: "Hot: Grammar quiz 4" over "Tomorrow 11:59 PM, JPN 102.
//! This week: 3h 20m across 5 tasks", or, while focus runs, "Today 4:00 PM,
//! JPN 201 · focus 18:40 left". The meter is that task's heat. A port of
//! `lcd.ts`.

use super::copy;
use super::estimate::{estimate_context, weekly_load, weekly_load_line};
use super::heat::{by_heat, due_phrase, heat_of, tube_fill};
use super::records::{ser, Course, FocusSession, Id, Milestone, Task, TaskOccurrence};
use super::recurrence::open_tasks;
use super::spaces::group_name;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LcdData {
    pub tasks: Vec<Task>,
    pub occurrences: Vec<TaskOccurrence>,
    pub sessions: Vec<FocusSession>,
    pub courses: Vec<Course>,
    pub milestones: Vec<Milestone>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskHalf {
    pub task_id: Option<Id>,
    pub line1: String,
    pub line2: String,
    #[serde(serialize_with = "ser::num")]
    pub meter: f64,
}

/// `focus` is [`super::focus::focus_strip`]'s text ("focus 18:40 left"), or
/// None when no round is under way.
pub fn task_half(
    data: &LcdData,
    now: f64,
    tz: &TimeZone,
    current_task_id: Option<&str>,
    focus: Option<&str>,
) -> TaskHalf {
    let open = open_tasks(&data.tasks, &data.occurrences, now, tz);
    let shown = current_task_id
        .and_then(|id| open.iter().find(|t| t.id == id))
        .or_else(|| by_heat(&open, now).into_iter().next());
    let Some(shown) = shown else {
        return TaskHalf {
            task_id: None,
            line1: copy::strip::ALL_CLEAR.to_string(),
            line2: copy::strip::NOTHING_OPEN.to_string(),
            meter: 0.0,
        };
    };
    let heat = heat_of(shown, now);
    let about: Vec<String> = [
        shown.due.map(|due| due_phrase(due, now, tz)),
        group_name(shown, &data.courses, &data.milestones),
    ]
    .into_iter()
    .flatten()
    .collect();
    let about = about.join(", ");
    let tail = match focus {
        Some(f) => f.to_string(),
        None => weekly_load_line(&weekly_load(
            &open,
            &estimate_context(&data.tasks, &data.sessions),
            now,
        )),
    };
    let line2 = if about.is_empty() {
        tail
    } else {
        let joiner = if focus.is_some_and(|f| !f.is_empty()) {
            " · "
        } else {
            ". "
        };
        format!("{about}{joiner}{tail}")
    };
    TaskHalf {
        task_id: Some(shown.id.clone()),
        line1: copy::strip::task(heat.level.as_str(), &shown.title),
        line2,
        meter: tube_fill(&heat),
    }
}
