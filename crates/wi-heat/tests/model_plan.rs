//! `plan.test.ts`, case for case. (The Today header and the "Block ends" line
//! are display and stay in TypeScript, with their tests.)

mod common;

use common::*;
use wi_heat::model::estimate::{estimate_context, estimate_min};
use wi_heat::model::plan::{
    block_length, draft_key, drag_onto_column, initial_scroll_top, minutes_to_y, move_block,
    now_line_y, plan_my_day, plan_next, plan_sections, plan_subtitle, resize_block, snap,
    y_to_minutes, BlockTarget, Draft, PlanData, PlanOptions, PlanSection, COLUMN_HEIGHT,
    DEFAULT_BLOCK_MIN, HOUR_PX,
};
use wi_heat::model::records::{CalendarEvent, Origin, TaskOccurrence, TimeBlock};

const TODAY: &str = "2026-10-06";

fn hm(h: f64, m: f64) -> f64 {
    h * 60.0 + m
}

fn day(over: impl FnOnce(&mut PlanData)) -> PlanData {
    let mut d = PlanData::default();
    over(&mut d);
    d
}

fn event(start: &str, end: &str, over: impl FnOnce(&mut CalendarEvent)) -> CalendarEvent {
    let mut e = CalendarEvent {
        id: format!("ev-{start}"),
        title: "Class".into(),
        start: ny(start),
        end: ny(end),
        all_day: false,
    };
    over(&mut e);
    e
}

fn plan(data: &PlanData, now: f64) -> Vec<Draft> {
    plan_my_day(data, now, &ny_zone(), &PlanOptions::default())
}

fn task_target(id: &str) -> BlockTarget {
    BlockTarget::Task { task_id: id.into() }
}

fn planned(id: &str, start: f64, minutes: f64, origin: Origin, block_id: &str) -> TimeBlock {
    TimeBlock {
        id: block_id.into(),
        task_id: Some(id.into()),
        habit_id: None,
        date: TODAY.into(),
        start,
        minutes,
        origin,
    }
}

// --- the time column scale (3.5, from the PKM calendar) ----------------------------------------------

#[test]
fn is_44_px_an_hour_from_7_am_to_midnight() {
    assert_eq!(HOUR_PX, 44.0);
    assert_eq!(minutes_to_y(hm(7.0, 0.0)), 0.0);
    assert_eq!(minutes_to_y(hm(8.0, 0.0)), 44.0);
    assert_eq!(minutes_to_y(hm(24.0, 0.0)), 748.0);
    assert_eq!(COLUMN_HEIGHT, 748.0);
    assert_eq!(y_to_minutes(66.0), hm(8.0, 30.0));
}

#[test]
fn snaps_to_15_minutes_to_the_nearest_mark_and_blocks_default_to_30_minutes() {
    assert_eq!(snap(hm(8.0, 7.0)), hm(8.0, 0.0));
    assert_eq!(snap(hm(8.0, 8.0)), hm(8.0, 15.0));
    assert_eq!(DEFAULT_BLOCK_MIN, 30.0);
}

#[test]
fn scrolls_so_now_sits_a_third_of_the_way_down() {
    assert_eq!(
        initial_scroll_top(hm(14.0, 0.0), 600.0),
        minutes_to_y(hm(14.0, 0.0)) - 200.0
    );
    assert_eq!(initial_scroll_top(hm(7.0, 30.0), 600.0), 0.0);
    assert_eq!(
        initial_scroll_top(hm(23.0, 30.0), 600.0),
        COLUMN_HEIGHT - 600.0
    );
}

#[test]
fn draws_the_red_now_line_only_inside_the_column() {
    assert_eq!(now_line_y(hm(9.0, 30.0)), Some(110.0));
    assert_eq!(now_line_y(hm(6.0, 59.0)), None);
}

// --- Plan my day (3.5) ----------------------------------------------------------------------------------

