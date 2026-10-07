//! `recurrence.test.ts`, case for case. (The Repeat menu's preset labels stay
//! in TypeScript with the display, and so does their test.)

mod common;

use common::*;
use wi_heat::model::heat::{heat_of, HeatLevel};
use wi_heat::model::records::{Task, TaskOccurrence};
use wi_heat::model::recurrence::{
    check_occurrence, next_open_occurrence, occurrences_between, open_tasks, parse_rule,
    reopen_occurrence, with_effective_due, Freq, Rule, SeriesStart, Until,
};

fn utc(text: &str) -> f64 {
    text.parse::<jiff::Timestamp>()
        .expect("an instant")
        .as_millisecond() as f64
}

// The series' first occurrence: a day and a minute of that day, in New York.
fn dates(rrule: &str, day: &str, minute: f64, from: &str, to: &str) -> Vec<String> {
    let start = SeriesStart {
        day: day.into(),
        minute,
    };
    occurrences_between(
        &parse_rule(rrule).expect("a rule"),
        &start,
        &ny_zone(),
        from,
        to,
    )
    .into_iter()
    .map(|o| o.date)
    .collect()
}

fn rule(freq: Freq, interval: f64, by_day: &[i32], by_month_day: &[i32]) -> Rule {
    Rule {
        freq,
        interval,
        by_day: by_day.to_vec(),
        by_month_day: by_month_day.to_vec(),
        count: None,
        until: None,
    }
}

// --- parsing the RRULE subset -------------------------------------------------------------

#[test]
fn reads_freq_interval_byday_bymonthday_count_and_until() {
    assert_eq!(
        parse_rule("FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR"),
        Some(rule(Freq::Weekly, 1.0, &[1, 2, 3, 4, 5], &[]))
    );
    assert_eq!(
        parse_rule("RRULE:FREQ=MONTHLY;INTERVAL=2;BYMONTHDAY=1,-1;COUNT=6"),
        Some(Rule {
            count: Some(6.0),
            ..rule(Freq::Monthly, 2.0, &[], &[1, -1])
        })
    );
    assert_eq!(
        parse_rule("FREQ=DAILY;UNTIL=20261231T235959Z")
            .unwrap()
            .until,
        Some(Until::Instant {
            instant: utc("2026-12-31T23:59:59Z")
        })
    );
    assert_eq!(
        parse_rule("FREQ=DAILY;UNTIL=20261231").unwrap().until,
        Some(Until::Day {
            day: "2026-12-31".into()
        })
    );
}

#[test]
fn refuses_what_heat_does_not_handle_rather_than_guessing() {
    for bad in [
        "",
        "nonsense",
        "FREQ=HOURLY",
        "FREQ=MONTHLY;BYDAY=2TU",
        "FREQ=WEEKLY;BYSETPOS=1",
        "FREQ=WEEKLY;BYMONTHDAY=3",
        "FREQ=DAILY;INTERVAL=0",
        "FREQ=DAILY;COUNT=2;UNTIL=20261231",
        "FREQ=MONTHLY;BYMONTHDAY=32",
        "INTERVAL=2",
    ] {
        assert_eq!(parse_rule(bad), None, "{bad}");
    }
}

// --- expanding occurrences --------------------------------------------------------------------

#[test]
fn repeats_daily_and_every_other_day_up_to_a_count() {
    assert_eq!(
        dates("FREQ=DAILY", "2026-10-06", 0.0, "2026-10-06", "2026-10-09"),
        ["2026-10-06", "2026-10-07", "2026-10-08", "2026-10-09"]
    );
    assert_eq!(
        dates(
            "FREQ=DAILY;INTERVAL=2;COUNT=3",
            "2026-10-06",
            0.0,
            "2026-10-01",
            "2026-12-31"
        ),
        ["2026-10-06", "2026-10-08", "2026-10-10"]
    );
}

