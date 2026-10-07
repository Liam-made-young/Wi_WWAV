//! `lcd.test.ts`, case for case.

mod common;

use common::*;
use wi_heat::model::focus::{
    focus_step, focus_strip, initial_focus, FocusEvent, FocusSettings, FocusTarget, TargetKind,
};
use wi_heat::model::lcd::{task_half, LcdData, TaskHalf};
use wi_heat::model::records::{Course, Room};

fn course(id: &str, code: &str) -> Course {
    Course {
        id: id.into(),
        term_id: "t".into(),
        code: code.into(),
        name: "Japanese".into(),
        categories: vec![],
        scale: None,
        notes: String::new(),
    }
}

fn data(over: impl FnOnce(&mut LcdData)) -> LcdData {
    let mut d = LcdData {
        courses: vec![course("jpn102", "JPN 102"), course("jpn201", "JPN 201")],
        ..LcdData::default()
    };
    over(&mut d);
    d
}

#[test]
fn shows_the_hottest_open_task_its_due_and_group_and_the_weeks_load() {
    let now = ny("2026-10-06 12:30");
    let quiz = task(|t| {
        t.title = "Grammar quiz 4".into();
        t.course_id = Some("jpn102".into());
        t.difficulty = 2.0;
        t.due = Some(ny("2026-10-07 23:59"));
        t.est_min = Some(45.0);
    });
    let mut tasks: Vec<_> = [40.0, 35.0, 60.0, 20.0]
        .iter()
        .enumerate()
        .map(|(i, est)| {
            task(|t| {
                t.est_min = Some(*est);
                t.difficulty = 1.0;
                t.due = Some(ny("2026-10-10 12:00") + i as f64);
            })
        })
        .collect();
    tasks.push(quiz.clone());
    let half = task_half(&data(|d| d.tasks = tasks), now, &ny_zone(), None, None);
    assert_eq!(half.task_id.as_deref(), Some(quiz.id.as_str()));
    assert_eq!(half.line1, "Hot: Grammar quiz 4");
    assert_eq!(
        half.line2,
        "Tomorrow 11:59 PM, JPN 102. This week: 3h 20m across 5 tasks"
    );
    assert!((half.meter - (1.0 - 35.483 / 24.0 / 5.0)).abs() < 0.0005);
}

#[test]
fn shows_the_current_task_over_the_hottest_with_the_focus_countdown() {
    let now = ny("2026-10-06 12:30");
    let hot = task(|t| {
        t.title = "Problem set".into();
        t.difficulty = 5.0;
        t.due = Some(ny("2026-10-06 18:00"));
    });
    let quiz = task(|t| {
        t.title = "Grammar quiz 4".into();
        t.course_id = Some("jpn201".into());
        t.difficulty = 3.0;
        t.due = Some(ny("2026-10-06 16:00"));
    });
    let press = FocusEvent::Press {
        target: Some(FocusTarget {
            kind: TargetKind::Task,
            id: quiz.id.clone(),
            title: quiz.title.clone(),
            minutes: None,
        }),
        room: Some(Room::Heat),
    };
    let focus = focus_step(&initial_focus(), &press, now, FocusSettings::default()).state;
    let later = now + 6.0 * MIN + 20_000.0;
    let half = task_half(
        &data(|d| d.tasks = vec![hot, quiz.clone()]),
        later,
        &ny_zone(),
        Some(&quiz.id),
        focus_strip(&focus, later).as_deref(),
    );
    assert_eq!(half.line1, "Hot: Grammar quiz 4");
    assert_eq!(half.line2, "Today 4:00 PM, JPN 201 · focus 18:40 left");
}

#[test]
fn falls_back_to_the_hottest_task_once_the_current_one_is_done() {
    let now = ny("2026-10-06 12:30");
    let shut = task(|t| {
        t.title = "Shipped".into();
        t.done = true;
        t.done_at = Some(now);
    });
    let next = task(|t| {
        t.title = "Next".into();
        t.due = Some(ny("2026-10-09 12:00"));
    });
    let half = task_half(
        &data(|d| d.tasks = vec![shut.clone(), next.clone()]),
        now,
        &ny_zone(),
        Some(&shut.id),
        None,
    );
    assert_eq!(half.task_id.as_deref(), Some(next.id.as_str()));
}

#[test]
fn leaves_out_what_a_task_doesnt_have() {
    let now = ny("2026-10-06 12:30");
    let plain = task(|t| {
        t.title = "Call the bank".into();
        t.difficulty = 1.0;
    });
    let d = data(|d| d.tasks = vec![plain.clone()]);
    let z = ny_zone();
    assert_eq!(
        task_half(&d, now, &z, None, None).line2,
        "This week: 0m across 0 tasks"
    );
    assert_eq!(
        task_half(&d, now, &z, Some(&plain.id), Some("focus 3:00 left")).line2,
        "focus 3:00 left"
    );
    assert_eq!(
        task_half(&d, now, &z, None, None).line1,
        "Cool: Call the bank"
    );
}

#[test]
fn says_all_clear_when_nothing_is_open() {
    let now = ny("2026-10-06 12:30");
    assert_eq!(
        task_half(&data(|_| {}), now, &ny_zone(), None, None),
        TaskHalf {
            task_id: None,
            line1: "All clear".into(),
            line2: "Nothing open right now.".into(),
            meter: 0.0
        }
    );
}
