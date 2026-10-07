//! `review.test.ts`, case for case.

mod common;

use common::*;
use wi_heat::model::records::{GroupKind, Milestone, Space, TaskOccurrence};
use wi_heat::model::review::{fact_lines, last_week_facts, Accuracy, ReviewData, SpaceFacts};

fn space(id: &str, name: &str, hue: f64, kind: GroupKind, label: &str, types: &[&str]) -> Space {
    Space {
        id: id.into(),
        name: name.into(),
        hue,
        group_kind: kind,
        group_label: label.into(),
        types: types.iter().map(|t| (*t).to_string()).collect(),
        persona: String::new(),
    }
}

fn data(over: impl FnOnce(&mut ReviewData)) -> ReviewData {
    let mut d = ReviewData {
        spaces: vec![
            space(
                "classes",
                "Classes",
                211.0,
                GroupKind::Course,
                "Course",
                &["Homework", "Other"],
            ),
            space(
                "wwav",
                "WWAV",
                6.0,
                GroupKind::Milestone,
                "Milestone",
                &["Software", "Other"],
            ),
        ],
        ..ReviewData::default()
    };
    over(&mut d);
    d
}

// A Monday; last week is October 5 to 11.
fn now() -> f64 {
    ny("2026-10-12 10:00")
}

fn in_week() -> f64 {
    ny("2026-10-08 15:00")
}

fn bead(id: &str, title: &str, date: &str, done: bool, order: f64) -> Milestone {
    Milestone {
        id: id.into(),
        space_id: "wwav".into(),
        project_id: None,
        title: title.into(),
        date: date.into(),
        done,
        order,
        link: None,
    }
}

#[test]
fn counts_tasks_done_and_focus_time_per_space_in_the_last_7_days_only() {
    let done_in = |space: &str, at: f64| {
        task(|t| {
            t.space_id = space.into();
            t.done = true;
            t.done_at = Some(at);
        })
    };
    let a = done_in("classes", in_week());
    let b = done_in("classes", ny("2026-10-05 00:00"));
    let early = done_in("classes", ny("2026-10-04 23:59"));
    let today = done_in("classes", ny("2026-10-12 00:00"));
    let c = task(|t| t.space_id = "wwav".into());
    let series = task(|t| {
        t.space_id = "wwav".into();
        t.rrule = Some("FREQ=DAILY".into());
        t.due = Some(ny("2026-10-01 09:00"));
    });
    let occ = TaskOccurrence {
        id: "o".into(),
        task_id: series.id.clone(),
        date: "2026-10-07".into(),
        done_at: in_week(),
    };
    let sessions = vec![
        session(|s| {
            s.task_id = Some(a.id.clone());
            s.focus_min = 25.0;
            s.ended_at = in_week();
        }),
        session(|s| {
            s.task_id = Some(c.id.clone());
            s.focus_min = 50.0;
            s.ended_at = in_week();
        }),
        session(|s| {
            s.task_id = Some(c.id.clone());
            s.focus_min = 50.0;
            s.ended_at = ny("2026-10-04 12:00");
        }),
        session(|s| {
            s.habit_id = Some("kanji".into());
            s.focus_min = 20.0;
            s.ended_at = in_week();
        }),
    ];
    let facts = last_week_facts(
        &data(|d| {
            d.tasks = vec![a, b, early, today, c, series];
            d.occurrences = vec![occ];
            d.sessions = sessions;
        }),
        now(),
        &ny_zone(),
    );
    assert_eq!(facts.from, "2026-10-05");
    assert_eq!(facts.to, "2026-10-11");
    assert_eq!(
        facts.spaces,
        vec![
            SpaceFacts {
                space_id: "classes".into(),
                name: "Classes".into(),
                tasks_done: 2.0,
                focus_min: 25.0
            },
            SpaceFacts {
                space_id: "wwav".into(),
                name: "WWAV".into(),
                tasks_done: 1.0,
                focus_min: 50.0
            },
        ]
    );
    assert_eq!(facts.habit_focus_min, 20.0);
}

#[test]
fn lists_the_milestones_reached_that_week() {
    let reached = bead("m1", "Enclosure v2", "2026-10-09", true, 1.0);
    let missed = bead("m2", "Firmware 1.0", "2026-10-10", false, 2.0);
    let old = bead("m3", "Schematic", "2026-09-01", true, 0.0);
    let facts = last_week_facts(
        &data(|d| d.milestones = vec![reached.clone(), missed, old]),
        now(),
        &ny_zone(),
    );
    assert_eq!(facts.milestones_reached, vec![reached]);
}

