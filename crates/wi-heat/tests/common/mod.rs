//! Builders shared by the model's tests: the Rust side of `testkit.ts`. Times
//! are written as New York wall clocks, the zone 3.11 says URI's students get
//! from the system. `ny` asks jiff directly, not the model's own zone code, so
//! the tests don't lean on what they test.

#![allow(dead_code)]

use jiff::tz::TimeZone;
use std::sync::atomic::{AtomicUsize, Ordering};
use wi_heat::model::records::{FocusSession, Habit, Origin, Room, Task, TaskSource, TimeBlock};

pub const NY: &str = "America/New_York";
pub const DAY_MS: f64 = 86_400_000.0;
pub const MIN: f64 = 60_000.0;

pub fn ny_zone() -> TimeZone {
    TimeZone::get(NY).expect("America/New_York")
}

/// "2026-10-06 08:40" in New York, as epoch ms.
pub fn ny(text: &str) -> f64 {
    let (date, time) = text.split_once(' ').unwrap_or((text, "00:00"));
    let d: Vec<i16> = date
        .split('-')
        .map(|p| p.parse().expect("a date part"))
        .collect();
    let t: Vec<i8> = time
        .split(':')
        .map(|p| p.parse().expect("a time part"))
        .collect();
    let civil = jiff::civil::date(d[0], d[1] as i8, d[2] as i8).at(t[0], t[1], 0, 0);
    civil
        .in_tz(NY)
        .expect("a New York time")
        .timestamp()
        .as_millisecond() as f64
}

static SERIAL: AtomicUsize = AtomicUsize::new(0);

fn serial() -> usize {
    SERIAL.fetch_add(1, Ordering::SeqCst) + 1
}

/// A task as `testkit.ts` makes one, with `over` changing what it likes.
pub fn task(over: impl FnOnce(&mut Task)) -> Task {
    let n = serial();
    let mut t = Task {
        id: format!("task-{n}"),
        space_id: "classes".into(),
        title: format!("Task {n}"),
        r#type: "Homework".into(),
        course_id: None,
        project_id: None,
        milestone_id: None,
        group: None,
        parent_task_id: None,
        due: None,
        scheduled_date: None,
        rrule: None,
        difficulty: 3.0,
        est_min: None,
        adjust_min: 0.0,
        notes: String::new(),
        link: None,
        done: false,
        done_at: None,
        source: TaskSource::You,
    };
    over(&mut t);
    t
}

pub fn session(over: impl FnOnce(&mut FocusSession)) -> FocusSession {
    let n = serial();
    let mut s = FocusSession {
        id: format!("session-{n}"),
        task_id: None,
        habit_id: None,
        started_at: 0.0,
        ended_at: 0.0,
        focus_min: 25.0,
        interruptions: 0.0,
        room: Room::Heat,
    };
    over(&mut s);
    s
}

pub fn block(over: impl FnOnce(&mut TimeBlock)) -> TimeBlock {
    let n = serial();
    let mut b = TimeBlock {
        id: format!("block-{n}"),
        task_id: None,
        habit_id: None,
        date: "2026-10-06".into(),
        start: 9.0 * 60.0,
        minutes: 30.0,
        origin: Origin::You,
    };
    over(&mut b);
    b
}

pub fn habit(over: impl FnOnce(&mut Habit)) -> Habit {
    let n = serial();
    let mut h = Habit {
        id: format!("habit-{n}"),
        title: format!("Habit {n}"),
        minutes: None,
        log: Default::default(),
        show_counter: false,
    };
    over(&mut h);
    h
}

/// Ids for records a function makes: "id-1", "id-2", …
pub fn ids(prefix: &str) -> impl FnMut() -> String {
    let prefix = prefix.to_string();
    let mut n = 0;
    move || {
        n += 1;
        format!("{prefix}-{n}")
    }
}