#[test]
fn places_unplanned_open_tasks_in_heat_order_each_in_the_first_gap_that_fits() {
    let now = ny("2026-10-06 08:41");
    let hot = task(|t| {
        t.title = "hot".into();
        t.difficulty = 3.0;
        t.due = Some(ny("2026-10-06 16:00"));
        t.est_min = Some(45.0);
    });
    let warm = task(|t| {
        t.title = "warm".into();
        t.difficulty = 3.0;
        t.due = Some(ny("2026-10-08 23:59"));
        t.est_min = Some(10.0);
    });
    let cool = task(|t| {
        t.title = "cool".into();
        t.difficulty = 1.0;
        t.due = Some(ny("2026-10-20 23:59"));
        t.est_min = Some(130.0);
    });
    let undated = task(|t| {
        t.title = "undated".into();
        t.est_min = Some(60.0);
    });
    let drafts = plan(
        &day(|d| {
            d.tasks = vec![undated.clone(), cool.clone(), warm.clone(), hot.clone()];
            d.events = vec![event("2026-10-06 09:00", "2026-10-06 10:00", |_| {})];
            d.blocks = vec![block(|b| {
                b.date = TODAY.into();
                b.start = hm(13.0, 0.0);
                b.minutes = 60.0;
            })];
        }),
        now,
    );
    let got: Vec<(&str, f64, f64, f64)> = drafts
        .iter()
        .map(|d| (d.task_id.as_str(), d.start, d.minutes, d.left_min))
        .collect();
    assert_eq!(
        got,
        vec![
            // 8:45 to 9:00 is too short for 45m; 10:00 is the first gap that fits.
            (hot.id.as_str(), hm(10.0, 0.0), 45.0, 0.0),
            // 10m rounds up to 15, and 8:45 to 9:00 fits it.
            (warm.id.as_str(), hm(8.0, 45.0), 15.0, 0.0),
            // 130m rounds up to 135, capped at 90: "45m left to plan".
            (cool.id.as_str(), hm(10.0, 45.0), 90.0, 45.0),
            // 12:15 to 13:00 is too short for 60m; the block ends at 14:00.
            (undated.id.as_str(), hm(14.0, 0.0), 60.0, 0.0),
        ]
    );
    assert!(drafts.iter().all(|d| d.date == TODAY));
    assert_eq!(drafts[2].left_line.as_deref(), Some("45m left to plan"));
    assert_eq!(drafts[0].left_line, None);
}

#[test]
fn gives_each_draft_its_reason() {
    let now = ny("2026-10-06 08:41");
    let quiz = task(|t| {
        t.title = "Grammar quiz 4".into();
        t.difficulty = 2.0;
        t.due = Some(ny("2026-10-07 23:59"));
        t.est_min = Some(45.0);
    });
    let hot = task(|t| {
        t.difficulty = 2.0;
        t.due = Some(ny("2026-10-07 09:00"));
        t.est_min = Some(15.0);
    });
    let thursday = task(|t| {
        t.difficulty = 3.0;
        t.due = Some(ny("2026-10-08 23:59"));
        t.est_min = Some(15.0);
    });
    let late = task(|t| {
        t.due = Some(ny("2026-10-04 23:59"));
        t.est_min = Some(15.0);
    });
    let undated = task(|t| t.est_min = Some(15.0));
    let all = vec![
        quiz.clone(),
        hot.clone(),
        thursday.clone(),
        late.clone(),
        undated.clone(),
    ];
    let drafts = plan(&day(|d| d.tasks = all), now);
    let reason = |id: &str| {
        drafts
            .iter()
            .find(|d| d.task_id == id)
            .unwrap()
            .reason
            .clone()
    };
    // 1.6: "Due tomorrow 11:59 PM, Warm."
    assert_eq!(reason(&quiz.id), "Due tomorrow 11:59 PM, Warm.");
    // 3.5: "Due tomorrow 11:59 PM, Hot." has the same shape.
    assert_eq!(reason(&hot.id), "Due tomorrow 9:00 AM, Hot.");
    assert_eq!(reason(&thursday.id), "Due Thursday 11:59 PM, Warm.");
    assert_eq!(reason(&late.id), "1d overdue.");
    assert_eq!(reason(&undated.id), "No due date.");
}

