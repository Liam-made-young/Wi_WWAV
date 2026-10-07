//! `estimate.test.ts`, case for case.

mod common;

use common::*;
use wi_heat::model::estimate::{
    actual_min, average_lines, estimate_context, estimate_min, format_minutes, weekly_load,
    weekly_load_line, Load,
};
use wi_heat::model::records::{GroupKind, Space, Task};

// --- minutes, as written everywhere -----------------------------------------------------

#[test]
fn writes_45m_1h_15m_3h_10m_2h() {
    assert_eq!(format_minutes(45.0), "45m");
    assert_eq!(format_minutes(75.0), "1h 15m");
    assert_eq!(format_minutes(190.0), "3h 10m");
    assert_eq!(format_minutes(120.0), "2h");
    assert_eq!(format_minutes(0.0), "0m");
    assert_eq!(format_minutes(44.6), "45m");
}

// --- actualMin -------------------------------------------------------------------------------

#[test]
fn is_the_sum_of_focus_minutes_plus_the_hand_adjustment() {
    let t = task(|t| t.adjust_min = 10.0);
    let sessions = vec![
        session(|s| {
            s.task_id = Some(t.id.clone());
            s.focus_min = 25.0;
        }),
        session(|s| {
            s.task_id = Some(t.id.clone());
            s.focus_min = 18.0;
        }),
        session(|s| {
            s.task_id = Some("other".into());
            s.focus_min = 50.0;
        }),
    ];
    assert_eq!(actual_min(&t, &sessions), 53.0);
}

// --- the estimate chain (3.1) -----------------------------------------------------------------

fn done(kind: &str, minutes: f64, space_id: &str) -> Task {
    task(|t| {
        t.space_id = space_id.into();
        t.r#type = kind.into();
        t.done = true;
        t.done_at = Some(1.0);
        t.adjust_min = minutes;
    })
}

fn with(history: &[Task], t: &Task) -> Vec<Task> {
    history.iter().cloned().chain([t.clone()]).collect()
}

#[test]
fn takes_the_tasks_own_est_min_first() {
    let history = [done("Homework", 60.0, "classes")];
    let t = task(|t| {
        t.r#type = "Homework".into();
        t.est_min = Some(45.0);
        t.difficulty = 4.0;
    });
    let all = with(&history, &t);
    assert_eq!(estimate_min(&t, &estimate_context(&all, &[])), 45.0);
}

#[test]
fn then_the_average_for_its_type_in_its_space() {
    let history = [
        done("Homework", 60.0, "classes"),
        done("Homework", 90.0, "classes"),
        done("Homework", 0.0, "classes"),
        done("Homework", 500.0, "wwav"),
    ];
    let t = task(|t| {
        t.r#type = "Homework".into();
        t.difficulty = 4.0;
    });
    // The task with no time and the other space's task don't count.
    let all = with(&history, &t);
    assert_eq!(estimate_min(&t, &estimate_context(&all, &[])), 75.0);
}

#[test]
fn then_difficulty_times_20_minutes() {
    let t = task(|t| {
        t.r#type = "Quiz".into();
        t.difficulty = 2.0;
    });
    let all = with(&[done("Homework", 60.0, "classes")], &t);
    assert_eq!(estimate_min(&t, &estimate_context(&all, &[])), 40.0);
}

#[test]
fn learns_from_focus_minutes_not_only_typed_ones() {
    let past = task(|t| {
        t.r#type = "Lab".into();
        t.done = true;
        t.done_at = Some(1.0);
    });
    let t = task(|t| {
        t.r#type = "Lab".into();
        t.difficulty = 1.0;
    });
    let sessions = vec![
        session(|s| {
            s.task_id = Some(past.id.clone());
            s.focus_min = 25.0;
        }),
        session(|s| {
            s.task_id = Some(past.id.clone());
            s.focus_min = 25.0;
        }),
    ];
    let all = vec![past, t.clone()];
    assert_eq!(estimate_min(&t, &estimate_context(&all, &sessions)), 50.0);
}

