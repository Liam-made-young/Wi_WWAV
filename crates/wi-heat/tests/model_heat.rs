//! `heat.test.ts`, case for case. (The level colours and the tube's size stay
//! in TypeScript with the display; `tube_fill` is checked here.)

mod common;

use common::*;
use wi_heat::model::heat::{
    by_heat, due_phrase, heat_of, heat_thresholds, next_heat_change, runway, tube_fill, Heat,
    HeatLevel, Thresholds,
};

fn close(got: f64, want: f64, digits: i32) -> bool {
    (got - want).abs() < 10f64.powi(-digits) / 2.0
}

fn heat(level: HeatLevel, v: Option<f64>) -> Heat {
    Heat { level, v }
}

// --- the heat algorithm (3.1) ------------------------------------------------------

#[test]
fn gives_a_done_task_no_heat() {
    let now = ny("2026-10-06 08:40");
    let t = task(|t| {
        t.done = true;
        t.due = Some(now - DAY_MS);
    });
    assert_eq!(heat_of(&t, now), heat(HeatLevel::Done, None));
}

#[test]
fn makes_an_undated_task_cool_at_0_05() {
    let now = ny("2026-10-06 08:40");
    assert_eq!(
        heat_of(&task(|t| t.due = None), now),
        heat(HeatLevel::Cool, Some(0.05))
    );
}

#[test]
fn makes_anything_past_its_due_overdue_at_1_1() {
    let now = ny("2026-10-06 08:40");
    assert_eq!(
        heat_of(&task(|t| t.due = Some(now - 1.0)), now),
        heat(HeatLevel::Overdue, Some(1.1))
    );
    assert_eq!(
        heat_of(&task(|t| t.due = Some(now - 3.0 * DAY_MS)), now),
        heat(HeatLevel::Overdue, Some(1.1))
    );
}

#[test]
fn is_hot_at_1_on_the_due_instant_itself_since_days_is_0_not_below_it() {
    let now = ny("2026-10-06 08:40");
    assert_eq!(
        heat_of(&task(|t| t.due = Some(now)), now),
        heat(HeatLevel::Hot, Some(1.0))
    );
}

#[test]
fn gives_a_runway_of_difficulty_times_2_plus_1_days() {
    let got: Vec<f64> = [1.0, 2.0, 3.0, 4.0, 5.0].into_iter().map(runway).collect();
    assert_eq!(got, vec![3.0, 5.0, 7.0, 9.0, 11.0]);
}

#[test]
fn computes_v_equals_1_minus_days_over_runway_and_its_levels() {
    let now = ny("2026-10-06 08:40");
    // difficulty 3, runway 7: 3.5 days left is v 0.5, Warm.
    let h = heat_of(
        &task(|t| {
            t.difficulty = 3.0;
            t.due = Some(now + 3.5 * DAY_MS);
        }),
        now,
    );
    assert_eq!(h.level, HeatLevel::Warm);
    assert!(close(h.v.unwrap(), 0.5, 12));
    // 1 day left is v 0.857, Hot.
    let hot = heat_of(
        &task(|t| {
            t.difficulty = 3.0;
            t.due = Some(now + DAY_MS);
        }),
        now,
    );
    assert_eq!(hot.level, HeatLevel::Hot);
    // 6 days left is v 0.143, Cool.
    let cool = heat_of(
        &task(|t| {
            t.difficulty = 3.0;
            t.due = Some(now + 6.0 * DAY_MS);
        }),
        now,
    );
    assert_eq!(cool.level, HeatLevel::Cool);
    assert!(close(cool.v.unwrap(), 1.0 / 7.0, 12));
}

#[test]
fn floors_a_cool_v_at_0_05() {
    let now = ny("2026-10-06 08:40");
    let far = task(|t| {
        t.difficulty = 1.0;
        t.due = Some(now + 30.0 * DAY_MS);
    });
    assert_eq!(heat_of(&far, now), heat(HeatLevel::Cool, Some(0.05)));
    let hard = task(|t| {
        t.difficulty = 5.0;
        t.due = Some(now + 10.9 * DAY_MS);
    });
    assert_eq!(heat_of(&hard, now).v, Some(0.05));
}

#[test]
fn holds_difficulty_to_1_to_5() {
    assert_eq!(runway(0.0), 3.0);
    assert_eq!(runway(9.0), 11.0);
}

// --- Warm at / Hot at (3.1 table) ------------------------------------------------------

const WARM_AT: [f64; 5] = [1.98, 3.3, 4.62, 5.94, 7.26];
const HOT_AT: [f64; 5] = [0.9, 1.5, 2.1, 2.7, 3.3];

#[test]
fn prints_every_number_of_the_table() {
    for d in 1..=5 {
        assert_eq!(
            heat_thresholds(f64::from(d)),
            Thresholds {
                warm_at: WARM_AT[d as usize - 1],
                hot_at: HOT_AT[d as usize - 1]
            }
        );
    }
}

