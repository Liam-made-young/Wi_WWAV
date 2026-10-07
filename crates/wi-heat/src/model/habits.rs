//! Habits (`docs/SPEC.md` 3.9). Up to 6, each with an orb for today, a 14-day
//! grid, and "Show the year". The log is a set of day keys and is never
//! pruned. By default a habit shows a record that only grows ("Done 41 days
//! since August 26"); the streak counter is a per-habit setting, off by
//! default (Open #9), and a streak survives until midnight. A port of
//! `habits.ts`.

use super::estimate::format_minutes;
use super::records::{Habit, Id};
use super::zone::{add_days, day_key, key_parts, weekday_of, DayKey};
use super::{copy, format, js};
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub const HABIT_LIMIT: usize = 6;

/// A new habit, or the sentence that says why there isn't one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AddHabit {
    Added { habit: Habit },
    Refused { error: String },
}

pub fn add_habit(
    habits: &[Habit],
    title: &str,
    minutes: Option<f64>,
    new_id: &mut dyn FnMut() -> Id,
) -> AddHabit {
    if habits.len() >= HABIT_LIMIT {
        return AddHabit::Refused {
            error: copy::habits::LIMIT.to_string(),
        };
    }
    let title = js::trim(title);
    if title.is_empty() {
        return AddHabit::Refused {
            error: copy::habits::NAME_FIRST.to_string(),
        };
    }
    AddHabit::Added {
        habit: Habit {
            id: new_id(),
            title: title.to_string(),
            minutes,
            log: Default::default(),
            show_counter: false,
        },
    }
}

/// "Practise kanji, 20m"
pub fn habit_label(h: &Habit) -> String {
    match h.minutes {
        None => h.title.clone(),
        Some(m) => format!("{}, {}", h.title, format_minutes(m)),
    }
}

fn is_done(h: &Habit, day: &str) -> bool {
    h.log.get(day).copied().unwrap_or(false)
}

/// Ticks a day without unticking it: how a focus session's `tickHabit` effect is applied.
pub fn mark_habit_done(h: &Habit, day: &str) -> Habit {
    if is_done(h, day) {
        return h.clone();
    }
    let mut out = h.clone();
    out.log.insert(day.to_string(), true);
    out
}

/// A click on an orb.
pub fn toggle_habit(h: &Habit, day: &str) -> Habit {
    let mut out = h.clone();
    if is_done(h, day) {
        out.log.remove(day);
    } else {
        out.log.insert(day.to_string(), true);
    }
    out
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Orb {
    pub habit: Habit,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TodayOrbs {
    pub orbs: Vec<Orb>,
    pub line: Option<String>,
}

/// Today's orbs and "3 of 5 done".
pub fn today_orbs(habits: &[Habit], now: f64, tz: &TimeZone) -> TodayOrbs {
    let today = day_key(now, tz);
    let orbs: Vec<Orb> = habits
        .iter()
        .map(|habit| Orb {
            habit: habit.clone(),
            done: is_done(habit, &today),
        })
        .collect();
    let done = orbs.iter().filter(|o| o.done).count();
    TodayOrbs {
        line: (!habits.is_empty()).then(|| copy::habits::of_done(done as f64, habits.len() as f64)),
        orbs,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DayDone {
    pub date: DayKey,
    pub done: bool,
}

/// The 14-day grid, oldest first, ending today.
pub fn last_fourteen(h: &Habit, today: &str) -> Vec<DayDone> {
    (0..14)
        .map(|i| {
            let date = add_days(today, f64::from(i) - 13.0);
            let done = is_done(h, &date);
            DayDone { date, done }
        })
        .collect()
}

/// "Show the year": 53 weeks of 7 days from Sunday, ending with this week; days still to come are None.
pub fn year_grid(h: &Habit, today: &str) -> Vec<Vec<Option<DayDone>>> {
    let first = add_days(today, -weekday_of(today) - 52.0 * 7.0);
    (0..53)
        .map(|w| {
            (0..7)
                .map(|d| {
                    let date = add_days(&first, f64::from(w * 7 + d));
                    if js::cmp(&date, today) == Ordering::Greater {
                        None
                    } else {
                        let done = is_done(h, &date);
                        Some(DayDone { date, done })
                    }
                })
                .collect()
        })
        .collect()
}

/// The record that only grows: "Done 41 days since August 26".
pub fn done_record(h: &Habit, today: &str) -> String {
    let days = js::sort_by(
        h.log
            .iter()
            .filter(|(_, done)| **done)
            .map(|(d, _)| d.as_str())
            .collect::<Vec<_>>(),
        |a, b| js::cmp(a, b),
    );
    let Some(first) = days.first() else {
        return copy::habits::NOT_YET.to_string();
    };
    let year = key_parts(first).0;
    let since = if year == key_parts(today).0 {
        format::month_day(first)
    } else {
        format!("{}, {}", format::month_day(first), js::num_to_string(year))
    };
    copy::habits::record(days.len() as f64, &since)
}

/// Days in a row, ending today or, until midnight, yesterday.
pub fn streak(h: &Habit, today: &str) -> f64 {
    let mut day = if is_done(h, today) {
        today.to_string()
    } else {
        add_days(today, -1.0)
    };
    let mut n = 0.0;
    while is_done(h, &day) {
        n += 1.0;
        day = add_days(&day, -1.0);
    }
    n
}

/// The line under a habit: its counter when switched on (and above 0), else its record.
pub fn habit_line(h: &Habit, today: &str) -> Option<String> {
    if !h.show_counter {
        return Some(done_record(h, today));
    }
    let n = streak(h, today);
    (n > 0.0).then(|| copy::habits::streak(n))
}