#[test]
fn counts_count_from_the_start_of_the_series_not_the_window() {
    assert_eq!(
        dates(
            "FREQ=DAILY;COUNT=5",
            "2026-10-06",
            0.0,
            "2026-10-09",
            "2026-10-20"
        ),
        ["2026-10-09", "2026-10-10"]
    );
}

#[test]
fn repeats_weekly_on_the_starts_weekday_or_on_byday_never_before_the_start() {
    assert_eq!(
        dates("FREQ=WEEKLY", "2026-10-06", 0.0, "2026-10-01", "2026-10-20"),
        ["2026-10-06", "2026-10-13", "2026-10-20"]
    );
    assert_eq!(
        dates(
            "FREQ=WEEKLY;BYDAY=MO,WE",
            "2026-10-06",
            0.0,
            "2026-10-01",
            "2026-10-14"
        ),
        ["2026-10-07", "2026-10-12", "2026-10-14"]
    );
}

#[test]
fn counts_interval_in_weeks_that_start_on_monday() {
    assert_eq!(
        dates(
            "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH",
            "2026-10-06",
            0.0,
            "2026-10-01",
            "2026-10-31"
        ),
        ["2026-10-06", "2026-10-08", "2026-10-20", "2026-10-22"]
    );
}

#[test]
fn gives_every_weekday_monday_to_friday() {
    assert_eq!(
        dates(
            "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR",
            "2026-10-09",
            0.0,
            "2026-10-09",
            "2026-10-19"
        ),
        [
            "2026-10-09",
            "2026-10-12",
            "2026-10-13",
            "2026-10-14",
            "2026-10-15",
            "2026-10-16",
            "2026-10-19"
        ]
    );
}

#[test]
fn repeats_monthly_on_the_starts_day_skipping_months_without_it() {
    assert_eq!(
        dates(
            "FREQ=MONTHLY",
            "2026-01-31",
            0.0,
            "2026-01-01",
            "2026-06-30"
        ),
        ["2026-01-31", "2026-03-31", "2026-05-31"]
    );
}

#[test]
fn takes_bymonthday_counting_negatives_from_the_months_end() {
    assert_eq!(
        dates(
            "FREQ=MONTHLY;BYMONTHDAY=1,15",
            "2026-10-06",
            0.0,
            "2026-10-01",
            "2026-11-30"
        ),
        ["2026-10-15", "2026-11-01", "2026-11-15"]
    );
    assert_eq!(
        dates(
            "FREQ=MONTHLY;BYMONTHDAY=-1",
            "2026-01-10",
            0.0,
            "2026-01-01",
            "2026-04-30"
        ),
        ["2026-01-31", "2026-02-28", "2026-03-31", "2026-04-30"]
    );
}

#[test]
fn takes_byday_and_bymonthday_together_as_both() {
    assert_eq!(
        dates(
            "FREQ=MONTHLY;BYDAY=FR;BYMONTHDAY=13",
            "2026-01-01",
            0.0,
            "2026-01-01",
            "2026-12-31"
        ),
        ["2026-02-13", "2026-03-13", "2026-11-13"]
    );
}

#[test]
fn repeats_yearly_with_february_29_only_in_leap_years() {
    assert_eq!(
        dates("FREQ=YEARLY", "2026-10-06", 0.0, "2026-01-01", "2028-12-31"),
        ["2026-10-06", "2027-10-06", "2028-10-06"]
    );
    assert_eq!(
        dates("FREQ=YEARLY", "2028-02-29", 0.0, "2028-01-01", "2032-12-31"),
        ["2028-02-29", "2032-02-29"]
    );
}

#[test]
fn stops_at_until_a_day_or_an_instant_inclusive() {
    assert_eq!(
        dates(
            "FREQ=DAILY;UNTIL=20261008",
            "2026-10-06",
            23.0 * 60.0 + 59.0,
            "2026-10-01",
            "2026-10-31"
        ),
        ["2026-10-06", "2026-10-07", "2026-10-08"]
    );
    // 03:59 UTC on October 8 is 11:59 PM on October 7 in New York.
    assert_eq!(
        dates(
            "FREQ=DAILY;UNTIL=20261008T035900Z",
            "2026-10-06",
            23.0 * 60.0 + 59.0,
            "2026-10-01",
            "2026-10-31"
        ),
        ["2026-10-06", "2026-10-07"]
    );
}

