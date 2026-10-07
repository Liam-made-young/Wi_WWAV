//! Time zones for the model: a port of `app/ui/src/shared/time/zone.ts`.
//!
//! An instant is epoch ms. A day is a `"YYYY-MM-DD"` key in the person's zone
//! (the system's, 3.11). Day keys are plain calendar dates, so their
//! arithmetic runs in UTC and never meets DST. The TypeScript asks `Intl` what
//! the wall clock reads at an instant; here that is jiff's answer for an
//! explicit [`TimeZone`], which must be the same zone the TypeScript was given.
//!
//! Every number is an `f64`, as in the TypeScript, and day keys are read the
//! way it reads them: `key.split('-').map(Number)`. An instant `Intl` can't
//! format throws there; here a NaN or out-of-range instant is clamped to the
//! range jiff holds (years -9999 to 9999), so nothing panics. Neither can come
//! out of stored JSON.

use super::js::{self, civil_from_days, date_utc, TIME_CLIP};
use jiff::tz::TimeZone;
use jiff::Timestamp;

/// `"YYYY-MM-DD"` in the person's time zone.
pub type DayKey = String;

pub const DAY_MS: f64 = 86_400_000.0;

/// A wall clock reading, as `Intl` gives it: the month is 1 to 12.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WallTime {
    #[serde(serialize_with = "super::records::ser::num")]
    pub year: f64,
    #[serde(serialize_with = "super::records::ser::num")]
    pub month: f64,
    #[serde(serialize_with = "super::records::ser::num")]
    pub day: f64,
    #[serde(serialize_with = "super::records::ser::num")]
    pub hour: f64,
    #[serde(serialize_with = "super::records::ser::num")]
    pub minute: f64,
    #[serde(default, serialize_with = "super::records::ser::num")]
    pub second: f64,
}

impl WallTime {
    pub fn new(year: f64, month: f64, day: f64, hour: f64, minute: f64) -> Self {
        WallTime {
            year,
            month,
            day,
            hour,
            minute,
            second: 0.0,
        }
    }
}

/// The zone for an IANA name such as `"America/New_York"`; None if jiff's
/// bundled database doesn't know it.
pub fn zone(name: &str) -> Option<TimeZone> {
    TimeZone::get(name).ok()
}

fn timestamp(ms: f64) -> Timestamp {
    // Intl formats TimeClip(ms), which drops a fraction toward zero.
    let t = if ms.is_nan() { 0.0 } else { ms.trunc() };
    let lo = Timestamp::MIN.as_millisecond() as f64;
    let hi = Timestamp::MAX.as_millisecond() as f64;
    Timestamp::from_millisecond(t.clamp(lo, hi) as i64).unwrap_or(Timestamp::UNIX_EPOCH)
}

/// The wall clock in `tz` at instant `ms`.
pub fn wall_time(ms: f64, tz: &TimeZone) -> WallTime {
    let dt = tz.to_datetime(timestamp(ms));
    WallTime {
        year: f64::from(dt.year()),
        month: f64::from(dt.month()),
        day: f64::from(dt.day()),
        hour: f64::from(dt.hour()),
        minute: f64::from(dt.minute()),
        second: f64::from(dt.second()),
    }
}

// How far the wall clock runs ahead of UTC at an instant.
fn offset_at(ms: f64, tz: &TimeZone) -> f64 {
    let w = wall_time(ms, tz);
    let as_utc = date_utc(w.year, w.month - 1.0, w.day, w.hour, w.minute, w.second);
    as_utc - (ms / 1000.0).floor() * 1000.0
}