#[test]
fn rounds_each_estimate_up_to_15_minutes_never_less_than_15() {
    let now = ny("2026-10-06 08:41");
    let tasks: Vec<_> = [46.0, 15.0, 1.0]
        .iter()
        .enumerate()
        .map(|(i, est)| {
            task(|t| {
                t.est_min = Some(*est);
                t.due = Some(ny("2026-10-06 20:00") + i as f64);
            })
        })
        .collect();
    let minutes: Vec<f64> = plan(&day(|d| d.tasks = tasks), now)
        .iter()
        .map(|d| d.minutes)
        .collect();
    assert_eq!(minutes, [60.0, 15.0, 15.0]);
}

#[test]
fn uses_the_estimate_chain_when_a_task_has_no_est_min() {
    let now = ny("2026-10-06 08:41");
    let t = task(|t| {
        t.difficulty = 2.0;
        t.est_min = None;
        t.due = Some(ny("2026-10-07 23:59"));
    });
    assert_eq!(plan(&day(|d| d.tasks = vec![t]), now)[0].minutes, 45.0); // 40m rounds up to 45
}

#[test]
fn leaves_out_done_tasks_tasks_already_planned_today_and_parents_of_open_subtasks() {
    let now = ny("2026-10-06 08:41");
    let done = task(|t| {
        t.done = true;
        t.done_at = Some(now);
        t.est_min = Some(15.0);
    });
    let planned_today = task(|t| t.est_min = Some(15.0));
    let planned_tomorrow = task(|t| t.est_min = Some(15.0));
    let parent = task(|t| t.est_min = Some(15.0));
    let child = task(|t| {
        t.parent_task_id = Some(parent.id.clone());
        t.est_min = Some(15.0);
    });
    let drafts = plan(
        &day(|d| {
            d.tasks = vec![
                done,
                planned_today.clone(),
                planned_tomorrow.clone(),
                parent,
                child.clone(),
            ];
            d.blocks = vec![
                block(|b| {
                    b.task_id = Some(planned_today.id.clone());
                    b.date = TODAY.into();
                    b.start = hm(20.0, 0.0);
                }),
                block(|b| {
                    b.task_id = Some(planned_tomorrow.id.clone());
                    b.date = "2026-10-07".into();
                    b.start = hm(20.0, 0.0);
                }),
            ];
        }),
        now,
    );
    let mut got: Vec<&str> = drafts.iter().map(|d| d.task_id.as_str()).collect();
    got.sort_unstable();
    let mut want = vec![planned_tomorrow.id.as_str(), child.id.as_str()];
    want.sort_unstable();
    assert_eq!(got, want);
}

#[test]
fn stops_at_day_ends_at_11_pm_by_default() {
    let now = ny("2026-10-06 08:41");
    let late = ny("2026-10-06 22:20");
    let long = task(|t| {
        t.est_min = Some(45.0);
        t.due = Some(ny("2026-10-07 09:00"));
    });
    let short = task(|t| {
        t.est_min = Some(30.0);
        t.due = Some(ny("2026-10-07 10:00"));
    });
    let both = day(|d| d.tasks = vec![long.clone(), short.clone()]);
    let got: Vec<(&str, f64)> = plan(&both, late)
        .iter()
        .map(|d| (d.task_id.as_str(), d.start))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|(id, start)| (if id == short.id { "short" } else { "long" }, start))
        .collect();
    assert_eq!(got, [("short", hm(22.0, 30.0))]);
    let early = plan_my_day(
        &both,
        now,
        &ny_zone(),
        &PlanOptions {
            day_ends_at: Some(hm(9.0, 30.0)),
            space_id: None,
        },
    );
    assert_eq!(
        early.iter().map(|d| d.task_id.as_str()).collect::<Vec<_>>(),
        [long.id.as_str()]
    );
    assert_eq!(
        plan(&day(|d| d.tasks = vec![short]), ny("2026-10-06 23:10")),
        vec![]
    );
}

#[test]
fn works_around_timed_events_including_one_from_yesterday_but_not_all_day_ones() {
    let now = ny("2026-10-06 08:41");
    let t = task(|t| {
        t.est_min = Some(60.0);
        t.due = Some(ny("2026-10-07 09:00"));
    });
    let drafts = plan(
        &day(|d| {
            d.tasks = vec![t];
            d.events = vec![
                event("2026-10-05 22:00", "2026-10-06 10:00", |e| {
                    e.title = "Overnight".into()
                }),
                event("2026-10-06 00:00", "2026-10-07 00:00", |e| e.all_day = true),
            ];
        }),
        now,
    );
    assert_eq!(drafts[0].start, hm(10.0, 0.0));
}

