//! The zone and clock maths the model stands on: `shared/time/zone.test.ts`
//! and `shared/time/format.test.ts`, case for case. (`longDay` is only the
//! Today header's and the Calendar title's, which stay in TypeScript.)

mod common;

use common::*;
use jiff::tz::TimeZone;
use wi_heat::model::format::{clock, clock_at, countdown, month_day, short_month_day};
use wi_heat::model::zone::{
    add_days, at_minute, day_key, days_between, epoch_of, minute_of_day, start_of_day, wall_time,
    weekday_of, WallTime,
};

fn utc(text: &str) -> f64 {
    text.parse::<jiff::Timestamp>()
        .expect("an instant")
        .as_millisecond() as f64
}

fn wall(year: f64, month: f64, day: f64, hour: f64, minute: f64) -> WallTime {
    WallTime::new(year, month, day, hour, minute)
}

// --- wall time in a zone ---------------------------------------------------------

#[test]
fn reads_the_wall_clock_of_an_instant() {
    // 1.6's morning: Tuesday, October 6, 8:40 AM in New York (EDT, UTC-4).
    let t = utc("2026-10-06T12:40:00Z");
    let z = ny_zone();
    assert_eq!(
        wall_time(t, &z),
        WallTime {
            year: 2026.0,
            month: 10.0,
            day: 6.0,
            hour: 8.0,
            minute: 40.0,
            second: 0.0
        }
    );
    assert_eq!(day_key(t, &z), "2026-10-06");
    assert_eq!(minute_of_day(t, &z), 8.0 * 60.0 + 40.0);
}

#[test]
fn puts_an_instant_on_the_day_it_is_in_that_zone_not_in_utc() {
    // 11:59 PM Wednesday in New York is already Thursday in UTC.
    let t = utc("2026-10-08T03:59:00Z");
    assert_eq!(day_key(t, &ny_zone()), "2026-10-07");
    assert_eq!(day_key(t, &TimeZone::UTC), "2026-10-08");
}

#[test]
fn turns_a_wall_clock_back_into_the_same_instant() {
    let z = ny_zone();
    assert_eq!(
        epoch_of(&wall(2026.0, 10.0, 7.0, 23.0, 59.0), &z),
        utc("2026-10-08T03:59:00Z")
    );
    assert_eq!(
        epoch_of(&wall(2026.0, 12.0, 1.0, 9.0, 0.0), &z),
        utc("2026-12-01T14:00:00Z")
    );
}

#[test]
fn moves_a_time_that_does_not_exist_forward_by_the_gap_as_rfc_5545_does() {
    // March 8, 2026: 2:00 AM jumps to 3:00 AM, so 2:30 AM is read as 3:30 AM EDT.
    assert_eq!(
        epoch_of(&wall(2026.0, 3.0, 8.0, 2.0, 30.0), &ny_zone()),
        utc("2026-03-08T07:30:00Z")
    );
}

#[test]
fn takes_the_first_of_a_time_that_happens_twice() {
    // November 1, 2026: 1:30 AM happens in EDT, then again in EST.
    assert_eq!(
        epoch_of(&wall(2026.0, 11.0, 1.0, 1.0, 30.0), &ny_zone()),
        utc("2026-11-01T05:30:00Z")
    );
}

#[test]
fn finds_local_midnight_and_a_minute_of_the_day_across_a_dst_change() {
    let z = ny_zone();
    assert_eq!(start_of_day("2026-11-01", &z), utc("2026-11-01T04:00:00Z"));
    assert_eq!(start_of_day("2026-11-02", &z), utc("2026-11-02T05:00:00Z"));
    assert_eq!(
        at_minute("2026-11-01", 23.0 * 60.0 + 59.0, &z),
        utc("2026-11-02T04:59:00Z")
    );
    assert_eq!(
        at_minute("2026-10-06", 24.0 * 60.0, &z),
        start_of_day("2026-10-07", &z)
    );
}

// --- day keys ----------------------------------------------------------------------

#[test]
fn adds_days_across_months_and_years() {
    assert_eq!(add_days("2026-10-06", 1.0), "2026-10-07");
    assert_eq!(add_days("2026-10-31", 1.0), "2026-11-01");
    assert_eq!(add_days("2026-12-31", 1.0), "2027-01-01");
    assert_eq!(add_days("2028-03-01", -1.0), "2028-02-29");
    assert_eq!(add_days("2026-10-06", -400.0), "2025-09-01");
}

#[test]
fn counts_days_between_keys() {
    assert_eq!(days_between("2026-10-06", "2026-10-07"), 1.0);
    assert_eq!(days_between("2026-10-06", "2026-08-26"), -41.0);
    assert_eq!(days_between("2026-03-07", "2026-03-09"), 2.0);
}

#[test]
fn gives_the_weekday_from_sunday() {
    assert_eq!(weekday_of("2026-10-04"), 0.0);
    assert_eq!(weekday_of("2026-10-06"), 2.0);
    assert_eq!(weekday_of("2026-10-10"), 6.0);
}

// --- clock times (format.test.ts) ----------------------------------------------------

#[test]
fn writes_minutes_after_midnight_as_a_12_hour_clock() {
    assert_eq!(clock(0.0), "12:00 AM");
    assert_eq!(clock(9.0 * 60.0), "9:00 AM");
    assert_eq!(clock(12.0 * 60.0), "12:00 PM");
    assert_eq!(clock(16.0 * 60.0), "4:00 PM");
    assert_eq!(clock(15.0 * 60.0 + 30.0), "3:30 PM");
    assert_eq!(clock(23.0 * 60.0 + 59.0), "11:59 PM");
    assert_eq!(clock(24.0 * 60.0), "12:00 AM");
}

#[test]
fn writes_an_instant_in_its_zone_with_a_plain_space_before_am_or_pm() {
    let z = ny_zone();
    assert_eq!(clock_at(utc("2026-10-08T03:59:00Z"), &z), "11:59 PM");
    assert_eq!(clock_at(utc("2026-10-06T19:41:00Z"), &z), "3:41 PM");
}

#[test]
fn writes_the_month_day_and_the_short_month_day() {
    assert_eq!(month_day("2026-08-26"), "August 26");
    assert_eq!(short_month_day("2026-10-21"), "Oct 21");
}

#[test]
fn rounds_up_to_the_second_so_0_00_shows_only_at_the_end() {
    assert_eq!(countdown(25.0 * MIN), "25:00");
    assert_eq!(countdown(25.0 * MIN - 1.0), "25:00");
    assert_eq!(countdown(25.0 * MIN - 1000.0), "24:59");
    assert_eq!(countdown(5.0 * MIN), "5:00");
    assert_eq!(countdown(18.0 * MIN + 40_000.0), "18:40");
    assert_eq!(countdown(0.0), "0:00");
    assert_eq!(countdown(-5.0), "0:00");
}