#[test]
fn makes_a_parent_the_sum_of_its_open_children_all_the_way_down() {
    let parent = task(|t| t.est_min = Some(999.0));
    let a = task(|t| {
        t.parent_task_id = Some(parent.id.clone());
        t.est_min = Some(30.0);
    });
    let b = task(|t| {
        t.parent_task_id = Some(parent.id.clone());
        t.est_min = Some(999.0);
    });
    let b1 = task(|t| {
        t.parent_task_id = Some(b.id.clone());
        t.est_min = Some(20.0);
    });
    let b2 = task(|t| {
        t.parent_task_id = Some(b.id.clone());
        t.est_min = Some(25.0);
    });
    let shut = task(|t| {
        t.parent_task_id = Some(parent.id.clone());
        t.est_min = Some(60.0);
        t.done = true;
        t.done_at = Some(1.0);
    });
    let all = vec![parent.clone(), a, b.clone(), b1, b2, shut];
    let ctx = estimate_context(&all, &[]);
    assert_eq!(estimate_min(&b, &ctx), 45.0);
    assert_eq!(estimate_min(&parent, &ctx), 75.0);
}

#[test]
fn falls_back_to_the_parents_own_chain_once_every_child_is_done() {
    let parent = task(|t| t.est_min = Some(40.0));
    let child = task(|t| {
        t.parent_task_id = Some(parent.id.clone());
        t.est_min = Some(60.0);
        t.done = true;
        t.done_at = Some(1.0);
    });
    let all = vec![parent.clone(), child];
    assert_eq!(estimate_min(&parent, &estimate_context(&all, &[])), 40.0);
}

// --- your average time -----------------------------------------------------------------------------

#[test]
fn reads_homework_1h_15m_6_in_the_spaces_type_order() {
    let space = Space {
        id: "classes".into(),
        name: "Classes".into(),
        hue: 0.0,
        group_kind: GroupKind::Course,
        group_label: "Course".into(),
        types: vec!["Homework".into(), "Quiz".into(), "Listening".into()],
        persona: String::new(),
    };
    let homework: Vec<Task> = [60.0, 70.0, 80.0, 70.0, 80.0, 90.0]
        .into_iter()
        .map(|m| {
            task(|t| {
                t.r#type = "Homework".into();
                t.done = true;
                t.done_at = Some(1.0);
                t.adjust_min = m;
            })
        })
        .collect();
    let quiz = task(|t| {
        t.r#type = "Quiz".into();
        t.done = true;
        t.done_at = Some(1.0);
        t.adjust_min = 20.0;
    });
    let all: Vec<Task> = [quiz].into_iter().chain(homework).collect();
    assert_eq!(
        average_lines(&space, &estimate_context(&all, &[])),
        ["Homework 1h 15m (6)", "Quiz 20m (1)"]
    );
}

// --- weekly load (3.1) --------------------------------------------------------------------------------------

#[test]
fn sums_the_estimates_of_open_tasks_due_within_7_days() {
    let now = ny("2026-10-06 08:40");
    let t = |est: f64, due: Option<f64>, finished: bool| {
        task(|t| {
            t.est_min = Some(est);
            t.due = due;
            if finished {
                t.done = true;
                t.done_at = Some(now);
            }
        })
    };
    let tasks = vec![
        t(60.0, Some(now + DAY_MS), false),
        t(45.0, Some(now + 7.0 * DAY_MS), false),
        t(30.0, Some(now - DAY_MS), false), // overdue is still this week's work
        t(500.0, Some(now + 7.0 * DAY_MS + 1.0), false),
        t(500.0, None, false),
        t(500.0, Some(now + DAY_MS), true),
    ];
    assert_eq!(
        weekly_load(&tasks, &estimate_context(&tasks, &[]), now),
        Load {
            minutes: 135.0,
            count: 3.0
        }
    );
}

#[test]
fn counts_a_subtask_once_inside_its_parent() {
    let now = ny("2026-10-06 08:40");
    let parent = task(|t| t.due = Some(now + 2.0 * DAY_MS));
    let child = task(|t| {
        t.parent_task_id = Some(parent.id.clone());
        t.est_min = Some(40.0);
        t.due = Some(now + DAY_MS);
    });
    let tasks = vec![parent, child];
    assert_eq!(
        weekly_load(&tasks, &estimate_context(&tasks, &[]), now),
        Load {
            minutes: 40.0,
            count: 1.0
        }
    );
}

#[test]
fn reads_this_week_3h_20m_across_5_tasks() {
    assert_eq!(
        weekly_load_line(&Load {
            minutes: 200.0,
            count: 5.0
        }),
        "This week: 3h 20m across 5 tasks"
    );
    assert_eq!(
        weekly_load_line(&Load {
            minutes: 45.0,
            count: 1.0
        }),
        "This week: 45m across 1 task"
    );
}