#[test]
fn plans_only_the_selected_spaces_tasks_but_steers_round_every_block() {
    let now = ny("2026-10-06 08:41");
    let mine = task(|t| {
        t.space_id = "wwav".into();
        t.est_min = Some(30.0);
        t.due = Some(ny("2026-10-07 09:00"));
    });
    let other = task(|t| {
        t.space_id = "classes".into();
        t.est_min = Some(30.0);
        t.due = Some(ny("2026-10-06 12:00"));
    });
    let drafts = plan_my_day(
        &day(|d| {
            d.tasks = vec![mine.clone(), other.clone()];
            d.blocks = vec![block(|b| {
                b.task_id = Some(other.id.clone());
                b.date = TODAY.into();
                b.start = hm(8.0, 45.0);
                b.minutes = 60.0;
            })];
        }),
        now,
        &ny_zone(),
        &PlanOptions {
            day_ends_at: None,
            space_id: Some("wwav".into()),
        },
    );
    assert_eq!(
        drafts
            .iter()
            .map(|d| (d.task_id.as_str(), d.start))
            .collect::<Vec<_>>(),
        [(mine.id.as_str(), hm(9.0, 45.0))]
    );
}

#[test]
fn accepts_all_on_return_clears_on_esc_and_ignores_other_keys() {
    let now = ny("2026-10-06 08:41");
    let tasks = vec![
        task(|t| {
            t.est_min = Some(30.0);
            t.due = Some(ny("2026-10-07 09:00"));
        }),
        task(|t| {
            t.est_min = Some(15.0);
            t.due = Some(ny("2026-10-07 10:00"));
        }),
    ];
    let drafts = plan(&day(|d| d.tasks = tasks.clone()), now);
    let accepted = draft_key(&drafts, "Enter", &mut ids("blk")).unwrap();
    assert_eq!(accepted.drafts, vec![]);
    assert_eq!(
        accepted.blocks,
        vec![
            planned(&tasks[0].id, hm(8.0, 45.0), 30.0, Origin::Plan, "blk-1"),
            planned(&tasks[1].id, hm(9.0, 15.0), 15.0, Origin::Plan, "blk-2"),
        ]
    );
    let cleared = draft_key(&drafts, "Escape", &mut ids("id")).unwrap();
    assert_eq!((cleared.drafts, cleared.blocks), (vec![], vec![]));
    assert_eq!(draft_key(&drafts, "p", &mut ids("id")), None);
    assert_eq!(draft_key(&[], "Enter", &mut ids("id")), None);
}

// --- making and changing blocks by hand ---------------------------------------------------------------------------

#[test]
fn p_puts_a_task_in_the_next_free_gap_after_now_as_long_as_its_estimate_rounded_up_to_15() {
    let now = ny("2026-10-06 08:41");
    let t = task(|t| t.est_min = Some(50.0));
    let data = day(|d| {
        d.tasks = vec![t.clone()];
        d.events = vec![event("2026-10-06 09:00", "2026-10-06 10:00", |_| {})];
    });
    assert_eq!(
        plan_next(&task_target(&t.id), &data, now, &ny_zone(), &mut ids("blk")),
        Some(planned(&t.id, hm(10.0, 0.0), 60.0, Origin::You, "blk-1"))
    );
}

#[test]
fn p_blocks_a_habit_for_its_length_or_30_minutes_without_one() {
    let now = ny("2026-10-06 08:41");
    let short = habit(|h| h.minutes = Some(15.0));
    let plain = habit(|_| {});
    let data = day(|d| {
        d.habits = vec![short.clone(), plain.clone()];
        d.events = vec![event("2026-10-06 09:00", "2026-10-06 10:00", |_| {})];
    });
    let z = ny_zone();
    let for_short = plan_next(
        &BlockTarget::Habit {
            habit_id: short.id.clone(),
        },
        &data,
        now,
        &z,
        &mut ids("id"),
    );
    assert_eq!(for_short.unwrap().start, hm(8.0, 45.0));
    let for_plain = plan_next(
        &BlockTarget::Habit {
            habit_id: plain.id.clone(),
        },
        &data,
        now,
        &z,
        &mut ids("id"),
    )
    .unwrap();
    assert_eq!((for_plain.start, for_plain.minutes), (hm(10.0, 0.0), 30.0));
}

