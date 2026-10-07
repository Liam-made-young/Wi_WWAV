//! `habits.test.ts`, case for case.

mod common;

use std::collections::BTreeMap;
use wi_heat::model::habits::{
    add_habit, done_record, habit_label, habit_line, last_fourteen, streak, today_orbs,
    toggle_habit, year_grid, AddHabit, DayDone, HABIT_LIMIT,
};
use wi_heat::model::records::Habit;
use wi_heat::model::zone::add_days;

use common::*;

const TODAY: &str = "2026-10-06";

fn log_of(days: &[&str]) -> BTreeMap<String, bool> {
    days.iter().map(|d| ((*d).to_string(), true)).collect()
}

fn log_from(days: Vec<String>) -> BTreeMap<String, bool> {
    days.into_iter().map(|d| (d, true)).collect()
}

fn with_log(log: BTreeMap<String, bool>) -> Habit {
    habit(|h| h.log = log)
}

// --- adding habits --------------------------------------------------------------------------

#[test]
fn keeps_the_limit_of_6_a_7th_doesnt_fit_and_says_why() {
    assert_eq!(HABIT_LIMIT, 6);
    let mut new_id = ids("habit");
    let mut habits: Vec<Habit> = vec![];
    for i in 0..6 {
        match add_habit(&habits, &format!("Habit {i}"), None, &mut new_id) {
            AddHabit::Added { habit } => habits.push(habit),
            AddHabit::Refused { error } => panic!("{error}"),
        }
    }
    assert_eq!(
        add_habit(&habits, "One more", None, &mut new_id),
        AddHabit::Refused {
            error: "Habit limit reached".into()
        }
    );
}

#[test]
fn makes_a_habit_with_the_counter_off_and_an_empty_log() {
    assert_eq!(
        add_habit(&[], "Practise kanji", Some(20.0), &mut ids("h")),
        AddHabit::Added {
            habit: Habit {
                id: "h-1".into(),
                title: "Practise kanji".into(),
                minutes: Some(20.0),
                log: BTreeMap::new(),
                show_counter: false,
                public: false
            }
        }
    );
}

#[test]
fn asks_for_a_name_first() {
    assert_eq!(
        add_habit(&[], "  ", None, &mut ids("id")),
        AddHabit::Refused {
            error: "Give the habit a name first.".into()
        }
    );
}

#[test]
fn writes_a_habit_with_a_length_as_practise_kanji_20m() {
    assert_eq!(
        habit_label(&habit(|h| {
            h.title = "Practise kanji".into();
            h.minutes = Some(20.0);
        })),
        "Practise kanji, 20m"
    );
    assert_eq!(
        habit_label(&habit(|h| h.title = "Stretch".into())),
        "Stretch"
    );
}

// --- the log -----------------------------------------------------------------------------------

#[test]
fn toggles_a_day_on_and_off() {
    let h = habit(|_| {});
    let on = toggle_habit(&h, TODAY);
    assert_eq!(on.log, log_of(&[TODAY]));
    assert_eq!(toggle_habit(&on, TODAY).log, BTreeMap::new());
}

#[test]
fn is_never_pruned_at_400_days() {
    let old: Vec<String> = (0..800)
        .map(|i| add_days(TODAY, f64::from(-i - 1)))
        .collect();
    let h = toggle_habit(&with_log(log_from(old)), TODAY);
    assert_eq!(h.log.len(), 801);
    assert_eq!(h.log.get(&add_days(TODAY, -800.0)), Some(&true));
}

// --- today's orbs ----------------------------------------------------------------------------------

#[test]
fn shows_each_habits_orb_for_today_and_3_of_5_done() {
    let habits: Vec<Habit> = [true, true, false, true, false]
        .iter()
        .enumerate()
        .map(|(i, done)| {
            habit(|h| {
                h.title = format!("h{i}");
                h.log = if *done {
                    log_of(&[TODAY])
                } else {
                    BTreeMap::new()
                };
            })
        })
        .collect();
    let z = ny_zone();
    let shown = today_orbs(&habits, ny("2026-10-06 21:00"), &z);
    assert_eq!(
        shown.orbs.iter().map(|o| o.done).collect::<Vec<_>>(),
        [true, true, false, true, false]
    );
    assert_eq!(shown.line.as_deref(), Some("3 of 5 done"));
    assert_eq!(today_orbs(&[], ny("2026-10-06 21:00"), &z).line, None);
}