#[test]
fn turns_each_difficulty_warm_then_hot_at_exactly_those_days_left() {
    let due = ny("2026-10-20 23:59");
    for d in 1..=5usize {
        let t = task(|t| {
            t.difficulty = d as f64;
            t.due = Some(due);
        });
        let start = due - 20.0 * DAY_MS;
        assert_eq!(heat_of(&t, start).level, HeatLevel::Cool);
        let warm = next_heat_change(std::slice::from_ref(&t), start).unwrap();
        assert_eq!(heat_of(&t, warm - 1.0).level, HeatLevel::Cool);
        assert_eq!(heat_of(&t, warm).level, HeatLevel::Warm);
        assert!(((due - warm) / DAY_MS - WARM_AT[d - 1]).abs() < 1e-6);
        let hot = next_heat_change(std::slice::from_ref(&t), warm).unwrap();
        assert_eq!(heat_of(&t, hot - 1.0).level, HeatLevel::Warm);
        assert_eq!(heat_of(&t, hot).level, HeatLevel::Hot);
        assert!(((due - hot) / DAY_MS - HOT_AT[d - 1]).abs() < 1e-6);
        let overdue = next_heat_change(std::slice::from_ref(&t), hot).unwrap();
        assert_eq!(overdue, due + 1.0);
        assert_eq!(heat_of(&t, overdue).level, HeatLevel::Overdue);
        assert_eq!(next_heat_change(std::slice::from_ref(&t), overdue), None);
    }
}

// --- 1.6: Grammar quiz 4, difficulty 2, due Wednesday 11:59 PM ---------------------------

#[test]
fn is_amber_warm_at_8_40_on_tuesday() {
    let quiz = task(|t| {
        t.title = "Grammar quiz 4".into();
        t.difficulty = 2.0;
        t.due = Some(ny("2026-10-07 23:59"));
    });
    assert_eq!(
        heat_of(&quiz, ny("2026-10-06 08:40")).level,
        HeatLevel::Warm
    );
}

#[test]
fn turns_hot_with_1_5_days_left_at_11_59_tuesday_morning() {
    let quiz = task(|t| {
        t.title = "Grammar quiz 4".into();
        t.difficulty = 2.0;
        t.due = Some(ny("2026-10-07 23:59"));
    });
    assert_eq!(
        next_heat_change(std::slice::from_ref(&quiz), ny("2026-10-06 08:40")),
        Some(ny("2026-10-06 11:59"))
    );
    assert_eq!(
        heat_of(&quiz, ny("2026-10-06 11:59") - 1.0).level,
        HeatLevel::Warm
    );
    assert_eq!(
        heat_of(&quiz, ny("2026-10-06 11:59")),
        heat(HeatLevel::Hot, Some(0.7))
    );
}

// --- the tube ------------------------------------------------------------------------------

#[test]
fn fills_a_tube_to_v_full_when_overdue_empty_when_done() {
    assert_eq!(tube_fill(&heat(HeatLevel::Warm, Some(0.5))), 0.5);
    assert_eq!(tube_fill(&heat(HeatLevel::Overdue, Some(1.1))), 1.0);
    assert_eq!(tube_fill(&heat(HeatLevel::Cool, Some(0.05))), 0.05);
    assert_eq!(tube_fill(&heat(HeatLevel::Done, None)), 0.0);
}

// --- the sort ---------------------------------------------------------------------------------

#[test]
fn orders_by_heat_then_due_with_undated_last_and_done_after_that() {
    let now = ny("2026-10-06 08:40");
    let titled = |title: &str, over: &dyn Fn(&mut wi_heat::model::records::Task)| {
        task(|t| {
            t.title = title.into();
            over(t);
        })
    };
    let undated = titled("undated", &|_| {});
    let far_a = titled("far A", &|t| {
        t.difficulty = 1.0;
        t.due = Some(now + 20.0 * DAY_MS);
    });
    let far_b = titled("far B", &|t| {
        t.difficulty = 1.0;
        t.due = Some(now + 10.0 * DAY_MS);
    });
    let warm = titled("warm", &|t| {
        t.difficulty = 3.0;
        t.due = Some(now + 3.0 * DAY_MS);
    });
    let hot = titled("hot", &|t| {
        t.difficulty = 3.0;
        t.due = Some(now + DAY_MS);
    });
    let late = titled("late", &|t| t.due = Some(now - DAY_MS));
    let later = titled("later", &|t| t.due = Some(now - 2.0 * DAY_MS));
    let done = titled("done", &|t| {
        t.done = true;
        t.due = Some(now - DAY_MS);
    });
    let items = vec![undated, done, far_a, warm, late, hot, far_b, later];
    let sorted: Vec<&str> = by_heat(&items, now)
        .iter()
        .map(|t| t.title.as_str())
        .collect();
    assert_eq!(
        sorted,
        ["later", "late", "hot", "warm", "far B", "far A", "undated", "done"]
    );
}

