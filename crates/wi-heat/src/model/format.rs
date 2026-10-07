//! Dates and times in the words the app prints: the part of
//! `app/ui/src/shared/time/format.ts` the model's sentences use. The clock is
//! built by hand, as there, so no locale's spacing gets in.

use super::js;
use super::zone::{key_parts, minute_of_day};
use jiff::tz::TimeZone;

pub const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// `MONTHS[index]`, which is the text "undefined" past the list, as the
/// template literal in the TypeScript prints it.
fn month_name(index: f64) -> &'static str {
    if js::is_integer(index) && (0.0..12.0).contains(&index) {
        MONTHS[index as usize]
    } else {
        "undefined"
    }
}

/// `WEEKDAYS[index]`, or "undefined" past the list.
pub fn weekday_name(index: f64) -> &'static str {
    if js::is_integer(index) && (0.0..7.0).contains(&index) {
        WEEKDAYS[index as usize]
    } else {
        "undefined"
    }
}

fn pad2(n: f64) -> String {
    let s = js::num_to_string(n);
    if s.chars().count() >= 2 {
        s
    } else {
        format!("0{s}")
    }
}

/// "4:00 PM" for minutes after midnight.
pub fn clock(minutes: f64) -> String {
    let m = ((js::round(minutes) % 1440.0) + 1440.0) % 1440.0;
    let h = (m / 60.0).floor();
    let h12 = if h % 12.0 == 0.0 { 12.0 } else { h % 12.0 };
    format!(
        "{}:{} {}",
        js::num_to_string(h12),
        pad2(m % 60.0),
        if h < 12.0 { "AM" } else { "PM" }
    )
}

/// "11:59 PM" for an instant in `tz`.
pub fn clock_at(ms: f64, tz: &TimeZone) -> String {
    clock(minute_of_day(ms, tz))
}

/// "August 26"
pub fn month_day(key: &str) -> String {
    let (_, month, day) = key_parts(key);
    format!("{} {}", month_name(month - 1.0), js::num_to_string(day))
}

/// "Oct 21"
pub fn short_month_day(key: &str) -> String {
    let (_, month, day) = key_parts(key);
    let name = month_name(month - 1.0);
    // `.slice(0, 3)` counts UTF-16 units, and the names are ASCII.
    format!(
        "{} {}",
        name.chars().take(3).collect::<String>(),
        js::num_to_string(day)
    )
}

/// "24:59": time left, rounded up to the second so 0:00 shows only at the end.
pub fn countdown(ms: f64) -> String {
    let s = js::max2(0.0, (ms / 1000.0).ceil());
    format!(
        "{}:{}",
        js::num_to_string((s / 60.0).floor()),
        pad2(s % 60.0)
    )
}
