//! Time: the estimate chain, measured time, and the sums built from them
//! (`docs/SPEC.md` 3.1, 3.5, 3.6).
//!
//! A task's estimate is its own `estMin` if it has one, else the average for
//! its type in its space, else difficulty × 20 minutes. A parent's estimate is
//! the sum of its open children. `actualMin` = Σ focus minutes + `adjustMin`.
//! A port of `estimate.ts`.

use super::heat::DAY_MS;
use super::records::{ser, FocusSession, Space, Task};
use super::{copy, js};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Average {
    #[serde(serialize_with = "ser::num")]
    pub minutes: f64,
    #[serde(serialize_with = "ser::num")]
    pub count: f64,
}

/// The averages by space and type, and each parent's children, worked out
/// once from the tasks and sessions and then read by [`estimate_min`].
#[derive(Clone, Debug)]
pub struct EstimateContext<'a> {
    /// Average measured minutes by space and type (see [`type_key`]), from done tasks with time.
    pub averages: HashMap<String, Average>,
    pub children: HashMap<&'a str, Vec<&'a Task>>,
}

/// "45m", "1h 15m", "3h 10m", "2h".
pub fn format_minutes(minutes: f64) -> String {
    let m = js::round(minutes);
    let h = (m / 60.0).floor();
    if h == 0.0 {
        return format!("{}m", js::num_to_string(m));
    }
    if m % 60.0 == 0.0 {
        format!("{}h", js::num_to_string(h))
    } else {
        format!("{}h {}m", js::num_to_string(h), js::num_to_string(m % 60.0))
    }
}

fn focus_minutes_by_task(sessions: &[FocusSession]) -> HashMap<&str, f64> {
    let mut out: HashMap<&str, f64> = HashMap::new();
    for s in sessions {
        if let Some(id) = s.task_id.as_deref().filter(|id| !id.is_empty()) {
            *out.entry(id).or_insert(0.0) += s.focus_min;
        }
    }
    out
}

pub fn actual_min(task: &Task, sessions: &[FocusSession]) -> f64 {
    focus_minutes_by_task(sessions)
        .get(task.id.as_str())
        .copied()
        .unwrap_or(0.0)
        + task.adjust_min
}

/// The key of a space's type in [`EstimateContext::averages`].
pub fn type_key(space_id: &str, kind: &str) -> String {
    format!("{space_id}\u{0}{kind}")
}

pub fn estimate_context<'a>(tasks: &'a [Task], sessions: &[FocusSession]) -> EstimateContext<'a> {
    let focus = focus_minutes_by_task(sessions);
    let mut sums: HashMap<String, (f64, f64)> = HashMap::new();
    let mut children: HashMap<&'a str, Vec<&'a Task>> = HashMap::new();
    for t in tasks {
        if let Some(parent) = t.parent_task_id.as_deref().filter(|p| !p.is_empty()) {
            children.entry(parent).or_default().push(t);
        }
        let actual = focus.get(t.id.as_str()).copied().unwrap_or(0.0) + t.adjust_min;
        if !t.done || actual <= 0.0 {
            continue;
        }
        let sum = sums
            .entry(type_key(&t.space_id, &t.r#type))
            .or_insert((0.0, 0.0));
        *sum = (sum.0 + actual, sum.1 + 1.0);
    }
    let averages = sums
        .into_iter()
        .map(|(key, (total, count))| {
            (
                key,
                Average {
                    minutes: js::round(total / count),
                    count,
                },
            )
        })
        .collect();
    EstimateContext { averages, children }
}

/// A task's estimate. Parent links that loop (a task its own parent, or
/// A → B → A) are walked once: a task already on the way down counts as no
/// child of the one below it.
pub fn estimate_min(task: &Task, ctx: &EstimateContext) -> f64 {
    estimate_below(task, ctx, &[])
}

fn estimate_below<'a>(task: &'a Task, ctx: &EstimateContext<'a>, above: &[&'a str]) -> f64 {
    let mut path = above.to_vec();
    path.push(&task.id);
    let open: Vec<&Task> = ctx
        .children
        .get(task.id.as_str())
        .map(|kids| {
            kids.iter()
                .copied()
                .filter(|c| !c.done && !path.contains(&c.id.as_str()))
                .collect()
        })
        .unwrap_or_default();
    if !open.is_empty() {
        return open
            .iter()
            .fold(0.0, |sum, c| sum + estimate_below(c, ctx, &path));
    }
    if let Some(own) = task.est_min.filter(|m| *m > 0.0) {
        return own;
    }
    if let Some(average) = ctx.averages.get(&type_key(&task.space_id, &task.r#type)) {
        return average.minutes;
    }
    js::min2(5.0, js::max2(1.0, js::round(task.difficulty))) * 20.0
}

/// The sidebar's "Your average time": "Homework 1h 15m (6)", in the space's type order.
pub fn average_lines(space: &Space, ctx: &EstimateContext) -> Vec<String> {
    space
        .types
        .iter()
        .filter_map(|kind| {
            let a = ctx.averages.get(&type_key(&space.id, kind))?;
            Some(copy::average_time(
                kind,
                &format_minutes(a.minutes),
                a.count,
            ))
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Load {
    #[serde(serialize_with = "ser::num")]
    pub minutes: f64,
    #[serde(serialize_with = "ser::num")]
    pub count: f64,
}

/// The sum of estimates for open tasks due within 7 days, overdue ones
/// included. A subtask whose parent is also counted is counted once, inside
/// the parent's estimate.
pub fn weekly_load(tasks: &[Task], ctx: &EstimateContext, now: f64) -> Load {
    let in_week: Vec<&Task> = tasks
        .iter()
        .filter(|t| !t.done && t.due.is_some_and(|due| due - now <= 7.0 * DAY_MS))
        .collect();
    let counted: HashSet<&str> = in_week.iter().map(|t| t.id.as_str()).collect();
    let by_id: HashMap<&str, &Task> = tasks.iter().map(|t| (t.id.as_str(), t)).collect();
    // Stops where parent links loop back, so a malformed chain can't hang the re-render.
    let has_counted_ancestor = |t: &Task| -> bool {
        let mut seen: HashSet<&str> = HashSet::from([t.id.as_str()]);
        let mut p = t.parent_task_id.as_deref().filter(|p| !p.is_empty());
        while let Some(id) = p {
            if seen.contains(id) {
                break;
            }
            if counted.contains(id) {
                return true;
            }
            seen.insert(id);
            p = by_id
                .get(id)
                .and_then(|a| a.parent_task_id.as_deref())
                .filter(|p| !p.is_empty());
        }
        false
    };
    let top: Vec<&Task> = in_week
        .into_iter()
        .filter(|t| !has_counted_ancestor(t))
        .collect();
    Load {
        minutes: top.iter().fold(0.0, |sum, t| sum + estimate_min(t, ctx)),
        count: top.len() as f64,
    }
}

/// "This week: 3h 20m across 5 tasks"
pub fn weekly_load_line(load: &Load) -> String {
    copy::weekly_load(&format_minutes(load.minutes), load.count)
}
