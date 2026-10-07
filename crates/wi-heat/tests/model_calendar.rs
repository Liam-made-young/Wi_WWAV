//! `calendar.test.ts`, case for case. (Each view's title is display and stays
//! in TypeScript, with its test.) The pills' colours come from the token file.

mod common;

use common::*;
use wi_heat::model::calendar::{
    day_layout, due_at_end_of_day, month_grid, page, unscheduled_tray, week_days, AllDay,
    AllDayDue, CalendarData, CalendarMode, DueFlag, LaidOutKind, GRID_HOUR_PX,
};
use wi_heat::model::heat::HeatLevel;
use wi_heat::model::records::{CalendarEvent, Milestone};

fn data(over: impl FnOnce(&mut CalendarData)) -> CalendarData {
    let mut d = CalendarData::default();
    over(&mut d);
    d
}

fn now() -> f64 {
    ny("2026-10-06 08:40")
}

// --- the month grid (3.1) ----------------------------------------------------------------------------

#[test]
fn runs_six_weeks_from_the_sunday_before_the_1st_and_circles_today() {
    let cells = month_grid("2026-10-15", &CalendarData::default(), now(), &ny_zone());
    assert_eq!(cells.len(), 42);
    assert_eq!(cells[0].date, "2026-09-27");
    assert_eq!(cells[41].date, "2026-11-07");
    assert_eq!(cells.iter().filter(|c| c.in_month).count(), 31);
    assert_eq!(
        cells
            .iter()
            .filter(|c| c.is_today)
            .map(|c| c.date.as_str())
            .collect::<Vec<_>>(),
        ["2026-10-06"]
    );
}

#[test]
fn shows_3_pills_a_cell_in_heat_order_with_their_heat_colour_then_n_more() {
    let due = |h: u32, difficulty: f64, title: &str| {
        task(|t| {
            t.title = title.into();
            t.difficulty = difficulty;
            t.due = Some(ny(&format!("2026-10-08 {h:02}:00")));
        })
    };
    let tasks = vec![
        due(9, 1.0, "a"),
        due(10, 5.0, "b"),
        due(11, 2.0, "c"),
        due(12, 1.0, "d"),
        due(13, 3.0, "e"),
    ];
    let z = ny_zone();
    let cells = month_grid("2026-10-01", &data(|d| d.tasks = tasks.clone()), now(), &z);
    let cell = cells.iter().find(|c| c.date == "2026-10-08").unwrap();
    let pills: Vec<(&str, Option<HeatLevel>, Option<&str>)> = cell
        .pills
        .iter()
        .map(|p| (p.title.as_str(), p.level, p.colour.as_deref()))
        .collect();
    assert_eq!(
        pills,
        [
            ("b", Some(HeatLevel::Hot), Some("#e0402c")),
            ("e", Some(HeatLevel::Warm), Some("#efa431")),
            ("c", Some(HeatLevel::Warm), Some("#efa431")),
        ]
    );
    assert_eq!(cell.more.as_deref(), Some("2 more"));
    let three = month_grid(
        "2026-10-01",
        &data(|d| d.tasks = tasks[..3].to_vec()),
        now(),
        &z,
    );
    assert_eq!(three[11].more, None);
}

#[test]
fn shows_a_task_whose_rule_heat_cant_read_on_its_own_due_date() {
    let odd = task(|t| {
        t.title = "Odd".into();
        t.due = Some(ny("2026-10-09 23:59"));
        t.rrule = Some("FREQ=HOURLY".into());
    });
    let cells = month_grid(
        "2026-10-01",
        &data(|d| d.tasks = vec![odd]),
        now(),
        &ny_zone(),
    );
    let with_pills: Vec<&str> = cells
        .iter()
        .filter(|c| !c.pills.is_empty())
        .map(|c| c.date.as_str())
        .collect();
    assert_eq!(with_pills, ["2026-10-09"]);
}