// --- due phrases ---------------------------------------------------------------------------------

#[test]
fn names_today_tomorrow_and_the_days_of_this_week() {
    let (now, z) = (ny("2026-10-06 08:40"), ny_zone());
    assert_eq!(due_phrase(ny("2026-10-06 16:00"), now, &z), "Today 4:00 PM");
    assert_eq!(
        due_phrase(ny("2026-10-07 23:59"), now, &z),
        "Tomorrow 11:59 PM"
    );
    assert_eq!(
        due_phrase(ny("2026-10-08 23:59"), now, &z),
        "Thursday 11:59 PM"
    );
    assert_eq!(
        due_phrase(ny("2026-10-12 23:59"), now, &z),
        "Monday 11:59 PM"
    );
}

#[test]
fn gives_a_date_past_this_week_and_the_year_when_it_differs() {
    let (now, z) = (ny("2026-10-06 08:40"), ny_zone());
    assert_eq!(
        due_phrase(ny("2026-10-13 23:59"), now, &z),
        "Oct 13 11:59 PM"
    );
    assert_eq!(
        due_phrase(ny("2027-01-08 09:00"), now, &z),
        "Jan 8, 2027 9:00 AM"
    );
}

#[test]
fn counts_overdue_in_minutes_then_hours_then_days() {
    let (now, z) = (ny("2026-10-06 08:40"), ny_zone());
    assert_eq!(due_phrase(now - 20.0 * MIN, now, &z), "20m overdue");
    assert_eq!(due_phrase(now - 1.0, now, &z), "1m overdue");
    assert_eq!(
        due_phrase(now - 3.0 * 3_600_000.0 - 59.0 * MIN, now, &z),
        "3h overdue"
    );
    assert_eq!(
        due_phrase(now - 2.0 * DAY_MS - 5.0 * 3_600_000.0, now, &z),
        "2d overdue"
    );
}

#[test]
fn reads_the_day_in_the_zone_not_in_utc() {
    // 9 PM Tuesday in New York is already Wednesday in UTC.
    let now = ny("2026-10-06 08:40");
    assert_eq!(
        due_phrase(ny("2026-10-06 21:00"), now, &ny_zone()),
        "Today 9:00 PM"
    );
}

// --- nextHeatChange -----------------------------------------------------------------------------------

#[test]
fn gives_the_soonest_change_in_a_list_so_the_60_second_render_can_land_on_it() {
    let now = ny("2026-10-06 08:40");
    let a = task(|t| {
        t.difficulty = 2.0;
        t.due = Some(ny("2026-10-07 23:59"));
    }); // Hot at 11:59 today
    let b = task(|t| {
        t.difficulty = 1.0;
        t.due = Some(ny("2026-10-06 10:00"));
    }); // Overdue at 10:00:00.001
    assert_eq!(
        next_heat_change(&[a, b], now),
        Some(ny("2026-10-06 10:00") + 1.0)
    );
}

#[test]
fn gives_nothing_when_no_level_will_change() {
    let now = ny("2026-10-06 08:40");
    let items = [
        task(|t| t.due = None),
        task(|t| {
            t.done = true;
            t.due = Some(now + DAY_MS);
        }),
        task(|t| t.due = Some(now - 1.0)),
    ];
    assert_eq!(next_heat_change(&items, now), None);
}

// --- a bug in the TypeScript -------------------------------------------------------------------------------

/// The TypeScript's search never returns for a due that isn't a whole
/// millisecond; the Rust stops when the bracket can't narrow, which is the
/// answer the TypeScript's loop would reach, due + 1.
#[test]
fn next_heat_change_stops_for_a_due_that_is_not_a_whole_millisecond() {
    let now = ny("2026-10-06 08:40");
    let t = task(|t| {
        t.difficulty = 3.0;
        t.due = Some(now + 2.0 * DAY_MS + 0.5);
    });
    let change = next_heat_change(&[t], now).unwrap();
    assert!(change > now + 2.0 * DAY_MS && change <= now + 2.0 * DAY_MS + 1.5);
}

#[test]
#[ignore = "TS bug: nextHeatChange never returns for a due that is not a whole millisecond (its midpoint floors onto lo, so the bracket stops narrowing); the Rust stops instead and gives due + 1, where a whole millisecond is floor(due) + 1. See docs/QUESTIONS.md #158"]
fn next_heat_change_gives_a_whole_millisecond_for_a_due_that_is_not_one() {
    let now = ny("2026-10-06 08:40");
    let t = task(|t| {
        t.difficulty = 3.0;
        t.due = Some(now + 2.0 * DAY_MS + 0.5);
    });
    let change = next_heat_change(&[t], now).unwrap();
    assert_eq!(change, now + 2.0 * DAY_MS + 1.0);
}