#[test]
fn p_finds_nothing_when_the_day_is_full() {
    let t = task(|t| t.est_min = Some(90.0));
    let data = day(|d| d.tasks = vec![t.clone()]);
    assert_eq!(
        plan_next(
            &task_target(&t.id),
            &data,
            ny("2026-10-06 23:00"),
            &ny_zone(),
            &mut ids("id")
        ),
        None
    );
}

#[test]
fn dragging_a_task_onto_the_column_makes_a_block_as_long_as_its_estimate_rounded_up_to_15() {
    let t = task(|t| t.est_min = Some(50.0));
    let data = day(|d| d.tasks = vec![t.clone()]);
    assert_eq!(
        drag_onto_column(
            &task_target(&t.id),
            minutes_to_y(hm(9.0, 7.0)),
            TODAY,
            &data,
            &mut ids("blk")
        ),
        Some(planned(&t.id, hm(9.0, 0.0), 60.0, Origin::You, "blk-1"))
    );
    // Dropped too low, it ends at midnight rather than past it.
    let low = drag_onto_column(
        &task_target(&t.id),
        minutes_to_y(hm(23.0, 30.0)),
        TODAY,
        &data,
        &mut ids("id"),
    );
    assert_eq!(low.unwrap().start, hm(23.0, 0.0));
}

#[test]
fn resizing_changes_the_block_never_the_estimate() {
    let t = task(|t| t.est_min = Some(30.0));
    let b = block(|b| {
        b.task_id = Some(t.id.clone());
        b.start = hm(9.0, 0.0);
        b.minutes = 30.0;
    });
    let before = estimate_min(&t, &estimate_context(std::slice::from_ref(&t), &[]));
    assert_eq!(
        resize_block(&b, minutes_to_y(hm(10.0, 10.0))),
        TimeBlock {
            minutes: 75.0,
            ..b.clone()
        }
    );
    assert_eq!(
        resize_block(&b, minutes_to_y(hm(9.0, 2.0))),
        TimeBlock {
            minutes: 15.0,
            ..b.clone()
        }
    );
    let late = TimeBlock {
        start: hm(23.0, 0.0),
        ..b.clone()
    };
    assert_eq!(
        resize_block(&late, minutes_to_y(hm(24.0, 0.0)) + 200.0),
        TimeBlock {
            start: hm(23.0, 0.0),
            minutes: 60.0,
            ..b.clone()
        }
    );
    assert_eq!(t.est_min, Some(30.0));
    assert_eq!(
        estimate_min(&t, &estimate_context(std::slice::from_ref(&t), &[])),
        before
    );
}

#[test]
fn moving_a_block_snaps_its_start_and_keeps_it_inside_the_column() {
    let b = block(|b| {
        b.start = hm(9.0, 0.0);
        b.minutes = 60.0;
    });
    assert_eq!(
        move_block(&b, minutes_to_y(hm(13.0, 5.0))),
        TimeBlock {
            start: hm(13.0, 0.0),
            ..b.clone()
        }
    );
    assert_eq!(
        move_block(&b, -50.0),
        TimeBlock {
            start: hm(7.0, 0.0),
            ..b.clone()
        }
    );
    assert_eq!(
        move_block(&b, COLUMN_HEIGHT),
        TimeBlock {
            start: hm(23.0, 0.0),
            ..b.clone()
        }
    );
}

// --- the plan list (3.5) -----------------------------------------------------------------------------------------------