#[test]
fn keeps_the_wall_clock_time_across_dst() {
    let start = SeriesStart {
        day: "2026-10-23".into(),
        minute: 23.0 * 60.0 + 59.0,
    };
    let fridays = occurrences_between(
        &parse_rule("FREQ=WEEKLY").unwrap(),
        &start,
        &ny_zone(),
        "2026-10-01",
        "2026-11-13",
    );
    let at: Vec<f64> = fridays.iter().map(|o| o.at).collect();
    assert_eq!(
        at,
        [
            ny("2026-10-23 23:59"),
            ny("2026-10-30 23:59"),
            ny("2026-11-06 23:59"),
            ny("2026-11-13 23:59")
        ]
    );
    // Across November 1 the gap is a week and an hour.
    assert_eq!(
        fridays[2].at - fridays[1].at,
        7.0 * 86_400_000.0 + 3_600_000.0
    );
}

#[test]
fn moves_a_time_inside_the_spring_gap_forward_by_the_gap() {
    let start = SeriesStart {
        day: "2026-03-07".into(),
        minute: 2.0 * 60.0 + 30.0,
    };
    let days = occurrences_between(
        &parse_rule("FREQ=DAILY").unwrap(),
        &start,
        &ny_zone(),
        "2026-03-07",
        "2026-03-09",
    );
    let at: Vec<f64> = days.iter().map(|o| o.at).collect();
    assert_eq!(
        at,
        [
            ny("2026-03-07 02:30"),
            ny("2026-03-08 03:30"),
            ny("2026-03-09 02:30")
        ]
    );
}

// --- recurring tasks ------------------------------------------------------------------------------

// A quiz every Friday at 11:59 PM, from October 2.
fn weekly_quiz() -> Task {
    task(|t| {
        t.title = "Weekly quiz".into();
        t.difficulty = 2.0;
        t.due = Some(ny("2026-10-02 23:59"));
        t.rrule = Some("FREQ=WEEKLY".into());
    })
}

#[test]
fn recurring_tasks_take_their_heat_from_the_next_occurrence() {
    let quiz = weekly_quiz();
    let z = ny_zone();
    let now = ny("2026-10-08 08:00");
    let next = next_open_occurrence(&quiz, &[], now, &z).unwrap();
    assert_eq!(
        (next.date.as_str(), next.at),
        ("2026-10-09", ny("2026-10-09 23:59"))
    );
    let effective = with_effective_due(&quiz, &[], now, &z);
    assert_eq!(effective.due, Some(ny("2026-10-09 23:59")));
    // Not Overdue from October 2: 1.67 days left at difficulty 2 is Warm.
    assert_eq!(heat_of(&quiz, now).level, HeatLevel::Overdue);
    assert_eq!(heat_of(&effective, now).level, HeatLevel::Warm);
}

#[test]
fn recurring_tasks_skip_a_finished_occurrence_and_leave_past_days_behind() {
    let quiz = weekly_quiz();
    let z = ny_zone();
    let done = vec![TaskOccurrence {
        id: "o1".into(),
        task_id: quiz.id.clone(),
        date: "2026-10-09".into(),
        done_at: ny("2026-10-09 20:00"),
    }];
    assert_eq!(
        next_open_occurrence(&quiz, &done, ny("2026-10-08 08:00"), &z)
            .unwrap()
            .date,
        "2026-10-16"
    );
    // October 9 went by unticked; on the 12th the next is the 16th, not a week overdue.
    assert_eq!(
        next_open_occurrence(&quiz, &[], ny("2026-10-12 08:00"), &z)
            .unwrap()
            .date,
        "2026-10-16"
    );
}

