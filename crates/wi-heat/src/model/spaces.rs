//! Spaces (`docs/SPEC.md` 3.4): named filters over Today, Tasks, Calendar and
//! Mail. Grades, Habits and calendar events ignore them, because they belong
//! to the person rather than to a project. Classes, WWAV and Personal are
//! created with the names, group labels and types 3.1 gives, and the WWAV
//! persona is the one 3.4 rewrites to cover the app. The spec doesn't give the
//! artifact's hues or its Classes and Personal personas, so the ones below are
//! stand-ins until the founder supplies the artifact's text (QUESTIONS.md,
//! "Artifact values the spec doesn't give"). A port of `spaces.ts` (the
//! sidebar's group heading and empty-list line are display and stay in
//! TypeScript).

use super::heat::{by_heat, heat_of, HeatLevel, DAY_MS};
use super::js;
use super::records::{
    Capture, Course, GroupKind, Id, Milestone, Project, ProjectStatus, Space, Task, TaskOccurrence,
};
use super::recurrence::open_tasks;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SidebarData {
    pub tasks: Vec<Task>,
    pub occurrences: Vec<TaskOccurrence>,
    pub captures: Vec<Capture>,
    pub projects: Vec<Project>,
    pub milestones: Vec<Milestone>,
    pub courses: Vec<Course>,
}

pub fn default_spaces(new_id: &mut dyn FnMut() -> Id) -> Vec<Space> {
    let strings = |xs: &[&str]| xs.iter().map(|x| (*x).to_string()).collect::<Vec<_>>();
    vec![
        Space {
            id: new_id(),
            name: "Classes".into(),
            hue: 211.0,
            group_kind: GroupKind::Course,
            group_label: "Course".into(),
            types: strings(&["Homework", "Quiz", "Listening", "Reading", "Lab", "Project", "Exam prep", "Other"]),
            persona: "a university student keeping up with coursework across several classes.".into(),
        },
        Space {
            id: new_id(),
            name: "WWAV".into(),
            hue: 6.0,
            group_kind: GroupKind::Milestone,
            group_label: "Milestone".into(),
            types: strings(&["Hardware", "Software", "Design", "Music", "Business", "Content", "Other"]),
            persona: "a solo founder building WWAV: the PRANA handheld (Teensy 4.1, C++ firmware, PCBs), the Wi_WWAV desktop app, and an album.".into(),
        },
        Space {
            id: new_id(),
            name: "Personal".into(),
            hue: 145.0,
            group_kind: GroupKind::Free,
            group_label: "Area".into(),
            types: strings(&["Errand", "Admin", "Money", "Health", "Home", "Social", "Other"]),
            persona: "a person keeping up with errands, admin, money, health, home and friends.".into(),
        },
    ]
}