/// The instant a wall clock names in `tz`. A time that happens twice (the
/// autumn hour) is the first one. A time that never happens (the spring gap)
/// is read with the offset before the gap, so it lands that far after it, as
/// RFC 5545 reads it.
pub fn epoch_of(w: &WallTime, tz: &TimeZone) -> f64 {
    let as_utc = date_utc(w.year, w.month - 1.0, w.day, w.hour, w.minute, w.second);
    if as_utc.is_nan() {
        return f64::NAN;
    }
    let before = offset_at(as_utc - DAY_MS, tz);
    let after = offset_at(as_utc + DAY_MS, tz);
    let mut valid: Vec<f64> = [before, after]
        .iter()
        .map(|o| as_utc - o)
        .filter(|t| offset_at(*t, tz) == as_utc - t)
        .collect();
    valid.sort_by(f64::total_cmp);
    valid.first().copied().unwrap_or(as_utc - before)
}

fn pad(n: f64) -> String {
    let s = js::num_to_string(n);
    if s.chars().count() >= 2 {
        s
    } else {
        format!("0{s}")
    }
}

pub fn key_of(year: f64, month: f64, day: f64) -> DayKey {
    format!("{}-{}-{}", js::num_to_string(year), pad(month), pad(day))
}

/// `key.split('-').map(Number)`, as (year, month, day); NaN for a part the
/// key doesn't have.
pub fn key_parts(key: &str) -> (f64, f64, f64) {
    let mut parts = key.split('-').map(js::to_number);
    let mut next = || parts.next().unwrap_or(f64::NAN);
    (next(), next(), next())
}

/// The day an instant falls on in `tz`.
pub fn day_key(ms: f64, tz: &TimeZone) -> DayKey {
    let w = wall_time(ms, tz);
    key_of(w.year, w.month, w.day)
}

/// Minutes after local midnight, by the wall clock.
pub fn minute_of_day(ms: f64, tz: &TimeZone) -> f64 {
    let w = wall_time(ms, tz);
    w.hour * 60.0 + w.minute
}

fn key_to_utc(key: &str) -> f64 {
    let (year, month, day) = key_parts(key);
    date_utc(year, month - 1.0, day, 0.0, 0.0, 0.0)
}

fn utc_to_key(ms: f64) -> DayKey {
    // `new Date(ms)` of an instant past the Date range is invalid, and its
    // fields are NaN.
    if !ms.is_finite() || ms.abs() > TIME_CLIP {
        return key_of(f64::NAN, f64::NAN, f64::NAN);
    }
    let (y, m, d) = civil_from_days((ms.trunc() / DAY_MS).floor() as i64);
    key_of(y as f64, m as f64, d as f64)
}

pub fn add_days(key: &str, n: f64) -> DayKey {
    utc_to_key(key_to_utc(key) + n * DAY_MS)
}

/// Whole days from `a` to `b`.
pub fn days_between(a: &str, b: &str) -> f64 {
    js::round((key_to_utc(b) - key_to_utc(a)) / DAY_MS)
}

/// 0 is Sunday, as the calendar's weeks start (3.1). NaN for a key that isn't a date.
pub fn weekday_of(key: &str) -> f64 {
    let t = key_to_utc(key);
    if t.is_nan() {
        return f64::NAN;
    }
    ((t / DAY_MS).floor() as i64 + 4).rem_euclid(7) as f64
}

/// The days in a month (month 1 to 12) of a year.
pub fn days_in_month(year: f64, month: f64) -> f64 {
    let t = date_utc(year, month, 0.0, 0.0, 0.0, 0.0);
    if t.is_nan() {
        return f64::NAN;
    }
    civil_from_days((t / DAY_MS).floor() as i64).2 as f64
}

/// The instant a day's wall clock reads `minutes` after midnight; 1440 is the next midnight.
pub fn at_minute(key: &str, minutes: f64, tz: &TimeZone) -> f64 {
    let day = add_days(key, (minutes / 1440.0).floor());
    let m = ((minutes % 1440.0) + 1440.0) % 1440.0;
    let (year, month, d) = key_parts(&day);
    epoch_of(
        &WallTime {
            year,
            month,
            day: d,
            hour: (m / 60.0).floor(),
            minute: m % 60.0,
            second: 0.0,
        },
        tz,
    )
}

pub fn start_of_day(key: &str, tz: &TimeZone) -> f64 {
    at_minute(key, 0.0, tz)
}