#[test]
fn recurring_tasks_keep_todays_occurrence_until_midnight_overdue_once_its_time_passes() {
    let four_pm = task(|t| {
        t.due = Some(ny("2026-10-05 16:00"));
        t.rrule = Some("FREQ=DAILY".into());
    });
    let now = ny("2026-10-06 17:00");
    let effective = with_effective_due(&four_pm, &[], now, &ny_zone());
    assert_eq!(effective.due, Some(ny("2026-10-06 16:00")));
    assert_eq!(heat_of(&effective, now).level, HeatLevel::Overdue);
}

#[test]
fn recurring_tasks_end_with_the_series() {
    let three = task(|t| {
        t.due = Some(ny("2026-10-01 09:00"));
        t.rrule = Some("FREQ=DAILY;COUNT=3".into());
    });
    let z = ny_zone();
    assert_eq!(
        next_open_occurrence(&three, &[], ny("2026-10-06 08:00"), &z),
        None
    );
    assert_eq!(
        with_effective_due(&three, &[], ny("2026-10-06 08:00"), &z).due,
        None
    );
}

#[test]
fn recurring_tasks_tick_as_rows_of_their_own_and_the_series_never_flips_to_done() {
    let quiz = weekly_quiz();
    let z = ny_zone();
    let now = ny("2026-10-08 08:00");
    let mut new_id = ids("occ");
    let first = check_occurrence(&quiz, &[], now, &z, &mut new_id).unwrap();
    assert_eq!(
        first,
        TaskOccurrence {
            id: "occ-1".into(),
            task_id: quiz.id.clone(),
            date: "2026-10-09".into(),
            done_at: now
        }
    );
    let second =
        check_occurrence(&quiz, std::slice::from_ref(&first), now, &z, &mut new_id).unwrap();
    assert_eq!(second.date, "2026-10-16");
    assert!(!quiz.done);
    assert!(!with_effective_due(&quiz, &[first.clone(), second.clone()], now, &z).done);
    assert_eq!(
        reopen_occurrence(&[first, second.clone()], &quiz.id, "2026-10-09"),
        vec![second]
    );
}

#[test]
fn recurring_tasks_leave_a_task_without_a_rule_as_it_is() {
    let plain = task(|t| t.due = Some(ny("2026-10-09 23:59")));
    assert_eq!(
        with_effective_due(&plain, &[], ny("2026-10-06 08:00"), &ny_zone()),
        plain
    );
}

#[test]
fn recurring_tasks_run_a_series_without_a_due_date_from_its_scheduled_date() {
    let scales = task(|t| {
        t.scheduled_date = Some("2026-10-05".into());
        t.rrule = Some("FREQ=DAILY".into());
    });
    let z = ny_zone();
    let next = next_open_occurrence(&scales, &[], ny("2026-10-06 08:00"), &z).unwrap();
    assert_eq!(
        (next.date.as_str(), next.at),
        ("2026-10-06", ny("2026-10-06 00:00"))
    );
    assert_eq!(
        with_effective_due(&scales, &[], ny("2026-10-06 08:00"), &z).due,
        None
    );
}

// --- openTasks -----------------------------------------------------------------------------------------

#[test]
fn keeps_a_task_whose_rule_heat_cant_read_as_a_plain_task_rather_than_losing_it() {
    let now = ny("2026-10-06 08:00");
    let odd = task(|t| {
        t.due = Some(ny("2026-10-09 23:59"));
        t.rrule = Some("FREQ=HOURLY".into());
    });
    let z = ny_zone();
    assert_eq!(
        open_tasks(std::slice::from_ref(&odd), &[], now, &z),
        vec![odd.clone()]
    );
    assert_eq!(with_effective_due(&odd, &[], now, &z), odd);
}