/// The space filter; None, or an empty id, is All.
pub fn in_space(space_id: Option<&str>) -> impl Fn(&Task) -> bool + '_ {
    move |t| match space_id {
        None | Some("") => true,
        Some(id) => t.space_id == id,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpaceOpen {
    pub space: Space,
    pub open: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpaceCounts {
    pub all: usize,
    pub spaces: Vec<SpaceOpen>,
}

/// The sidebar's All, then each space with its open count.
pub fn space_counts(
    spaces: &[Space],
    tasks: &[Task],
    occurrences: &[TaskOccurrence],
    now: f64,
    tz: &TimeZone,
) -> SpaceCounts {
    let open = open_tasks(tasks, occurrences, now, tz);
    SpaceCounts {
        all: open.len(),
        spaces: spaces
            .iter()
            .map(|space| SpaceOpen {
                space: space.clone(),
                open: open.iter().filter(|t| in_space(Some(&space.id))(t)).count(),
            })
            .collect(),
    }
}

/// The Tasks sidebar's lists (3.6), each in heat order. Due this week is the
/// window weekly load sums: due within 7 days, overdue included.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryLists {
    pub inbox: Vec<Capture>,
    pub all_open: Vec<Task>,
    pub hot: Vec<Task>,
    pub due_this_week: Vec<Task>,
    pub scheduled: Vec<Task>,
    pub someday: Vec<Task>,
    pub done: Vec<Task>,
}

pub fn library_lists(
    data: &SidebarData,
    now: f64,
    tz: &TimeZone,
    space_id: Option<&str>,
) -> LibraryLists {
    let filter = in_space(space_id);
    let in_scope: Vec<Task> = data.tasks.iter().filter(|t| filter(t)).cloned().collect();
    let opened = open_tasks(&in_scope, &data.occurrences, now, tz);
    let open: Vec<Task> = by_heat(&opened, now).into_iter().cloned().collect();
    let someday_projects: HashSet<&str> = data
        .projects
        .iter()
        .filter(|p| p.status == ProjectStatus::Someday)
        .map(|p| p.id.as_str())
        .collect();
    let pick = |keep: &dyn Fn(&Task) -> bool| -> Vec<Task> {
        open.iter().filter(|t| keep(t)).cloned().collect()
    };
    LibraryLists {
        inbox: data
            .captures
            .iter()
            .filter(|c| c.triaged_at.is_none())
            .cloned()
            .collect(),
        all_open: open.clone(),
        hot: pick(&|t| matches!(heat_of(t, now).level, HeatLevel::Hot | HeatLevel::Overdue)),
        due_this_week: pick(&|t| t.due.is_some_and(|due| due - now <= 7.0 * DAY_MS)),
        scheduled: pick(&|t| t.scheduled_date.is_some()),
        someday: pick(&|t| {
            t.project_id
                .as_deref()
                .is_some_and(|p| someday_projects.contains(p))
        }),
        done: data
            .tasks
            .iter()
            .filter(|t| t.done && filter(t))
            .cloned()
            .collect(),
    }
}

/// A task's group as the LCD names it: the course code, the milestone, or the area.
pub fn group_name(task: &Task, courses: &[Course], milestones: &[Milestone]) -> Option<String> {
    if let Some(id) = task.course_id.as_deref().filter(|id| !id.is_empty()) {
        return courses.iter().find(|c| c.id == id).map(|c| c.code.clone());
    }
    if let Some(id) = task.milestone_id.as_deref().filter(|id| !id.is_empty()) {
        return milestones
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.title.clone());
    }
    task.group.clone()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupCount {
    pub id: String,
    pub name: String,
    pub open: usize,
}

/// The groups under a space in the sidebar, with their open counts.
pub fn group_counts(space: &Space, data: &SidebarData, now: f64, tz: &TimeZone) -> Vec<GroupCount> {
    let filter = in_space(Some(&space.id));
    let all: Vec<Task> = data.tasks.iter().filter(|t| filter(t)).cloned().collect();
    let open = open_tasks(&all, &data.occurrences, now, tz);
    let count = |key: &dyn Fn(&Task) -> Option<&str>, id: &str| {
        open.iter().filter(|t| key(t) == Some(id)).count()
    };
    match space.group_kind {
        GroupKind::Course => {
            let ids: HashSet<&str> = all.iter().filter_map(|t| t.course_id.as_deref()).collect();
            let courses: Vec<&Course> = data
                .courses
                .iter()
                .filter(|c| ids.contains(c.id.as_str()))
                .collect();
            js::sort_by(courses, |a, b| js::locale_compare(&a.code, &b.code))
                .into_iter()
                .map(|c| GroupCount {
                    id: c.id.clone(),
                    name: c.code.clone(),
                    open: count(&|t| t.course_id.as_deref(), &c.id),
                })
                .collect()
        }
        GroupKind::Milestone => {
            let beads: Vec<&Milestone> = data
                .milestones
                .iter()
                .filter(|m| m.space_id == space.id)
                .collect();
            js::sort_by(beads, |a, b| super::heat::order_of(a.order - b.order))
                .into_iter()
                .map(|m| GroupCount {
                    id: m.id.clone(),
                    name: m.title.clone(),
                    open: count(&|t| t.milestone_id.as_deref(), &m.id),
                })
                .collect()
        }
        GroupKind::Free => {
            let mut seen: HashSet<&str> = HashSet::new();
            let mut names: Vec<&str> = vec![];
            for t in &all {
                if let Some(g) = t.group.as_deref().filter(|g| !g.is_empty()) {
                    if seen.insert(g) {
                        names.push(g);
                    }
                }
            }
            js::sort_by(names, |a, b| -> Ordering { js::locale_compare(a, b) })
                .into_iter()
                .map(|g| GroupCount {
                    id: g.to_string(),
                    name: g.to_string(),
                    open: count(&|t| t.group.as_deref(), g),
                })
                .collect()
        }
    }
}