#[test]
fn puts_a_recurring_task_on_each_of_its_days_and_shows_done_tasks_last_without_a_colour() {
    let weekly = task(|t| {
        t.title = "Weekly quiz".into();
        t.due = Some(ny("2026-10-02 23:59"));
        t.rrule = Some("FREQ=WEEKLY".into());
    });
    let shut = task(|t| {
        t.title = "Shipped".into();
        t.done = true;
        t.done_at = Some(now());
        t.due = Some(ny("2026-10-09 12:00"));
    });
    let cells = month_grid(
        "2026-10-01",
        &data(|d| d.tasks = vec![weekly, shut]),
        now(),
        &ny_zone(),
    );
    let fridays: Vec<&str> = cells
        .iter()
        .filter(|c| c.pills.iter().any(|p| p.title == "Weekly quiz"))
        .map(|c| c.date.as_str())
        .collect();
    assert_eq!(
        fridays,
        [
            "2026-10-02",
            "2026-10-09",
            "2026-10-16",
            "2026-10-23",
            "2026-10-30",
            "2026-11-06"
        ]
    );
    let ninth = cells.iter().find(|c| c.date == "2026-10-09").unwrap();
    let pills: Vec<(&str, Option<&str>)> = ninth
        .pills
        .iter()
        .map(|p| (p.title.as_str(), p.colour.as_deref()))
        .collect();
    assert_eq!(pills, [("Weekly quiz", Some("#efa431")), ("Shipped", None)]);
}

// --- the week and day views (3.7) -----------------------------------------------------------------------

#[test]
fn gives_the_week_from_sunday() {
    assert_eq!(
        week_days("2026-10-08"),
        [
            "2026-10-04",
            "2026-10-05",
            "2026-10-06",
            "2026-10-07",
            "2026-10-08",
            "2026-10-09",
            "2026-10-10"
        ]
    );
}