#[test]
fn reads_today_in_the_persons_zone() {
    // 9 PM in New York is already October 7 in UTC.
    let h = with_log(log_of(&[TODAY]));
    assert!(today_orbs(&[h], ny("2026-10-06 21:00"), &ny_zone()).orbs[0].done);
}

// --- the grids ----------------------------------------------------------------------------------------

#[test]
fn draws_the_last_14_days_oldest_first_ending_today() {
    let h = with_log(log_of(&["2026-09-23", "2026-10-01", TODAY]));
    let days = last_fourteen(&h, TODAY);
    assert_eq!(days.len(), 14);
    assert_eq!(
        days[0],
        DayDone {
            date: "2026-09-23".into(),
            done: true
        }
    );
    assert_eq!(
        days[13],
        DayDone {
            date: TODAY.into(),
            done: true
        }
    );
    assert_eq!(days.iter().filter(|d| d.done).count(), 3);
}

#[test]
fn show_the_year_draws_53_weeks_of_7_days_from_sunday_ending_this_week_with_nothing_after_today() {
    let h = with_log(log_of(&["2026-09-23", "2026-10-01", TODAY]));
    let grid = year_grid(&h, TODAY);
    assert_eq!(grid.len(), 53);
    assert!(grid.iter().all(|week| week.len() == 7));
    // Today, a Tuesday, sits in the last column; the rest of its week is still to come.
    let last: Vec<Option<&str>> = grid[52]
        .iter()
        .map(|d| d.as_ref().map(|d| d.date.as_str()))
        .collect();
    assert_eq!(
        last,
        [
            Some("2026-10-04"),
            Some("2026-10-05"),
            Some(TODAY),
            None,
            None,
            None,
            None
        ]
    );
    assert_eq!(
        grid[52][2],
        Some(DayDone {
            date: TODAY.into(),
            done: true
        })
    );
    // The first column starts on the Sunday 52 weeks before this week's.
    assert_eq!(
        grid[0][0],
        Some(DayDone {
            date: "2025-10-05".into(),
            done: false
        })
    );
}

// --- the record and the counter (Open #9) ----------------------------------------------------------------

#[test]
fn shows_the_growing_record_by_default_done_41_days_since_august_26() {
    let since: Vec<String> = (0..41)
        .map(|i| add_days("2026-08-26", f64::from(i)))
        .collect();
    let h = with_log(log_from(since));
    assert!(!h.show_counter);
    assert_eq!(
        habit_line(&h, TODAY).as_deref(),
        Some("Done 41 days since August 26")
    );
    assert_eq!(
        done_record(&with_log(log_of(&[TODAY])), TODAY),
        "Done 1 day since October 6"
    );
    assert_eq!(
        done_record(&with_log(log_of(&["2025-08-26", TODAY])), TODAY),
        "Done 2 days since August 26, 2025"
    );
    assert_eq!(done_record(&habit(|_| {}), TODAY), "Not done yet");
}

#[test]
fn shows_the_streak_counter_only_when_switched_on_for_that_habit() {
    let since: Vec<String> = (0..41)
        .map(|i| add_days("2026-08-26", f64::from(i)))
        .collect();
    let mut h = with_log(log_from(since));
    h.show_counter = true;
    assert_eq!(
        habit_line(&h, "2026-10-05").as_deref(),
        Some("41-day streak")
    );
}

#[test]
fn keeps_a_streak_alive_until_midnight() {
    let run = log_of(&["2026-10-03", "2026-10-04", "2026-10-05"]);
    // Not done yet today: the streak still stands on yesterday's.
    assert_eq!(streak(&with_log(run.clone()), TODAY), 3.0);
    let mut with_today = run;
    with_today.insert(TODAY.into(), true);
    assert_eq!(streak(&with_log(with_today), TODAY), 4.0);
    // Missed yesterday: it is gone.
    assert_eq!(
        streak(&with_log(log_of(&["2026-10-03", "2026-10-04"])), TODAY),
        0.0
    );
    let counting = habit(|h| {
        h.show_counter = true;
        h.log = log_of(&["2026-10-01"]);
    });
    assert_eq!(habit_line(&counting, TODAY), None);
}