#[test]
fn measures_estimates_against_time_taken_homework_estimated_1h_15m_took_1h_32m_across_4() {
    let homework: Vec<_> = [(60.0, 80.0), (75.0, 95.0), (75.0, 100.0), (90.0, 93.0)]
        .iter()
        .map(|(est, took)| {
            task(|t| {
                t.r#type = "Homework".into();
                t.est_min = Some(*est);
                t.adjust_min = *took;
                t.done = true;
                t.done_at = Some(in_week());
            })
        })
        .collect();
    // Done before the week, so it is history, not part of the week's accuracy.
    let before = task(|t| {
        t.r#type = "Homework".into();
        t.est_min = Some(10.0);
        t.adjust_min = 500.0;
        t.done = true;
        t.done_at = Some(ny("2026-09-30 12:00"));
    });
    let untimed = task(|t| {
        t.r#type = "Homework".into();
        t.est_min = Some(30.0);
        t.done = true;
        t.done_at = Some(in_week());
    });
    let mut all = homework;
    all.push(before);
    all.push(untimed);
    let facts = last_week_facts(&data(|d| d.tasks = all), now(), &ny_zone());
    assert_eq!(
        facts.accuracy,
        vec![Accuracy {
            label: "Homework".into(),
            estimated_min: 75.0,
            took_min: 92.0,
            count: 4.0
        }]
    );
    assert!(fact_lines(&facts)
        .contains(&"Homework: estimated 1h 15m, took 1h 32m across 4".to_string()));
}

#[test]
fn estimates_a_task_without_est_min_from_the_averages_as_they_stood_when_the_week_began() {
    let history = task(|t| {
        t.r#type = "Homework".into();
        t.adjust_min = 40.0;
        t.done = true;
        t.done_at = Some(ny("2026-09-30 12:00"));
    });
    let week = task(|t| {
        t.r#type = "Homework".into();
        t.difficulty = 5.0;
        t.adjust_min = 70.0;
        t.done = true;
        t.done_at = Some(in_week());
    });
    let facts = last_week_facts(&data(|d| d.tasks = vec![history, week]), now(), &ny_zone());
    assert_eq!(
        facts.accuracy,
        vec![Accuracy {
            label: "Homework".into(),
            estimated_min: 40.0,
            took_min: 70.0,
            count: 1.0
        }]
    );
}

#[test]
fn names_the_space_when_two_spaces_share_a_type() {
    let both = |space: &str, est: f64, took: f64| {
        task(|t| {
            t.space_id = space.into();
            t.r#type = "Other".into();
            t.est_min = Some(est);
            t.adjust_min = took;
            t.done = true;
            t.done_at = Some(in_week());
        })
    };
    let facts = last_week_facts(
        &data(|d| d.tasks = vec![both("classes", 30.0, 30.0), both("wwav", 60.0, 45.0)]),
        now(),
        &ny_zone(),
    );
    assert_eq!(
        facts
            .accuracy
            .iter()
            .map(|x| x.label.as_str())
            .collect::<Vec<_>>(),
        ["Other (Classes)", "Other (WWAV)"]
    );
}

#[test]
fn writes_the_facts_as_plain_lines_and_nothing_else() {
    let reached = bead("m1", "Enclosure v2", "2026-10-09", true, 1.0);
    let a = task(|t| {
        t.space_id = "classes".into();
        t.r#type = "Homework".into();
        t.est_min = Some(60.0);
        t.adjust_min = 50.0;
        t.done = true;
        t.done_at = Some(in_week());
    });
    let sessions = vec![session(|s| {
        s.habit_id = Some("kanji".into());
        s.focus_min = 80.0;
        s.ended_at = in_week();
    })];
    let facts = last_week_facts(
        &data(|d| {
            d.tasks = vec![a];
            d.sessions = sessions;
            d.milestones = vec![reached];
        }),
        now(),
        &ny_zone(),
    );
    assert_eq!(
        fact_lines(&facts),
        [
            "Classes: 1 task done, 0m of focus",
            "WWAV: 0 tasks done, 0m of focus",
            "Habits: 1h 20m of focus",
            "Milestone reached: Enclosure v2",
            "Homework: estimated 1h, took 50m across 1",
        ]
    );
}