#[test]
fn lays_out_events_behind_blocks_and_a_heat_coloured_due_flag_at_its_time() {
    let quiz = task(|t| {
        t.title = "Grammar quiz 4".into();
        t.difficulty = 2.0;
        t.due = Some(ny("2026-10-07 23:59"));
    });
    let event = |id: &str, title: &str, start: &str, end: &str, all_day: bool| CalendarEvent {
        id: id.into(),
        title: title.into(),
        start: ny(start),
        end: ny(end),
        all_day,
    };
    let lecture = event(
        "e1",
        "JPN 201",
        "2026-10-07 09:00",
        "2026-10-07 10:15",
        false,
    );
    let overnight = event(
        "e2",
        "Flight",
        "2026-10-06 22:00",
        "2026-10-07 01:00",
        false,
    );
    let holiday = event(
        "e3",
        "Fall break",
        "2026-10-07 00:00",
        "2026-10-08 00:00",
        true,
    );
    let bead = Milestone {
        id: "m1".into(),
        space_id: "wwav".into(),
        project_id: None,
        title: "Enclosure v2".into(),
        date: "2026-10-07".into(),
        done: false,
        order: 1.0,
        link: None,
    };
    let b = block(|b| {
        b.task_id = Some(quiz.id.clone());
        b.date = "2026-10-07".into();
        b.start = 9.0 * 60.0 + 30.0;
        b.minutes = 45.0;
    });
    let layout = day_layout(
        "2026-10-07",
        &data(|d| {
            d.tasks = vec![quiz.clone()];
            d.events = vec![lecture, overnight, holiday.clone()];
            d.blocks = vec![b];
            d.milestones = vec![bead.clone()];
        }),
        now(),
        &ny_zone(),
    );
    let items: Vec<(LaidOutKind, &str, f64, f64, Option<&str>)> = layout
        .items
        .iter()
        .map(|i| {
            (
                i.kind,
                i.title.as_str(),
                i.top,
                i.height,
                i.colour.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        items,
        [
            (LaidOutKind::Event, "Flight", 0.0, GRID_HOUR_PX, None),
            (
                LaidOutKind::Event,
                "JPN 201",
                9.0 * GRID_HOUR_PX,
                1.25 * GRID_HOUR_PX,
                None
            ),
            (
                LaidOutKind::Block,
                "Grammar quiz 4",
                9.5 * GRID_HOUR_PX,
                0.75 * GRID_HOUR_PX,
                Some("#efa431")
            ),
        ]
    );
    assert_eq!(
        layout.due_flags,
        vec![DueFlag {
            task_id: quiz.id.clone(),
            title: "Grammar quiz 4".into(),
            top: ((23.0 * 60.0 + 59.0) / 60.0) * GRID_HOUR_PX,
            label: "due 11:59 PM".into(),
            colour: Some("#efa431".into()),
        }]
    );
    assert_eq!(
        layout.all_day,
        AllDay {
            due: vec![AllDayDue {
                task_id: quiz.id,
                title: "Grammar quiz 4".into(),
                colour: Some("#efa431".into())
            }],
            beads: vec![bead],
            events: vec![holiday],
        }
    );
}

#[test]
fn never_draws_a_brightspace_item_as_a_grey_event() {
    let synced = task(|t| {
        t.source = wi_heat::model::records::TaskSource::Ical;
        t.due = Some(ny("2026-10-07 23:59"));
    });
    let layout = day_layout(
        "2026-10-07",
        &data(|d| d.tasks = vec![synced]),
        now(),
        &ny_zone(),
    );
    assert_eq!(layout.items, vec![]);
    assert_eq!(layout.due_flags.len(), 1);
}

#[test]
fn keeps_a_short_block_tall_enough_to_read() {
    let b = block(|b| {
        b.date = "2026-10-07".into();
        b.start = 9.0 * 60.0;
        b.minutes = 15.0;
        b.task_id = Some("x".into());
    });
    let layout = day_layout(
        "2026-10-07",
        &data(|d| {
            d.tasks = vec![task(|t| t.id = "x".into())];
            d.blocks = vec![b];
        }),
        now(),
        &ny_zone(),
    );
    assert_eq!(layout.items[0].height, 16.0);
}

// --- the unscheduled tray (3.7) ---------------------------------------------------------------------------------

#[test]
fn lists_this_weeks_open_tasks_with_no_block_in_heat_order() {
    let titled = |title: &str, over: &dyn Fn(&mut wi_heat::model::records::Task)| {
        task(|t| {
            t.title = title.into();
            over(t);
        })
    };
    let blocked = titled("blocked", &|t| t.due = Some(ny("2026-10-08 23:59")));
    let hot = titled("hot", &|t| {
        t.difficulty = 3.0;
        t.due = Some(ny("2026-10-07 12:00"));
    });
    let warm = titled("warm", &|t| {
        t.difficulty = 3.0;
        t.due = Some(ny("2026-10-10 12:00"));
    });
    let scheduled = titled("scheduled", &|t| {
        t.scheduled_date = Some("2026-10-09".into())
    });
    let next = titled("next week", &|t| t.due = Some(ny("2026-10-12 12:00")));
    let done = titled("done", &|t| {
        t.done = true;
        t.done_at = Some(now());
        t.due = Some(ny("2026-10-08 12:00"));
    });
    let tray = unscheduled_tray(
        "2026-10-06",
        &data(|d| {
            d.tasks = vec![blocked.clone(), warm, scheduled, next, done, hot];
            d.blocks = vec![block(|b| {
                b.task_id = Some(blocked.id.clone());
                b.date = "2026-10-05".into();
            })];
        }),
        now(),
        &ny_zone(),
    );
    assert_eq!(
        tray.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
        ["hot", "warm", "scheduled"]
    );
}

// --- moving around ------------------------------------------------------------------------------------------------

#[test]
fn pages_a_month_a_week_or_a_day() {
    assert_eq!(page(CalendarMode::Month, "2026-10-31", 1.0), "2026-11-01");
    assert_eq!(page(CalendarMode::Month, "2026-01-15", -1.0), "2025-12-01");
    assert_eq!(page(CalendarMode::Week, "2026-10-06", 1.0), "2026-10-13");
    assert_eq!(page(CalendarMode::Day, "2026-10-06", -1.0), "2026-10-05");
}

#[test]
fn plus_adds_a_task_due_11_59_pm_on_the_selected_day() {
    assert_eq!(
        due_at_end_of_day("2026-11-01", &ny_zone()),
        ny("2026-11-01 23:59")
    );
}
