//! The weekly review's step 2, "Last week" (`docs/SPEC.md` 3.14): facts only.
//! Tasks done and focus time per space, milestones reached, and estimate
//! accuracy. Claude's note restates these and may not add a number of its
//! own (3.12); its draft is built elsewhere from what this returns. A port of
//! `review.ts`.

use super::estimate::{actual_min, estimate_context, estimate_min, format_minutes, type_key};
use super::records::{ser, FocusSession, Id, Milestone, Space, Task, TaskOccurrence};
use super::zone::{add_days, day_key, start_of_day, DayKey};
use super::{copy, js};
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReviewData {
    pub spaces: Vec<Space>,
    pub tasks: Vec<Task>,
    pub occurrences: Vec<TaskOccurrence>,
    pub sessions: Vec<FocusSession>,
    pub milestones: Vec<Milestone>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceFacts {
    pub space_id: Id,
    pub name: String,
    #[serde(serialize_with = "ser::num")]
    pub tasks_done: f64,
    #[serde(serialize_with = "ser::num")]
    pub focus_min: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Accuracy {
    pub label: String,
    #[serde(serialize_with = "ser::num")]
    pub estimated_min: f64,
    #[serde(serialize_with = "ser::num")]
    pub took_min: f64,
    #[serde(serialize_with = "ser::num")]
    pub count: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekFacts {
    /// The 7 days before today, both ends inclusive.
    pub from: DayKey,
    pub to: DayKey,
    pub spaces: Vec<SpaceFacts>,
    #[serde(serialize_with = "ser::num")]
    pub habit_focus_min: f64,
    pub milestones_reached: Vec<Milestone>,
    pub accuracy: Vec<Accuracy>,
}

struct Group {
    space_id: Id,
    kind: String,
    estimated: f64,
    took: f64,
    count: f64,
}

pub fn last_week_facts(data: &ReviewData, now: f64, tz: &TimeZone) -> WeekFacts {
    let today = day_key(now, tz);
    let from = add_days(&today, -7.0);
    let start = start_of_day(&from, tz);
    let end = start_of_day(&today, tz);
    let in_week = |ms: Option<f64>| ms.is_some_and(|ms| ms >= start && ms < end);
    let space_of: HashMap<&str, &str> = data
        .tasks
        .iter()
        .map(|t| (t.id.as_str(), t.space_id.as_str()))
        .collect();

    let done_tasks: Vec<&Task> = data
        .tasks
        .iter()
        .filter(|t| t.done && in_week(t.done_at))
        .collect();
    let done_occurrences: Vec<&TaskOccurrence> = data
        .occurrences
        .iter()
        .filter(|o| in_week(Some(o.done_at)))
        .collect();
    let week_sessions: Vec<&FocusSession> = data
        .sessions
        .iter()
        .filter(|s| in_week(Some(s.ended_at)))
        .collect();
    let spaces: Vec<SpaceFacts> = data
        .spaces
        .iter()
        .map(|space| SpaceFacts {
            space_id: space.id.clone(),
            name: space.name.clone(),
            tasks_done: (done_tasks.iter().filter(|t| t.space_id == space.id).count()
                + done_occurrences
                    .iter()
                    .filter(|o| space_of.get(o.task_id.as_str()) == Some(&space.id.as_str()))
                    .count()) as f64,
            focus_min: week_sessions
                .iter()
                .filter(|s| {
                    s.task_id
                        .as_deref()
                        .is_some_and(|id| space_of.get(id) == Some(&space.id.as_str()))
                })
                .fold(0.0, |sum, s| sum + s.focus_min),
        })
        .collect();

    // Estimates as Heat made them when the week began: the week's own tasks
    // don't yet count toward the averages they are measured against.
    let as_at_start: Vec<Task> = data
        .tasks
        .iter()
        .map(|t| {
            if t.done && in_week(t.done_at) {
                Task {
                    done: false,
                    ..t.clone()
                }
            } else {
                t.clone()
            }
        })
        .collect();
    let ctx = estimate_context(&as_at_start, &data.sessions);
    let mut groups: Vec<Group> = vec![];
    let mut index: HashMap<String, usize> = HashMap::new();
    for t in &done_tasks {
        let took = actual_min(t, &data.sessions);
        if took <= 0.0 {
            continue;
        }
        let key = type_key(&t.space_id, &t.r#type);
        let at = *index.entry(key).or_insert_with(|| {
            groups.push(Group {
                space_id: t.space_id.clone(),
                kind: t.r#type.clone(),
                estimated: 0.0,
                took: 0.0,
                count: 0.0,
            });
            groups.len() - 1
        });
        let g = &mut groups[at];
        g.estimated += estimate_min(
            &Task {
                done: false,
                ..(*t).clone()
            },
            &ctx,
        );
        g.took += took;
        g.count += 1.0;
    }
    let mut type_count: HashMap<&str, f64> = HashMap::new();
    for g in &groups {
        *type_count.entry(g.kind.as_str()).or_insert(0.0) += 1.0;
    }
    let space_name = |id: &str| -> String {
        data.spaces
            .iter()
            .find(|s| s.id == id)
            .map_or_else(|| id.to_string(), |s| s.name.clone())
    };

    WeekFacts {
        from: from.clone(),
        to: add_days(&today, -1.0),
        spaces,
        habit_focus_min: week_sessions
            .iter()
            .filter(|s| s.habit_id.is_some())
            .fold(0.0, |sum, s| sum + s.focus_min),
        milestones_reached: data
            .milestones
            .iter()
            .filter(|m| {
                m.done
                    && js::cmp(&m.date, &from) != Ordering::Less
                    && js::cmp(&m.date, &today) == Ordering::Less
            })
            .cloned()
            .collect(),
        accuracy: groups
            .iter()
            .map(|g| Accuracy {
                label: if type_count.get(g.kind.as_str()).copied().unwrap_or(0.0) > 1.0 {
                    format!("{} ({})", g.kind, space_name(&g.space_id))
                } else {
                    g.kind.clone()
                },
                estimated_min: js::round(g.estimated / g.count),
                took_min: js::round(g.took / g.count),
                count: g.count,
            })
            .collect(),
    }
}

/// The facts as the review shows them, and as Claude receives them.
pub fn fact_lines(f: &WeekFacts) -> Vec<String> {
    let mut lines: Vec<String> = f
        .spaces
        .iter()
        .map(|s| copy::review::space(&s.name, s.tasks_done, &format_minutes(s.focus_min)))
        .collect();
    if f.habit_focus_min > 0.0 {
        lines.push(copy::review::habits(&format_minutes(f.habit_focus_min)));
    }
    lines.extend(
        f.milestones_reached
            .iter()
            .map(|m| copy::review::milestone(&m.title)),
    );
    lines.extend(f.accuracy.iter().map(|a| {
        copy::review::accuracy(
            &a.label,
            &format_minutes(a.estimated_min),
            &format_minutes(a.took_min),
            a.count,
        )
    }));
    lines
}