#[test]
fn drops_done_tasks_and_ended_series_and_moves_a_series_due_to_its_next_occurrence() {
    let now = ny("2026-10-06 08:00");
    let plain = task(|t| t.due = Some(ny("2026-10-09 23:59")));
    let done = task(|t| {
        t.done = true;
        t.done_at = Some(now);
    });
    let ended = task(|t| {
        t.due = Some(ny("2026-10-01 09:00"));
        t.rrule = Some("FREQ=DAILY;COUNT=3".into());
    });
    let weekly = task(|t| {
        t.due = Some(ny("2026-10-02 23:59"));
        t.rrule = Some("FREQ=WEEKLY".into());
    });
    let all = [plain.clone(), done, ended, weekly.clone()];
    let open = open_tasks(&all, &[], now, &ny_zone());
    assert_eq!(
        open.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
        [plain.id.as_str(), weekly.id.as_str()]
    );
    assert_eq!(open[1].due, Some(ny("2026-10-09 23:59")));
}

// --- a window far from the start -----------------------------------------------------------------------------

#[test]
fn gives_exactly_what_walking_the_whole_series_from_its_start_gives() {
    let rules = [
        "FREQ=DAILY;INTERVAL=3",
        "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH",
        "FREQ=WEEKLY;INTERVAL=3",
        "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR",
        "FREQ=MONTHLY;INTERVAL=2;BYMONTHDAY=31",
        "FREQ=MONTHLY;BYMONTHDAY=-1",
        "FREQ=MONTHLY;INTERVAL=5",
        "FREQ=YEARLY;INTERVAL=4",
        "FREQ=YEARLY;BYMONTHDAY=15",
        "FREQ=DAILY;INTERVAL=2;UNTIL=20290301",
    ];
    let starts = ["2019-01-31", "2020-02-29", "2023-06-13"];
    let windows = [
        ("2026-10-01", "2026-11-15"),
        ("2028-02-01", "2028-03-31"),
        ("2029-02-20", "2029-03-10"),
    ];
    let z = ny_zone();
    for text in rules {
        let rule = parse_rule(text).unwrap();
        for day in starts {
            let start = SeriesStart {
                day: day.into(),
                minute: 9.0 * 60.0,
            };
            for (from, to) in windows {
                let walked: Vec<_> = occurrences_between(&rule, &start, &z, day, to)
                    .into_iter()
                    .filter(|o| o.date.as_str() >= from)
                    .collect();
                assert_eq!(
                    occurrences_between(&rule, &start, &z, from, to),
                    walked,
                    "{text} from {day}, {from}..{to}"
                );
            }
        }
    }
}

// --- bugs in the TypeScript ------------------------------------------------------------------------------------

/// The TypeScript walks the periods of a yearly or monthly rule for ever once
/// they pass the years a Date holds, because an empty period never meets the
/// horizon check. The Rust ends the series there.
#[test]
fn a_rule_with_a_huge_interval_ends_where_the_calendar_does() {
    let z = ny_zone();
    for text in [
        "FREQ=YEARLY;INTERVAL=999999",
        "FREQ=MONTHLY;INTERVAL=99999999",
        "FREQ=YEARLY;INTERVAL=100000000000000000000",
    ] {
        let start = SeriesStart {
            day: "2026-10-06".into(),
            minute: 540.0,
        };
        let found = occurrences_between(
            &parse_rule(text).unwrap(),
            &start,
            &z,
            "2026-01-01",
            "2200-12-31",
        );
        assert_eq!(
            found.iter().map(|o| o.date.as_str()).collect::<Vec<_>>(),
            ["2026-10-06"],
            "{text}"
        );
    }
}

#[test]
#[ignore = "TS bug: parseRule reads FREQ=YEARLY;INTERVAL=200000 (and any MONTHLY or YEARLY interval that reaches past the years a Date holds) as a rule, and its series walk then never returns. The Rust ends the series instead; the cure is for parseRule to refuse such an interval. See docs/QUESTIONS.md #176"]
fn parse_rule_refuses_an_interval_no_plan_could_reach() {
    assert_eq!(parse_rule("FREQ=YEARLY;INTERVAL=200000"), None);
}