#[test]
fn has_four_sections_planned_in_time_order_with_start_times_and_hides_the_empty_ones() {
    let now = ny("2026-10-06 10:15");
    let a = task(|t| {
        t.title = "Read chapter 3".into();
        t.est_min = Some(45.0);
        t.due = Some(ny("2026-10-09 23:59"));
    });
    let b = task(|t| {
        t.title = "Mix the second verse".into();
        t.space_id = "wwav".into();
    });
    let due_a = task(|t| {
        t.title = "Problem set".into();
        t.difficulty = 2.0;
        t.due = Some(ny("2026-10-06 16:00"));
    });
    let due_b = task(|t| {
        t.title = "Lab report".into();
        t.difficulty = 2.0;
        t.due = Some(ny("2026-10-06 23:59"));
    });
    let done_today = task(|t| {
        t.title = "Shipped".into();
        t.done = true;
        t.done_at = Some(now);
        t.due = Some(ny("2026-10-06 12:00"));
    });
    let scales = task(|t| {
        t.title = "Scales".into();
        t.rrule = Some("FREQ=DAILY".into());
        t.due = Some(ny("2026-10-01 07:00"));
    });
    let kanji = habit(|h| {
        h.title = "Practise kanji".into();
        h.minutes = Some(20.0);
    });
    let stretch = habit(|h| h.title = "Stretch".into());
    let hot_ones: Vec<_> = (1..=6)
        .map(|i| {
            task(|t| {
                t.title = format!("hot {i}");
                t.difficulty = 5.0;
                t.due = Some(ny("2026-10-07 12:00") + f64::from(i) * 60_000.0);
            })
        })
        .collect();
    let occ = TaskOccurrence {
        id: "o".into(),
        task_id: scales.id.clone(),
        date: TODAY.into(),
        done_at: ny("2026-10-06 07:30"),
    };
    let mut tasks = vec![
        a.clone(),
        b.clone(),
        due_a,
        due_b,
        done_today,
        scales.clone(),
    ];
    tasks.extend(hot_ones);
    let sections = plan_sections(
        &day(|d| {
            d.tasks = tasks;
            d.habits = vec![kanji.clone(), stretch];
            d.occurrences = vec![occ];
            d.blocks = vec![
                block(|x| {
                    x.task_id = Some(b.id.clone());
                    x.date = TODAY.into();
                    x.start = hm(14.0, 0.0);
                    x.minutes = 90.0;
                }),
                block(|x| {
                    x.task_id = Some(a.id.clone());
                    x.date = TODAY.into();
                    x.start = hm(9.0, 45.0);
                    x.minutes = 45.0;
                }),
                block(|x| {
                    x.task_id = Some(a.id.clone());
                    x.date = "2026-10-07".into();
                    x.start = hm(9.0, 0.0);
                }),
            ];
        }),
        now,
        &ny_zone(),
        None,
    );
    let titles: Vec<&str> = sections
        .iter()
        .map(|s| match s {
            PlanSection::Planned { title, .. }
            | PlanSection::DueToday { title, .. }
            | PlanSection::Recurring { title, .. }
            | PlanSection::Hot { title, .. } => title.as_str(),
        })
        .collect();
    assert_eq!(
        titles,
        [
            "Planned",
            "Due today, not planned",
            "Recurring today ↻",
            "Hot, not planned"
        ]
    );
    let (PlanSection::Planned { items: rows, .. }, PlanSection::DueToday { items: due, .. }) =
        (&sections[0], &sections[1])
    else {
        panic!("sections out of order")
    };
    assert_eq!(
        rows.iter()
            .map(|r| (
                r.title.as_str(),
                r.time.as_str(),
                r.length.as_str(),
                r.current,
                r.finished
            ))
            .collect::<Vec<_>>(),
        [
            ("Read chapter 3", "9:45 AM", "45m", true, false),
            ("Mix the second verse", "2:00 PM", "1h 30m", false, false)
        ]
    );
    assert_eq!(
        due.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
        ["Problem set", "Lab report"]
    );
    let PlanSection::Recurring {
        items: recurring, ..
    } = &sections[2]
    else {
        panic!("no recurring section")
    };
    assert_eq!(
        recurring
            .iter()
            .map(|r| (r.id.as_str(), r.title.as_str(), r.done))
            .collect::<Vec<_>>(),
        [
            (scales.id.as_str(), "Scales", true),
            (kanji.id.as_str(), "Practise kanji", false)
        ]
    );
    // Up to 5, and not the ones already listed as due today.
    let PlanSection::Hot { items: hot, .. } = &sections[3] else {
        panic!("no hot section")
    };
    assert_eq!(
        hot.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
        ["hot 1", "hot 2", "hot 3", "hot 4", "hot 5"]
    );
}

#[test]
fn shows_nothing_at_all_for_an_empty_day() {
    let now = ny("2026-10-06 10:15");
    assert_eq!(
        plan_sections(&PlanData::default(), now, &ny_zone(), None),
        vec![]
    );
}

#[test]
fn applies_the_space_filter_to_tasks_not_to_habits() {
    let now = ny("2026-10-06 10:15");
    let mine = task(|t| {
        t.space_id = "wwav".into();
        t.due = Some(ny("2026-10-06 16:00"));
    });
    let other = task(|t| {
        t.space_id = "classes".into();
        t.due = Some(ny("2026-10-06 16:00"));
    });
    let kanji = habit(|h| h.minutes = Some(20.0));
    let sections = plan_sections(
        &day(|d| {
            d.tasks = vec![mine.clone(), other];
            d.habits = vec![kanji.clone()];
        }),
        now,
        &ny_zone(),
        Some("wwav"),
    );
    let ids: Vec<Vec<String>> = sections
        .iter()
        .map(|s| match s {
            PlanSection::Planned { items, .. } => {
                items.iter().map(|r| r.block.id.clone()).collect()
            }
            PlanSection::DueToday { items, .. } | PlanSection::Hot { items, .. } => {
                items.iter().map(|t| t.id.clone()).collect()
            }
            PlanSection::Recurring { items, .. } => items.iter().map(|r| r.id.clone()).collect(),
        })
        .collect();
    assert_eq!(ids, vec![vec![mine.id.clone()], vec![kanji.id.clone()]]);
}

// --- the subtitle ---------------------------------------------------------------------------------------------------------

#[test]
fn reads_4_blocks_3h_10m_planned_2_due_today_1_6() {
    let now = ny("2026-10-06 08:41");
    let quiz = task(|t| t.due = Some(ny("2026-10-06 16:00")));
    let lab = task(|t| t.due = Some(ny("2026-10-06 23:59")));
    let shut = task(|t| {
        t.due = Some(ny("2026-10-06 12:00"));
        t.done = true;
        t.done_at = Some(now);
    });
    let mix = task(|t| t.title = "Mix the second verse".into());
    let kanji = habit(|h| h.minutes = Some(25.0));
    let on =
        |task_id: Option<&str>, habit_id: Option<&str>, start: f64, minutes: f64, date: &str| {
            block(|b| {
                b.task_id = task_id.map(str::to_string);
                b.habit_id = habit_id.map(str::to_string);
                b.date = date.into();
                b.start = start;
                b.minutes = minutes;
            })
        };
    let blocks = [
        on(Some(&quiz.id), None, hm(9.0, 0.0), 45.0, TODAY),
        on(Some(&lab.id), None, hm(11.0, 0.0), 30.0, TODAY),
        on(None, Some(&kanji.id), hm(13.0, 0.0), 25.0, TODAY),
        on(Some(&mix.id), None, hm(14.0, 0.0), 90.0, TODAY),
    ];
    let tomorrow = on(Some(&mix.id), None, hm(9.0, 0.0), 30.0, "2026-10-07");
    let data = day(|d| {
        d.tasks = vec![quiz, lab, shut, mix.clone()];
        d.habits = vec![kanji];
        d.blocks = blocks.iter().cloned().chain([tomorrow]).collect();
    });
    let z = ny_zone();
    assert_eq!(
        plan_subtitle(&data, now, &z, None),
        "4 blocks · 3h 10m planned · 2 due today"
    );
    let one = day(|d| {
        d.tasks = vec![mix];
        d.blocks = vec![blocks[3].clone()];
    });
    assert_eq!(
        plan_subtitle(&one, now, &z, None),
        "1 block · 1h 30m planned · 0 due today"
    );
}

// `block_length` is the rule behind every length above.
#[test]
fn a_block_is_as_long_as_its_estimate_rounded_up_to_15_and_never_under_15() {
    assert_eq!(
        [0.0, 1.0, 15.0, 16.0, 46.0, 90.0].map(block_length),
        [15.0, 15.0, 15.0, 30.0, 60.0, 90.0]
    );
}
